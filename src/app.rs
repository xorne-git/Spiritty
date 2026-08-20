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
    pty::PtyProcess,
    session::{Session, SessionStorage},
    system::{ActiveSession, HostsStore, SystemContext},
    ui::components::{ConfigModalState, SessionModalAction, SessionModalState},
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

pub enum ModalState {
    None,
    Help,
    Config(ConfigModalState),
    Sessions(SessionModalState),
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

pub struct PtyToolCapture {
    pub command: String,
    pub result_tx: Option<tokio::sync::oneshot::Sender<String>>,
    pub output_bytes: Vec<u8>,
    pub start_time: std::time::Instant,
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
    pub last_injected_cmd: Option<String>,
    pub detected_context_window: Arc<AtomicUsize>,
    pub active_pty_tool: Option<PtyToolCapture>,
    pub chat_scroll_from_bottom: u16,
    pub chat_scroll_extra_down: u16,
    pub chat_history: Vec<String>,
    pub history_index: Option<usize>,
    pub input_draft: String,
    pub system_context: SystemContext,
    pub generation_start_time: Option<std::time::Instant>,
    pub last_chunk_time: Option<std::time::Instant>,
    pub current_turn_tokens: usize,
    pub last_tokens_per_sec: Option<f64>,
    pub mouse_selection: Option<MouseSelection>,
    pub clipboard_toast: Option<(std::time::Instant, usize)>,
    pub copied_current_selection: bool,
    pub current_session: Session,
    pub hosts_store: HostsStore,
    pub toast_message: Option<(std::time::Instant, String)>,
    pub is_probing_host: bool,
    pub probe_buffer: String,
}

impl App {
    pub fn new(
        event_tx: UnboundedSender<AppEvent>,
        initial_rows: u16,
        initial_cols: u16,
    ) -> Result<Self> {
        let (pty_tx, mut pty_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let pty = PtyProcess::spawn(initial_rows, initial_cols, pty_tx)?;

        // Forward raw PTY output to unified event channel
        let forward_tx = event_tx.clone();
        tokio::spawn(async move {
            while let Some(bytes) = pty_rx.recv().await {
                if forward_tx.send(AppEvent::PtyOutput(bytes)).is_err() {
                    break;
                }
            }
        });

        let config = Config::load();
        let agent = AgentEngine::new(config.clone());
        let detected_context_window = Arc::new(AtomicUsize::new(0));
        probe_model_context(&config, detected_context_window.clone());
        let system_context = SystemContext::detect();

        let active_provider = config.default_provider.display_name();
        let active_model = config.get_active_provider_config().model.clone();
        let current_session = Session::new(active_provider, &active_model);

        let app = Self {
            focus: Focus::Chat, // Default focus on chat prompt
            chat_input: String::new(),
            cursor_pos: 0,
            messages: Vec::new(),
            pty,
            should_quit: false,
            split_ratio: 50, // 50% Chat / 50% Terminal
            terminal_inner_size: (initial_rows, initial_cols),
            chat_area: Rect::default(),
            terminal_area: Rect::default(),
            is_dragging_split: false,
            config,
            agent,
            modal: ModalState::None,
            event_tx,
            spinner_frame: 0,
            pending_tool_approval: None,
            last_injected_cmd: None,
            detected_context_window,
            active_pty_tool: None,
            chat_scroll_from_bottom: 0,
            chat_scroll_extra_down: 0,
            chat_history: Vec::new(),
            history_index: None,
            input_draft: String::new(),
            system_context,
            generation_start_time: None,
            last_chunk_time: None,
            current_turn_tokens: 0,
            last_tokens_per_sec: None,
            mouse_selection: None,
            clipboard_toast: None,
            copied_current_selection: false,
            current_session,
            hosts_store: HostsStore::load(),
            toast_message: None,
            is_probing_host: false,
            probe_buffer: String::new(),
        };

        app.probe_provider_models(ProviderType::LmStudio);
        app.probe_provider_models(ProviderType::Ollama);
        if app.config.default_provider != ProviderType::LmStudio && app.config.default_provider != ProviderType::Ollama {
            app.probe_provider_models(app.config.default_provider);
        }

        Ok(app)
    }

    pub fn probe_provider_models(&self, provider: ProviderType) {
        let key = provider.key_str().to_string();
        let p_cfg = self.config.providers.get(&key).cloned();
        let base_url = p_cfg.as_ref().and_then(|c| c.base_url.clone());
        let api_key = Config::resolve_api_key_for_provider(provider, p_cfg.as_ref().and_then(|c| c.api_key.as_deref()));
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let fetched = crate::agent::providers::fetch_available_models(
                provider,
                base_url.as_deref(),
                api_key.as_deref(),
            ).await;

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
            let entry = config_state.models_per_provider.entry(provider_key.clone()).or_default();
            for m in &models {
                if !entry.contains(m) {
                    entry.push(m.clone());
                }
            }
            if config_state.selected_provider.key_str() == provider_key {
                if let Some(current_models) = config_state.models_per_provider.get(&provider_key) {
                    if let Some(pos) = current_models.iter().position(|m| *m == config_state.model_input) {
                        config_state.dropdown_selected_idx = pos;
                    }
                }
            }
        }
    }

    pub fn scroll_chat_up(&mut self, lines: u16) {
        if self.chat_scroll_extra_down > 0 {
            let rem = lines.saturating_sub(self.chat_scroll_extra_down);
            self.chat_scroll_extra_down = self.chat_scroll_extra_down.saturating_sub(lines);
            if rem > 0 {
                self.chat_scroll_from_bottom = self.chat_scroll_from_bottom.saturating_add(rem);
            }
        } else {
            self.chat_scroll_from_bottom = self.chat_scroll_from_bottom.saturating_add(lines);
        }
    }

    pub fn scroll_chat_down(&mut self, lines: u16) {
        if self.chat_scroll_from_bottom > 0 {
            let rem = lines.saturating_sub(self.chat_scroll_from_bottom);
            self.chat_scroll_from_bottom = self.chat_scroll_from_bottom.saturating_sub(lines);
            if rem > 0 {
                self.chat_scroll_extra_down = (self.chat_scroll_extra_down + rem).min(12);
            }
        } else {
            // Force-scroll / overscroll down past estimated end by up to 12 rows
            self.chat_scroll_extra_down = (self.chat_scroll_extra_down + lines).min(12);
        }
    }

    pub fn reset_chat_scroll(&mut self) {
        self.chat_scroll_from_bottom = 0;
        self.chat_scroll_extra_down = 0;
    }

    pub fn save_current_session(&mut self) {
        if self.messages.is_empty() {
            return;
        }
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
        // Automatically compact context & history by default
        self.current_session.compact();
        let _ = SessionStorage::save(&self.current_session);
    }

    pub fn load_session(&mut self, session_id: &str) {
        self.save_current_session();
        if let Ok(loaded) = SessionStorage::load(session_id) {
            self.messages = loaded.messages.clone();
            // Restore prompt history
            if !loaded.prompt_history.is_empty() {
                self.chat_history = loaded.prompt_history.clone();
            } else {
                // Fallback: extract from previous user messages
                self.chat_history = loaded
                    .messages
                    .iter()
                    .filter(|m| m.role == MessageRole::User && !m.content.starts_with("💻 `") && !m.content.starts_with("[RÉSULTAT"))
                    .map(|m| m.content.clone())
                    .collect();
            }
            self.history_index = None;
            self.input_draft.clear();
            self.current_session = loaded;
            self.chat_input.clear();
            self.cursor_pos = 0;
            self.reset_chat_scroll();
        }
    }

    pub fn new_session(&mut self) {
        self.save_current_session();
        let active_provider = self.config.default_provider.display_name();
        let active_model = self.config.get_active_provider_config().model.clone();
        self.current_session = Session::new(active_provider, &active_model);
        self.messages.clear();
        self.chat_history.clear();
        self.history_index = None;
        self.input_draft.clear();
        self.chat_input.clear();
        self.cursor_pos = 0;
        self.reset_chat_scroll();
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
        {
            131_072
        } else {
            // Local models fallback context window
            8_192
        }
    }

    pub fn get_total_tokens_used(&self) -> usize {
        let total_chars: usize = self.messages.iter().map(|m| m.content.len()).sum::<usize>() + self.chat_input.len();
        ((total_chars as f64) / 3.8).ceil() as usize
    }

    pub fn get_context_used_tokens(&self) -> usize {
        let total_chars: usize = self.messages.iter().map(|m| m.content.len()).sum::<usize>() + self.chat_input.len() + 1500;
        ((total_chars as f64) / 3.8).ceil() as usize
    }

    pub fn get_tokens_per_sec(&self) -> Option<f64> {
        if self.agent.is_generating && self.pending_tool_approval.is_none() && self.active_pty_tool.is_none() {
            if let (Some(start), Some(last_chunk)) = (self.generation_start_time, self.last_chunk_time) {
                // If model is actively emitting chunks (< 600ms), compute live speed
                if last_chunk.elapsed().as_millis() < 600 {
                    let secs = start.elapsed().as_secs_f64();
                    if secs > 0.1 && self.current_turn_tokens > 0 {
                        return Some(self.current_turn_tokens as f64 / secs);
                    }
                }
            }
        }
        self.last_tokens_per_sec
    }

    pub fn update_terminal_size(&mut self, area: Rect) {
        if area.width > 0 && area.height > 0 && self.terminal_inner_size != (area.height, area.width) {
            self.terminal_inner_size = (area.height, area.width);
            let _ = self.pty.resize(area.height, area.width);
        }
    }

    pub fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent, total_width: u16) {
        use crossterm::event::{MouseButton, MouseEventKind};
        // Do not handle split dragging if a modal is open
        if !matches!(self.modal, ModalState::None) {
            return;
        }

        let x = mouse.column;
        let y = mouse.row;
        let border_x = self.chat_area.right();
        
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if x >= border_x.saturating_sub(1) && x <= border_x.saturating_add(1) {
                    self.is_dragging_split = true;
                    self.mouse_selection = None;
                } else if self.chat_area.contains(ratatui::layout::Position { x, y }) {
                    self.focus = Focus::Chat;
                    self.is_dragging_split = false;
                    self.mouse_selection = Some(MouseSelection {
                        panel: SelectionPanel::Chat,
                        start: (x, y),
                        end: (x, y),
                        is_selecting: true,
                    });
                } else if self.terminal_area.contains(ratatui::layout::Position { x, y }) {
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
                self.is_dragging_split = false;
                if let Some(ref mut sel) = self.mouse_selection {
                    sel.end = (x, y);
                    sel.is_selecting = false;
                }
            }
            MouseEventKind::ScrollUp => {
                if self.chat_area.contains(ratatui::layout::Position { x, y }) {
                    self.scroll_chat_up(3);
                } else if self.terminal_area.contains(ratatui::layout::Position { x, y }) {
                    self.pty.scroll_up(5);
                }
            }
            MouseEventKind::ScrollDown => {
                if self.chat_area.contains(ratatui::layout::Position { x, y }) {
                    self.scroll_chat_down(3);
                } else if self.terminal_area.contains(ratatui::layout::Position { x, y }) {
                    self.pty.scroll_down(5);
                }
            }
            _ => {}
        }
    }

    pub fn handle_paste(&mut self, text: String) {
        match &mut self.modal {
            ModalState::Config(config_state) => {
                config_state.handle_paste(text);
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
        self.agent.reload_config(self.config.clone());
        let _ = self.config.save();
        next_level
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
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
            || (key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')))
        {
            self.cycle_auto_approve();
            return;
        }

        // Alt+S triggers remote host probe/scan when in SSH session
        if key.modifiers.contains(KeyModifiers::ALT) && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S')) {
            self.trigger_host_scan();
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('p') | KeyCode::Char('P')) {
            self.modal = match self.modal {
                ModalState::Config(_) => ModalState::None,
                _ => {
                    self.probe_provider_models(ProviderType::LmStudio);
                    self.probe_provider_models(ProviderType::Ollama);
                    if self.config.default_provider != ProviderType::LmStudio && self.config.default_provider != ProviderType::Ollama {
                        self.probe_provider_models(self.config.default_provider);
                    }
                    ModalState::Config(ConfigModalState::from_config(&self.config))
                }
            };
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('h') | KeyCode::Char('H')) {
            self.save_current_session();
            self.modal = match self.modal {
                ModalState::Sessions(_) => ModalState::None,
                _ => ModalState::Sessions(SessionModalState::new(self.current_session.id.clone())),
            };
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('n') | KeyCode::Char('N')) {
            self.new_session();
            return;
        }

        // 2. If a modal is open, it captures all keys
        let session_action = if let ModalState::Sessions(ref mut session_state) = self.modal {
            session_state.handle_key(key)
        } else {
            None
        };

        if let Some(action) = session_action {
            match action {
                SessionModalAction::Load(id) => {
                    self.load_session(&id);
                    self.modal = ModalState::None;
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

        match &mut self.modal {
            ModalState::Help => {
                if key.code == KeyCode::Esc || key.code == KeyCode::Enter {
                    self.modal = ModalState::None;
                }
                return;
            }
            ModalState::Config(config_state) => {
                let should_close = config_state.handle_key(key, &mut self.config);
                if should_close {
                    self.agent.reload_config(self.config.clone());
                    self.trigger_context_probe();
                    self.modal = ModalState::None;
                }
                return;
            }
            ModalState::Sessions(_) => return,
            ModalState::None => {}
        }

        // 3. Alt + 1..9 / AZERTY to execute proposed command cards, and Alt+Left / Alt+Right for split resize
        if key.modifiers.contains(KeyModifiers::ALT) {
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
        let is_ctrl_space = key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(' ');
        let is_f6 = key.code == KeyCode::F(6);

        if is_shift_tab || is_ctrl_space || is_f6 {
            self.toggle_focus();
            return;
        }

        // 5. Global quit
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
            self.should_quit = true;
            return;
        }

        match self.focus {
            Focus::Terminal => self.handle_terminal_key(key),
            Focus::Chat => self.handle_chat_key(key),
        }
    }

    fn handle_terminal_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::PageUp || (key.code == KeyCode::Up && key.modifiers.contains(KeyModifiers::SHIFT)) {
            self.pty.scroll_up(15);
            return;
        }
        if key.code == KeyCode::PageDown || (key.code == KeyCode::Down && key.modifiers.contains(KeyModifiers::SHIFT)) {
            self.pty.scroll_down(15);
            return;
        }

        // Any regular keystroke resets scroll to 0 (live terminal)
        if self.pty.scroll_offset() > 0 {
            self.pty.reset_scroll();
        }

        let bytes = key_event_to_pty_bytes(key);
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
            if capture.start_time.elapsed() > std::time::Duration::from_secs(30) {
                if let Some(tx) = capture.result_tx.take() {
                    let _ = tx.send("(Délai d'attente de 30s dépassé pour la commande)".to_string());
                }
                self.active_pty_tool = None;
            }
        }
    }

    pub fn poll_active_session(&mut self) {
        if let Some(child_pid) = self.pty.child_pid() {
            let new_session = crate::system::detect_active_session(child_pid);
            if new_session != self.system_context.active_session {
                self.on_active_session_changed(new_session);
            }
        }
    }

    pub fn on_active_session_changed(&mut self, new_session: ActiveSession) {
        match &new_session {
            ActiveSession::Ssh { target, .. } => {
                if let Some(profile) = self.hosts_store.get(target) {
                    self.system_context.active_remote_profile = Some(profile.clone());
                    self.set_toast(format!("🌐 SSH: {} ({})", target, profile.distro));
                } else {
                    self.system_context.active_remote_profile = None;
                    self.set_toast(format!("🌐 SSH: {} — [Alt+S] pour scanner l'hôte", target));
                }
            }
            ActiveSession::Container { runtime, container_id } => {
                self.system_context.active_remote_profile = None;
                self.set_toast(format!("📦 {}: {}", runtime, container_id));
            }
            ActiveSession::Local { .. } => {
                let was_ssh = self.system_context.active_session.is_ssh();
                self.system_context.active_remote_profile = None;
                if was_ssh {
                    self.set_toast("🖥️ Retour à l'environnement local".to_string());
                }
            }
        }
        self.system_context.active_session = new_session;
    }

    pub fn trigger_host_scan(&mut self) {
        if !self.system_context.active_session.is_ssh() {
            self.set_toast("ℹ️ Le scan est réservé aux sessions SSH distantes".to_string());
            return;
        }
        self.is_probing_host = true;
        self.probe_buffer.clear();
        let probe_cmd = HostsStore::generate_probe_command();
        let _ = self.pty.write_all(format!("{}\n", probe_cmd).as_bytes());
        self.set_toast("🌐 Scan de l'environnement distant en cours...".to_string());
    }

    pub fn set_toast(&mut self, msg: String) {
        self.toast_message = Some((std::time::Instant::now(), msg));
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

    pub fn execute_command_by_index(&mut self, index: usize, auto_run: bool) -> bool {
        let proposals = self.all_command_proposals();
        if let Some(cmd) = proposals.get(index).cloned() {
            let clean_cmd = clean_multiline_command(&cmd);

            if auto_run {
                self.last_injected_cmd = None;
                self.agent.is_generating = true;

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

                // 2. Launch execution in live PTY with output capture
                let (result_tx, result_rx) = tokio::sync::oneshot::channel::<String>();
                self.on_agent_pty_tool_execute(clean_cmd.clone(), result_tx);

                // 3. When PTY execution completes, pass the real output to the AI agent for analysis!
                let event_tx = self.event_tx.clone();
                let mut agent = self.agent.clone();
                let mut conversation = self.messages.clone();
                let sys_ctx = self.system_context.clone();

                tokio::spawn(async move {
                    if let Ok(tool_output) = result_rx.await {
                        // Pop empty placeholder and push structured tool result to conversation
                        if let Some(last) = conversation.last() {
                            if last.role == MessageRole::Assistant && last.content.is_empty() {
                                conversation.pop();
                            }
                        }
                        conversation.push(ChatMessage {
                            role: MessageRole::User,
                            content: format!(
                                "[RÉSULTAT DE L'EXÉCUTION DE LA COMMANDE '{}']:\n{}\n[Analysez ce résultat et expliquez la situation à l'utilisateur]",
                                cmd, tool_output
                            ),
                            command_proposal: None,
                        });
                        conversation.push(ChatMessage {
                            role: MessageRole::Assistant,
                            content: String::new(),
                            command_proposal: None,
                        });
                        let _ = agent.send_prompt(conversation, &sys_ctx, event_tx);
                    }
                });

                return true;
            } else {
                let shell = self.pty.shell();
                let pty_cmd = format_command_for_pty(&cmd, shell);
                let _ = self.pty.write_all(pty_cmd.as_bytes());
                self.last_injected_cmd = Some(cmd.clone());
                self.focus = Focus::Terminal;
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
                } else if is_natural_approval_phrase(&input) {
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

        // 2. Esc cancels active generation or active PTY tool
        if key.code == KeyCode::Esc && self.agent.is_generating {
            self.agent.is_generating = false;
            self.pending_tool_approval = None;
            self.active_pty_tool = None;
            if let Some(last_msg) = self.messages.last_mut() {
                if last_msg.role == MessageRole::Assistant {
                    if last_msg.content.is_empty() {
                        last_msg.content = "(Génération interrompue par l'utilisateur)".to_string();
                    } else {
                        last_msg.content.push_str("\n\n(Génération interrompue)");
                    }
                }
            }
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
                    if let Some(target_idx) = parse_command_execution_request(&input, proposals.len()) {
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
                    let _ = self.agent.send_prompt(self.messages.clone(), &self.system_context, self.event_tx.clone());
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
            KeyCode::Char(c) => {
                if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) {
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
                if key.modifiers.contains(KeyModifiers::SHIFT) || key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.scroll_chat_up(500);
                } else {
                    self.cursor_pos = 0;
                }
            }
            KeyCode::End => {
                if key.modifiers.contains(KeyModifiers::SHIFT) || key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.reset_chat_scroll();
                } else {
                    self.cursor_pos = self.chat_input.len();
                }
            }
            KeyCode::PageUp => {
                self.scroll_chat_up(15);
            }
            KeyCode::PageDown => {
                self.scroll_chat_down(15);
            }
            KeyCode::Up => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.scroll_chat_up(3);
                } else if !self.chat_history.is_empty() {
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
            }
            KeyCode::Down => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.scroll_chat_down(3);
                } else if let Some(idx) = self.history_index {
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
            KeyCode::Esc if !self.chat_input.is_empty() => {
                self.chat_input.clear();
                self.cursor_pos = 0;
                self.history_index = None;
                self.input_draft.clear();
            }
            _ => {}
        }
    }

    pub fn on_agent_chunk(&mut self, chunk: String) {
        let tok_estimate = ((chunk.len() as f64) / 3.8).max(1.0) as usize;
        self.current_turn_tokens += tok_estimate;
        let now = std::time::Instant::now();
        if let Some(start) = self.generation_start_time {
            let secs = start.elapsed().as_secs_f64();
            if secs > 0.1 && self.current_turn_tokens > 0 {
                self.last_tokens_per_sec = Some(self.current_turn_tokens as f64 / secs);
            }
        } else {
            self.generation_start_time = Some(now);
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
                let trimmed = last_msg.content.trim();
                let clean_base = if let Some(idx) = trimmed.find("```tool:") {
                    trimmed[..idx].trim()
                } else {
                    trimmed
                };
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

    pub fn on_agent_tool_done(&mut self, command: String, _output: String) {
        self.pending_tool_approval = None;
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                let trimmed = last_msg.content.trim();
                let clean_base = if let Some(idx) = trimmed.find("💻 ") {
                    trimmed[..idx].trim()
                } else if let Some(idx) = trimmed.find("🌐 ") {
                    trimmed[..idx].trim()
                } else if let Some(idx) = trimmed.find("```tool:") {
                    trimmed[..idx].trim()
                } else {
                    trimmed
                };
                if command.starts_with("🌐") {
                    if clean_base.is_empty() {
                        last_msg.content = command;
                    } else {
                        last_msg.content = format!("{}\n\n{}", clean_base, command);
                    }
                } else if clean_base.is_empty() {
                    last_msg.content = format!("💻 `{}`", command);
                } else {
                    last_msg.content = format!("{}\n\n💻 `{}`", clean_base, command);
                }
            }
        }
        self.chat_scroll_from_bottom = 0;
    }

    pub fn on_agent_pty_tool_execute(
        &mut self,
        command: String,
        result_tx: tokio::sync::oneshot::Sender<String>,
    ) {
        let shell = self.pty.shell();
        let formatted_cmd = format_command_for_pty(&command, shell);

        let _ = self.pty.write_all(formatted_cmd.as_bytes());

        // Switch focus to Terminal so user can interact if prompt/pager opens
        self.focus = Focus::Terminal;

        self.active_pty_tool = Some(PtyToolCapture {
            command,
            result_tx: Some(result_tx),
            output_bytes: Vec::new(),
            start_time: std::time::Instant::now(),
        });
    }

    pub fn on_pty_output(&mut self, bytes: &[u8]) {
        // 1. Capture remote host probe output if active
        if self.is_probing_host {
            self.probe_buffer.push_str(&String::from_utf8_lossy(bytes));
            if self.probe_buffer.contains("SPIRITTY_PROBE_END") {
                self.is_probing_host = false;
                let target = self
                    .system_context
                    .active_session
                    .ssh_target()
                    .unwrap_or("remote-host")
                    .to_string();

                if let Some(profile) = HostsStore::parse_probe_output(&target, &self.probe_buffer) {
                    let distro_name = profile.distro.clone();
                    let _ = self.hosts_store.upsert(profile.clone());
                    self.system_context.active_remote_profile = Some(profile);
                    self.set_toast(format!("🌐 {} — Profil {} enregistré", target, distro_name));
                } else {
                    self.set_toast("⚠️ Échec de l'analyse du serveur distant".to_string());
                }
                self.probe_buffer.clear();
            }
        }

        if let Some(ref mut capture) = self.active_pty_tool {
            capture.output_bytes.extend_from_slice(bytes);

            if let Ok(text) = std::str::from_utf8(&capture.output_bytes) {
                // Check for OSC 777 sentinel: \x1b]777;spiritty_done;<status>\x1b\\ or \x07 or fallback __SPIRITTY_DONE__:<status>
                let sentinel_pattern = if let Some(pos) = text.find("777;spiritty_done;") {
                    Some((pos, 18, true))
                } else {
                    text.find("__SPIRITTY_DONE__:").map(|pos| (pos, 18, false))
                };

                if let Some((pos, prefix_len, is_osc)) = sentinel_pattern {
                    let after = &text[pos + prefix_len..];
                    let found_terminator = if is_osc {
                        after.find('\x1b').or_else(|| after.find('\x07')).or_else(|| after.find('\n')).or_else(|| after.find('\r'))
                    } else {
                        after.find('\n').or_else(|| after.find('\r'))
                    };

                    if let Some(end_idx) = found_terminator {
                        let code_str = after[..end_idx].trim_matches(|c: char| !c.is_ascii_digit());
                        let exit_code: i32 = code_str.parse().unwrap_or(0);
                        let raw_output = &text[..pos];

                        let clean_output = clean_pty_output(raw_output, &capture.command);
                        let final_summary = if clean_output.is_empty() {
                            format!("(Commande exécutée avec succès dans le terminal - code {})", exit_code)
                        } else {
                            format!("Sortie dans le terminal (code {}):\n{}", exit_code, clean_output)
                        };

                        if let Some(tx) = capture.result_tx.take() {
                            let _ = tx.send(final_summary);
                        }
                        self.active_pty_tool = None;
                        self.focus = Focus::Chat;
                    }
                }
            }
        }
    }

    pub fn on_agent_new_turn(&mut self) {
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: String::new(),
            command_proposal: None,
        });
        self.chat_scroll_from_bottom = 0;
    }

    pub fn on_agent_done(&mut self) {
        if let Some(start) = self.generation_start_time.take() {
            let secs = start.elapsed().as_secs_f64();
            if secs > 0.1 && self.current_turn_tokens > 0 {
                self.last_tokens_per_sec = Some(self.current_turn_tokens as f64 / secs);
            }
        }
        self.last_chunk_time = None;

        self.agent.is_generating = false;
        self.pending_tool_approval = None;
        self.focus = Focus::Chat;
        self.chat_scroll_from_bottom = 0;
        if let Some(last_msg) = self.messages.last() {
            if last_msg.role == MessageRole::Assistant && last_msg.content.trim().is_empty() {
                self.messages.pop();
            }
        }
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
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant {
                if last_msg.content.is_empty() {
                    last_msg.content = format!("⚠️ Erreur : {}", error);
                } else {
                    last_msg.content.push_str(&format!("\n\n⚠️ Erreur : {}", error));
                }
            }
        }
    }
}

/// Determines if a code block is an actual executable shell command vs passive text/log/tree output
pub fn is_executable_command_block(fence_tag: &str, content: &str) -> bool {
    let tag = fence_tag.trim().to_lowercase();

    // 1. Tags that are explicitly output or data formats
    if matches!(
        tag.as_str(),
        "output" | "result" | "text" | "txt" | "log" | "logs" | "tree" | "table"
            | "json" | "yaml" | "toml" | "md" | "markdown" | "diff" | "status" | "info"
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

    // 2. Reject process trees, log formats, systemd status trees
    if trimmed.contains('├') || trimmed.contains('└') || trimmed.contains('│') || trimmed.contains("──") {
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
    }) {
        return false;
    }

    // 4. Known shell language tags
    if matches!(tag.as_str(), "bash" | "sh" | "zsh" | "fish" | "shell" | "cmd" | "terminal" | "console") {
        return true;
    }

    // 5. Untagged blocks (""): only accept if simple command line
    if tag.is_empty() {
        let first_line = trimmed.lines().next().unwrap_or("").trim();
        if first_line.starts_with('{') || first_line.starts_with('[') || first_line.starts_with('<') || first_line.starts_with('#') {
            return false;
        }
        let colon_count = trimmed.matches(':').count();
        let line_count = trimmed.lines().count();
        if line_count > 2 && colon_count >= line_count {
            return false;
        }
        return true;
    }

    false
}

/// Extracts all proposed shell commands from markdown code blocks (excluding output/tools/trees)
pub fn extract_all_command_proposals(text: &str) -> Vec<String> {
    let mut list = Vec::new();
    let mut remaining = text;

    while let Some(start_idx) = remaining.find("```") {
        let after_fence = &remaining[start_idx + 3..];
        let (fence_tag, code_rest) = if let Some(first_nl) = after_fence.find('\n') {
            (after_fence[..first_nl].trim(), &after_fence[first_nl + 1..])
        } else {
            ("", after_fence)
        };

        if let Some(end_idx) = code_rest.find("```") {
            let code_content = code_rest[..end_idx].trim();
            if is_executable_command_block(fence_tag, code_content) && !list.contains(&code_content.to_string()) {
                list.push(code_content.to_string());
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

pub fn is_natural_approval_phrase(text: &str) -> bool {
    let clean = text.trim().to_lowercase();
    matches!(
        clean.as_str(),
        "" | "ok"
            | "oui"
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
        "commande #", "commande ", "cmd #", "cmd ", "lance la commande #", "lance la commande ",
        "lance la ", "lance le ", "lance #", "lance ", "exécute la commande #", "exécute la commande ",
        "exécute la ", "exécute #", "exécute ", "execute #", "execute ", "run #", "run ", "alt+", "alt ", "la "
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
fn key_event_to_pty_bytes(key: KeyEvent) -> Vec<u8> {
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
        KeyCode::Up => vec![0x1B, b'[', b'A'],
        KeyCode::Down => vec![0x1B, b'[', b'B'],
        KeyCode::Right => vec![0x1B, b'[', b'C'],
        KeyCode::Left => vec![0x1B, b'[', b'D'],
        KeyCode::Home => vec![0x1B, b'[', b'H'],
        KeyCode::End => vec![0x1B, b'[', b'F'],
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
                let base_url = p_cfg.base_url.as_deref().unwrap_or("http://localhost:1234/v1");
                let root_url = base_url.trim_end_matches("/v1").trim_end_matches('/');
                let api_url = format!("{}/api/v0/models", root_url);

                if let Ok(resp) = client.get(&api_url).send().await {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                            for item in data {
                                let id = item.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                                let is_loaded = item.get("state").and_then(|v| v.as_str()) == Some("loaded");
                                if is_loaded || id == p_cfg.model {
                                    if let Some(loaded_ctx) = item.get("loaded_context_length").and_then(|v| v.as_u64()) {
                                        if loaded_ctx > 0 {
                                            target.store(loaded_ctx as usize, Ordering::Relaxed);
                                            return;
                                        }
                                    }
                                    if let Some(max_ctx) = item.get("max_context_length").and_then(|v| v.as_u64()) {
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
                let base_url = p_cfg.base_url.as_deref().unwrap_or("http://localhost:11434");
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

/// Cleans captured PTY output by stripping ANSI colors, CRs, and prompt echoes.
fn clean_pty_output(raw: &str, command: &str) -> String {
    let no_ansi = strip_ansi_sequences(raw);
    let no_cr = no_ansi.replace('\r', "");

    let mut lines: Vec<&str> = no_cr.lines().collect();

    // Filter out internal sentinel or command echo remnants
    lines.retain(|l| {
        let t = l.trim();
        !t.is_empty()
            && !t.contains("spiritty_done")
            && !t.contains("__spiritty")
            && !t.contains("__SPIRITTY")
            && !t.contains("printf '\\e]777")
    });

    if let Some(first) = lines.first() {
        if first.contains(command) || first.contains("bash -c") {
            lines.remove(0);
        }
    }

    lines.join("\n").trim().to_string()
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

    // 2. Heredocs (`<<EOF`, `<< 'EOF'`, `<<-EOF`, etc.): preserve full multiline structure verbatim!
    if trimmed.contains("<<") {
        let script_lines: Vec<&str> = trimmed
            .lines()
            .filter(|l| !l.trim().starts_with("#!"))
            .collect();
        let script_body = script_lines.join("\n");
        let escaped = script_body.replace('\'', "'\\''");
        return format!("bash -c '{}'", escaped);
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
            || (l.contains('=') && !l.starts_with("echo ") && !l.starts_with("printf ") && l.split('=').next().map(|v| v.chars().all(|c| c.is_alphanumeric() || c == '_')).unwrap_or(false))
    });

    if has_shebang || has_bash_keywords {
        let script_lines: Vec<&str> = raw_lines
            .into_iter()
            .filter(|l| !l.starts_with("#!"))
            .collect();
        let script_body = script_lines.join("\n");
        let escaped = script_body.replace('\'', "'\\''");
        return format!("bash -c '{}'", escaped);
    }

    let lines: Vec<&str> = raw_lines
        .into_iter()
        .filter(|l| !l.starts_with('#'))
        .collect();

    let mut result = String::new();
    for (i, line) in lines.iter().enumerate() {
        let mut l = line.trim();
        // Strip trailing line-continuation backslashes (but not \( or escaped chars)
        while l.ends_with('\\') && !l.ends_with(r"\(") && !l.ends_with(r"\)") && !l.ends_with(r"\;") {
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
                            if prev != ';' && prev != '&' && prev != '|' && prev != '\n' && prev != '{' {
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
        || lower.starts_with("puis ")
        || lower.starts_with("ensuite ")
        || lower.starts_with("in order to ")
        || lower.starts_with("if you ")
        || lower.starts_with("here is ")
        || lower.starts_with("this will ")
    {
        return false;
    }

    true
}

/// Formats a command for reliable execution in the PTY.
/// When the user shell is non-POSIX (like Fish or Nushell), wraps commands in `bash -c '...'`
/// so all bashisms, wildcards, pipelines, and variable expressions execute seamlessly,
/// while preserving interactive builtins like `cd`.
fn format_command_for_pty(command: &str, user_shell: &str) -> String {
    let clean = clean_multiline_command(command);

    let is_non_bash = user_shell.contains("fish")
        || user_shell.contains("nu")
        || user_shell.contains("csh")
        || user_shell.contains("tcsh");
    let is_cd = clean.starts_with("cd ") || clean == "cd";
    let is_already_bash = clean.starts_with("bash -c");

    if is_non_bash && !is_cd && !is_already_bash {
        let escaped = clean.replace('\'', "'\\''");
        return format!("bash -c '{}'\n", escaped);
    }

    format!("{}\n", clean)
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

