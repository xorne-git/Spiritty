use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    agent::AgentEngine,
    config::{Config, ProviderType},
    event::AppEvent,
    i18n::Language,
    pty::PtyProcess,
    session::{Session, SessionStorage},
    system::{ActiveSession, HostsStore, SystemContext},
    ui::{
        chat_panel::prompt_visual_rows,
        components::{
            BookmarksModalAction, BookmarksModalState, ConfigModalState, ExportModalAction,
            ExportModalState, SessionModalAction, SessionModalState,
        },
        theme::ThemeId,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Chat,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub command_proposal: Option<String>,
}

/// Per-message render artifact kept alive between frames so the chat panel only
/// recomposes what actually changed (see `ui/chat_panel.rs`).
///
/// Freshness is validated positionally against the live message:
/// `(generation, role_tag, byte_len)`. Byte length is a sufficient mutation
/// witness here because every existing code path mutates message content by
/// wholesale growth, shrink or replacement — never by a same-length substitution.
/// Everything that invalidates the whole chat render cache at once:
/// panel width, language, debug mode and the active color theme.
///
/// Identity tags are packed by the caller (`ui/chat_panel.rs`) from values that
/// are all `Copy + Eq`, so comparison is allocation-free and collision-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatGenKey {
    pub width: u16,
    /// Tag byte the caller builds from locale + debug visibility of tool dumps.
    pub flags: u8,
    /// Palette identity tag (UI-theme discriminant).
    pub theme: u8,
}

pub struct ChatCacheEntry {
    pub generation: ChatGenKey,
    pub role_tag: u8,
    pub byte_len: usize,
    /// `pending_tool_approval.is_some()` snapshot taken when composed; only
    /// meaningful for the tail assistant message whose ghost prefix reacts to it.
    pub pending_flag: bool,
    /// Whether the message's reasoning block is expanded (click-to-expand). Bumped
    /// so the entry invalidates and re-renders on toggle.
    pub expanded: bool,
    /// Visual wrapped rows of [`ChatCacheEntry::lines`] at the cached panel width
    /// (already includes any trailing blank separator line).
    pub rows: u16,
    pub lines: Vec<ratatui::text::Line<'static>>,
}

/// Frame-to-frame render cache for the chat history. Lives inside a `RefCell`
/// on `App` because the draw pass legitimately fills misses while reading
/// messages (single-threaded UI thread, per AGENTS.md threading rules).
#[derive(Default)]
pub struct ChatRenderCache {
    seen_generation: Option<ChatGenKey>,
    entries: Vec<Option<ChatCacheEntry>>,
}

impl ChatRenderCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Aligns the cache with the current frame generation and live message count.
    ///
    /// A generation change wipes everything at once (resize, theme switch,
    /// language toggle, debug-mode flip). Otherwise the entry list merely tracks
    /// the append/shrink pattern of `App::messages`.
    pub fn begin_frame(&mut self, generation: ChatGenKey, message_count: usize) {
        if self.seen_generation != Some(generation) {
            self.entries.clear();
            self.seen_generation = Some(generation);
        }
        self.entries.truncate(message_count);
    }

    /// Wholescale wipe used when `App::messages` is replaced or emptied
    /// (session load, new session): entries are positional and must not survive.
    pub fn hard_reset(&mut self) {
        self.entries.clear();
        self.seen_generation = None;
    }

    /// Returns the (possibly empty) slot for message `idx`, extending storage as needed.
    pub fn slot_mut(&mut self, idx: usize) -> &mut Option<ChatCacheEntry> {
        while self.entries.len() <= idx {
            self.entries.push(None);
        }
        &mut self.entries[idx]
    }

    pub fn entry_lines(&self, idx: usize) -> Option<&[ratatui::text::Line<'static>]> {
        self.entries
            .get(idx)
            .and_then(|e| e.as_ref())
            .map(|e| e.lines.as_slice())
    }

    /// Read-only lookup used by the draw pass for freshness checks and row totals.
    pub fn entry(&self, idx: usize) -> Option<&ChatCacheEntry> {
        self.entries.get(idx).and_then(|e| e.as_ref())
    }
}

#[derive(Debug, Clone)]
pub struct ProactiveDiagnosis {
    pub command: String,
    pub error_message: String,
}

pub enum ModalState {
    None,
    Help,
    Config(ConfigModalState),
    Sessions(SessionModalState),
    Bookmarks(BookmarksModalState),
    Export(ExportModalState),
    Mcp(crate::ui::components::McpModalState),
    /// Small prompt offering to reconnect to the SSH host of a `-c`-resumed session
    /// whose PTY is currently local.
    SshReconnect {
        target: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionPanel {
    Chat,
    Terminal,
}

#[derive(Debug, Clone)]
pub struct MouseSelection {
    pub panel: SelectionPanel,
    pub start: (u16, u16),
    pub end: (u16, u16),
    pub is_selecting: bool,
}

pub struct PendingToolApproval {
    pub command: String,
    pub approval_tx: Option<tokio::sync::oneshot::Sender<bool>>,
}

/// Temporary capture-path debugging (SPIRITTY_CAPTURE_DEBUG=1). Appends one line per
/// event to /tmp/spiritty_capture_debug.log so a full-timeout capture can be diagnosed
/// after the fact (conclusion reason, shell hooks, remote misdetection, timing).
fn capture_debug(msg: &str) {
    use std::sync::OnceLock;
    static ENABLED: OnceLock<bool> = OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var("SPIRITTY_CAPTURE_DEBUG").is_ok()) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/spiritty_capture_debug.log")
        {
            let _ = writeln!(f, "[{:?}] {}", std::time::SystemTime::now(), msg);
        }
    }
}

pub struct PtyToolCapture {
    pub command: String,
    pub result_tx: Option<tokio::sync::oneshot::Sender<String>>,
    pub output_bytes: Vec<u8>,
    /// Incrementally decoded text of `output_bytes`: each arriving chunk is UTF-8 decoded
    /// once (with a small carry buffer for split multi-byte sequences) instead of the
    /// entire buffer being re-decoded on every chunk — the old behavior degraded
    /// quadratically during verbose command output.
    pub decoded_text: String,
    /// Raw tail bytes of an incomplete multi-byte UTF-8 sequence, flushed once complete.
    pub pending_utf8: Vec<u8>,
    /// First-complete sentinel sighting not yet terminated (`pos`, `prefix_len`, is_osc),
    /// so later chunks only scan for its terminator instead of the whole buffer again.
    pub sentinel_pending: Option<(usize, usize, bool)>,
    /// Byte offset in `decoded_text` up to which the sentinel scan has run (exclusive).
    /// Each chunk rescans only from `watermark - (max_prefix_len - 1)` so a pattern
    /// straddling a chunk boundary is still caught, while bytes already proven
    /// sentinel-free are not rescanned (amortized O(new bytes) per chunk).
    pub sentinel_scan_upto: usize,
    pub start_time: std::time::Instant,
    pub last_output_time: std::time::Instant,
    /// When `true`, the app records the `[RÉSULTAT...]` in the chat history and triggers the
    /// next model turn itself (used for user-executed command cards). When `false`, the caller
    /// (the agent engine) awaits `result_tx` and handles the result (agent-requested tools).
    pub auto_prompt: bool,
    /// Set once buffering stopped because `MAX_CAPTURE_BYTES` was reached: further output
    /// is counted in `overflow_bytes` and scanned for the completion sentinel without
    /// being buffered. A runaway/hallucinating command previously grew the buffer without
    /// limit while `on_tick` re-ran the full O(N) `clean_pty_output` every frame — the UI
    /// degraded quadratically and appeared frozen.
    pub truncated: bool,
    /// Total bytes received (and discarded) after the cap was reached.
    pub overflow_bytes: u64,
    /// Small rolling tail of the raw stream, used in overflow mode to detect the OSC 777
    /// completion sentinel even though buffering has stopped.
    pub overflow_tail: Vec<u8>,
    /// Cache of the last `clean_pty_output` result keyed by `decoded_text.len()`, so the
    /// per-tick settle check does not re-clean the whole buffer on every frame.
    pub clean_cache: Option<(usize, String)>,
}

pub struct App {
    pub focus: Focus,
    pub chat_input: String,
    pub cursor_pos: usize,
    pub messages: Vec<ChatMessage>,
    pub pty: PtyProcess,
    pub should_quit: bool,
    pub split_ratio: u16,
    pub terminal_inner_size: (u16, u16),
    pub chat_area: Rect,
    pub terminal_area: Rect,
    pub is_dragging_split: bool,
    pub config: Config,
    pub agent: AgentEngine,
    pub modal: ModalState,
    pub event_tx: UnboundedSender<AppEvent>,
    pub spinner_frame: usize,
    pub pending_tool_approval: Option<PendingToolApproval>,
    /// Render cache feeding the chat panel draw pass (filled from the UI thread only).
    pub chat_render_cache: std::cell::RefCell<ChatRenderCache>,
    pub last_injected_cmd: Option<String>,
    pub detected_context_window: Arc<AtomicUsize>,
    pub active_pty_tool: Option<PtyToolCapture>,
    pub chat_scroll_from_bottom: u16,
    pub chat_scroll_extra_down: u16,
    /// Index (into `messages`) of the conversation entry whose reasoning block is
    /// expanded via click-to-expand; `None` = all collapsed.
    pub expanded_thought: Option<usize>,
    /// Per-frame hit region (message idx, content_top_row, content_bottom_row) of each
    /// collapsed "Think · …" toggle line, in content (pre-scroll) row coordinates.
    /// Filled by the chat draw pass, consumed by the mouse-click handler (1-frame lag
    /// is acceptable for a click).
    pub chat_thought_hits: std::cell::RefCell<Vec<(usize, u16, u16)>>,
    /// The chat messages area rectangle and current scroll offset, filled by the draw
    /// pass so the click handler can translate screen coords to content rows.
    pub chat_messages_geo: std::cell::RefCell<(ratatui::layout::Rect, u16)>,
    pub chat_history: Vec<String>,
    pub history_index: Option<usize>,
    pub input_draft: String,
    pub system_context: SystemContext,
    pub generation_start_time: Option<std::time::Instant>,
    pub first_chunk_time: Option<std::time::Instant>,
    pub last_chunk_time: Option<std::time::Instant>,
    pub current_turn_chars: usize,
    pub current_turn_tokens: usize,
    pub last_tokens_per_sec: Option<f64>,
    pub mouse_selection: Option<MouseSelection>,
    pub clipboard_toast: Option<(std::time::Instant, usize)>,
    pub copied_current_selection: bool,
    pub current_session: Session,
    pub hosts_store: HostsStore,
    pub toast_message: Option<(std::time::Instant, String)>,
    pub theme: ThemeId,
    pub proactive_error_diagnosis: Option<ProactiveDiagnosis>,
    pub terminal_input_buffer: String,
    pub last_user_terminal_command: Option<String>,
    pub chat_search_active: bool,
    pub chat_search_query: String,
    pub chat_search_cursor: usize,
    pub chat_search_match_idx: usize,
    pub pricing_registry: Arc<tokio::sync::RwLock<crate::pricing::PricingRegistry>>,
    pub mouse_pos: Option<(u16, u16)>,
    /// Debug mode — when set, raw tool-result `[RÉSULTAT…]` blocks are shown in the chat.
    pub debug: bool,
}

/// Moves the prompt cursor vertically across the VISUAL rows of a (possibly
/// multi-line or wrapped) chat input, preserving the cursor column when the
/// target row is wide enough (clamped to its end otherwise).
///
/// Returns `None` when the move cannot happen (single visual row, or already on
/// the first/last row) so the caller can fall back to history navigation.
fn prompt_move_cursor_vertical(
    input: &str,
    width: usize,
    cursor_byte: usize,
    delta: i32,
) -> Option<usize> {
    use crate::ui::chat_panel::str_visual_width;

    let rows = prompt_visual_rows(input, width);
    if rows.len() < 2 {
        return None;
    }

    let cur_row = rows.iter().rposition(|r| cursor_byte >= r.byte_start)?;
    let target_row = cur_row as i64 + i64::from(delta);
    if target_row < 0 || target_row >= rows.len() as i64 {
        return None;
    }

    let from = &rows[cur_row];
    let to = &rows[target_row as usize];
    let clamped = cursor_byte.clamp(from.byte_start, from.byte_end);
    let target_col = str_visual_width(&input[from.byte_start..clamped]);

    // A row's range includes its terminating '\n'; the cursor may rest on the
    // content only, never on that newline byte itself.
    let content_end = if to.byte_end > to.byte_start && input.as_bytes()[to.byte_end - 1] == b'\n' {
        to.byte_end - 1
    } else {
        to.byte_end
    };

    // Walk the target row until the accumulated visual width reaches the column.
    let mut acc = 0usize;
    let mut byte = content_end;
    for (i, ch) in input[to.byte_start..content_end].char_indices() {
        if acc >= target_col {
            byte = to.byte_start + i;
            break;
        }
        acc += crate::ui::chat_panel::char_visual_width(ch);
    }
    Some(byte.min(content_end))
}

impl App {
    pub fn new(
        event_tx: UnboundedSender<AppEvent>,
        initial_rows: u16,
        initial_cols: u16,
    ) -> Result<Self> {
        let (pty_tx, mut pty_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let (pty_exit_tx, mut pty_exit_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        let pty = PtyProcess::spawn(initial_rows, initial_cols, pty_tx, pty_exit_tx)?;

        // Forward raw PTY output to unified event channel
        let forward_tx = event_tx.clone();
        tokio::spawn(async move {
            while let Some(bytes) = pty_rx.recv().await {
                if forward_tx.send(AppEvent::PtyOutput(bytes)).is_err() {
                    break;
                }
            }
        });

        // Forward shell termination (EOF or reaped child) so the UI quits cleanly
        // instead of staying on a dead PTY when the user types `exit`.
        let exit_forward_tx = event_tx.clone();
        tokio::spawn(async move {
            while pty_exit_rx.recv().await.is_some() {
                if exit_forward_tx.send(AppEvent::PtyExit).is_err() {
                    break;
                }
            }
        });

        let mut config = Config::load();

        // Inherit the last used provider & model from the most recent saved session
        if let Ok(sessions) = SessionStorage::list_sessions() {
            if let Some(last_sess) = sessions.first() {
                if let Some(p_type) = crate::config::ProviderType::from_key(&last_sess.provider) {
                    config.default_provider = p_type;
                    if !last_sess.model.is_empty() {
                        let key = p_type.key_str();
                        if let Some(p_cfg) = config.providers.get_mut(key) {
                            p_cfg.model = last_sess.model.clone();
                        }
                    }
                }
            }
        }

        let agent = AgentEngine::new_with_event_tx(config.clone(), Some(event_tx.clone()));
        let detected_context_window = Arc::new(AtomicUsize::new(0));
        probe_model_context(&config, detected_context_window.clone());
        let system_context = SystemContext::detect();

        let active_provider = config.default_provider.display_name();
        let active_model = config.get_active_provider_config().model.clone();
        let current_session = Session::new(active_provider, &active_model);

        let split_ratio = config.get_split_ratio();
        let theme = ThemeId::parse_or_default(&config.get_theme());

        let app = Self {
            focus: Focus::Chat, // Default focus on chat prompt
            chat_input: String::new(),
            cursor_pos: 0,
            messages: Vec::new(),
            pty,
            should_quit: false,
            split_ratio,
            terminal_inner_size: (initial_rows, initial_cols),
            chat_area: Rect::default(),
            terminal_area: Rect::default(),
            is_dragging_split: false,
            config: config.clone(),
            agent,
            modal: ModalState::None,
            event_tx,
            spinner_frame: 0,
            pending_tool_approval: None,
            chat_render_cache: std::cell::RefCell::new(ChatRenderCache::new()),
            last_injected_cmd: None,
            detected_context_window,
            active_pty_tool: None,
            chat_scroll_from_bottom: 0,
            chat_scroll_extra_down: 0,
            expanded_thought: None,
            chat_thought_hits: std::cell::RefCell::new(Vec::new()),
            chat_messages_geo: std::cell::RefCell::new((ratatui::layout::Rect::ZERO, 0)),
            chat_history: Vec::new(),
            history_index: None,
            input_draft: String::new(),
            system_context,
            generation_start_time: None,
            first_chunk_time: None,
            last_chunk_time: None,
            current_turn_chars: 0,
            current_turn_tokens: 0,
            last_tokens_per_sec: None,
            mouse_selection: None,
            clipboard_toast: None,
            copied_current_selection: false,
            current_session,
            hosts_store: HostsStore::load(),
            toast_message: None,
            theme,
            proactive_error_diagnosis: None,
            terminal_input_buffer: String::new(),
            last_user_terminal_command: None,
            chat_search_active: false,
            chat_search_query: String::new(),
            chat_search_cursor: 0,
            chat_search_match_idx: 0,
            pricing_registry: Arc::new(tokio::sync::RwLock::new(
                crate::pricing::PricingRegistry::load_with_overrides(config.pricing),
            )),
            mouse_pos: None,
            debug: false,
        };

        app.probe_provider_models(ProviderType::LmStudio);
        app.probe_provider_models(ProviderType::Ollama);
        if app.config.default_provider != ProviderType::LmStudio
            && app.config.default_provider != ProviderType::Ollama
        {
            app.probe_provider_models(app.config.default_provider);
        }

        Ok(app)
    }

    /// Refreshes the online pricing registry (multi-provider listing). When `announce` is
    /// false the refresh runs silently (startup auto-refresh): the cache is updated but no
    /// toast is emitted on either outcome.
    pub fn trigger_pricing_update_announced(&self, announce: bool) {
        let registry_arc = self.pricing_registry.clone();
        let event_tx = self.event_tx.clone();
        let client = reqwest::Client::new();

        tokio::spawn(async move {
            let mut reg = registry_arc.write().await;
            let result = reg.fetch_online_and_update(&client).await;
            drop(reg);
            if !announce {
                return;
            }
            let event_payload = match result {
                Ok(count) => Ok(count),
                Err(e) => Err(e.to_string()),
            };
            let _ = event_tx.send(AppEvent::PricingUpdated(event_payload));
        });
    }

    pub fn trigger_pricing_update(&self) {
        self.trigger_pricing_update_announced(true);
    }

    pub fn on_pricing_updated(&mut self, res: Result<usize, String>) {
        let lang = self.config.get_language();
        match res {
            Ok(count) => {
                let msg = format!(
                    "{} ({} modèles)",
                    lang.t(crate::i18n::I18nKey::PricingUpdateSuccess),
                    count
                );
                if let ModalState::Config(ref mut config_state) = self.modal {
                    config_state.pricing_status = Some((
                        std::time::Instant::now(),
                        msg.clone(),
                        ratatui::style::Color::Green,
                    ));
                }
                self.set_toast(msg);
            }
            Err(err) => {
                let msg = format!(
                    "{}: {}",
                    lang.t(crate::i18n::I18nKey::PricingUpdateFailed),
                    err
                );
                if let ModalState::Config(ref mut config_state) = self.modal {
                    config_state.pricing_status = Some((
                        std::time::Instant::now(),
                        msg.clone(),
                        ratatui::style::Color::Red,
                    ));
                }
                self.set_toast(msg);
            }
        }
    }

    pub fn probe_provider_models(&self, provider: ProviderType) {
        let key = provider.key_str().to_string();
        let p_cfg = self.config.providers.get(&key).cloned();
        let base_url = p_cfg.as_ref().and_then(|c| c.base_url.clone());
        let api_key = Config::resolve_api_key_for_provider(
            provider,
            p_cfg.as_ref().and_then(|c| c.api_key.as_deref()),
        );
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let fetched = crate::agent::providers::fetch_available_models(
                provider,
                base_url.as_deref(),
                api_key.as_deref(),
            )
            .await;

            if !fetched.is_empty() {
                let _ = event_tx.send(AppEvent::ModelsLoaded {
                    provider_key: key,
                    models: fetched,
                });
            }
        });
    }

    pub fn on_models_loaded(&mut self, provider_key: String, models: Vec<String>) {
        if let Some(p_cfg) = self.config.providers.get_mut(&provider_key) {
            for m in &models {
                if !p_cfg.models.contains(m) {
                    p_cfg.models.push(m.clone());
                }
            }
        }

        if let ModalState::Config(ref mut config_state) = self.modal {
            let entry = config_state
                .models_per_provider
                .entry(provider_key.clone())
                .or_default();
            for m in &models {
                if !entry.contains(m) {
                    entry.push(m.clone());
                }
            }
            if config_state.selected_provider.key_str() == provider_key {
                if let Some(current_models) = config_state.models_per_provider.get(&provider_key) {
                    if let Some(pos) = current_models
                        .iter()
                        .position(|m| *m == config_state.model_input)
                    {
                        config_state.dropdown_selected_idx = pos;
                    }
                }
            }
        }
    }

    pub fn scroll_chat_up(&mut self, lines: u16) {
        self.chat_scroll_from_bottom = self.chat_scroll_from_bottom.saturating_add(lines);
    }

    pub fn scroll_chat_down(&mut self, lines: u16) {
        self.chat_scroll_from_bottom = self.chat_scroll_from_bottom.saturating_sub(lines);
    }

    pub fn reset_chat_scroll(&mut self) {
        self.chat_scroll_from_bottom = 0;
        self.chat_scroll_extra_down = 0;
    }

    /// Click-to-expand/collapse a reasoning block: if the click lands on a "Think · "
    /// toggle row (recorded by the draw pass in `chat_thought_hits`), flip expansion
    /// for that message and swallow the click (no text selection). Rows are translated
    /// from screen coords to content coords via `chat_messages_geo`.
    pub fn toggle_thought_on_click(&mut self, _x: u16, y: u16) -> bool {
        let (area, scroll_offset) = *self.chat_messages_geo.borrow();
        if y < area.top() || y >= area.bottom() {
            return false;
        }
        let content_row = y - area.top() + scroll_offset;
        let hits = self.chat_thought_hits.borrow();
        if let Some(&(idx, ..)) = hits
            .iter()
            .find(|&&(_, t, b)| content_row >= t && content_row < b)
        {
            self.expanded_thought = if self.expanded_thought == Some(idx) {
                None
            } else {
                Some(idx)
            };
            drop(hits);
            self.chat_thought_hits.borrow_mut().clear();
            return true;
        }
        false
    }

    pub fn save_current_session(&mut self) {
        if self.messages.is_empty() || self.messages.iter().all(|m| m.content.trim().is_empty()) {
            return;
        }
        self.refresh_session_ssh_hint();
        let active_provider = self.config.default_provider.display_name();
        let active_model = self.config.get_active_provider_config().model.clone();
        let total_tokens = self.get_total_tokens_used();
        self.current_session.update_from_chat(
            &self.messages,
            &self.chat_history,
            total_tokens,
            active_provider,
            &active_model,
        );
        // v0.5.2: the session JSON keeps the FULL history — no compaction on
        // save. The LLM context is compacted at request time instead (see
        // agent::send_prompt → compact_chat_messages), so reloading shows the
        // complete conversation while token cost stays bounded.
        let _ = SessionStorage::save(&self.current_session);
        let _ = self.config.save();
    }

    /// Closes the sessions list modal after a load — UNLESS `load_session` raised the
    /// SSH-reconnect offer (a remote session resumed on a local PTY): that modal MUST
    /// survive the list closing, otherwise the in-app reload path silently swallowed
    /// the reconnect offer (it only ever showed on `-c` startup, where nothing closed
    /// it afterwards).
    fn close_sessions_modal_after_load(&mut self) {
        if !matches!(self.modal, ModalState::SshReconnect { .. }) {
            self.modal = ModalState::None;
        }
    }

    /// After resuming a session that WAS remote while the PTY is still local, offer
    /// a one-keystroke reconnection to the recorded SSH host. Never interrupts a
    /// running PTY capture, and never shows when already connected (the live 🌐
    /// title covers that case).
    fn maybe_offer_ssh_reconnect(&mut self, resumed_ssh: Option<String>) {
        if let Some(target) = resumed_ssh.filter(|t| !t.is_empty()) {
            if !self.system_context.active_session.is_ssh() && self.active_pty_tool.is_none() {
                // A prompt-inferred target may be a bare remote hostname (`prod`),
                // which is not directly connectable — resolve it to the address that
                // actually reached that machine (store: `ducasse-seine.com`), and
                // heal the persisted hint so the next resume offers it directly.
                let connectable = self.hosts_store.resolve_connectable_target(&target);
                if connectable != target {
                    self.current_session.last_ssh_target = Some(connectable.clone());
                }
                self.modal = ModalState::SshReconnect {
                    target: connectable,
                };
            }
        }
    }

    /// Keeps the persisted SSH hint fresh (sticky): records the live SSH target when
    /// a remote session is detected, keeps the last known one otherwise — a session
    /// that WAS remote stays flagged so a `-c` resume can display the SSH hint.
    fn refresh_session_ssh_hint(&mut self) {
        if let crate::system::ActiveSession::Ssh { target, .. } =
            &self.system_context.active_session
        {
            if self.current_session.last_ssh_target.as_deref() != Some(target.as_str()) {
                self.current_session.last_ssh_target = Some(target.clone());
            }
        }
    }

    pub fn load_session(&mut self, session_id: &str) {
        self.save_current_session();
        let loaded_res = if let Ok(exact) = SessionStorage::load(session_id) {
            Ok(exact)
        } else if let Ok(sessions) = SessionStorage::list_sessions() {
            let lower_id = session_id.to_lowercase();
            if let Some(matching) = sessions.iter().find(|s| {
                s.id == session_id
                    || s.id.to_lowercase().contains(&lower_id)
                    || s.title.to_lowercase().contains(&lower_id)
            }) {
                SessionStorage::load(&matching.id)
            } else {
                Err(anyhow::anyhow!("Session '{}' introuvable", session_id))
            }
        } else {
            SessionStorage::load(session_id)
        };

        match loaded_res {
            Ok(mut loaded) => {
                let title = loaded.title.clone();
                let count = loaded.messages.len();
                // Resumed-SSH hint: prefer the persisted target; fall back to a
                // best-effort scan of the history for sessions saved by older builds.
                loaded.infer_last_ssh_target();
                let resumed_ssh = loaded.last_ssh_target.clone();
                self.messages = loaded.messages.clone();
                // Wholesale replacement: render cache entries are positional, wipe them.
                self.chat_render_cache.borrow_mut().hard_reset();
                // Restore prompt history
                if !loaded.prompt_history.is_empty() {
                    self.chat_history = loaded.prompt_history.clone();
                } else {
                    // Fallback: extract from previous user messages
                    self.chat_history = loaded
                        .messages
                        .iter()
                        .filter(|m| {
                            m.role == MessageRole::User
                                && !m.content.starts_with("💻 `")
                                && !m.content.starts_with("[RÉSULTAT")
                        })
                        .map(|m| m.content.clone())
                        .collect();
                }
                self.history_index = None;
                self.input_draft.clear();

                // Restore active provider & model from loaded session
                if let Some(p_type) = crate::config::ProviderType::from_key(&loaded.provider) {
                    self.config.default_provider = p_type;
                    if !loaded.model.is_empty() {
                        let key = p_type.key_str();
                        if let Some(p_cfg) = self.config.providers.get_mut(key) {
                            p_cfg.model = loaded.model.clone();
                        }
                    }
                    let _ = self.config.save();
                    self.agent
                        .reload_config(self.config.clone(), Some(self.event_tx.clone()));
                    self.trigger_context_probe();
                }

                self.current_session = loaded;
                self.chat_input.clear();
                self.cursor_pos = 0;
                self.reset_chat_scroll();
                let ssh_note = match resumed_ssh.as_deref() {
                    Some(t) if !t.is_empty() => format!(" · 🔗 SSH ({})", t),
                    Some(_) => " · 🔗 SSH".to_string(),
                    None => String::new(),
                };
                self.set_toast(format!(
                    "📂 Session '{}' restaurée ({} messages){}",
                    title, count, ssh_note
                ));
                self.maybe_offer_ssh_reconnect(resumed_ssh);
            }
            Err(e) => {
                self.set_toast(format!("⚠️ Erreur chargement session : {}", e));
            }
        }
    }

    pub fn submit_initial_prompt(&mut self, prompt: &str) {
        let trimmed = prompt.trim().to_string();
        if !trimmed.is_empty() {
            self.chat_input = trimmed;
            self.cursor_pos = self.chat_input.len();
            let enter = crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Enter,
                crossterm::event::KeyModifiers::NONE,
            );
            self.handle_key(enter);
        }
    }

    pub fn apply_cli_overrides(
        &mut self,
        provider: Option<String>,
        model: Option<String>,
        auto_approve: Option<String>,
        ssh_target: Option<String>,
    ) {
        if let Some(prov_str) = provider {
            if let Some(p_type) = crate::config::ProviderType::from_key(&prov_str) {
                self.config.default_provider = p_type;
                self.agent
                    .reload_config(self.config.clone(), Some(self.event_tx.clone()));
            }
        }
        if let Some(model_str) = model {
            let p_key = self.config.default_provider.key_str().to_string();
            if let Some(p_conf) = self.config.providers.get_mut(&p_key) {
                p_conf.model = model_str;
                self.agent
                    .reload_config(self.config.clone(), Some(self.event_tx.clone()));
            }
        }
        if let Some(lvl_str) = auto_approve {
            match lvl_str.to_lowercase().as_str() {
                "off" | "none" => self.config.auto_approve = crate::config::AutoApproveLevel::Off,
                "safe" | "read_only" | "readonly" => {
                    self.config.auto_approve = crate::config::AutoApproveLevel::Safe
                }
                "sudo" | "standard" => {
                    self.config.auto_approve = crate::config::AutoApproveLevel::Sudo
                }
                "yolo" | "all" | "auto" => {
                    self.config.auto_approve = crate::config::AutoApproveLevel::Yolo
                }
                _ => {}
            }
        }
        if let Some(ssh) = ssh_target {
            let cmd = format!("ssh {}\n", ssh);
            let _ = self.pty.write_all(cmd.as_bytes());
            self.focus = Focus::Terminal;
        }
    }

    pub fn new_session(&mut self) {
        if self.agent.is_generating {
            self.stop_agent_generation();
        }
        self.save_current_session();
        let active_provider = self.config.default_provider.display_name();
        let active_model = self.config.get_active_provider_config().model.clone();
        self.current_session = Session::new(active_provider, &active_model);
        self.messages.clear();
        // Wholesale reset: positional render cache must not leak across sessions.
        self.chat_render_cache.borrow_mut().hard_reset();
        self.chat_history.clear();
        self.history_index = None;
        self.input_draft.clear();
        self.chat_input.clear();
        self.cursor_pos = 0;
        self.reset_chat_scroll();
        self.modal = ModalState::None;
        let lang = self.config.get_language();
        self.set_toast(if lang == Language::Fr {
            format!(
                "✨ Nouvelle session démarrée ({} • {})",
                active_provider, active_model
            )
        } else {
            format!(
                "✨ New session started ({} • {})",
                active_provider, active_model
            )
        });
    }

    pub fn trigger_context_probe(&self) {
        probe_model_context(&self.config, self.detected_context_window.clone());
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Chat => Focus::Terminal,
            Focus::Terminal => Focus::Chat,
        };
    }

    pub fn adjust_split(&mut self, delta: i16) {
        let new_ratio = (self.split_ratio as i16 + delta).clamp(15, 85) as u16;
        self.split_ratio = new_ratio;
        self.config.split_ratio = Some(new_ratio);
        let _ = self.config.save();
    }

    pub fn set_theme(&mut self, theme_id: ThemeId) {
        self.theme = theme_id;
        self.config.theme = Some(theme_id.key_str().to_string());
        let _ = self.config.save();
        self.set_toast(format!("Thème : {}", theme_id.display_name()));
    }

    pub fn default_export_path(&self) -> String {
        let now = chrono::Local::now();
        let date_str = now.format("%Y-%m-%d").to_string();
        let title_slug = self
            .current_session
            .title
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != '-', "_")
            .trim_matches('_')
            .to_string();

        let file_name = if title_slug.is_empty() {
            format!("spiritty_rapport_{}.md", date_str)
        } else {
            let max_len = title_slug.len().min(35);
            // Safety: slice on a char boundary so a multi-byte (e.g. accented) title can't panic.
            let cut = title_slug.floor_char_boundary(max_len);
            format!("spiritty_rapport_{}_{}.md", date_str, &title_slug[..cut])
        };

        if let Some(ref dir) = self.config.export_dir {
            let clean_dir = dir.trim_end_matches('/');
            format!("{}/{}", clean_dir, file_name)
        } else {
            format!("~/{}", file_name)
        }
    }

    pub fn export_current_session_markdown_to(
        &mut self,
        target_path: &str,
    ) -> Result<String, std::io::Error> {
        let resolved_path = expand_tilde(target_path);
        if let Some(parent) = resolved_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let now = chrono::Local::now();
        let mut content = String::new();
        content.push_str(&format!(
            "# 👻 Rapport d'Intervention Spiritty — {}\n\n",
            self.current_session.title
        ));
        content.push_str(&format!(
            "- **Date & Heure :** {}\n",
            now.format("%Y-%m-%d %H:%M:%S")
        ));
        content.push_str(&format!(
            "- **Session ID :** `{}`\n",
            self.current_session.id
        ));
        content.push_str(&format!(
            "- **Fournisseur & Modèle :** {} (`{}`)\n",
            self.current_session.provider, self.current_session.model
        ));
        content.push_str(&format!(
            "- **Environnement Cible :** {}\n",
            self.system_context.active_session.display_label()
        ));
        if let Some(ref profile) = self.system_context.active_remote_profile {
            content.push_str(&format!(
                "- **Profil Serveur Distant :** {} (Kernel: {}, Init: {})\n",
                profile.distro, profile.kernel, profile.init_system
            ));
        } else {
            content.push_str(&format!(
                "- **Distribution Locale :** {} (Kernel: {})\n",
                self.system_context.distro, self.system_context.kernel
            ));
        }
        if let Some(ref cwd) = self.system_context.current_dir {
            content.push_str(&format!("- **Répertoire de travail (PWD) :** `{}`\n", cwd));
        }
        if let Some(ref branch) = self.system_context.git_branch {
            content.push_str(&format!("- **Branche Git :** `{}`\n", branch));
        }
        content.push_str("\n---\n\n## 📜 Historique des Échanges & Commandes\n\n");

        for msg in &self.messages {
            match msg.role {
                MessageRole::User => {
                    if msg
                        .content
                        .starts_with("[RÉSULTAT DE L'OUTIL POUR LA COMMANDE '")
                    {
                        content.push_str(&format!(
                            "> 💻 **Résultat d'exécution :**\n```\n{}\n```\n\n",
                            msg.content
                        ));
                    } else if msg.content.starts_with("💻 ") {
                        content.push_str(&format!(
                            "> 💻 **Commande exécutée :** `{}`\n\n",
                            msg.content.trim_start_matches("💻 ")
                        ));
                    } else {
                        content.push_str(&format!("### 👤 Utilisateur\n\n{}\n\n", msg.content));
                    }
                }
                MessageRole::Assistant => {
                    content.push_str(&format!(
                        "### 👻 Spiritty (Assistant IA)\n\n{}\n\n",
                        msg.content
                    ));
                }
                MessageRole::System => {}
            }
        }

        content.push_str("---\n*Rapport généré automatiquement par [Spiritty](https://github.com/xorne-git/Spiritty) — AI Companion for Sysadmins & DevOps.*\n");

        std::fs::write(&resolved_path, content)?;
        let path_str = resolved_path.to_string_lossy().to_string();
        Ok(path_str)
    }

    pub fn export_current_session_markdown(&mut self) -> Result<String, std::io::Error> {
        let default_path = self.default_export_path();
        self.export_current_session_markdown_to(&default_path)
    }

    pub fn trigger_proactive_diagnosis(&mut self) {
        if let Some(diag) = self.proactive_error_diagnosis.take() {
            let prompt_text = format!(
                "La commande suivante a échoué :\n```bash\n{}\n```\nVoici le message / code d'erreur obtenu :\n```\n{}\n```\nPeux-tu analyser précisément la cause de cet échec et me donner la solution / commande corrective ?",
                diag.command, diag.error_message
            );

            self.messages.push(ChatMessage {
                role: MessageRole::User,
                content: prompt_text,
                command_proposal: None,
            });

            self.messages.push(ChatMessage {
                role: MessageRole::Assistant,
                content: String::new(),
                command_proposal: None,
            });

            self.reset_chat_scroll();
            self.generation_start_time = Some(std::time::Instant::now());
            self.current_turn_tokens = 0;
            self.focus = Focus::Chat;

            let _ = self.agent.send_prompt(
                self.messages.clone(),
                &self.system_context,
                self.event_tx.clone(),
            );
        }
    }

    pub fn get_active_model_name(&self) -> String {
        self.config.get_active_provider_config().model
    }

    pub fn get_active_provider_name(&self) -> &'static str {
        self.config.default_provider.display_name()
    }

    pub fn get_context_window_limit(&self) -> usize {
        let active_cfg = self.config.get_active_provider_config();
        if let Some(ctx) = active_cfg.context_window {
            if ctx > 0 {
                return ctx;
            }
        }

        let probed = self.detected_context_window.load(Ordering::Relaxed);
        if probed > 0 {
            return probed;
        }

        let model = self.get_active_model_name().to_lowercase();

        // 1. Explicit size indicator in model name
        if model.contains("8k") || model.contains("8192") {
            return 8_192;
        }
        if model.contains("4k") || model.contains("4096") {
            return 4_096;
        }
        if model.contains("16k") || model.contains("16384") {
            return 16_384;
        }
        if model.contains("32k") || model.contains("32768") {
            return 32_768;
        }
        if model.contains("64k") || model.contains("65536") {
            return 65_536;
        }
        if model.contains("128k") || model.contains("131072") {
            return 131_072;
        }
        if model.contains("200k") {
            return 200_000;
        }
        if model.contains("1m") || model.contains("1000k") {
            return 1_048_576;
        }

        // 2. Known cloud / large models
        if model.contains("gemini") {
            1_048_576
        } else if model.contains("claude") {
            200_000
        } else if model.contains("deepseek-v4")
            || model.contains("grok")
            || model.contains("gpt-4")
            || model.contains("gpt-5")
            || model.contains("o1")
            || model.contains("o3")
            || model.contains("glm")
            || model.contains("zai")
        {
            131_072
        } else {
            // Local models fallback context window
            8_192
        }
    }

    pub fn get_total_tokens_used(&self) -> usize {
        let total_chars: usize =
            self.messages.iter().map(|m| m.content.len()).sum::<usize>() + self.chat_input.len();
        ((total_chars as f64) / 3.8).ceil() as usize
    }

    /// Estimates the tokens actually sent to the model for the current conversation.
    /// The session JSON keeps the full history (option C) while the LLM context is
    /// compacted at request time (summary + 8 most recent turns verbatim), so we must
    /// estimate the COMPACTED context — not the whole history. Otherwise the footer
    /// over-reports "Ctx: 178k / 131k" (full history vs the model window) and clamps
    /// to 100% even though the model never receives that much.
    pub fn get_context_used_tokens(&self) -> usize {
        let compacted = crate::session::compact_chat_messages(&self.messages);
        let mut total_chars: usize = self.chat_input.len() + 1500;
        if let Some(summary) = &compacted.summary {
            total_chars += summary.len();
        }
        total_chars += compacted
            .messages
            .iter()
            .map(|m| m.content.len())
            .sum::<usize>();
        ((total_chars as f64) / 3.8).ceil() as usize
    }

    pub fn get_tokens_per_sec(&self) -> Option<f64> {
        if self.agent.is_generating
            && self.pending_tool_approval.is_none()
            && self.active_pty_tool.is_none()
        {
            if let (Some(first), Some(last_chunk)) = (self.first_chunk_time, self.last_chunk_time) {
                // If model is actively emitting chunks (< 800ms), compute live streaming speed
                if last_chunk.elapsed().as_millis() < 800 {
                    let secs = first.elapsed().as_secs_f64();
                    if secs > 0.2 && self.current_turn_tokens > 0 {
                        return Some(self.current_turn_tokens as f64 / secs);
                    }
                }
            }
        }
        self.last_tokens_per_sec
    }

    pub fn update_terminal_size(&mut self, area: Rect) {
        if area.width > 0
            && area.height > 0
            && self.terminal_inner_size != (area.height, area.width)
        {
            self.terminal_inner_size = (area.height, area.width);
            let _ = self.pty.resize(area.height, area.width);
        }
    }

    pub fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent, total_width: u16) {
        use crossterm::event::{MouseButton, MouseEventKind};
        let x = mouse.column;
        let y = mouse.row;
        self.mouse_pos = Some((x, y));

        // Do not handle split dragging if a modal is open
        if !matches!(self.modal, ModalState::None) {
            return;
        }

        let border_x = self.chat_area.right();

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if x >= border_x.saturating_sub(1) && x <= border_x.saturating_add(1) {
                    self.is_dragging_split = true;
                    self.mouse_selection = None;
                } else if self.chat_area.contains(ratatui::layout::Position { x, y }) {
                    self.focus = Focus::Chat;
                    self.is_dragging_split = false;
                    if self.toggle_thought_on_click(x, y) {
                        return;
                    }
                    self.mouse_selection = Some(MouseSelection {
                        panel: SelectionPanel::Chat,
                        start: (x, y),
                        end: (x, y),
                        is_selecting: true,
                    });
                } else if self
                    .terminal_area
                    .contains(ratatui::layout::Position { x, y })
                {
                    self.focus = Focus::Terminal;
                    self.is_dragging_split = false;
                    self.mouse_selection = Some(MouseSelection {
                        panel: SelectionPanel::Terminal,
                        start: (x, y),
                        end: (x, y),
                        is_selecting: true,
                    });
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.is_dragging_split && total_width > 0 {
                    let pct = ((x as u32 * 100) / total_width as u32) as u16;
                    self.split_ratio = pct.clamp(15, 85);
                } else if let Some(ref mut sel) = self.mouse_selection {
                    sel.end = (x, y);
                    sel.is_selecting = true;
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if self.is_dragging_split {
                    self.config.split_ratio = Some(self.split_ratio);
                    let _ = self.config.save();
                }
                self.is_dragging_split = false;
                if let Some(ref mut sel) = self.mouse_selection {
                    sel.end = (x, y);
                    sel.is_selecting = false;
                }
            }
            MouseEventKind::ScrollUp => {
                if self.chat_area.contains(ratatui::layout::Position { x, y }) {
                    self.scroll_chat_up(1);
                } else if self
                    .terminal_area
                    .contains(ratatui::layout::Position { x, y })
                {
                    self.pty.scroll_up(2);
                }
            }
            MouseEventKind::ScrollDown => {
                if self.chat_area.contains(ratatui::layout::Position { x, y }) {
                    self.scroll_chat_down(1);
                } else if self
                    .terminal_area
                    .contains(ratatui::layout::Position { x, y })
                {
                    self.pty.scroll_down(2);
                }
            }
            _ => {}
        }
    }

    pub fn handle_paste(&mut self, text: String) {
        match &mut self.modal {
            ModalState::SshReconnect { .. } => {
                // Modal open: pasted text is swallowed.
            }
            ModalState::Config(config_state) => {
                config_state.handle_paste(text);
                return;
            }
            ModalState::Export(export_state) => {
                export_state.handle_paste(text);
                return;
            }
            ModalState::Mcp(mcp_state) => {
                mcp_state.handle_paste(text);
                return;
            }
            ModalState::Bookmarks(bm_state) => {
                bm_state.handle_paste(text);
                return;
            }
            ModalState::Help | ModalState::Sessions(_) => return,
            ModalState::None => {}
        }

        match self.focus {
            Focus::Chat => {
                for c in text.chars() {
                    if c == '\r' {
                        continue;
                    }
                    self.chat_input.insert(self.cursor_pos, c);
                    self.cursor_pos += c.len_utf8();
                }
            }
            Focus::Terminal => {
                let _ = self.pty.write_all(text.as_bytes());
            }
        }
    }

    pub fn cycle_auto_approve(&mut self) -> crate::config::AutoApproveLevel {
        let next_level = self.config.auto_approve.next();
        self.config.auto_approve = next_level;
        self.agent
            .reload_config(self.config.clone(), Some(self.event_tx.clone()));
        let _ = self.config.save();
        next_level
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // 0. F10 fast-track approval of a pending command card — works from BOTH panes
        // (typing ok/oui + Enter every time gets tedious during long audit sessions).
        if key.code == KeyCode::F(10) && self.pending_tool_approval.is_some() {
            self.chat_input.clear();
            self.cursor_pos = 0;
            if let Some(mut pending) = self.pending_tool_approval.take() {
                if let Some(tx) = pending.approval_tx.take() {
                    let _ = tx.send(true);
                }
            }
            return;
        }

        // 1. Global modal triggers & shortcuts
        if key.code == KeyCode::F(1) {
            self.modal = match self.modal {
                ModalState::Help => ModalState::None,
                _ => ModalState::Help,
            };
            return;
        }

        // F3 or Ctrl+Y cycles through Auto-Approve modes (Safe -> Sudo -> YOLO -> Off -> Safe)
        if key.code == KeyCode::F(3)
            || (key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')))
        {
            self.cycle_auto_approve();
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('p') | KeyCode::Char('P'))
        {
            self.modal = match self.modal {
                ModalState::Config(_) => ModalState::None,
                _ => {
                    self.probe_provider_models(ProviderType::LmStudio);
                    self.probe_provider_models(ProviderType::Ollama);
                    if self.config.default_provider != ProviderType::LmStudio
                        && self.config.default_provider != ProviderType::Ollama
                    {
                        self.probe_provider_models(self.config.default_provider);
                    }
                    ModalState::Config(ConfigModalState::from_config(&self.config))
                }
            };
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('h') | KeyCode::Char('H'))
        {
            self.save_current_session();
            self.modal = match self.modal {
                ModalState::Sessions(_) => ModalState::None,
                _ => ModalState::Sessions(SessionModalState::new(self.current_session.id.clone())),
            };
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('b') | KeyCode::Char('B'))
        {
            self.modal = match self.modal {
                ModalState::Bookmarks(_) => ModalState::None,
                _ => {
                    let active_ssh = self
                        .system_context
                        .active_session
                        .ssh_target()
                        .map(|s| s.to_string());
                    ModalState::Bookmarks(BookmarksModalState::new(&self.hosts_store, active_ssh))
                }
            };
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('e') | KeyCode::Char('E'))
        {
            self.modal = match self.modal {
                ModalState::Export(_) => ModalState::None,
                _ => {
                    let default_path = self.default_export_path();
                    ModalState::Export(ExportModalState::new(default_path))
                }
            };
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('m') | KeyCode::Char('M'))
        {
            self.modal = match self.modal {
                ModalState::Mcp(_) => ModalState::None,
                _ => {
                    let cached = self.agent.mcp_manager.get_server_statuses_cached();
                    let mut server_statuses = Vec::new();
                    for (name, s_cfg) in &self.config.mcp_servers {
                        if let Some(existing) = cached.iter().find(|s| s.name == *name) {
                            server_statuses.push(existing.clone());
                        } else {
                            server_statuses.push(crate::agent::mcp::manager::McpServerStatus {
                                name: name.clone(),
                                command: s_cfg.command.clone(),
                                args: s_cfg.args.clone(),
                                enabled: s_cfg.enabled,
                                status: if s_cfg.enabled {
                                    crate::agent::mcp::manager::McpStatus::Connected(0)
                                } else {
                                    crate::agent::mcp::manager::McpStatus::Disabled
                                },
                                tools: Vec::new(),
                            });
                        }
                    }
                    ModalState::Mcp(crate::ui::components::McpModalState::new(server_statuses))
                }
            };
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('f') | KeyCode::Char('F'))
        {
            self.chat_search_active = !self.chat_search_active;
            if self.chat_search_active {
                self.chat_search_query.clear();
                self.chat_search_cursor = 0;
                self.chat_search_match_idx = 0;
                self.focus = Focus::Chat;
            }
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('n') | KeyCode::Char('N'))
        {
            self.new_session();
            return;
        }

        // 2. If a modal is open, it captures all keys
        if let ModalState::SshReconnect { ref target } = self.modal {
            let target = target.clone();
            match key.code {
                KeyCode::Enter => {
                    let cmd = format!("ssh {}\n", target);
                    let _ = self.pty.write_all(cmd.as_bytes());
                    self.focus = Focus::Terminal;
                    self.modal = ModalState::None;
                    let lang = self.config.get_language();
                    self.set_toast(if lang == Language::Fr {
                        format!("🔗 Connexion à {}…", target)
                    } else {
                        format!("🔗 Connecting to {}…", target)
                    });
                }
                KeyCode::Esc | KeyCode::Char('n' | 'N' | 'q' | 'Q') => {
                    self.modal = ModalState::None;
                }
                _ => {}
            }
            return;
        }
        let session_action = if let ModalState::Sessions(ref mut session_state) = self.modal {
            session_state.handle_key(key)
        } else {
            None
        };

        if let Some(action) = session_action {
            match action {
                SessionModalAction::Load(id) => {
                    self.load_session(&id);
                    self.close_sessions_modal_after_load();
                }
                SessionModalAction::NewSession => {
                    self.new_session();
                    self.modal = ModalState::None;
                }
                SessionModalAction::Close => {
                    self.modal = ModalState::None;
                }
            }
            return;
        }

        let bookmark_action = if let ModalState::Bookmarks(ref mut bm_state) = self.modal {
            bm_state.handle_key(key, &mut self.hosts_store)
        } else {
            None
        };

        if let Some(action) = bookmark_action {
            match action {
                BookmarksModalAction::Connect(target) => {
                    self.modal = ModalState::None;
                    let cmd = format!("ssh {}\n", target);
                    let _ = self.pty.write_all(cmd.as_bytes());
                    self.focus = Focus::Terminal;
                }
                BookmarksModalAction::TriggerScan => {
                    self.trigger_host_scan();
                }
                BookmarksModalAction::Close => {
                    self.modal = ModalState::None;
                }
            }
            return;
        }

        let export_action = if let ModalState::Export(ref mut exp_state) = self.modal {
            exp_state.handle_key(key)
        } else {
            None
        };

        if let Some(action) = export_action {
            match action {
                ExportModalAction::Export(target_path) => {
                    self.modal = ModalState::None;
                    match self.export_current_session_markdown_to(&target_path) {
                        Ok(resolved_path) => {
                            let compact = crate::system::format_compact_path(&resolved_path);
                            self.set_toast(format!("📝 Rapport exporté : {}", compact));
                        }
                        Err(e) => {
                            self.set_toast(format!("❌ Erreur export : {}", e));
                        }
                    }
                }
                ExportModalAction::Close => {
                    self.modal = ModalState::None;
                }
            }
            return;
        }

        let mcp_action = if let ModalState::Mcp(ref mut mcp_state) = self.modal {
            mcp_state.handle_key(key, &mut self.config)
        } else {
            None
        };

        if let Some(action) = mcp_action {
            match action {
                crate::ui::components::McpModalAction::ServersChanged => {
                    self.agent
                        .reload_config(self.config.clone(), Some(self.event_tx.clone()));
                    if let ModalState::Mcp(ref mut mcp_state) = self.modal {
                        mcp_state.sync_with_config(&self.config);
                    }
                }
                crate::ui::components::McpModalAction::Close => {
                    self.modal = ModalState::None;
                }
            }
            return;
        }

        match &mut self.modal {
            ModalState::SshReconnect { .. } => {
                // Already fully handled (and returned) above; arm kept for
                // exhaustiveness of the generic modal dispatch.
            }
            ModalState::Help => {
                if key.code == KeyCode::Esc || key.code == KeyCode::Enter {
                    self.modal = ModalState::None;
                }
                return;
            }
            ModalState::Config(config_state) => {
                let action = config_state.handle_key(key, &mut self.config);
                self.theme = config_state.theme;
                match action {
                    crate::ui::components::ConfigModalAction::SaveAndClose => {
                        self.agent
                            .reload_config(self.config.clone(), Some(self.event_tx.clone()));
                        self.trigger_context_probe();
                        self.modal = ModalState::None;
                    }
                    crate::ui::components::ConfigModalAction::Close => {
                        self.modal = ModalState::None;
                    }
                    crate::ui::components::ConfigModalAction::UpdatePricing => {
                        self.trigger_pricing_update();
                    }
                    crate::ui::components::ConfigModalAction::None => {}
                }
                return;
            }
            ModalState::Sessions(_)
            | ModalState::Bookmarks(_)
            | ModalState::Export(_)
            | ModalState::Mcp(_) => return,
            ModalState::None => {}
        }

        // 3. Alt + D for proactive error diagnosis, Alt + X/C to dismiss, Alt + 1..9 / AZERTY to execute proposed command cards, and Alt+Left / Alt+Right for split resize
        if key.modifiers.contains(KeyModifiers::ALT) {
            if matches!(key.code, KeyCode::Char('d') | KeyCode::Char('D'))
                && self.proactive_error_diagnosis.is_some()
            {
                self.trigger_proactive_diagnosis();
                return;
            }

            if matches!(
                key.code,
                KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Char('c') | KeyCode::Char('C')
            ) && self.proactive_error_diagnosis.is_some()
            {
                self.proactive_error_diagnosis = None;
                return;
            }

            if let Some(idx) = key_to_card_index(key.code) {
                if self.execute_command_by_index(idx, true) {
                    return;
                }
            }

            match key.code {
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('[') => {
                    self.adjust_split(-3);
                    return;
                }
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(']') => {
                    self.adjust_split(3);
                    return;
                }
                _ => {}
            }
        }

        // 4. Universal focus toggle keys
        let is_shift_tab = key.code == KeyCode::BackTab;
        let is_ctrl_space =
            key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(' ');
        let is_f6 = key.code == KeyCode::F(6);

        if is_shift_tab || is_ctrl_space || is_f6 {
            self.toggle_focus();
            return;
        }

        // 5. Global quit
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
        {
            self.should_quit = true;
            return;
        }

        match self.focus {
            Focus::Terminal => self.handle_terminal_key(key),
            Focus::Chat => self.handle_chat_key(key),
        }
    }

    fn handle_terminal_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::PageUp
            || (key.code == KeyCode::Up && key.modifiers.contains(KeyModifiers::SHIFT))
        {
            self.pty.scroll_up(15);
            return;
        }
        if key.code == KeyCode::PageDown
            || (key.code == KeyCode::Down && key.modifiers.contains(KeyModifiers::SHIFT))
        {
            self.pty.scroll_down(15);
            return;
        }

        // Any regular keystroke resets scroll to 0 (live terminal)
        if self.pty.scroll_offset() > 0 {
            self.pty.reset_scroll();
        }

        // If user is typing normal commands, dismiss any lingering error toast
        if self.proactive_error_diagnosis.is_some()
            && (!key.modifiers.contains(KeyModifiers::ALT)
                || !matches!(
                    key.code,
                    KeyCode::Char('d')
                        | KeyCode::Char('D')
                        | KeyCode::Char('x')
                        | KeyCode::Char('X')
                        | KeyCode::Char('c')
                        | KeyCode::Char('C')
                ))
        {
            self.proactive_error_diagnosis = None;
        }

        match key.code {
            KeyCode::Enter => {
                let cmd = self.terminal_input_buffer.trim().to_string();
                if !cmd.is_empty() {
                    self.last_user_terminal_command = Some(cmd);
                }
                self.terminal_input_buffer.clear();
            }
            KeyCode::Backspace => {
                self.terminal_input_buffer.pop();
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.terminal_input_buffer.push(c);
            }
            _ => {}
        }

        let bytes = key_event_to_pty_bytes(key, self.pty.app_cursor_mode());
        if !bytes.is_empty() {
            let _ = self.pty.write_all(&bytes);
        }
    }

    pub fn on_tick(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);

        // Periodically poll active foreground session (every ~360ms)
        if self.spinner_frame.is_multiple_of(4) {
            self.poll_active_session();
        }

        if let Some(ref mut capture) = self.active_pty_tool {
            let elapsed_since_start = capture.start_time.elapsed();
            let elapsed_since_last_output = capture.last_output_time.elapsed();
            let is_waiting_password = is_waiting_for_password(char_safe_tail(
                &capture.decoded_text,
                PASSWORD_WINDOW_BYTES,
            ));

            // Debug: one status line per second while a capture is pending.
            if self.spinner_frame.is_multiple_of(11) {
                capture_debug(&format!(
                    "TICK elapsed={}ms quiet={}ms shell_has_hooks={} is_remote={} waiting_pw={} len={} pending_utf8={}",
                    elapsed_since_start.as_millis(),
                    elapsed_since_last_output.as_millis(),
                    self.pty.shell_name().to_lowercase().contains("bash")
                        || self.pty.shell_name().to_lowercase().contains("zsh")
                        || self.pty.shell_name().to_lowercase().contains("fish"),
                    matches!(
                        self.system_context.active_session,
                        crate::system::ActiveSession::Ssh { .. }
                    ),
                    is_waiting_password,
                    capture.decoded_text.len(),
                    capture.pending_utf8.len()
                ));
            }
            // Debug: once, around the 5s mark, dump the exact captured bytes so a
            // missing/malformed sentinel can be diagnosed after the fact.
            let dump_now = elapsed_since_start.as_millis() >= 5000
                && elapsed_since_start.as_millis() < 5100;
            if dump_now && capture.sentinel_pending.is_none() {
                capture_debug(&format!(
                    "DUMP decoded_text={:?} pending_utf8={:?} raw_len={}",
                    capture.decoded_text, capture.pending_utf8, capture.output_bytes.len()
                ));
            }

            let mut timeout_secs = 45u64;
            if is_waiting_password {
                // Still bounded (was u64::MAX — an infinite wait): a human answers a
                // sudo prompt within seconds, while a detector false-positive (output
                // tail merely containing "password:") would previously hang the capture
                // FOREVER on remote sessions where no sentinel ever arrives.
                timeout_secs = PASSWORD_WAIT_HARD_CAP_SECS;
            }

            // A hooked local shell (bash/zsh/fish) reliably emits the OSC 777 sentinel as soon
            // as the command completes. For those shells we MUST wait for the sentinel — the
            // silence-based settle below is only a fallback for shells that deliver no sentinel
            // (remote SSH without our hooks, or sh/dash/ash), where it previously fired too early
            // on a quiet stretch (network I/O, sudo password wait) and truncated the output.
            let is_remote = matches!(
                self.system_context.active_session,
                crate::system::ActiveSession::Ssh { .. }
            );
            let shell_name = self.pty.shell_name().to_lowercase();
            let shell_has_hooks = !is_remote
                && (shell_name.contains("bash")
                    || shell_name.contains("zsh")
                    || shell_name.contains("fish"));

            // Prompt-aware fast settle (non-hooked shells only): when a remote command
            // completes, the remote shell re-displays its PS1. If the output tail ends
            // with a prompt-looking line and has been quiet for 800ms, the command is
            // done — no need to burn the full 3s silence fallback on EVERY remote
            // command (this was the dominant latency in remote file-edit loops).
            let last_output_line =
                strip_ansi_sequences(char_safe_tail(&capture.decoded_text, PASSWORD_WINDOW_BYTES))
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .map(str::to_string)
                    .unwrap_or_default();
            let remote_prompt_quiet = !last_output_line.is_empty()
                && !shell_has_hooks
                && !is_waiting_password
                && elapsed_since_last_output >= std::time::Duration::from_millis(800)
                && is_prompt_remnant(&last_output_line);

            let has_output_settled = !shell_has_hooks
                && !is_waiting_password
                && !capture.decoded_text.is_empty()
                && elapsed_since_start >= std::time::Duration::from_millis(800)
                && (elapsed_since_last_output >= std::time::Duration::from_millis(3000)
                    || remote_prompt_quiet);

            // Never conclude from a capture that holds NOTHING beyond the echoed
            // command itself: on a slow remote (SSH latency, mysql handshakes) the
            // real output can land seconds after the echo, and settling then produced
            // an EMPTY result falsely reported as "exécutée avec succès" (user report:
            // two commands in a row captured nothing while working on the VPS). If the
            // cleaned output is empty we keep waiting — only the hard timeout (45s, or
            // the 120s password cap) may conclude, with an explicit warning below.
            //
            // `clean_pty_output` is O(N) over the WHOLE capture, so it must NOT run on
            // every tick: it used to (~11×/s while a capture was active) and a chatty
            // command made the per-frame cost grow quadratically into an apparent UI
            // freeze (freeze report, 2026-08-28). It is only needed once the output
            // has settled or the hard timeout has been reached, and the length-keyed
            // cache below makes repeated ticks on a quiet buffer free.
            let timeout_reached = elapsed_since_start.as_secs() >= timeout_secs;
            let mut clean_is_empty = true;
            if has_output_settled || timeout_reached {
                let len_now = capture.decoded_text.len();
                let cached_len = capture.clean_cache.as_ref().map(|(l, _)| *l);
                if cached_len != Some(len_now) {
                    let cleaned = clean_pty_output(&capture.decoded_text, &capture.command);
                    clean_is_empty = cleaned.is_empty();
                    capture.clean_cache = Some((len_now, cleaned));
                } else {
                    clean_is_empty = capture
                        .clean_cache
                        .as_ref()
                        .map(|(_, s)| s.is_empty())
                        .unwrap_or(true);
                }
            }
            let may_conclude = (!clean_is_empty && has_output_settled) || timeout_reached;

            if may_conclude {
                capture_debug(&format!(
                    "CONCLUDED via={} elapsed={}ms len={} clean_is_empty={} truncated={} waiting_pw={}",
                    if timeout_reached { "TIMEOUT" } else { "SETTLE" },
                    elapsed_since_start.as_millis(),
                    capture.decoded_text.len(),
                    clean_is_empty,
                    capture.truncated,
                    is_waiting_password
                ));
                let clean_output = match &capture.clean_cache {
                    Some((l, s)) if *l == capture.decoded_text.len() => s.clone(),
                    _ => clean_pty_output(&capture.decoded_text, &capture.command),
                };
                // Explicit failure signal instead of a false "success": lets the
                // model retry (shell not ready, session dropped, silent command)
                // instead of reasoning on an imaginary success.
                let final_summary = build_capture_summary(
                    0,
                    clean_output,
                    capture.truncated,
                    capture.overflow_bytes,
                    capture.decoded_text.len(),
                    "⚠️ Aucune sortie capturée : la commande a été injectée mais le terminal n'a rien renvoyé d'exploitable. Le shell distant n'était peut-être pas prêt, la session a pu être interrompue, ou la commande n'a produit ni sortie ni erreur. Vérifie l'état du terminal (invite visible ?) et relance une commande minimale (ex. `pwd`) si besoin.".to_string(),
                );

                let command = capture.command.clone();
                let auto_prompt = capture.auto_prompt;
                let result_tx = capture.result_tx.take();
                self.active_pty_tool = None;

                if auto_prompt {
                    self.record_command_result(command, final_summary);
                } else if let Some(tx) = result_tx {
                    let _ = tx.send(final_summary);
                }
            }
        }
    }

    pub fn poll_active_session(&mut self) {
        if let Some(child_pid) = self.pty.child_pid() {
            let new_session = crate::system::detect_active_session(child_pid);
            if new_session != self.system_context.active_session {
                self.on_active_session_changed(new_session);
            }

            // Also refresh PWD and Git branch on local sessions
            if !self.system_context.active_session.is_ssh() {
                if let Some(cwd) = crate::system::detect_current_working_dir(child_pid) {
                    let branch = crate::system::detect_git_branch(&cwd);
                    self.system_context.current_dir = Some(cwd);
                    self.system_context.git_branch = branch;
                }
            }
        }
    }

    pub fn on_active_session_changed(&mut self, new_session: ActiveSession) {
        let was_ssh = self.system_context.active_session.is_ssh();
        self.system_context.active_session = new_session.clone();

        match new_session {
            ActiveSession::Ssh { target, .. } => {
                if let Some(profile) = self.hosts_store.get(&target) {
                    self.system_context.active_remote_profile = Some(profile.clone());
                    self.set_toast(format!("🌐 SSH: {} ({})", target, profile.distro));
                } else {
                    self.system_context.active_remote_profile = None;
                    self.set_toast(format!("🌐 SSH: {}", target));
                    self.trigger_background_host_probe(target);
                }
            }
            ActiveSession::Container {
                runtime,
                container_id,
            } => {
                self.system_context.active_remote_profile = None;
                self.set_toast(format!("📦 {}: {}", runtime, container_id));
            }
            ActiveSession::Local { .. } => {
                self.system_context.active_remote_profile = None;
                if was_ssh {
                    self.set_toast("🖥️ Retour à l'environnement local".to_string());
                }
            }
        }
    }

    pub fn trigger_background_host_probe(&mut self, target: String) {
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            let probe_cmd = HostsStore::generate_probe_command();
            let res = tokio::process::Command::new("ssh")
                .args([
                    "-o",
                    "BatchMode=yes",
                    "-o",
                    "ConnectTimeout=4",
                    "-o",
                    "StrictHostKeyChecking=accept-new",
                    &target,
                    probe_cmd,
                ])
                .output()
                .await;
            if let Ok(out) = res {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout).to_string();
                    let _ = event_tx.send(AppEvent::RemoteHostProbed {
                        target,
                        output: text,
                    });
                }
            }
        });
    }

    pub fn on_remote_host_probed(&mut self, target: String, output: String) {
        if let Some(profile) = HostsStore::parse_probe_output(&target, &output) {
            let distro_name = profile.distro.clone();
            let _ = self.hosts_store.upsert(profile.clone());
            if let Some(active_target) = self.system_context.active_session.ssh_target() {
                if self
                    .hosts_store
                    .get(active_target)
                    .map(|p| p.target.as_str())
                    == Some(&profile.target)
                    || active_target == target
                    || target.contains(active_target)
                    || active_target.contains(&target)
                {
                    self.system_context.active_remote_profile = Some(profile);
                    self.set_toast(format!(
                        "🌐 {} — Profil {} enregistré",
                        active_target, distro_name
                    ));
                }
            }
            if let ModalState::Bookmarks(ref mut bm_state) = self.modal {
                bm_state.refresh(&self.hosts_store);
            }
        }
    }

    pub fn trigger_host_scan(&mut self) {
        if let Some(target) = self
            .system_context
            .active_session
            .ssh_target()
            .map(|s| s.to_string())
        {
            self.set_toast("🌐 Scan de l'environnement distant en arrière-plan...".to_string());
            self.trigger_background_host_probe(target);
        } else {
            self.set_toast("ℹ️ Le scan est réservé aux sessions SSH distantes".to_string());
        }
    }

    pub fn set_toast(&mut self, msg: String) {
        self.toast_message = Some((std::time::Instant::now(), msg));
    }

    pub fn find_search_matches(&self) -> Vec<usize> {
        if self.chat_search_query.trim().is_empty() {
            return Vec::new();
        }
        let q = self.chat_search_query.to_lowercase();
        let mut matches = Vec::new();
        for (idx, msg) in self.messages.iter().enumerate() {
            if msg.content.to_lowercase().contains(&q) {
                matches.push(idx);
            }
        }
        matches
    }

    pub fn all_command_proposals(&self) -> Vec<String> {
        if let Some(ref injected) = self.last_injected_cmd {
            return vec![injected.clone()];
        }
        for msg in self.messages.iter().rev() {
            if msg.role == MessageRole::Assistant {
                let proposals = extract_all_command_proposals(&msg.content);
                if !proposals.is_empty() {
                    return proposals;
                }
            }
        }
        Vec::new()
    }

    pub fn latest_command_proposal(&self) -> Option<String> {
        self.all_command_proposals().into_iter().next()
    }

    pub fn current_active_shell(&self) -> &str {
        match &self.system_context.active_session {
            crate::system::ActiveSession::Ssh { .. } => "bash",
            crate::system::ActiveSession::Local {
                foreground_process: Some(proc),
            } if proc == "bash"
                || proc == "zsh"
                || proc == "sh"
                || proc == "dash"
                || proc == "ash" =>
            {
                proc.as_str()
            }
            _ => self.pty.shell(),
        }
    }

    pub fn execute_command_by_index(&mut self, index: usize, auto_run: bool) -> bool {
        self.proactive_error_diagnosis = None;
        // One consent at a time: while a tool authorization is pending, Alt+N must NOT
        // fire — it would bypass the very consent being requested (the model's proposal
        // card and the approval card can carry the same command). While a PTY capture is
        // running, a new injection would hijack/overwrite the active capture.
        if self.pending_tool_approval.is_some() {
            let lang = self.config.get_language();
            self.set_toast(if lang == crate::i18n::Language::Fr {
                "⏳ Une demande d'autorisation est en cours : répondez d'abord (F10 / oui / Esc)"
                    .to_string()
            } else {
                "⏳ A permission request is pending: answer it first (F10 / yes / Esc)".to_string()
            });
            return false;
        }
        if self.active_pty_tool.is_some() {
            let lang = self.config.get_language();
            self.set_toast(if lang == crate::i18n::Language::Fr {
                "⏳ Une commande est en cours d'exécution dans le terminal — attends la fin de la capture".to_string()
            } else {
                "⏳ A command is currently running in the terminal — wait for the capture to finish".to_string()
            });
            return false;
        }
        let proposals = self.all_command_proposals();
        if let Some(cmd) = proposals.get(index).cloned() {
            let clean_cmd = clean_multiline_command(&cmd);

            if auto_run {
                self.last_injected_cmd = None;
                self.agent.is_generating = true;

                let is_sudo = clean_cmd.trim().starts_with("sudo") || clean_cmd.contains(" sudo ");
                if is_sudo {
                    self.focus = Focus::Terminal;
                }

                // 1. Add User action and Assistant placeholder in Chat history
                self.messages.push(ChatMessage {
                    role: MessageRole::User,
                    content: format!("💻 `{}`", clean_cmd),
                    command_proposal: None,
                });
                self.messages.push(ChatMessage {
                    role: MessageRole::Assistant,
                    content: String::new(),
                    command_proposal: None,
                });

                // 2. Launch execution in live PTY with output capture.
                //    `auto_prompt = true`: the app records the [RÉSULTAT...] in the chat history and
                //    triggers the next model turn itself once the capture completes, so the command
                //    output is persisted and visible (not only passed to the model ephemerally).
                let (result_tx, _result_rx) = tokio::sync::oneshot::channel::<String>();
                self.on_agent_pty_tool_execute(clean_cmd.clone(), result_tx, true);

                return true;
            } else {
                let shell = self.current_active_shell();
                let is_remote = matches!(
                    self.system_context.active_session,
                    crate::system::ActiveSession::Ssh { .. }
                );
                let pty_cmd = format_command_for_pty_with_session(&cmd, shell, is_remote, false);
                // Clear any dirty prompt buffer cleanly without printing ^C
                let _ = self.pty.write_all(b"\x15");
                let _ = self.pty.write_all(pty_cmd.as_bytes());
                self.last_injected_cmd = Some(cmd.clone());
                return true;
            }
        }
        false
    }

    pub fn execute_proposed_command(&mut self, auto_run: bool) -> bool {
        self.execute_command_by_index(0, auto_run)
    }

    fn handle_chat_key(&mut self, key: KeyEvent) {
        // 1. If there is a pending tool execution approval, intercept decisions & natural phrases
        if self.pending_tool_approval.is_some() {
            if key.code == KeyCode::Enter {
                let input = self.chat_input.trim().to_lowercase();
                if is_natural_decline_phrase(&input) {
                    self.chat_input.clear();
                    self.cursor_pos = 0;
                    if let Some(mut pending) = self.pending_tool_approval.take() {
                        if let Some(tx) = pending.approval_tx.take() {
                            let _ = tx.send(false);
                        }
                    }
                    return;
                } else if !input.is_empty() && is_natural_approval_phrase(&input) {
                    self.chat_input.clear();
                    self.cursor_pos = 0;
                    if let Some(mut pending) = self.pending_tool_approval.take() {
                        if let Some(tx) = pending.approval_tx.take() {
                            let _ = tx.send(true);
                        }
                    }
                    return;
                }
            } else if key.code == KeyCode::Esc {
                self.chat_input.clear();
                self.cursor_pos = 0;
                if let Some(mut pending) = self.pending_tool_approval.take() {
                    if let Some(tx) = pending.approval_tx.take() {
                        let _ = tx.send(false);
                    }
                }
                return;
            }
        }

        // 2. Chat Search input interceptor (Ctrl+F active)
        if self.chat_search_active {
            match key.code {
                KeyCode::Esc => {
                    self.chat_search_active = false;
                    self.chat_search_query.clear();
                    return;
                }
                KeyCode::Enter => {
                    let matches = self.find_search_matches();
                    if !matches.is_empty() {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            if self.chat_search_match_idx == 0 {
                                self.chat_search_match_idx = matches.len() - 1;
                            } else {
                                self.chat_search_match_idx -= 1;
                            }
                        } else {
                            self.chat_search_match_idx =
                                (self.chat_search_match_idx + 1) % matches.len();
                        }
                    }
                    return;
                }
                KeyCode::Backspace => {
                    if self.chat_search_cursor > 0 {
                        let mut chars: Vec<char> = self.chat_search_query.chars().collect();
                        chars.remove(self.chat_search_cursor - 1);
                        self.chat_search_query = chars.into_iter().collect();
                        self.chat_search_cursor -= 1;
                        self.chat_search_match_idx = 0;
                    }
                    return;
                }
                KeyCode::Left => {
                    self.chat_search_cursor = self.chat_search_cursor.saturating_sub(1);
                    return;
                }
                KeyCode::Right => {
                    if self.chat_search_cursor < self.chat_search_query.chars().count() {
                        self.chat_search_cursor += 1;
                    }
                    return;
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    let mut chars: Vec<char> = self.chat_search_query.chars().collect();
                    chars.insert(self.chat_search_cursor, c);
                    self.chat_search_query = chars.into_iter().collect();
                    self.chat_search_cursor += 1;
                    self.chat_search_match_idx = 0;
                    return;
                }
                _ => {}
            }
        }

        // 3. Esc or Ctrl+C / Ctrl+S cancels active generation or active PTY tool
        let is_stop_key = key.code == KeyCode::Esc
            || (key.modifiers.contains(KeyModifiers::CONTROL)
                && (key.code == KeyCode::Char('c') || key.code == KeyCode::Char('s')));

        if is_stop_key && self.agent.is_generating {
            self.stop_agent_generation();
            return;
        }

        // 3. Alt+1..9 or Alt+&.._ (AZERTY) to execute a specific proposed command card
        if key.modifiers.contains(KeyModifiers::ALT) {
            if let Some(idx) = key_to_card_index(key.code) {
                if self.execute_command_by_index(idx, true) {
                    return;
                }
            }
        }

        match key.code {
            KeyCode::Enter => {
                // Shift+Enter, Alt+Enter or Ctrl+Enter inserts a new line in the multiline prompt
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || key.modifiers.contains(KeyModifiers::ALT)
                    || key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    self.chat_input.insert(self.cursor_pos, '\n');
                    self.cursor_pos += 1;
                    return;
                }

                let input = self.chat_input.trim().to_string();
                if !input.is_empty() && !self.agent.is_generating {
                    // Check if input is a natural command execution request ("ok", "oui", "vas y", "lance", "2", "lance 2", etc.)
                    let proposals = self.all_command_proposals();
                    if let Some(target_idx) =
                        parse_command_execution_request(&input, proposals.len())
                    {
                        self.chat_input.clear();
                        self.cursor_pos = 0;
                        self.history_index = None;
                        self.input_draft.clear();
                        if self.execute_command_by_index(target_idx, true) {
                            return;
                        }
                    }

                    // Record in prompt history if non-empty and not identical to last entry
                    if self.chat_history.last() != Some(&input) {
                        self.chat_history.push(input.clone());
                    }
                    self.history_index = None;
                    self.input_draft.clear();

                    // Clear any lingering error diagnosis upon new message
                    self.proactive_error_diagnosis = None;

                    // Push User message
                    self.messages.push(ChatMessage {
                        role: MessageRole::User,
                        content: input.clone(),
                        command_proposal: None,
                    });

                    // Prepare placeholder for streaming response
                    self.messages.push(ChatMessage {
                        role: MessageRole::Assistant,
                        content: String::new(),
                        command_proposal: None,
                    });

                    self.chat_input.clear();
                    self.cursor_pos = 0;
                    self.reset_chat_scroll();
                    self.generation_start_time = Some(std::time::Instant::now());
                    self.current_turn_tokens = 0;

                    // Trigger LLM streaming with live system context
                    let _ = self.agent.send_prompt(
                        self.messages.clone(),
                        &self.system_context,
                        self.event_tx.clone(),
                    );
                }
            }

            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.cursor_pos > 0 {
                    let before = &self.chat_input[..self.cursor_pos];
                    let trimmed = before.trim_end();
                    let new_pos = trimmed.rfind(' ').map(|i| i + 1).unwrap_or(0);
                    self.chat_input.drain(new_pos..self.cursor_pos);
                    self.cursor_pos = new_pos;
                }
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.chat_input.drain(..self.cursor_pos);
                self.cursor_pos = 0;
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.chat_input.truncate(self.cursor_pos);
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor_pos = 0;
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor_pos = self.chat_input.len();
            }
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+J is ASCII linefeed (universal multiline newline shortcut)
                self.chat_input.insert(self.cursor_pos, '\n');
                self.cursor_pos += 1;
            }
            KeyCode::Char('v') | KeyCode::Char('V')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                // Asynchronous paste: the clipboard read happens on a background thread and
                // comes back as a regular Paste event, so a hung clipboard manager can never
                // freeze the UI thread (the old path blocked up to 1.5 s per keystroke).
                let paste_tx = self.event_tx.clone();
                crate::system::clipboard::spawn_paste_request(move |text| {
                    if let Some(text) = text {
                        let _ = paste_tx.send(AppEvent::Paste(text));
                    }
                });
            }
            KeyCode::Char('\n') | KeyCode::Char('\r') => {
                self.chat_input.insert(self.cursor_pos, '\n');
                self.cursor_pos += 1;
            }
            KeyCode::Char('a') | KeyCode::Char('A')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                // Readline muscle memory: start of the CURRENT logical line
                // (not of the whole buffer) so multi-line prompts stay editable.
                self.cursor_pos = self.chat_input[..self.cursor_pos.min(self.chat_input.len())]
                    .rfind('\n')
                    .map(|i| i + 1)
                    .unwrap_or(0);
            }
            KeyCode::Char('e') | KeyCode::Char('E')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                let base = self.cursor_pos.min(self.chat_input.len());
                self.cursor_pos = self.chat_input[base..]
                    .find('\n')
                    .map(|i| base + i)
                    .unwrap_or(self.chat_input.len());
            }
            KeyCode::Char(c) => {
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                {
                    self.chat_input.insert(self.cursor_pos, c);
                    self.cursor_pos += c.len_utf8();
                }
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    let prev_idx = self.chat_input[..self.cursor_pos]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.chat_input.remove(prev_idx);
                    self.cursor_pos = prev_idx;
                }
            }
            KeyCode::Delete => {
                if self.cursor_pos < self.chat_input.len() {
                    self.chat_input.remove(self.cursor_pos);
                }
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos = self.chat_input[..self.cursor_pos]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                }
            }
            KeyCode::Right => {
                if self.cursor_pos < self.chat_input.len() {
                    self.cursor_pos = self.chat_input[self.cursor_pos..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| self.cursor_pos + i)
                        .unwrap_or(self.chat_input.len());
                }
            }
            KeyCode::Home => {
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    self.scroll_chat_up(u16::MAX / 2);
                } else {
                    self.cursor_pos = 0;
                }
            }
            KeyCode::End => {
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    self.reset_chat_scroll();
                } else {
                    self.cursor_pos = self.chat_input.len();
                }
            }
            KeyCode::PageUp => {
                let page = self.chat_area.height.saturating_sub(2).max(5);
                self.scroll_chat_up(page);
            }
            KeyCode::PageDown => {
                let page = self.chat_area.height.saturating_sub(2).max(5);
                self.scroll_chat_down(page);
            }
            KeyCode::Up => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.scroll_chat_up(1);
                } else {
                    // Multi-line / wrapped input: navigate visual rows first.
                    let width = self.chat_area.width.saturating_sub(4).max(1) as usize;
                    match prompt_move_cursor_vertical(&self.chat_input, width, self.cursor_pos, -1)
                    {
                        Some(new_byte) => self.cursor_pos = new_byte,
                        // First visual row: fall back to history navigation.
                        None if !self.chat_history.is_empty() => {
                            let new_idx = match self.history_index {
                                None => {
                                    self.input_draft = self.chat_input.clone();
                                    self.chat_history.len().saturating_sub(1)
                                }
                                Some(idx) => idx.saturating_sub(1),
                            };
                            self.history_index = Some(new_idx);
                            if let Some(cmd) = self.chat_history.get(new_idx) {
                                self.chat_input = cmd.clone();
                                self.cursor_pos = self.chat_input.len();
                            }
                        }
                        None => {}
                    }
                }
            }
            KeyCode::Down => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.scroll_chat_down(1);
                } else {
                    let width = self.chat_area.width.saturating_sub(4).max(1) as usize;
                    match prompt_move_cursor_vertical(&self.chat_input, width, self.cursor_pos, 1) {
                        Some(new_byte) => self.cursor_pos = new_byte,
                        // Last visual row: fall back to history navigation.
                        None => {
                            if let Some(idx) = self.history_index {
                                if idx + 1 < self.chat_history.len() {
                                    let new_idx = idx + 1;
                                    self.history_index = Some(new_idx);
                                    if let Some(cmd) = self.chat_history.get(new_idx) {
                                        self.chat_input = cmd.clone();
                                        self.cursor_pos = self.chat_input.len();
                                    }
                                } else {
                                    self.history_index = None;
                                    self.chat_input = self.input_draft.clone();
                                    self.cursor_pos = self.chat_input.len();
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Esc => {
                if self.proactive_error_diagnosis.is_some() {
                    self.proactive_error_diagnosis = None;
                    return;
                }
                if !self.chat_input.is_empty() {
                    self.chat_input.clear();
                    self.cursor_pos = 0;
                    self.history_index = None;
                    self.input_draft.clear();
                }
            }
            _ => {}
        }
    }

    pub fn on_agent_chunk(&mut self, chunk: String) {
        let now = std::time::Instant::now();
        if self.first_chunk_time.is_none() {
            self.first_chunk_time = Some(now);
        }
        self.current_turn_chars += chunk.len();
        self.current_turn_tokens = ((self.current_turn_chars as f64) / 3.8).ceil() as usize;

        if let Some(first) = self.first_chunk_time {
            let secs = first.elapsed().as_secs_f64();
            if secs > 0.2 && self.current_turn_tokens > 0 {
                self.last_tokens_per_sec = Some(self.current_turn_tokens as f64 / secs);
            }
        }
        self.last_chunk_time = Some(now);

        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                last_msg.content.push_str(&chunk);
                last_msg.command_proposal = extract_command_proposal(&last_msg.content);
            }
        }
        self.chat_scroll_from_bottom = 0;
    }

    pub fn on_agent_tool_request(
        &mut self,
        command: String,
        approval_tx: tokio::sync::oneshot::Sender<bool>,
    ) {
        // While a tool awaits approval, the user must answer (oui / non) — put the focus on the
        // chat input so they can type the approval without switching panels.
        self.focus = Focus::Chat;
        self.chat_scroll_from_bottom = 0;

        if let Some(start) = self.generation_start_time.take() {
            let secs = start.elapsed().as_secs_f64();
            if secs > 0.2 && self.current_turn_tokens > 0 {
                self.last_tokens_per_sec = Some(self.current_turn_tokens as f64 / secs);
            }
        }

        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                if let Some(idx) = last_msg.content.find("```tool:") {
                    last_msg.content = last_msg.content[..idx].trim_end().to_string();
                }
            }
        }
        self.pending_tool_approval = Some(PendingToolApproval {
            command,
            approval_tx: Some(approval_tx),
        });
    }

    pub fn on_agent_tool_start(&mut self, command: String) {
        if let Some(start) = self.generation_start_time.take() {
            let secs = start.elapsed().as_secs_f64();
            if secs > 0.2 && self.current_turn_tokens > 0 {
                self.last_tokens_per_sec = Some(self.current_turn_tokens as f64 / secs);
            }
        }
        self.pending_tool_approval = None;
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                // Strip the trailing ```tool:... code block if present
                if let Some(idx) = last_msg.content.find("```tool:") {
                    last_msg.content.truncate(idx);
                }
                let clean_base = last_msg.content.trim_end().to_string();
                if command.starts_with("🌐") {
                    if clean_base.is_empty() {
                        last_msg.content = format!("{}...", command);
                    } else {
                        last_msg.content = format!("{}\n\n{}...", clean_base, command);
                    }
                } else if clean_base.is_empty() {
                    last_msg.content = format!("💻 `{}`...", command);
                } else {
                    last_msg.content = format!("{}\n\n💻 `{}`...", clean_base, command);
                }
            }
        }
        self.chat_scroll_from_bottom = 0;
    }

    pub fn on_agent_tool_done(&mut self, command: String, output: String) {
        self.pending_tool_approval = None;
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                let mut content = last_msg.content.clone();
                let tool_indicator_start = format!("💻 `{}`...", command);
                let web_indicator_start = format!("{}...", command);
                if let Some(pos) = content.rfind(&tool_indicator_start) {
                    content.replace_range(
                        pos..pos + tool_indicator_start.len(),
                        &format!("💻 `{}`", command),
                    );
                    last_msg.content = content;
                } else if let Some(pos) = content.rfind(&web_indicator_start) {
                    content.replace_range(pos..pos + web_indicator_start.len(), &command);
                    last_msg.content = content;
                }
            }
        }

        // Persist the tool result into the chat history so it survives across turns.
        // (The agent engine also keeps it in its internal conversation for the current turn.)
        // `🌐` = web search, `🔌` = MCP tool — those keep their own result format internally.
        if !command.starts_with("🌐") && !command.starts_with("🔌") {
            self.messages.push(ChatMessage {
                role: MessageRole::User,
                content: format!(
                    "[RÉSULTAT DE L'OUTIL POUR LA COMMANDE '{}']:\n{}",
                    command, output
                ),
                command_proposal: None,
            });
        }
        self.chat_scroll_from_bottom = 0;
    }

    pub fn on_agent_pty_tool_execute(
        &mut self,
        command: String,
        result_tx: tokio::sync::oneshot::Sender<String>,
        auto_prompt: bool,
    ) {
        let shell = self.current_active_shell();
        let is_remote = matches!(
            self.system_context.active_session,
            crate::system::ActiveSession::Ssh { .. }
        );
        let formatted_cmd = format_command_for_pty_with_session(&command, shell, is_remote, true);

        // Clear any dirty prompt buffer cleanly without printing ^C
        let _ = self.pty.write_all(b"\x15");
        let _ = self.pty.write_all(formatted_cmd.as_bytes());

        let is_sudo = command.trim().starts_with("sudo") || command.contains(" sudo ");
        if is_sudo {
            self.focus = Focus::Terminal;
        }

        let now = std::time::Instant::now();
        capture_debug(&format!(
            "ARMED cmd={:?} shell={} shell_name={} is_remote={} auto_prompt={}",
            command,
            self.pty.shell(),
            self.pty.shell_name(),
            is_remote,
            auto_prompt
        ));
        self.active_pty_tool = Some(PtyToolCapture {
            command,
            result_tx: Some(result_tx),
            output_bytes: Vec::new(),
            decoded_text: String::new(),
            pending_utf8: Vec::new(),
            sentinel_pending: None,
            sentinel_scan_upto: 0,
            start_time: now,
            last_output_time: now,
            auto_prompt,
            truncated: false,
            overflow_bytes: 0,
            overflow_tail: Vec::new(),
            clean_cache: None,
        });
    }

    /// Records a completed user-command execution into the chat history (as a `[RÉSULTAT...]`
    /// user message) and triggers the next model turn so the output is persisted and visible.
    fn record_command_result(&mut self, command: String, final_summary: String) {
        // Drop the empty assistant placeholder that was inserted when the command was submitted.
        if let Some(last) = self.messages.last() {
            if last.role == MessageRole::Assistant && last.content.trim().is_empty() {
                self.messages.pop();
            }
        }
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: format!(
                "[RÉSULTAT DE L'EXÉCUTION DE LA COMMANDE '{}']:\n{}\n[Analysez ce résultat et expliquez la situation à l'utilisateur]",
                command, final_summary
            ),
            command_proposal: None,
        });
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: String::new(),
            command_proposal: None,
        });
        self.chat_scroll_from_bottom = 0;
        self.focus = Focus::Chat;
        // Fresh generation segment for the analysis turn (no AgentNewTurn event is
        // emitted on this path): re-arm the reflection timer and per-segment counters
        // so the "Deep thinking… mm:ss" line and the tokens/sec accounting start clean.
        self.generation_start_time = Some(std::time::Instant::now());
        self.first_chunk_time = None;
        self.current_turn_chars = 0;
        self.current_turn_tokens = 0;
        let _ = self.agent.send_prompt(
            self.messages.clone(),
            &self.system_context,
            self.event_tx.clone(),
        );
    }

    pub fn on_pty_output(&mut self, bytes: &[u8]) {
        let mut password_prompt_detected = false;
        if let Some(ref mut capture) = self.active_pty_tool {
            capture.last_output_time = std::time::Instant::now();

            if capture.output_bytes.len() >= MAX_CAPTURE_BYTES {
                // Overflow mode: buffering stopped once the cap was hit. Keep counting
                // the discarded bytes and keep watching a small rolling tail of the raw
                // stream for the completion sentinel (it arrives at the very END of the
                // output, after everything that was discarded) so a runaway command
                // still concludes as soon as it finishes instead of burning the full
                // 45s hard timeout.
                capture.truncated = true;
                capture.overflow_bytes += bytes.len() as u64;
                capture.overflow_tail.extend_from_slice(bytes);
                let stale = capture.overflow_tail.len().saturating_sub(256);
                if stale > 0 {
                    capture.overflow_tail.drain(..stale);
                }
                if let Some(exit_code) = scan_completed_sentinel(&capture.overflow_tail) {
                    let clean_output = clean_pty_output(&capture.decoded_text, &capture.command);
                    let empty_summary = if exit_code == 0 {
                        "(Commande exécutée avec succès dans le terminal)".to_string()
                    } else {
                        format!("(Commande terminée avec le code {})", exit_code)
                    };
                    let final_summary = build_capture_summary(
                        exit_code,
                        clean_output,
                        capture.truncated,
                        capture.overflow_bytes,
                        capture.decoded_text.len(),
                        empty_summary,
                    );
                    let command = capture.command.clone();
                    let auto_prompt = capture.auto_prompt;
                    let result_tx = capture.result_tx.take();
                    self.active_pty_tool = None;

                    if auto_prompt {
                        self.record_command_result(command, final_summary);
                    } else if let Some(tx) = result_tx {
                        let _ = tx.send(final_summary);
                    }
                }
            } else {
                capture.output_bytes.extend_from_slice(bytes);

                // Incremental UTF-8 decode: only the newly arrived bytes are processed, with a
                // carry for multi-byte sequences split across chunks. The whole-buffer
                // from_utf8_lossy this replaces ran on every PTY chunk and made long-running,
                // verbose commands degrade quadratically.
                capture.pending_utf8.extend_from_slice(bytes);
                utf8_decode_incremental(&mut capture.decoded_text, &mut capture.pending_utf8);

                // Password prompts always appear at the tail of the current output — a bounded
                // window keeps the check O(1) instead of O(buffer).
                if is_waiting_for_password(char_safe_tail(
                    &capture.decoded_text,
                    PASSWORD_WINDOW_BYTES,
                )) {
                    password_prompt_detected = true;
                }

                // Locate the OSC 777 sentinel incrementally. Once seen (complete), its position
                // is parked in `sentinel_pending` and subsequent chunks only search onward for
                // the terminator instead of re-scanning the accumulated buffer.
                //
                // NOTE: we must NOT match the bare `777;spiritty_done;` substring — the command echo
                // is literally `printf '\033]777;spiritty_done;%s\007' $?`, which contains that text
                // (as literal `\033`, no real ESC byte). Matching it would end the capture the moment
                // the command is echoed, mis-parse `\007` as a bogus exit code, and truncate the output.
                const OSC_PREFIX_LEN: usize = "\x1b]777;spiritty_done;".len();
                const PLAIN_PREFIX_LEN: usize = "__SPIRITTY_DONE__:".len();

                if capture.sentinel_pending.is_none() {
                    // Incremental scan with a watermark: rescanning the whole buffer on
                    // every chunk is O(N²) on verbose output, but the old "last 20 chars
                    // only" window missed the sentinel whenever a chunk larger than the
                    // pattern delivered it mid-chunk (echo + output + sentinel coalesced
                    // into one PTY read — the capture then hung until the 45s timeout).
                    // Rescan from just before the watermark so a pattern straddling a
                    // chunk boundary is still caught, and advance the watermark only over
                    // bytes proven sentinel-free.
                    let max_prefix = OSC_PREFIX_LEN.max(PLAIN_PREFIX_LEN);
                    let from = char_safe_floor(
                        &capture.decoded_text,
                        capture.sentinel_scan_upto.saturating_sub(max_prefix - 1),
                    );
                    let hay = &capture.decoded_text[from..];
                    if let Some(rel) = hay.rfind("\x1b]777;spiritty_done;") {
                        capture_debug(&format!(
                            "SENTINEL OSC SEEN at +{}ms (len={})",
                            capture.start_time.elapsed().as_millis(),
                            capture.decoded_text.len()
                        ));
                        capture.sentinel_pending = Some((from + rel, OSC_PREFIX_LEN, true));
                        capture.sentinel_scan_upto = from + rel + 1;
                    } else if let Some(rel) = hay.rfind("__SPIRITTY_DONE__:") {
                        capture_debug("SENTINEL PLAIN SEEN");
                        capture.sentinel_pending = Some((from + rel, PLAIN_PREFIX_LEN, false));
                        capture.sentinel_scan_upto = from + rel + 1;
                    } else {
                        capture.sentinel_scan_upto = capture.decoded_text.len();
                    }
                }

                if let Some((pos, prefix_len, is_osc)) = capture.sentinel_pending {
                    let text = &capture.decoded_text;
                    if pos + prefix_len <= text.len() {
                        let after = &text[pos + prefix_len..];
                        let found_terminator = if is_osc {
                            after
                                .find('\x1b')
                                .or(after.find('\x07'))
                                .or(after.find('\n'))
                                .or(after.find('\r'))
                                .or(after.find('\\'))
                                .or(after.find(';'))
                        } else {
                            after.find('\n').or(after.find('\r'))
                        };

                        if let Some(end_idx) = found_terminator {
                            let code_str =
                                after[..end_idx].trim_matches(|c: char| !c.is_ascii_digit());
                            let exit_code: i32 = code_str.parse().unwrap_or(0);
                            let raw_output = &text[..pos];
                            let clean_output = clean_pty_output(raw_output, &capture.command);
                            capture_debug(&format!(
                                "CONCLUDED via=SENTINEL code={} elapsed={}ms raw_len={} clean_len={}",
                                exit_code,
                                capture.start_time.elapsed().as_millis(),
                                raw_output.len(),
                                clean_output.len()
                            ));
                            let empty_summary = if exit_code == 0 {
                                "(Commande exécutée avec succès dans le terminal)".to_string()
                            } else {
                                format!("(Commande terminée avec le code {})", exit_code)
                            };
                            let final_summary = build_capture_summary(
                                exit_code,
                                clean_output,
                                capture.truncated,
                                capture.overflow_bytes,
                                capture.decoded_text.len(),
                                empty_summary,
                            );

                            let command = capture.command.clone();
                            let auto_prompt = capture.auto_prompt;
                            let result_tx = capture.result_tx.take();
                            self.active_pty_tool = None;

                            if auto_prompt {
                                self.record_command_result(command, final_summary);
                            } else if let Some(tx) = result_tx {
                                let _ = tx.send(final_summary);
                            }
                        }
                    }
                }
            }
        }

        if password_prompt_detected && self.focus != Focus::Terminal {
            self.focus = Focus::Terminal;
            let toast_msg = if self.config.get_language() == crate::i18n::Language::Fr {
                "🔒 Saisie du mot de passe sudo requise dans le terminal".to_string()
            } else {
                "🔒 Sudo password required in terminal".to_string()
            };
            self.set_toast(toast_msg);
        }

        // Detect shell errors ONLY for manual user commands in the live terminal
        if self.active_pty_tool.is_none() && !self.agent.is_generating {
            if let Ok(text) = std::str::from_utf8(bytes) {
                let lower = text.to_lowercase();
                if (lower.contains("command not found")
                    || lower.contains("permission denied")
                    || lower.contains("syntax error near")
                    || lower.contains("syntax error:")
                    || lower.contains("no such file or directory")
                    || lower.contains("fatal:")
                    || lower.contains("failed to ")
                    || lower.contains("cannot create directory"))
                    && !lower.contains("debug")
                    && !lower.contains("spiritty")
                    && !lower.contains("spiritty_probe")
                {
                    let cmd = self
                        .last_user_terminal_command
                        .clone()
                        .unwrap_or_else(|| "(Dernière commande shell)".to_string());
                    self.proactive_error_diagnosis = Some(ProactiveDiagnosis {
                        command: cmd,
                        error_message: text.trim().to_string(),
                    });
                }
            }
        }
    }

    pub fn on_agent_new_turn(&mut self) {
        self.first_chunk_time = None;
        self.current_turn_chars = 0;
        self.current_turn_tokens = 0;
        // Re-arm the live "Deep thinking… mm:ss" reflection timer for this segment:
        // `generation_start_time` is consumed by the tokens/sec accounting on
        // AgentToolRequest/AgentToolStart, and the engine emits AgentNewTurn for every
        // continuation turn after a tool result. Without re-arming here, the timer
        // vanished from the first tool call to the end of the generation.
        self.generation_start_time = Some(std::time::Instant::now());
        if let Some(last) = self.messages.last() {
            if last.role == MessageRole::Assistant && last.content.trim().is_empty() {
                return;
            }
        }
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: String::new(),
            command_proposal: None,
        });
        self.chat_scroll_from_bottom = 0;
    }

    pub fn on_agent_usage(
        &mut self,
        prompt_tokens: usize,
        completion_tokens: usize,
        exact_speed: Option<f64>,
    ) {
        if prompt_tokens > 0 {
            self.current_session.prompt_tokens += prompt_tokens;
        }
        if completion_tokens > 0 {
            self.current_session.completion_tokens += completion_tokens;
            self.current_turn_tokens = completion_tokens;
        }
        self.current_session.total_tokens =
            self.current_session.prompt_tokens + self.current_session.completion_tokens;
        if let Some(speed) = exact_speed {
            self.last_tokens_per_sec = Some(speed);
        }
    }

    pub fn on_mcp_servers_updated(&mut self) {
        if let ModalState::Mcp(ref mut mcp_state) = self.modal {
            let cached = self.agent.mcp_manager.get_server_statuses_cached();
            mcp_state.update_statuses(cached);
        }
    }

    pub fn on_agent_done(&mut self) {
        if let Some(first) = self.first_chunk_time.take() {
            let secs = first.elapsed().as_secs_f64();
            if secs > 0.1 && self.current_turn_tokens > 0 && self.last_tokens_per_sec.is_none() {
                self.last_tokens_per_sec = Some(self.current_turn_tokens as f64 / secs);
            }
        }
        self.last_chunk_time = None;
        self.first_chunk_time = None;
        self.current_turn_chars = 0;

        // If provider did not send native AgentUsage event, aggregate estimated tokens
        if self.current_session.total_tokens == 0 && self.current_turn_tokens > 0 {
            self.current_session.completion_tokens += self.current_turn_tokens;
            self.current_session.total_tokens = self.get_total_tokens_used();
            self.current_session.prompt_tokens = self
                .current_session
                .total_tokens
                .saturating_sub(self.current_session.completion_tokens);
        }

        self.agent.is_generating = false;
        self.pending_tool_approval = None;
        self.chat_scroll_from_bottom = 0;

        // Clean up any trailing empty assistant placeholders
        self.messages
            .retain(|m| !m.content.trim().is_empty() || m.role != MessageRole::Assistant);

        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                last_msg.command_proposal = extract_command_proposal(&last_msg.content);
            }
        }
        self.save_current_session();
    }

    pub fn on_agent_error(&mut self, error: String) {
        self.agent.is_generating = false;
        self.pending_tool_approval = None;
        self.active_pty_tool = None;
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                if last_msg.content.is_empty() {
                    last_msg.content = format!("⚠️ Erreur : {}", error);
                } else {
                    last_msg
                        .content
                        .push_str(&format!("\n\n⚠️ Erreur : {}", error));
                }
            } else {
                self.messages.push(ChatMessage {
                    role: MessageRole::Assistant,
                    content: format!("⚠️ Erreur : {}", error),
                    command_proposal: None,
                });
            }
        } else {
            self.messages.push(ChatMessage {
                role: MessageRole::Assistant,
                content: format!("⚠️ Erreur : {}", error),
                command_proposal: None,
            });
        }
        self.set_toast(format!("Erreur : {}", error));
    }

    pub fn stop_agent_generation(&mut self) {
        if self.agent.is_generating {
            self.agent.stop_generation();
            self.pending_tool_approval = None;
            self.active_pty_tool = None;
            if let Some(last_msg) = self.messages.last_mut() {
                if last_msg.role == MessageRole::Assistant {
                    if last_msg.content.trim().is_empty() {
                        last_msg.content = "(Génération interrompue par l'utilisateur)".to_string();
                    } else {
                        last_msg.command_proposal = extract_command_proposal(&last_msg.content);
                    }
                }
            }
            let lang = self.config.get_language();
            self.set_toast(if lang == Language::Fr {
                "⏹ Génération interrompue".to_string()
            } else {
                "⏹ Generation stopped".to_string()
            });
            self.save_current_session();
        }
    }
}

/// Determines if a code block is an actual executable shell command vs passive text/log/tree output
pub fn is_executable_command_block(fence_tag: &str, content: &str) -> bool {
    let tag = fence_tag.trim().to_lowercase();

    // 1. Tags that are explicitly output or data formats
    if matches!(
        tag.as_str(),
        "output"
            | "result"
            | "text"
            | "txt"
            | "log"
            | "logs"
            | "tree"
            | "table"
            | "json"
            | "yaml"
            | "toml"
            | "md"
            | "markdown"
            | "diff"
            | "status"
            | "info"
    ) {
        return false;
    }
    if tag.starts_with("tool:") {
        return false;
    }

    let trimmed = content.trim();
    if trimmed.is_empty() {
        return false;
    }

    // 2. Data-only content (IP lists etc.) is never a command, whatever the tag.
    if looks_like_ip_list(trimmed) {
        return false;
    }

    // 2.5 Explicit shell-language tags are authorial intent: the block IS a command.
    // The heuristics below (arrows, conversational phrases) exist to catch prose or
    // pasted output in UNTAGGED fences; a legitimate command may contain "->" inside a
    // quoted title (e.g. `echo "=== CLONE DB resa-v3 -> resa_pp ==="`) and must not be
    // demoted to a non-executable snippet box for that (user-reported regression).
    if matches!(
        tag.as_str(),
        "bash" | "sh" | "zsh" | "fish" | "shell" | "cmd" | "terminal" | "console"
    ) {
        return true;
    }

    // 3. Reject process trees, log formats, systemd status trees, transition arrows
    if trimmed.contains('├')
        || trimmed.contains('└')
        || trimmed.contains('│')
        || trimmed.contains("──")
        || trimmed.contains('→')
        || trimmed.contains("->")
        || trimmed.contains("=>")
    {
        return false;
    }

    // 3. Reject blocks containing obvious conversational text
    if trimmed.lines().any(|l| {
        let low = l.trim().to_lowercase();
        low.starts_with("cela ")
            || low.starts_with("souhaitez-vous ")
            || low.starts_with("vous pouvez ")
            || low.starts_with("voici ")
            || low.starts_with("puis ")
            || low.starts_with("ensuite ")
            || low.starts_with("this will ")
            || low.contains(" puis ")
            || low.contains(" vers ")
    }) {
        return false;
    }

    // 5. Untagged blocks (""): accept only if the content actually looks like a shell command.
    if tag.is_empty() {
        let lines: Vec<&str> = trimmed.lines().collect();
        // 5.1 Every non-empty line must look like a raw command line, not prose/bullets/escapes.
        if lines
            .iter()
            .any(|l| !l.trim().is_empty() && !is_clean_command_line(l.trim()))
        {
            return false;
        }
        let first_line = lines.first().copied().unwrap_or("").trim();
        if first_line.starts_with('{')
            || first_line.starts_with('[')
            || first_line.starts_with('<')
            || first_line.starts_with('#')
        {
            return false;
        }
        let colon_count = trimmed.matches(':').count();
        let line_count = lines.iter().filter(|l| !l.trim().is_empty()).count();
        if line_count > 2 && colon_count >= line_count {
            return false;
        }
        // 5.2 Reject prose-like blocks: natural-language sentences (period / comma / ! / ? followed by a
        // space, or a block ending with sentence punctuation) are almost never valid commands.
        let word_count = trimmed.split_whitespace().count();
        if word_count >= 4
            && (trimmed.contains(". ")
                || trimmed.contains(", ")
                || trimmed.contains("! ")
                || trimmed.contains("? ")
                || trimmed.contains("…"))
        {
            return false;
        }
        if (trimmed.ends_with('.')
            || trimmed.ends_with('!')
            || trimmed.ends_with('?')
            || trimmed.ends_with('…'))
            && word_count >= 2
        {
            return false;
        }
        // 5.3 A block with many words and no shell metacharacters is very likely prose, not a command.
        let has_shell_meta = [
            "|", "&", ";", ">", "<", "$", "(", ")", "{", "}", "*", "~", "`", "&&", "=",
        ]
        .iter()
        .any(|m| trimmed.contains(m));
        if word_count >= 5 && !has_shell_meta {
            return false;
        }
        // 5.4 A single `Label: value value …` line with no shell metacharacters is tabular output
        //     (e.g. "Swap:  511Mi  0B  511Mi" quoted by the model as illustration), not a command
        //     to run. Reject it so it never becomes a spurious command proposal.
        if line_count == 1 && !has_shell_meta {
            if let Some((label, rest)) = first_line.split_once(':') {
                let label = label.trim();
                if !label.is_empty()
                    && !label.contains(char::is_whitespace)
                    && !rest.trim().is_empty()
                {
                    return false;
                }
            }
        }
        // 5.5 A short capitalized sentence without any shell metacharacter ("Mail queue is
        //     empty", "No certificates found") is output the model quoted inside a plain
        //     fence, not an executable command. Reject it too.
        if line_count == 1
            && word_count >= 3
            && !has_shell_meta
            && looks_like_capitalized_prose(trimmed)
        {
            return false;
        }
        return true;
    }

    false
}

/// True for a single-line natural-language sentence that starts with an uppercase letter and is
/// otherwise made of lowercase letters, spaces, apostrophes or hyphens only — the typical shape
/// of command output quoted by models ("Mail queue is empty"). Real shell commands are either
/// lowercase, contain paths/flags/options or use shell metacharacters, so this stays conservative.
fn looks_like_capitalized_prose(s: &str) -> bool {
    let first = match s.chars().next() {
        Some(c) if c.is_uppercase() => c,
        _ => return false,
    };
    let _ = first;
    s.chars()
        .skip(1)
        .all(|c| c.is_lowercase() || c.is_whitespace() || c == '\'' || c == '-')
}

/// True if every whitespace-separated token in `content` looks like an IPv4 address (optionally
/// with a `:port` suffix). Used to reject lists of IP addresses the model presents as output
/// (e.g. "103.213.238.91 202.165.15.132 210.79.142.201") so they are not treated as commands.
fn looks_like_ip_list(content: &str) -> bool {
    let tokens: Vec<&str> = content.split_whitespace().collect();
    if tokens.is_empty() {
        return false;
    }
    tokens.iter().all(|t| is_ipv4_token(t))
}

fn is_ipv4_token(t: &str) -> bool {
    // Ignore an optional `:port` (or `/mask`) suffix.
    let host = t.split(':').next().unwrap_or(t);
    let host = host.split('/').next().unwrap_or(host);
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Repairs AI glitches where the model outputs an empty code block like:
/// ```bash\n \n```\n<script>
/// and wraps the following script back into a proper ```bash\n<script>\n``` block.
pub fn repair_prematurely_closed_code_blocks(text: &str) -> String {
    let empty_fences = [
        "```bash\n \n```\n",
        "```bash\n\n```\n",
        "```sh\n \n```\n",
        "```sh\n\n```\n",
        "```\n \n```\n",
        "```\n\n```\n",
    ];

    let mut working = text.to_string();

    for fence in empty_fences {
        if let Some(pos) = working.find(fence) {
            let before = &working[..pos];
            let after = &working[pos + fence.len()..];
            let trimmed_after = after.trim_end_matches('`').trim();
            if !trimmed_after.is_empty() {
                working = format!("{}\n\n```bash\n{}\n```", before.trim_end(), trimmed_after);
                break;
            }
        }
    }

    // Auto-close any unclosed code block (odd number of ```)
    if !working.matches("```").count().is_multiple_of(2) {
        working.push_str("\n```\n");
    }

    working
}

/// Extracts all proposed shell commands from markdown code blocks (excluding output/tools/trees)
pub fn extract_all_command_proposals(text: &str) -> Vec<String> {
    let repaired = repair_prematurely_closed_code_blocks(text);
    let mut list = Vec::new();
    let mut remaining = repaired.as_str();

    while let Some(start_idx) = remaining.find("```") {
        let after_fence = &remaining[start_idx + 3..];
        let (fence_tag, code_rest) = if let Some(first_nl) = after_fence.find('\n') {
            (after_fence[..first_nl].trim(), &after_fence[first_nl + 1..])
        } else {
            ("", after_fence)
        };

        if let Some(end_idx) = code_rest.find("```") {
            let code_content = code_rest[..end_idx].trim();
            if is_executable_command_block(fence_tag, code_content) {
                // Strip spurious interpreter framing (`bash`, `sudo sh`, shebang lines, a
                // dangling trailing `exit`) so the proposal executes the real command instead
                // of spawning a nested shell and swallowing the rest of the block.
                let cleaned = sanitize_proposed_command(code_content);
                if !cleaned.is_empty() && !list.contains(&cleaned) {
                    list.push(cleaned);
                }
            }
            remaining = &code_rest[end_idx + 3..];
        } else {
            break;
        }
    }

    list
}

/// Extracts the first proposed shell command from markdown code blocks
pub fn extract_command_proposal(text: &str) -> Option<String> {
    extract_all_command_proposals(text).into_iter().next()
}

/// True when a line is a bare interpreter invocation framing a command transcript rather than
/// part of the command itself — e.g. `bash`, `sudo sh`, `zsh`, or a shebang line. LLMs often
/// prepend such a line to a ```bash``` block (mimicking an interactive transcript); injecting
/// it into the PTY spawns a nested shell that swallows the following lines.
fn is_interpreter_invocation_line(line: &str) -> bool {
    let t = line.trim();
    if t.starts_with("#!") {
        return true;
    }
    let rest = t.strip_prefix("sudo ").unwrap_or(t).trim();
    matches!(
        rest,
        "bash" | "sh" | "zsh" | "fish" | "dash" | "ksh" | "ash"
    )
}

/// Cleans a proposed command: drops leading blank/interpreter-invocation lines (`bash`,
/// `sudo sh`, `#!…`) and trailing dangling `exit`/`logout` lines. Returns an empty string when
/// nothing executable remains. Real commands like `bash -c '…'` or `bash <<EOF` are untouched
/// because the bare-token match never fires on them.
pub fn sanitize_proposed_command(raw: &str) -> String {
    let mut lines: Vec<&str> = raw.lines().collect();

    while let Some(first) = lines.first() {
        if first.trim().is_empty() || is_interpreter_invocation_line(first) {
            lines.remove(0);
        } else {
            break;
        }
    }

    // Mirror cleanup: a trailing bare `exit` / `exit N` / `logout` would close the user's
    // own interactive shell once executed in the PTY, so it is stripped as well.
    while let Some(last) = lines.last() {
        let t = last.trim();
        let is_exit_code = t
            .strip_prefix("exit ")
            .map(|n| n.trim().parse::<i32>().is_ok())
            .unwrap_or(false);
        if t.is_empty() || t == "exit" || t == "logout" || is_exit_code {
            lines.pop();
        } else {
            break;
        }
    }

    lines.join("\n")
}

pub fn is_natural_approval_phrase(text: &str) -> bool {
    let clean = text.trim().to_lowercase();
    // NOTE: an empty string must NOT be an approval phrase — pressing bare Enter while a
    // permission card is displayed used to execute the pending command (even Risky ones).
    matches!(
        clean.as_str(),
        "ok" | "oui"
            | "o"
            | "yes"
            | "y"
            | "vas y"
            | "vas-y"
            | "vazy"
            | "fais le"
            | "fais-le"
            | "faisle"
            | "go"
            | "lance"
            | "exécute"
            | "execute"
            | "continue"
            | "d'accord"
            | "daccord"
            | "sure"
            | "do it"
            | "proceed"
            | "yep"
            | "ouep"
    )
}

pub fn is_natural_decline_phrase(text: &str) -> bool {
    let clean = text.trim().to_lowercase();
    matches!(
        clean.as_str(),
        "non" | "no" | "n" | "stop" | "annule" | "cancel" | "refuse" | "non merci"
    )
}

/// Parses natural language requests to execute a proposed command (e.g., "ok", "vas y", "lance", "2", "lance 2", "cmd 1")
pub fn parse_command_execution_request(text: &str, num_proposals: usize) -> Option<usize> {
    if num_proposals == 0 {
        return None;
    }
    let clean = text.trim().to_lowercase();
    if clean.is_empty() {
        return None;
    }

    // Direct affirmative phrases when proposals exist -> run first proposal (index 0)
    if is_natural_approval_phrase(&clean) {
        return Some(0);
    }

    // Numbered requests: "1", "2", "cmd 1", "commande 2", "lance 1", "lance la 2", "alt 1", "la 1"
    let patterns = [
        "commande #",
        "commande ",
        "cmd #",
        "cmd ",
        "lance la commande #",
        "lance la commande ",
        "lance la ",
        "lance le ",
        "lance #",
        "lance ",
        "exécute la commande #",
        "exécute la commande ",
        "exécute la ",
        "exécute #",
        "exécute ",
        "execute #",
        "execute ",
        "run #",
        "run ",
        "alt+",
        "alt ",
        "la ",
    ];

    let mut candidate = clean.as_str();
    for p in &patterns {
        if let Some(rest) = candidate.strip_prefix(p) {
            candidate = rest.trim();
            break;
        }
    }

    if let Ok(num) = candidate.parse::<usize>() {
        if num >= 1 && num <= num_proposals {
            return Some(num - 1);
        }
    }

    None
}

/// Converts a crossterm `KeyEvent` to standard ANSI / VT100 byte sequences for the PTY
fn key_event_to_pty_bytes(key: KeyEvent, app_cursor_mode: bool) -> Vec<u8> {
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let ascii = c.to_ascii_lowercase();
                if ascii.is_ascii_lowercase() {
                    let ctrl_byte = (ascii as u8) - b'a' + 1;
                    vec![ctrl_byte]
                } else {
                    match c {
                        '@' | ' ' => vec![0x00],
                        '[' => vec![0x1B],
                        '\\' => vec![0x1C],
                        ']' => vec![0x1D],
                        '^' => vec![0x1E],
                        '_' => vec![0x1F],
                        _ => vec![],
                    }
                }
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                let mut buf = vec![0x1B];
                let mut char_buf = [0; 4];
                buf.extend_from_slice(c.encode_utf8(&mut char_buf).as_bytes());
                buf
            } else {
                let mut char_buf = [0; 4];
                c.encode_utf8(&mut char_buf).as_bytes().to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7F],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => vec![0x1B, b'[', b'Z'],
        KeyCode::Esc => vec![0x1B],
        // Arrow / navigation keys: while the child runs an application that
        // enabled DECCKM (vim via smkx), it expects SS3 sequences; otherwise CSI.
        KeyCode::Up => {
            if app_cursor_mode {
                vec![0x1B, b'O', b'A']
            } else {
                vec![0x1B, b'[', b'A']
            }
        }
        KeyCode::Down => {
            if app_cursor_mode {
                vec![0x1B, b'O', b'B']
            } else {
                vec![0x1B, b'[', b'B']
            }
        }
        KeyCode::Right => {
            if app_cursor_mode {
                vec![0x1B, b'O', b'C']
            } else {
                vec![0x1B, b'[', b'C']
            }
        }
        KeyCode::Left => {
            if app_cursor_mode {
                vec![0x1B, b'O', b'D']
            } else {
                vec![0x1B, b'[', b'D']
            }
        }
        KeyCode::Home => {
            if app_cursor_mode {
                vec![0x1B, b'O', b'H']
            } else {
                vec![0x1B, b'[', b'H']
            }
        }
        KeyCode::End => {
            if app_cursor_mode {
                vec![0x1B, b'O', b'F']
            } else {
                vec![0x1B, b'[', b'F']
            }
        }
        KeyCode::PageUp => vec![0x1B, b'[', b'5', b'~'],
        KeyCode::PageDown => vec![0x1B, b'[', b'6', b'~'],
        KeyCode::Delete => vec![0x1B, b'[', b'3', b'~'],
        KeyCode::Insert => vec![0x1B, b'[', b'2', b'~'],
        KeyCode::F(1) => vec![0x1B, b'O', b'P'],
        KeyCode::F(2) => vec![0x1B, b'O', b'Q'],
        KeyCode::F(3) => vec![0x1B, b'O', b'R'],
        KeyCode::F(4) => vec![0x1B, b'O', b'S'],
        KeyCode::F(5) => vec![0x1B, b'[', b'1', b'5', b'~'],
        KeyCode::F(6) => vec![0x1B, b'[', b'1', b'7', b'~'],
        KeyCode::F(7) => vec![0x1B, b'[', b'1', b'8', b'~'],
        KeyCode::F(8) => vec![0x1B, b'[', b'1', b'9', b'~'],
        KeyCode::F(9) => vec![0x1B, b'[', b'2', b'0', b'~'],
        KeyCode::F(10) => vec![0x1B, b'[', b'2', b'1', b'~'],
        KeyCode::F(11) => vec![0x1B, b'[', b'2', b'3', b'~'],
        KeyCode::F(12) => vec![0x1B, b'[', b'2', b'4', b'~'],
        _ => vec![],
    }
}

/// Maps keyboard keys (including French AZERTY top-row keys) to 0-based card indices (0..8)
fn key_to_card_index(code: KeyCode) -> Option<usize> {
    match code {
        KeyCode::Char('1') | KeyCode::Char('&') => Some(0),
        KeyCode::Char('2') | KeyCode::Char('é') | KeyCode::Char('É') => Some(1),
        KeyCode::Char('3') | KeyCode::Char('"') => Some(2),
        KeyCode::Char('4') | KeyCode::Char('\'') => Some(3),
        KeyCode::Char('5') | KeyCode::Char('(') => Some(4),
        KeyCode::Char('6') | KeyCode::Char('-') => Some(5),
        KeyCode::Char('7') | KeyCode::Char('è') | KeyCode::Char('È') => Some(6),
        KeyCode::Char('8') | KeyCode::Char('_') => Some(7),
        KeyCode::Char('9') | KeyCode::Char('ç') | KeyCode::Char('Ç') => Some(8),
        _ => None,
    }
}

/// Asynchronously queries local APIs (LM Studio native /api/v0/models, Ollama /api/show)
/// to detect the exact loaded context length in real-time.
fn probe_model_context(config: &Config, target: Arc<AtomicUsize>) {
    let config = config.clone();
    tokio::spawn(async move {
        let p_cfg = config.get_active_provider_config();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(1500))
            .build()
            .unwrap_or_default();

        match config.default_provider {
            ProviderType::LmStudio => {
                let base_url = p_cfg
                    .base_url
                    .as_deref()
                    .unwrap_or("http://localhost:1234/v1");
                let root_url = base_url.trim_end_matches("/v1").trim_end_matches('/');
                let api_url = format!("{}/api/v0/models", root_url);

                if let Ok(resp) = client.get(&api_url).send().await {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                            for item in data {
                                let id =
                                    item.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                                let is_loaded =
                                    item.get("state").and_then(|v| v.as_str()) == Some("loaded");
                                if is_loaded || id == p_cfg.model {
                                    if let Some(loaded_ctx) =
                                        item.get("loaded_context_length").and_then(|v| v.as_u64())
                                    {
                                        if loaded_ctx > 0 {
                                            target.store(loaded_ctx as usize, Ordering::Relaxed);
                                            return;
                                        }
                                    }
                                    if let Some(max_ctx) =
                                        item.get("max_context_length").and_then(|v| v.as_u64())
                                    {
                                        if max_ctx > 0 {
                                            target.store(max_ctx as usize, Ordering::Relaxed);
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            ProviderType::Ollama => {
                let base_url = p_cfg
                    .base_url
                    .as_deref()
                    .unwrap_or("http://localhost:11434");
                let root_url = base_url.trim_end_matches("/v1").trim_end_matches('/');
                let api_url = format!("{}/api/show", root_url);

                let body = serde_json::json!({ "name": p_cfg.model });
                if let Ok(resp) = client.post(&api_url).json(&body).send().await {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(ctx) = json
                            .get("model_info")
                            .and_then(|m| m.get("general.context_length"))
                            .and_then(|v| v.as_u64())
                        {
                            if ctx > 0 {
                                target.store(ctx as usize, Ordering::Relaxed);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    });
}

/// Checks if the raw terminal output indicates that sudo/doas/su is waiting for password entry.
pub fn is_waiting_for_password(raw_text: &str) -> bool {
    let clean = strip_ansi_sequences(raw_text);
    let trimmed = clean.trim_end();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_lowercase();
    let last_line = lower.lines().next_back().unwrap_or(&lower).trim();

    last_line.contains("password for")
        || last_line.contains("mot de passe de")
        || last_line.contains("mot de passe pour")
        || last_line.contains("mot de passe :")
        || last_line.contains("mot de passe:")
        || last_line.contains("password:")
        || last_line.contains("password :")
        || last_line.contains("passphrase")
        || last_line.contains("authentication required")
        || last_line.contains("authenticating")
        || last_line.contains("doas password")
        || last_line.starts_with("[sudo]")
}

/// Cleans captured PTY output by stripping ANSI colors, CRs, and prompt echoes.
/// Hard cap on the bytes buffered for ONE PTY tool capture. A runaway (or hallucinating)
/// command that dumps unbounded output previously grew the capture buffer without limit
/// while `on_tick` re-ran the full O(N) `clean_pty_output` on every frame (~11×/s): the
/// UI degraded quadratically and appeared frozen. Past the cap, incoming bytes are only
/// counted (and scanned for the completion sentinel via a small rolling tail) — the
/// reported result carries an explicit truncation notice.
const MAX_CAPTURE_BYTES: usize = 1024 * 1024;

/// Builds the model-facing summary of a concluded PTY capture, unifying the sentinel,
/// settle and overflow conclusion paths. `empty_output_summary` differs by path: the
/// settle path reports an explicit failure (lets the model retry), while a sentinel or
/// overflow conclusion PROVES completion, so an empty output is reported as a success
/// (or by exit code). A truncated capture appends an explicit truncation notice so the
/// model knows the result is partial instead of reasoning on a silently cut output.
fn build_capture_summary(
    exit_code: i32,
    clean_output: String,
    truncated: bool,
    overflow_bytes: u64,
    captured_len: usize,
    empty_output_summary: String,
) -> String {
    let mut body = clean_output;
    if truncated {
        body.push_str(&format!(
            "\n⚠️ [Sortie tronquée : seuls les premiers {} Ko ont été capturés ; {:.1} Mo supplémentaires ont été reçus puis ignorés.]",
            captured_len / 1024,
            overflow_bytes as f64 / (1024.0 * 1024.0)
        ));
    }
    if body.is_empty() {
        empty_output_summary
    } else if exit_code == 0 {
        format!("Sortie dans le terminal:\n{}", body)
    } else {
        format!("Sortie dans le terminal (code {}):\n{}", exit_code, body)
    }
}

/// Scans a small rolling byte buffer for a COMPLETED OSC 777 completion sentinel
/// (`ESC ] 777 ; spiritty_done ; <code>` terminated like the main scan) and returns the
/// exit code. Only used in overflow mode, where buffering has stopped and the sentinel
/// is detected on the raw stream tail. Matching requires a real `ESC` byte, which the
/// echoed `printf '\033]777…'` hook text never contains (its escape is literal text),
/// so the command echo cannot false-positive.
fn scan_completed_sentinel(buf: &[u8]) -> Option<i32> {
    const PREFIX: &[u8] = b"\x1b]777;spiritty_done;";
    let pos = buf.windows(PREFIX.len()).position(|w| w == PREFIX)?;
    let after = &buf[pos + PREFIX.len()..];
    let end = after
        .iter()
        .position(|b| matches!(b, b'\x1b' | b'\x07' | b'\n' | b'\r' | b'\\' | b';'))?;
    let code_str = std::str::from_utf8(&after[..end]).unwrap_or("");
    let digits: String = code_str.chars().filter(|c| c.is_ascii_digit()).collect();
    Some(digits.parse().unwrap_or(0))
}

fn clean_pty_output(raw: &str, command: &str) -> String {
    let no_ansi = strip_ansi_sequences(raw);
    let no_cr = no_ansi.replace('\r', "");
    // Strip stray terminal control characters (BEL \x07, etc.) that leak into the captured
    // output. The terminal renders them as a bell or nothing, but they pollute the model-facing
    // text (e.g. a leaked OSC-sentinel BEL terminator or a prompt-start marker).
    let no_ctrl: String = no_cr
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect();

    let mut lines: Vec<&str> = no_ctrl.lines().collect();

    // Filter out internal sentinel, command echo remnants, or sudo password prompts
    lines.retain(|l| {
        let t = l.trim();
        let lower = t.to_lowercase();
        !t.is_empty()
            && !t.contains("spiritty_done")
            && !t.contains("__spiritty")
            && !t.contains("__SPIRITTY")
            && !t.contains("printf '\\e]777")
            && !t.contains("printf '\\033]777")
            && !lower.contains("password for")
            && !lower.contains("mot de passe de")
            && !lower.contains("mot de passe pour")
            && !(lower.starts_with("[sudo]") && lower.contains("password"))
    });

    // Remove ONLY the exact command echo if it appears at the beginning
    if let Some(first) = lines.first() {
        let first_t = first.trim();
        let cmd_t = command.trim();
        if first_t == cmd_t
            || first_t.starts_with(cmd_t)
            || (first_t.contains(cmd_t)
                && (first_t.contains("printf '\\033]777") || first_t.contains("printf '\\e]777")))
            || (first_t.ends_with(cmd_t) && first_t.len() <= cmd_t.len() + 10)
        {
            lines.remove(0);
        }
    }

    // Garbled-echo fallback: remote line-editor redraws (zle over SSH) can interleave
    // redraw fragments into the raw echo, duplicating chars mid-line (`logs/` →
    // `llogs/`, `…8ecd…` → `…8ecdd…`). The exact-match strip above then misses, the
    // garbled echo survives cleaning and is reported as command output — the model
    // reads it as proof its commands were "altered" and starts working around an
    // imaginary saboteur. Recognize the echo by SUBSEQUENCE containment instead:
    // insertion-only corruption still keeps every sent character in order. Guarded
    // by a length window so a genuinely longer first output line is never stripped.
    let cmd_t = command.trim();
    if !cmd_t.is_empty() {
        if let Some(first) = lines.first() {
            let first_t = first.trim();
            // Fold to alphanumeric chars: the garbling inserts chars, it does not
            // change spacing/punctuation reliably, and folding absorbs that noise.
            let fold =
                |s: &str| -> Vec<char> { s.chars().filter(|c| c.is_alphanumeric()).collect() };
            let cmd_fold = fold(cmd_t);
            let line_fold = fold(first_t);
            let plausible_len =
                line_fold.len() >= cmd_fold.len() && line_fold.len() <= cmd_fold.len() * 3 / 2 + 16;
            if plausible_len {
                // Is `cmd` a subsequence of the line? Insertion-only corruption keeps
                // every sent character in order, so a garbled echo still matches.
                let mut cursor = 0usize;
                let mut is_subseq = true;
                for &c in &cmd_fold {
                    while cursor < line_fold.len() && line_fold[cursor] != c {
                        cursor += 1;
                    }
                    if cursor == line_fold.len() {
                        is_subseq = false;
                        break;
                    }
                    cursor += 1;
                }
                if is_subseq {
                    lines.remove(0);
                }
            }
        }
    }

    // Drop trailing shell-prompt remnants. For commands that produce no real output (e.g. an empty
    // mail log), the settle fallback captures the prompt the shell re-displays afterwards — which can
    // be a single decorative symbol like `∙` (U+2219). We strip those so the model sees a clean
    // "no output" result instead of a spurious `∙`.
    while let Some(last) = lines.last() {
        if is_prompt_remnant(last) {
            lines.pop();
        } else {
            break;
        }
    }

    lines.join("\n").trim().to_string()
}

/// True if a line is (trailing) shell-prompt noise that should not be reported as command output.
/// Handles decorative one-symbol prompts (`∙`, `❯`, `➜`, `λ`, …) and standard `user@host:path$` /
/// `host:~#` style prompts.
fn is_prompt_remnant(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return true;
    }
    // Single decorative prompt symbol (e.g. `∙`, `❯`, `➜`, `λ`, `›`, `±`)
    if t.chars().count() <= 2 && t.chars().all(|c| "∙•·❯➜λ›±◆✗✔".contains(c)) {
        return true;
    }
    // Standard shell prompt: user@host:path…$ / host:~# / host:/path>  (short, ends with a marker).
    // Also matches git-aware prompts like `[user@host:path] branch(+0/-7) ±`.
    let ends_marker = t.ends_with('$')
        || t.ends_with('#')
        || t.ends_with('>')
        || t.ends_with('%')
        || t.ends_with('❯')
        || t.ends_with('➜')
        || t.ends_with('λ')
        || t.ends_with('±');
    if ends_marker && t.len() <= 96 && (t.contains(':') || t.contains('~') || t.contains('@')) {
        return true;
    }
    false
}

/// Cleans and formats a multiline command into a valid single-line command or bash script wrapper.
/// - If the command is a heredoc (`<<EOF`), script (shebang #!), or contains bash keywords (while/for/if/IFS), wraps it safely in `bash -c '...'` preserving newlines and markdown content verbatim.
/// - Otherwise strips comments (# ...), empty lines, and line-continuation backslashes (\), preserving pipelines (|) and logical operators (&&, ||).
pub fn clean_multiline_command(command: &str) -> String {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // 1. If already wrapped in `bash -c`, don't re-wrap
    if trimmed.starts_with("bash -c") {
        return trimmed.to_string();
    }

    // 2. Heredocs (`<<EOF`, `<< 'EOF'`, `<<-EOF`, etc.): strip chatter and preserve full multiline structure verbatim!
    if trimmed.contains("<<") {
        return clean_heredoc_script(trimmed);
    }

    let raw_lines: Vec<&str> = trimmed
        .lines()
        .map(|l| l.trim())
        .filter(|l| is_clean_command_line(l))
        .collect();

    if raw_lines.is_empty() {
        return String::new();
    }

    let has_shebang = raw_lines.iter().any(|l| l.starts_with("#!"));
    let has_bash_keywords = raw_lines.iter().any(|l| {
        l.starts_with("while ")
            || l.starts_with("for ")
            || l.starts_with("if [")
            || l.starts_with("if [[")
            || l.ends_with("; then")
            || l.ends_with("; do")
            || *l == "then"
            || *l == "do"
            || *l == "done"
            || *l == "fi"
            || l.contains("IFS=")
            || (l.contains('=')
                && !l.starts_with("echo ")
                && !l.starts_with("printf ")
                && l.split('=')
                    .next()
                    .map(|v| v.chars().all(|c| c.is_alphanumeric() || c == '_'))
                    .unwrap_or(false))
    });

    if has_shebang || has_bash_keywords {
        let script_lines: Vec<&str> = raw_lines
            .into_iter()
            .filter(|l| !l.starts_with("#!"))
            .collect();
        return script_lines.join("\n");
    }

    let lines: Vec<&str> = raw_lines
        .into_iter()
        .filter(|l| !l.starts_with('#'))
        .collect();

    let mut result = String::new();
    for (i, line) in lines.iter().enumerate() {
        let mut l = line.trim();
        // Strip trailing line-continuation backslashes (but not \( or escaped chars)
        while l.ends_with('\\') && !l.ends_with(r"\(") && !l.ends_with(r"\)") && !l.ends_with(r"\;")
        {
            l = l[..l.len() - 1].trim();
        }

        if i > 0 {
            let prev_trimmed = result.trim_end();
            // If the previous line didn't end with a chaining operator, pipe, or open paren, insert ' && '
            let is_open_expr = prev_trimmed.ends_with('(')
                || prev_trimmed.ends_with(r"\(")
                || prev_trimmed.ends_with('{')
                || prev_trimmed.ends_with('[')
                || l.starts_with(')')
                || l.starts_with(r"\)")
                || l.starts_with('}')
                || l.starts_with(']');

            if !prev_trimmed.ends_with("&&")
                && !prev_trimmed.ends_with("||")
                && !prev_trimmed.ends_with('|')
                && !prev_trimmed.ends_with(';')
                && !prev_trimmed.ends_with('&')
                && !is_open_expr
            {
                result.push_str(" && ");
            } else {
                result.push(' ');
            }
        }

        result.push_str(l);
    }

    sanitize_bash_command_syntax(result.trim())
}

/// Auto-corrects common LLM bash syntax errors, such as compound `{ ... }` blocks lacking `;` before `}`.
pub fn sanitize_bash_command_syntax(cmd: &str) -> String {
    let mut out = String::with_capacity(cmd.len() + 8);
    let chars: Vec<char> = cmd.chars().collect();
    let mut i = 0;
    let mut inside_single_quotes = false;
    let mut inside_double_quotes = false;
    let mut brace_stack: Vec<bool> = Vec::new(); // true = compound brace `{ ... }`

    while i < chars.len() {
        let c = chars[i];
        if c == '\'' && !inside_double_quotes {
            inside_single_quotes = !inside_single_quotes;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '"' && !inside_single_quotes {
            inside_double_quotes = !inside_double_quotes;
            out.push(c);
            i += 1;
            continue;
        }

        if !inside_single_quotes && !inside_double_quotes {
            if c == '{' {
                // Determine if this is a compound command brace `{` (preceded by whitespace, start, or operator)
                // vs parameter expansion `${VAR}` (preceded by `$`)
                let is_param = if i > 0 { chars[i - 1] == '$' } else { false };
                brace_stack.push(!is_param);
                out.push(c);
                i += 1;
                continue;
            } else if c == '}' {
                if let Some(is_compound) = brace_stack.pop() {
                    if is_compound {
                        // Check preceding non-whitespace character in `out`
                        let prev_non_ws = out.trim_end().chars().last();
                        if let Some(prev) = prev_non_ws {
                            if prev != ';'
                                && prev != '&'
                                && prev != '|'
                                && prev != '\n'
                                && prev != '{'
                            {
                                let trimmed_len = out.trim_end().len();
                                out.truncate(trimmed_len);
                                out.push(';');
                                out.push(' ');
                            }
                        }
                    }
                }
                out.push(c);
                i += 1;
                continue;
            }
        }

        out.push(c);
        i += 1;
    }

    out
}

fn is_clean_command_line(line: &str) -> bool {
    let l = line.trim();
    if l.is_empty()
        || l.starts_with("//")
        || l.starts_with("tool:run_command")
        || l.starts_with("tool:execute_command")
        || l.starts_with("```")
        || l.starts_with("---")
        || l.starts_with("===")
        || l.starts_with("• ")
        || l.starts_with("? ")
        || l.starts_with("! ")
        || l.starts_with("📌")
        || l.starts_with('✓')
        || l.starts_with('✔')
        || l.starts_with('✅')
        || l.starts_with('❌')
        || l.starts_with('>')
        || l.starts_with('|')
        || l.starts_with("</")
    {
        return false;
    }

    let lower = l.to_lowercase();
    if lower.starts_with("cela ")
        || lower.starts_with("pour ")
        || lower.starts_with("si ")
        || lower.starts_with("voici ")
        || lower.starts_with("souhaitez-vous ")
        || lower.starts_with("l'utilisateur ")
        || lower.starts_with("vous pouvez ")
        || lower.starts_with("cette commande ")
        || lower.starts_with("vérifie ")
        || lower.starts_with("verifie ")
        || lower.starts_with("fichier ")
        || lower.starts_with("note ")
        || lower.starts_with("note :")
        || lower.starts_with("exécute ")
        || lower.starts_with("execute ")
        || lower.starts_with("puis ")
        || lower.starts_with("ensuite ")
        || lower.starts_with("in order to ")
        || lower.starts_with("if you ")
        || lower.starts_with("here is ")
        || lower.starts_with("this will ")
        || lower.starts_with("check ")
    {
        return false;
    }

    true
}

/// Extracts a clean heredoc script by discarding conversational introductory or concluding text
/// while preserving the full multiline body of the heredocs verbatim.
pub fn clean_heredoc_script(script: &str) -> String {
    let lines: Vec<&str> = script.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    // 1. Find the first line where a command or heredoc starts, skipping leading comment
    //    lines (`# ...`). A leading comment is a complete (empty) command: once injected into
    //    the PTY, bash executes it immediately and re-displays the prompt, emitting the OSC
    //    completion sentinel before the heredoc body even runs — truncating the capture to the
    //    comment line alone ("seul le début du heredoc apparaît").
    let first_cmd_idx = lines
        .iter()
        .position(|l| {
            !l.trim_start().starts_with('#') && (l.contains("<<") || is_clean_command_line(l))
        })
        .unwrap_or(0);

    let mut result_lines: Vec<&str> = Vec::new();
    let mut in_heredoc = false;
    let mut active_delimiters: Vec<String> = Vec::new();

    for &line in &lines[first_cmd_idx..] {
        let trimmed = line.trim();

        if in_heredoc {
            result_lines.push(line);
            // Check if this line closes the active heredoc
            if let Some(last_delim) = active_delimiters.last() {
                if trimmed == last_delim {
                    active_delimiters.pop();
                    if active_delimiters.is_empty() {
                        in_heredoc = false;
                    }
                }
            }
        } else {
            // Outside heredoc: only include line if it opens a heredoc or is a valid command line
            if line.contains("<<") {
                result_lines.push(line);
                if let Some(delim) = extract_heredoc_delimiter(line) {
                    active_delimiters.push(delim);
                    in_heredoc = true;
                }
            } else if !line.trim_start().starts_with('#') && is_clean_command_line(line) {
                result_lines.push(line);
            }
        }
    }

    let joined = result_lines.join("\n");
    repair_missing_heredoc_terminator(&joined)
}

fn extract_heredoc_delimiter(line: &str) -> Option<String> {
    let pos = line.find("<<")?;
    let rest = &line[pos + 2..];
    let rest_trimmed = rest.trim_start_matches(['-', ' ']);

    if let Some(stripped) = rest_trimmed.strip_prefix('\'') {
        let end = stripped.find('\'')?;
        Some(stripped[..end].to_string())
    } else if let Some(stripped) = rest_trimmed.strip_prefix('"') {
        let end = stripped.find('"')?;
        Some(stripped[..end].to_string())
    } else {
        let token: String = rest_trimmed
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !token.is_empty() && !token.chars().all(|c| c.is_numeric()) {
            Some(token)
        } else {
            None
        }
    }
}

/// If a script contains a here-document (`<< EOF`, `<< 'EOF'`, `<< "EOF"`, `<<- EOF`, `<<ENDOFFILE`, etc.)
/// but the model forgot to write the closing delimiter line before the end of the code block,
/// automatically appends the missing delimiter on a new line so bash executes it cleanly.
pub fn repair_missing_heredoc_terminator(script: &str) -> String {
    let lines: Vec<&str> = script.lines().collect();
    if lines.is_empty() {
        return script.to_string();
    }

    let mut open_delimiters: Vec<String> = Vec::new();

    for line in &lines {
        let trimmed = line.trim();
        // Check if this line closes the most recent open heredoc
        if let Some(last_delim) = open_delimiters.last() {
            if trimmed == last_delim {
                open_delimiters.pop();
                continue;
            }
        }

        // Check if this line opens one or more here-documents: e.g. `cat << 'EOF'`, `cat <<EOF > file`, `<<-END`
        let mut search_from = 0;
        while let Some(pos) = line[search_from..].find("<<") {
            let abs_pos = search_from + pos + 2;
            let rest = &line[abs_pos..];
            let rest_trimmed = rest.trim_start_matches(['-', ' ']);

            // Extract the delimiter token (surrounded optionally by ' or " or naked)
            let (delim, delim_end_pos) = if let Some(stripped) = rest_trimmed.strip_prefix('\'') {
                if let Some(end) = stripped.find('\'') {
                    (
                        &stripped[..end],
                        abs_pos + (rest.len() - rest_trimmed.len()) + 1 + end + 1,
                    )
                } else {
                    ("", abs_pos)
                }
            } else if let Some(stripped) = rest_trimmed.strip_prefix('"') {
                if let Some(end) = stripped.find('"') {
                    (
                        &stripped[..end],
                        abs_pos + (rest.len() - rest_trimmed.len()) + 1 + end + 1,
                    )
                } else {
                    ("", abs_pos)
                }
            } else {
                let token: String = rest_trimmed
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                let token_len = token.len();
                if !token.is_empty() {
                    let d = &rest_trimmed[..token_len];
                    (d, abs_pos + (rest.len() - rest_trimmed.len()) + token_len)
                } else {
                    ("", abs_pos)
                }
            };

            if !delim.is_empty() && !delim.chars().all(|c| c.is_numeric()) {
                open_delimiters.push(delim.to_string());
            }

            search_from = delim_end_pos.max(abs_pos + 1);
            if search_from >= line.len() {
                break;
            }
        }
    }

    if open_delimiters.is_empty() {
        return script.to_string();
    }

    // Append missing delimiters in reverse order
    let mut repaired = script.trim_end().to_string();
    for delim in open_delimiters.into_iter().rev() {
        repaired.push('\n');
        repaired.push_str(&delim);
    }
    repaired.push('\n');
    repaired
}

/// Detects if a command uses Bash-specific syntax that Fish shell cannot parse natively
/// (such as inline variable assignments VAR=val, subshells $(..), $!, loops, or heredocs).
pub fn is_bash_specific_syntax(cmd: &str) -> bool {
    let t = cmd.trim();
    if t.contains("<<")
        || t.contains("$!")
        || t.contains("$?")
        || t.contains("${")
        || t.contains('`')
        || t.contains("export ")
        || t.contains("while ")
        || t.contains("for ")
        || t.contains("if [")
        || t.contains("if [[")
        || t.contains("; do")
        || t.contains("; then")
        || has_standalone_token(t, "done")
        || has_standalone_token(t, "fi")
        || t.contains("&& (")
        || t.starts_with('(')
    {
        return true;
    }

    // Check for inline variable assignments like BGPID=$! or PORT=3000 cmd
    for part in t.split(&[' ', ';', '&', '|'][..]) {
        let p = part.trim();
        if let Some(eq_idx) = p.find('=') {
            if eq_idx > 0 {
                let var_name = &p[..eq_idx];
                if var_name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && !p.starts_with("--")
                    && !p.starts_with('-')
                {
                    return true;
                }
            }
        }
    }

    false
}

/// Returns `true` if `word` appears as a standalone shell token (surrounded by non-identifier
/// characters), as opposed to being a substring of a larger word. This avoids false positives
/// such as `file`/`find`/`config` being detected as the bash keyword `fi`, or `done` matching
/// substrings like `done` in a filename.
fn has_standalone_token(cmd: &str, word: &str) -> bool {
    cmd.split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
        .any(|tok| tok == word)
}

/// Formats a command for reliable execution in the PTY.
/// - If the shell is Fish and the command contains Bash-specific syntax (e.g. BGPID=$!), wraps safely in `bash -c '...'`
/// - Otherwise sends the clean direct command.
///
/// NOTE: we intentionally do NOT append an inline OSC sentinel for remote-SSH commands anymore —
/// the remote shell would echo it (`; printf '\033]777;spiritty_done;%s\007' $?`), polluting the
/// terminal display. Remote completion is instead detected by the silence settle fallback in `on_tick`
/// (local hooked shells already emit the sentinel silently via PROMPT_COMMAND / precmd / fish_postexec).
pub fn format_command_for_pty_with_session(
    command: &str,
    user_shell: &str,
    is_remote: bool,
    _is_tool_capture: bool,
) -> String {
    let clean = clean_multiline_command(command);
    if clean.is_empty() {
        return String::new();
    }

    let is_fish = user_shell.contains("fish") && !is_remote;
    let needs_bash = is_fish && is_bash_specific_syntax(&clean);

    if needs_bash {
        let escaped = clean.replace('\'', "'\\''");
        format!(" bash -c '{}'\n", escaped)
    } else {
        format!(" {}\n", clean)
    }
}

pub fn format_command_for_pty(command: &str, user_shell: &str) -> String {
    format_command_for_pty_with_session(command, user_shell, false, false)
}

/// Expands `~` or `~/...` to the user's home directory.
pub fn expand_tilde(path: &str) -> std::path::PathBuf {
    if path == "~" {
        return dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

/// Strips all ANSI escape sequences, CSI controls, and OSC strings from raw PTY text.
fn strip_ansi_sequences(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1B' {
            if let Some(&next) = chars.peek() {
                if next == '[' {
                    chars.next();
                    // Consume until terminating char (@ through ~)
                    for c2 in chars.by_ref() {
                        if ('@'..='~').contains(&c2) {
                            break;
                        }
                    }
                    continue;
                } else if next == ']' {
                    chars.next();
                    // OSC sequence: consume until BEL (\x07) or ST (\x1B\\)
                    let mut prev = '\0';
                    for c2 in chars.by_ref() {
                        if c2 == '\x07' || (prev == '\x1B' && c2 == '\\') {
                            break;
                        }
                        prev = c2;
                    }
                    continue;
                } else if next == '(' || next == ')' {
                    chars.next();
                    chars.next(); // Charset designator
                    continue;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Max window used for tail-only checks (password prompts appear at the end of output).
const PASSWORD_WINDOW_BYTES: usize = 1024;

/// Hard cap for captures that LOOK like they are waiting on a password prompt.
/// A human answers sudo within seconds; the cap only guards against infinite hangs
/// when the detector false-positives (command output merely *mentioning* a password,
/// e.g. echoing a config/YAML dump) on sessions where no completion sentinel exists.
const PASSWORD_WAIT_HARD_CAP_SECS: u64 = 120;

/// Largest index ≤ `byte_idx` that lies on a UTF-8 char boundary of `s`.
fn char_safe_floor(s: &str, byte_idx: usize) -> usize {
    let mut i = byte_idx.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Char-boundary-safe slice of at most the last `max_bytes` bytes of `s`.
fn char_safe_tail(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        s
    } else {
        &s[char_safe_floor(s, s.len() - max_bytes)..]
    }
}

/// Incremental UTF-8 decoder: appends newly valid bytes to `text`, carrying incomplete
/// multi-byte sequences across calls and replacing genuinely invalid bytes with U+FFFD
/// (matching `String::from_utf8_lossy` semantics without re-scanning the whole buffer).
fn utf8_decode_incremental(text: &mut String, pending: &mut Vec<u8>) {
    loop {
        match std::str::from_utf8(pending) {
            Ok(valid) => {
                text.push_str(valid);
                pending.clear();
                break;
            }
            Err(err) => {
                let valid_up_to = err.valid_up_to();
                if valid_up_to > 0 {
                    if let Ok(chunk) = std::str::from_utf8(&pending[..valid_up_to]) {
                        text.push_str(chunk);
                    }
                    pending.drain(..valid_up_to);
                }
                match err.error_len() {
                    Some(bad_len) => {
                        // Genuinely invalid bytes: consume them, emit a replacement marker.
                        text.push('\u{FFFD}');
                        pending.drain(..bad_len.max(1));
                    }
                    None => break, // incomplete sequence: keep as carry
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn clean_strips_garbled_echo_by_subsequence() {
        // Live case: remote line-editor redraw duplicated chars mid-echo (`logs/` →
        // `llogs/`, `…8ecd…` → `…8ecdd…`). The exact-match strip misses; the garbled
        // echo must NOT be reported as command output.
        let cmd = "cd /var/www/xorne/resa-prod && grep -rn \"ae147f1d8ecd6b34b30ad432250e5f71\" logs/ | head -20";
        let raw = "cd /var/www/xorne/resa-prod && grep -rn \"ae147f1d8ecd6b34b30ad432250e5f71\" llogs/ | head -20";
        assert_eq!(clean_pty_output(raw, cmd), "");
    }

    #[test]
    fn clean_keeps_real_output_after_garbled_echo() {
        let cmd = "grep -n 61509 logs/app.log";
        let raw = "grep -n 61509 llogs/app.log\n123:ST2026082761509 found";
        assert_eq!(clean_pty_output(raw, cmd), "123:ST2026082761509 found");
    }

    #[test]
    fn clean_does_not_strip_unrelated_first_line() {
        // A first output line that is NOT an echo must be preserved as-is.
        let cmd = "cat /etc/hostname";
        let raw = "prod-server\nprod-server";
        assert_eq!(clean_pty_output(raw, cmd), "prod-server\nprod-server");
    }

    #[test]
    fn clean_subsequence_guard_rejects_much_longer_lines() {
        // A first line that merely CONTAINS fragments but is way longer than the
        // command is real output, not a garbled echo.
        let cmd = "echo hi";
        let raw = "some very long first line of real output mentioning echo and hi many times over and over";
        assert_eq!(clean_pty_output(raw, cmd), raw);
    }

    #[tokio::test]
    async fn in_app_session_load_keeps_ssh_reconnect_modal() {
        use crate::ui::components::session_modal::SessionModalState;

        let (event_tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 60, 110).unwrap();

        // In-app reload path: load_session raised the SSH-reconnect offer while the
        // sessions list modal was still active. Closing the list must NOT swallow it
        // (regression: the offer only ever survived on the `-c` startup path).
        app.modal = ModalState::SshReconnect {
            target: "ducasse-seine.com".to_string(),
        };
        app.close_sessions_modal_after_load();
        assert!(
            matches!(app.modal, ModalState::SshReconnect { .. }),
            "reconnect offer must survive the sessions list closing"
        );

        // Plain path: without a pending reconnect offer, the list modal closes.
        app.modal = ModalState::Sessions(SessionModalState::new(app.current_session.id.clone()));
        app.close_sessions_modal_after_load();
        assert!(matches!(app.modal, ModalState::None));
    }

    #[tokio::test]
    async fn deep_thinking_timer_rearms_on_continuation_turns() {
        let (event_tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 60, 110).unwrap();
        app.generation_start_time = Some(std::time::Instant::now());

        // A tool request/start consumes the timer Instant for tokens/sec accounting.
        app.on_agent_tool_start("echo hi".to_string());
        assert!(
            app.generation_start_time.is_none(),
            "timer Instant is consumed by the tokens/sec accounting"
        );

        // The continuation turn after the tool result must re-arm it, otherwise the
        // "💭 Deep thinking… mm:ss" line loses its timer for every later segment.
        app.on_agent_new_turn();
        assert!(
            app.generation_start_time.is_some(),
            "reflection timer must re-arm on the continuation turn"
        );
    }

    #[test]
    fn overflow_sentinel_scan_matches_real_esc_only() {
        assert_eq!(
            scan_completed_sentinel(b"\x1b]777;spiritty_done;0\x07"),
            Some(0)
        );
        assert_eq!(
            scan_completed_sentinel(b"noise\x1b]777;spiritty_done;127\ntrailing"),
            Some(127)
        );
        // Literal echo of the printf hook: the escape is TEXT (`\033`), never a real
        // ESC byte — the command echo must not be mistaken for a completion sentinel.
        assert_eq!(
            scan_completed_sentinel(b"printf '\\033]777;spiritty_done;%s\\007' $?"),
            None
        );
        // Unterminated prefix (stream cut) → keep waiting.
        assert_eq!(scan_completed_sentinel(b"\x1b]777;spiritty_done;"), None);
        assert_eq!(scan_completed_sentinel(b"no sentinel here"), None);
    }

    #[test]
    fn capture_summary_appends_truncation_notice() {
        let s = build_capture_summary(
            0,
            "out".to_string(),
            true,
            3 * 1024 * 1024,
            1024 * 1024,
            "(Commande exécutée avec succès dans le terminal)".to_string(),
        );
        assert!(s.starts_with("Sortie dans le terminal:\nout"), "{s}");
        assert!(s.contains("tronquée"), "{s}");
        assert!(s.contains("3.0 Mo"), "{s}");
        // Non-truncated summaries stay untouched.
        let plain = build_capture_summary(
            2,
            String::new(),
            false,
            0,
            0,
            "(Commande terminée avec le code 2)".to_string(),
        );
        assert_eq!(plain, "(Commande terminée avec le code 2)");
    }

    #[tokio::test]
    async fn overflow_capture_concludes_on_sentinel_and_marks_truncation() {
        use std::time::Instant;

        let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 60, 110).unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        app.active_pty_tool = Some(crate::app::PtyToolCapture {
            command: "cat big.log".to_string(),
            result_tx: Some(tx),
            output_bytes: Vec::new(),
            decoded_text: String::new(),
            pending_utf8: Vec::new(),
            sentinel_pending: None,
            sentinel_scan_upto: 0,
            start_time: Instant::now(),
            last_output_time: Instant::now(),
            auto_prompt: false,
            truncated: false,
            overflow_bytes: 0,
            overflow_tail: Vec::new(),
            clean_cache: None,
        });

        // Fill the buffer exactly to the cap, then keep streaming: further bytes must
        // be counted (truncated) and NOT buffered.
        let chunk = vec![b'x'; 4096];
        let full_chunks = MAX_CAPTURE_BYTES / chunk.len();
        for _ in 0..full_chunks {
            app.on_pty_output(&chunk);
        }
        let capture = app.active_pty_tool.as_ref().unwrap();
        assert_eq!(capture.output_bytes.len(), MAX_CAPTURE_BYTES);
        assert!(!capture.truncated);
        app.on_pty_output(&chunk);
        let capture = app.active_pty_tool.as_ref().unwrap();
        assert!(capture.truncated, "cap crossed: overflow mode expected");
        assert_eq!(capture.overflow_bytes, chunk.len() as u64);
        assert!(capture.decoded_text.len() <= MAX_CAPTURE_BYTES + chunk.len());

        // The completion sentinel arrives after the cap and must conclude immediately.
        let mut sentinel = b"\x1b]777;spiritty_done;".to_vec();
        sentinel.extend_from_slice(b"3\x07");
        app.on_pty_output(&sentinel);

        assert!(app.active_pty_tool.is_none(), "sentinel must conclude");
        let summary = rx.await.expect("result sent");
        assert!(
            summary.contains("Sortie dans le terminal (code 3):"),
            "{summary}"
        );
        assert!(summary.contains("tronquée"), "{summary}");
        assert!(summary.contains("1024 Ko"), "{summary}");
    }

    #[tokio::test]
    async fn ssh_reconnect_modal_offered_when_resuming_remote_session_locally() {
        use crate::system::hosts::{HostProfile, HostsStore};

        let (event_tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 60, 110).unwrap();
        // Deterministic store fixture (do NOT depend on the user's real hosts.json):
        // the prompt-inferred `xorne@prod` resolves to the connectable address.
        let mut store = HostsStore::default();
        store.profiles.insert(
            "ducasse-seine.com".to_string(),
            HostProfile {
                target: "ducasse-seine.com".to_string(),
                hostname: Some("prod".to_string()),
                os_name: "Linux".to_string(),
                distro: "Debian".to_string(),
                kernel: "4.9".to_string(),
                user: "xorne".to_string(),
                package_managers: vec!["apt".to_string()],
                init_system: "systemd".to_string(),
                last_seen: "2026-08-26T11:33:07+00:00".to_string(),
            },
        );
        app.hosts_store = store;

        app.maybe_offer_ssh_reconnect(Some("xorne@prod".to_string()));
        assert!(matches!(
            app.modal,
            ModalState::SshReconnect { ref target } if target == "ducasse-seine.com"
        ));
        // The persisted hint is healed to the connectable form.
        assert_eq!(
            app.current_session.last_ssh_target.as_deref(),
            Some("ducasse-seine.com")
        );
    }

    #[tokio::test]
    async fn ssh_reconnect_modal_skipped_for_local_only_session() {
        let (event_tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 60, 110).unwrap();
        app.maybe_offer_ssh_reconnect(None);
        assert!(matches!(app.modal, ModalState::None));
        app.maybe_offer_ssh_reconnect(Some(String::new()));
        assert!(matches!(app.modal, ModalState::None));
    }

    #[tokio::test]
    async fn ssh_reconnect_modal_skipped_when_already_connected() {
        use crate::system::process_watcher::ActiveSession;
        let (event_tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 60, 110).unwrap();
        app.system_context.active_session = ActiveSession::Ssh {
            target: "vps".to_string(),
            user: Some("xorne".to_string()),
            host: "1.2.3.4".to_string(),
            port: Some(22),
        };
        app.maybe_offer_ssh_reconnect(Some("xorne@prod".to_string()));
        assert!(
            matches!(app.modal, ModalState::None),
            "already connected: no modal"
        );
    }

    #[tokio::test]
    async fn ssh_reconnect_modal_esc_dismisses_and_enter_reconnects() {
        use crate::app::ModalState;
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let (event_tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 60, 110).unwrap();
        app.modal = ModalState::SshReconnect {
            target: "spiritty-test.invalid".to_string(),
        };

        // Esc dismisses without touching the terminal.
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(app.modal, ModalState::None));
        assert_eq!(app.focus, Focus::Chat, "focus unchanged on dismiss");

        // Enter reconnects: writes to the PTY, focuses the terminal, closes the modal.
        app.modal = ModalState::SshReconnect {
            target: "spiritty-test.invalid".to_string(),
        };
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(app.modal, ModalState::None));
        assert_eq!(
            app.focus,
            Focus::Terminal,
            "terminal focused for ssh prompt"
        );
    }

    #[tokio::test]
    async fn resumed_ssh_hint_shows_in_terminal_title_when_local() {
        use crate::ui::terminal_panel::TerminalPanel;
        use ratatui::{backend::TestBackend, Terminal};

        let (event_tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 60, 110).unwrap();
        // Session resumed with -c: it WAS remote, PTY is local right now.
        app.current_session.last_ssh_target = Some("vps".to_string());

        let mut term = Terminal::new(TestBackend::new(200, 12)).unwrap();
        term.draw(|f| {
            let area = ratatui::layout::Rect::new(0, 0, 110, 12);
            TerminalPanel::new(&mut app).render_panel(area, f.buffer_mut());
        })
        .unwrap();
        let text: String = term
            .backend()
            .buffer()
            .clone()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("SSH vps (reprise)"),
            "resumed hint expected, got first rows: {:?}",
            text.lines().take(2).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn no_ssh_hint_for_local_only_session() {
        use crate::ui::terminal_panel::TerminalPanel;
        use ratatui::{backend::TestBackend, Terminal};

        let (event_tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 60, 110).unwrap();
        assert!(app.current_session.last_ssh_target.is_none());

        let mut term = Terminal::new(TestBackend::new(200, 12)).unwrap();
        term.draw(|f| {
            let area = ratatui::layout::Rect::new(0, 0, 110, 12);
            TerminalPanel::new(&mut app).render_panel(area, f.buffer_mut());
        })
        .unwrap();
        let text: String = term
            .backend()
            .buffer()
            .clone()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            !text.contains("reprise"),
            "no hint expected for local-only session"
        );
    }

    #[tokio::test]
    async fn resumed_ssh_hint_falls_back_to_compact_form_on_narrow_panel() {
        use crate::ui::terminal_panel::TerminalPanel;
        use ratatui::{backend::TestBackend, Terminal};

        let (event_tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 60, 110).unwrap();
        app.current_session.last_ssh_target = Some("xorne@prod".to_string());

        // Simulate the terminal panel of a 120-col terminal: the split gives it
        // ~60 cells, where only the compact tiers fit next to the title.
        let mut term = Terminal::new(TestBackend::new(60, 12)).unwrap();
        term.draw(|f| {
            let area = ratatui::layout::Rect::new(0, 0, 60, 12);
            TerminalPanel::new(&mut app).render_panel(area, f.buffer_mut());
        })
        .unwrap();
        let text: String = term
            .backend()
            .buffer()
            .clone()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("SSH (reprise)"),
            "compact resumed hint expected on a narrow panel, got: {:?}",
            text.lines().take(2).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn settle_never_concludes_on_echo_only_capture() {
        use crate::system::process_watcher::ActiveSession;
        use std::time::{Duration, Instant};

        let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 55, 100).unwrap();
        app.system_context.active_session = ActiveSession::Ssh {
            target: "vps".to_string(),
            user: Some("xorne".to_string()),
            host: "1.2.3.4".to_string(),
            port: Some(22),
        };
        let (tx, _rx) = tokio::sync::oneshot::channel::<String>();
        // Only the ECHO of the injected command is in the buffer (remote slow to
        // answer). Both settle paths (800ms fast / 3s silence) are elapsed, but the
        // capture must NOT conclude on an empty cleaned output.
        let cmd = "cd /var/www/xorne/resa-prod && mysql -uxorne -p'pw' resa-v3 -e \"SELECT id FROM paiements WHERE id=61464\"";
        app.active_pty_tool = Some(crate::app::PtyToolCapture {
            command: cmd.to_string(),
            result_tx: Some(tx),
            output_bytes: Vec::new(),
            decoded_text: format!("{}\r\n", cmd),
            pending_utf8: Vec::new(),
            sentinel_pending: None,
            sentinel_scan_upto: 0,
            start_time: Instant::now() - Duration::from_secs(2),
            last_output_time: Instant::now() - Duration::from_millis(1500),
            auto_prompt: false,
            truncated: false,
            overflow_bytes: 0,
            overflow_tail: Vec::new(),
            clean_cache: None,
        });

        app.on_tick();
        assert!(
            app.active_pty_tool.is_some(),
            "echo-only capture must keep waiting for real remote output"
        );
    }

    #[tokio::test]
    async fn sentinel_inside_single_large_chunk_concludes_immediately() {
        use std::time::Instant;

        let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 55, 100).unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        app.active_pty_tool = Some(crate::app::PtyToolCapture {
            command: "uname -r".to_string(),
            result_tx: Some(tx),
            output_bytes: Vec::new(),
            decoded_text: String::new(),
            pending_utf8: Vec::new(),
            sentinel_pending: None,
            sentinel_scan_upto: 0,
            start_time: Instant::now(),
            last_output_time: Instant::now(),
            auto_prompt: false,
            truncated: false,
            overflow_bytes: 0,
            overflow_tail: Vec::new(),
            clean_cache: None,
        });
        // One single PTY read coalescing echo + output + sentinel + next prompt: the
        // sentinel sits mid-chunk, far outside any "last 20 chars" incremental window.
        // Regression: the watermark scan only examined the tail of each chunk, so this
        // exact byte layout hung the capture until the 45s hard timeout (local fast
        // shells coalesce output into large reads depending on scheduling).
        let chunk = "\r\u{1b}[1;36mSpiritty\u{1b}[0m main ❯  uname -r\r\n\u{1b}[?2004l\r7.1.9-arch1-2\r\n\u{1b}]777;spiritty_done;0\u{7}\u{1b}]0;xorne@host:~\u{7}\u{1b}[?2004h\r\n\u{1b}[1;36mSpiritty\u{1b}[0m main ❯ ";
        app.on_pty_output(chunk.as_bytes());
        assert!(
            app.active_pty_tool.is_none(),
            "sentinel inside a single large chunk must conclude the capture immediately"
        );
        let summary = rx.await.unwrap();
        assert!(
            summary.contains("7.1.9-arch1-2"),
            "command output must reach the model, got: {summary:?}"
        );
    }

    #[tokio::test]
    async fn empty_capture_timeout_reports_explicit_warning() {
        use crate::system::process_watcher::ActiveSession;
        use std::time::{Duration, Instant};

        let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 55, 100).unwrap();
        app.system_context.active_session = ActiveSession::Ssh {
            target: "vps".to_string(),
            user: Some("xorne".to_string()),
            host: "1.2.3.4".to_string(),
            port: Some(22),
        };
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        let cmd = "cd /var/www/xorne/resa-prod && mysql -e 'SELECT 1'";
        app.active_pty_tool = Some(crate::app::PtyToolCapture {
            command: cmd.to_string(),
            result_tx: Some(tx),
            output_bytes: Vec::new(),
            decoded_text: format!("{}\r\n", cmd),
            pending_utf8: Vec::new(),
            sentinel_pending: None,
            sentinel_scan_upto: 0,
            start_time: Instant::now() - Duration::from_secs(50),
            last_output_time: Instant::now() - Duration::from_secs(49),
            auto_prompt: false,
            truncated: false,
            overflow_bytes: 0,
            overflow_tail: Vec::new(),
            clean_cache: None,
        });

        app.on_tick();
        assert!(app.active_pty_tool.is_none(), "timeout must conclude");
        let summary = rx.await.expect("result sent");
        assert!(
            summary.contains("Aucune sortie capturée"),
            "explicit warning expected, got: {summary}"
        );
    }

    use super::*;

    #[test]
    fn password_wait_cap_is_bounded() {
        assert_eq!(
            super::PASSWORD_WAIT_HARD_CAP_SECS,
            120,
            "password-wait must stay bounded (was u64::MAX => infinite stuck)"
        );
    }

    #[tokio::test]
    async fn remote_capture_settles_fast_on_redisplayed_prompt() {
        use crate::system::process_watcher::ActiveSession;
        use std::time::{Duration, Instant};

        let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 55, 100).expect("create app");
        // Remote session: no local hooks, so the settle path is the only completion signal.
        app.system_context.active_session = ActiveSession::Ssh {
            target: "vps".to_string(),
            user: Some("xorne".to_string()),
            host: "1.2.3.4".to_string(),
            port: Some(22),
        };
        let (tx, _rx) = tokio::sync::oneshot::channel::<String>();
        app.active_pty_tool = Some(crate::app::PtyToolCapture {
            command: "cat > /tmp/f.php <<'EOF'".to_string(),
            result_tx: Some(tx),
            output_bytes: Vec::new(),
            decoded_text: "ligne1\nligne2\nxorne@vps:/var/www/app$ ".to_string(),
            pending_utf8: Vec::new(),
            sentinel_pending: None,
            sentinel_scan_upto: 0,
            start_time: Instant::now() - Duration::from_millis(2000),
            last_output_time: Instant::now() - Duration::from_millis(1000),
            auto_prompt: false,
            truncated: false,
            overflow_bytes: 0,
            overflow_tail: Vec::new(),
            clean_cache: None,
        });

        app.on_tick();
        assert!(
            app.active_pty_tool.is_none(),
            "quiet tail ending with a redisplayed PS1 must settle at 800ms, not wait 3s"
        );
    }

    #[tokio::test]
    async fn remote_capture_still_waits_without_prompt_tail() {
        use crate::system::process_watcher::ActiveSession;
        use std::time::{Duration, Instant};

        let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 55, 100).expect("create app");
        app.system_context.active_session = ActiveSession::Ssh {
            target: "vps".to_string(),
            user: Some("xorne".to_string()),
            host: "1.2.3.4".to_string(),
            port: Some(22),
        };
        let (tx, _rx) = tokio::sync::oneshot::channel::<String>();
        // Output does NOT end with a prompt (e.g. a command still streaming) and has
        // only been quiet 1s: neither the 800ms fast path nor the 3s fallback applies.
        app.active_pty_tool = Some(crate::app::PtyToolCapture {
            command: "tail -f /var/log/syslog".to_string(),
            result_tx: Some(tx),
            output_bytes: Vec::new(),
            decoded_text: "mysql> select 1;\n... encore des lignes".to_string(),
            pending_utf8: Vec::new(),
            sentinel_pending: None,
            sentinel_scan_upto: 0,
            start_time: Instant::now() - Duration::from_millis(2000),
            last_output_time: Instant::now() - Duration::from_millis(1000),
            auto_prompt: false,
            truncated: false,
            overflow_bytes: 0,
            overflow_tail: Vec::new(),
            clean_cache: None,
        });

        app.on_tick();
        assert!(
            app.active_pty_tool.is_some(),
            "must NOT settle before 3s without a prompt tail"
        );
    }

    #[tokio::test]
    async fn password_false_positive_blocks_settle_but_stays_bounded() {
        use crate::system::process_watcher::ActiveSession;
        use std::time::{Duration, Instant};

        let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(event_tx, 55, 100).expect("create app");
        app.system_context.active_session = ActiveSession::Ssh {
            target: "vps".to_string(),
            user: Some("xorne".to_string()),
            host: "1.2.3.4".to_string(),
            port: Some(22),
        };
        let (tx, _rx) = tokio::sync::oneshot::channel::<String>();
        // Output tail merely MENTIONS a password (config echo): the detector flags a
        // password wait, settle is paused — but the hard cap (120s) now bounds it.
        app.active_pty_tool = Some(crate::app::PtyToolCapture {
            command: "cat config/database.yml".to_string(),
            result_tx: Some(tx),
            output_bytes: Vec::new(),
            decoded_text: "default:\n  password: hunter2\n".to_string(),
            pending_utf8: Vec::new(),
            sentinel_pending: None,
            sentinel_scan_upto: 0,
            start_time: Instant::now() - Duration::from_millis(5000),
            last_output_time: Instant::now() - Duration::from_millis(4000),
            auto_prompt: false,
            truncated: false,
            overflow_bytes: 0,
            overflow_tail: Vec::new(),
            clean_cache: None,
        });

        app.on_tick();
        assert!(
            app.active_pty_tool.is_some(),
            "password-looking tail must pause the settle fallback (real prompts need it)"
        );
        assert!(super::is_waiting_for_password(
            "default:\n  password: hunter2\n"
        ));
    }

    #[test]
    fn shell_tagged_block_with_arrow_in_quoted_title_is_executable() {
        // User regression: `echo "=== CLONE DB resa-v3 -> resa_pp ==="` inside a
        // ```bash fence triggered the log-arrow heuristic, demoting the block to a
        // non-executable snippet box labelled "bash".
        let content = "Texte\n\n```bash\ncd /tmp && echo \"=== CLONE DB resa-v3 -> resa_pp ===\" && ls\n```\n";
        assert!(super::is_executable_command_block(
            "bash",
            "cd /tmp && echo \"=== a -> b ===\" && ls"
        ));
        let proposals = super::extract_all_command_proposals(content);
        assert_eq!(
            proposals.len(),
            1,
            "bash-tagged block must yield exactly one proposal"
        );
        assert!(
            proposals[0].starts_with("cd /tmp"),
            "proposal must be the clean command"
        );
        assert!(
            !proposals[0].contains('\n'),
            "no stray framing line expected"
        );
    }

    #[test]
    fn untagged_block_with_arrows_is_still_rejected() {
        // The arrow heuristic keeps protecting UNTAGGED fences (pasted output).
        assert!(!super::is_executable_command_block(
            "",
            "resa-v3 -> resa_pp\ndone"
        ));
    }

    use super::{
        char_safe_floor, char_safe_tail, clean_pty_output, is_prompt_remnant,
        key_event_to_pty_bytes, prompt_move_cursor_vertical, utf8_decode_incremental,
    };
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn pty_arrow_encoding_csi_vs_ss3() {
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        // Normal mode: standard CSI sequences.
        assert_eq!(
            key_event_to_pty_bytes(key(KeyCode::Up), false),
            vec![0x1B, b'[', b'A']
        );
        assert_eq!(
            key_event_to_pty_bytes(key(KeyCode::Down), false),
            vec![0x1B, b'[', b'B']
        );
        // Application cursor mode (vim after smkx): SS3 sequences.
        assert_eq!(
            key_event_to_pty_bytes(key(KeyCode::Up), true),
            vec![0x1B, b'O', b'A']
        );
        assert_eq!(
            key_event_to_pty_bytes(key(KeyCode::Down), true),
            vec![0x1B, b'O', b'B']
        );
        assert_eq!(
            key_event_to_pty_bytes(key(KeyCode::Right), true),
            vec![0x1B, b'O', b'C']
        );
        assert_eq!(
            key_event_to_pty_bytes(key(KeyCode::Left), true),
            vec![0x1B, b'O', b'D']
        );
        assert_eq!(
            key_event_to_pty_bytes(key(KeyCode::Home), true),
            vec![0x1B, b'O', b'H']
        );
        assert_eq!(
            key_event_to_pty_bytes(key(KeyCode::End), true),
            vec![0x1B, b'O', b'F']
        );
        // Plain letters are unaffected by the mode.
        let a = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
        assert_eq!(key_event_to_pty_bytes(a, true), b"i".to_vec());
    }

    #[test]
    fn vertical_movement_across_logical_lines() {
        let input = "ligne un\nligne deux\n\nligne quatre";
        let width = 40;
        let end = input.len();
        // From end of buffer (row 3) → Up lands on the empty row 2 (byte of '\n'+1 == position after second '\n').
        let up_from_end = prompt_move_cursor_vertical(input, width, end, -1).unwrap();
        // Blank logical line sits between the 2nd and 3rd '\n' → byte 20.
        assert_eq!(up_from_end, 20, "must land on the blank logical line start");
        // Up again → row 1 ("ligne deux").
        let up2 = prompt_move_cursor_vertical(input, width, up_from_end, -1).unwrap();
        assert_eq!(up2, 9);
        // Down twice returns through the blank line to the last row's start.
        let down1 = prompt_move_cursor_vertical(input, width, up2, 1).unwrap();
        assert_eq!(down1, 20);
        let down2 = prompt_move_cursor_vertical(input, width, down1, 1).unwrap();
        assert_eq!(down2, 21);
    }

    #[test]
    fn vertical_movement_across_wrapped_single_paragraph() {
        // One long paragraph: visual rows come from word-wrapping, not '\n'.
        let input = "mot0 mot1 mot2 mot3 mot4 mot5 mot6 mot7 mot8 mot9";
        let width = 12; // ~2 words per visual row
        let end = input.len();
        let rows_up = prompt_move_cursor_vertical(input, width, end, -1);
        assert!(
            rows_up.is_some(),
            "wrapped paragraph must offer vertical moves"
        );
        let up = rows_up.unwrap();
        assert!(up < end);
        // Column preservation: going back down must restore the byte position.
        let back = prompt_move_cursor_vertical(input, width, up, 1).unwrap();
        assert_eq!(back, end);
    }

    #[test]
    fn vertical_movement_edges_return_none() {
        let input = "seule ligne un peu longue mais pas hors limite";
        assert_eq!(prompt_move_cursor_vertical(input, 100, 5, -1), None);
        assert_eq!(prompt_move_cursor_vertical(input, 100, 5, 1), None);
        // Multi-row: Up from the very first row falls back (None → history).
        let multi = "a\nb";
        assert_eq!(prompt_move_cursor_vertical(multi, 10, 0, -1), None);
        assert_eq!(prompt_move_cursor_vertical(multi, 10, multi.len(), 1), None);
    }

    #[test]
    fn incremental_utf8_decoder_matches_lossy_semantics() {
        let mut text = String::new();
        let mut pending: Vec<u8> = Vec::new();

        // A 4-byte emoji split across four chunks must survive intact.
        for b in [0xF0u8, 0x9F, 0x91, 0x8D] {
            pending.extend_from_slice(&[b]);
            utf8_decode_incremental(&mut text, &mut pending);
        }
        assert_eq!(text, "\u{1F44D}");
        assert!(pending.is_empty());

        // Mixed: valid ASCII then é (2 bytes) split across two calls.
        pending.extend_from_slice(b"ab\xc3");
        utf8_decode_incremental(&mut text, &mut pending);
        assert_eq!(text, "\u{1F44D}ab");
        pending.extend_from_slice(&[0xA9, b'c']);
        utf8_decode_incremental(&mut text, &mut pending);
        assert_eq!(text, "\u{1F44D}ab\u{e9}c");
    }

    #[test]
    fn incremental_decoder_replaces_invalid_bytes() {
        let mut text = String::new();
        let mut pending: Vec<u8> = Vec::new();
        pending.extend_from_slice(b"ok");
        utf8_decode_incremental(&mut text, &mut pending);
        // 0xFF is invalid UTF-8: consumed in one stride, replaced like from_utf8_lossy does.
        pending.extend_from_slice(&[0xFF, b'!']);
        utf8_decode_incremental(&mut text, &mut pending);
        assert_eq!(text, "ok\u{FFFD}!");
    }

    #[test]
    fn char_safe_helpers_never_split_multibyte() {
        let s = "aé👍xyz"; // multibyte interior
        assert_eq!(char_safe_floor(s, s.len()), s.len());
        assert!(s.is_char_boundary(char_safe_floor(s, s.len() - 1)));
        assert!(s.is_char_boundary(char_safe_floor(s, 2)));
        let tail = char_safe_tail(s, 3);
        assert!(tail.chars().count() <= 6);
        assert!(s.ends_with(tail));
    }

    #[test]
    fn strips_git_aware_prompt_remnant() {
        assert!(is_prompt_remnant(
            "[xorne@prod:/var/www/xorne/resa-prod/config] main(+0/-7) ±"
        ));
        assert!(is_prompt_remnant("[user@host:~] main ±"));
        assert!(!is_prompt_remnant("HTTP 200 - 0.062107s"));
        assert!(!is_prompt_remnant("active"));
    }

    #[test]
    fn strips_bell_and_prompt_from_captured_output() {
        let raw = "curl -sk -o /dev/null -w \"HTTP %{http_code}\"\x07 https://x/\r\nHTTP 200 - 0.062107s\r\n[xorne@prod:/var/www/xorne/resa-prod/config] main(+0/-7) ±";
        let cmd = "curl -sk -o /dev/null -w \"HTTP %{http_code}\" https://x/";
        let cleaned = clean_pty_output(raw, cmd);
        // The echo line (with leaked BEL) and the trailing prompt must be removed.
        assert_eq!(cleaned, "HTTP 200 - 0.062107s");
    }

    #[test]
    fn rejects_ip_list_as_command_proposal() {
        use super::is_executable_command_block;
        // A list of IP addresses the model presents as output must NOT become a command card.
        assert!(!is_executable_command_block(
            "",
            "103.213.238.91 202.165.15.132 210.79.142.201"
        ));
        assert!(!is_executable_command_block(
            "bash",
            "51.38.226.204   121.161.242.168   43.164.190.60   37.59.112.122"
        ));
        assert!(is_executable_command_block(
            "bash",
            "ss -tn state established | grep '185.177.72'"
        ));
        assert!(is_executable_command_block(
            "",
            "systemctl status fail2ban --no-pager | head -30"
        ));
    }

    #[test]
    fn rejects_quoted_output_prose_as_command_proposal() {
        use super::is_executable_command_block;
        // Regression (user report screen 3): "Mail queue is empty" was rendered inside a
        // plain fence by the model and became a spurious "⚡ COMMANDE #1" card.
        assert!(!is_executable_command_block("", "Mail queue is empty"));
        assert!(!is_executable_command_block("", "No certificates found"));
        // Real commands keep passing.
        assert!(is_executable_command_block("", "df -h"));
        assert!(is_executable_command_block("", "git status"));
        assert!(is_executable_command_block("bash", "systemctl list-units --type=service --state=running --no-pager | grep -Ei 'php|apache'"));
    }

    #[test]
    fn strips_interpreter_framing_lines_from_proposals() {
        use super::{extract_all_command_proposals, sanitize_proposed_command};
        let real_cmd = "echo \"=== FSTAB (entrées swap) ===\" && grep -n 'swap' /etc/fstab";

        // Direct helper: leading blank + `bash` framing lines are gone.
        let cleaned = sanitize_proposed_command(&format!("bash\n\n{}", real_cmd));
        assert_eq!(cleaned, real_cmd);

        // sudo-prefixed and shebang framings too.
        assert_eq!(
            sanitize_proposed_command(&format!("sudo sh\n#! /bin/bash\n{}", real_cmd)),
            real_cmd
        );

        // A trailing dangling exit would kill the user's own shell once injected.
        assert_eq!(
            sanitize_proposed_command(&format!("{}\nexit", real_cmd)),
            real_cmd
        );
        assert_eq!(
            sanitize_proposed_command(&format!("bash\n{}\nlogout", real_cmd)),
            real_cmd
        );

        // Real commands using the interpreter explicitly stay untouched…
        assert_eq!(
            sanitize_proposed_command("bash -c 'echo hi'"),
            "bash -c 'echo hi'"
        );
        assert_eq!(
            sanitize_proposed_command("bash <<EOF\necho hi\nEOF"),
            "bash <<EOF\necho hi\nEOF"
        );
        // …and a block that contains nothing executable yields no proposal at all.
        assert_eq!(sanitize_proposed_command("bash\nexit"), "");

        // End-to-end through the extractor (user report screen 2): the bare `bash` line must
        // not be part of what gets injected into the PTY.
        let md = format!("```bash\nbash\n{real}\n```\n", real = real_cmd);
        let proposals = extract_all_command_proposals(&md);
        assert_eq!(proposals, vec![real_cmd.to_string()]);
    }

    #[test]
    fn empty_input_is_not_an_approval_phrase() {
        use super::{is_natural_approval_phrase, is_natural_decline_phrase};
        // Regression: bare Enter while a permission card is displayed used to approve it
        // (even Risky commands) because "" matched the approval phrase list.
        assert!(!is_natural_approval_phrase(""));
        assert!(!is_natural_approval_phrase("   "));
        assert!(is_natural_approval_phrase("ok"));
        assert!(is_natural_approval_phrase("oui"));
        // Decline phrases were never empty-matching; keep it that way.
        assert!(!is_natural_decline_phrase(""));
    }
}
