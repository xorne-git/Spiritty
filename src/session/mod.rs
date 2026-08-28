pub mod storage;

use serde::{Deserialize, Serialize};

use crate::app::{ChatMessage, MessageRole};
pub use storage::{SessionHeader, SessionStorage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub provider: String,
    pub model: String,
    pub total_tokens: usize,
    #[serde(default)]
    pub prompt_tokens: usize,
    #[serde(default)]
    pub completion_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub compacted_summary: Option<String>,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub prompt_history: Vec<String>,
    /// Last SSH target seen while this session was active (sticky). Set
    /// automatically whenever a save happens while an SSH session is detected, and
    /// reused to display a "SSH resumed" hint when continuing the session with `-c`
    /// before the user reconnects to the remote host.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub last_ssh_target: Option<String>,
}

use std::sync::atomic::{AtomicUsize, Ordering};
static SESSION_SEQ: AtomicUsize = AtomicUsize::new(1);

impl Session {
    pub fn new(provider: &str, model: &str) -> Self {
        let seq = SESSION_SEQ.fetch_add(1, Ordering::Relaxed);
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        // Microsecond precision + monotonic sequence makes the id unique across app restarts
        // (the previous second-granularity id could collide after a quick quit/relaunch).
        let timestamp_id = format!(
            "sess_{}_{:03}",
            chrono::Local::now().format("%Y%m%d_%H%M%S_%6f"),
            seq % 1000
        );

        Self {
            id: timestamp_id,
            title: "Nouvelle session".to_string(),
            created_at: now.clone(),
            updated_at: now,
            provider: provider.to_string(),
            model: model.to_string(),
            total_tokens: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            compacted_summary: None,
            messages: Vec::new(),
            prompt_history: Vec::new(),
            last_ssh_target: None,
        }
    }

    /// Best-effort detection of a SSH context from the conversation history — used
    /// when resuming a session saved by an older build without `last_ssh_target`.
    /// Scans the most recent messages for an explicit `ssh …` command or a remote
    /// prompt remnant (`user@host:~$`), newest first.
    pub fn infer_last_ssh_target(&mut self) {
        if self.last_ssh_target.is_some() {
            return;
        }
        for m in self.messages.iter().rev().take(80) {
            if let Some(target) = extract_ssh_target(&m.content) {
                self.last_ssh_target = Some(target);
                return;
            }
        }
    }

    /// Calculates estimated session cost in USD using built-in defaults or cached registry
    pub fn estimated_cost_usd(&self) -> f64 {
        let registry = crate::pricing::PricingRegistry::default();
        self.estimated_cost_with_pricing(&registry)
    }

    /// Calculates estimated session cost in USD with a specific PricingRegistry
    pub fn estimated_cost_with_pricing(&self, registry: &crate::pricing::PricingRegistry) -> f64 {
        self.estimated_cost_opt(registry).unwrap_or(0.0)
    }

    /// Estimated session cost only when a tariff is actually configured for this session's
    /// provider/model — `None` otherwise, so the UI can hide the cost instead of guessing.
    pub fn estimated_cost_opt(&self, registry: &crate::pricing::PricingRegistry) -> Option<f64> {
        let pricing = registry.get_pricing(&self.provider, &self.model)?;
        Some(pricing.calculate_cost(self.prompt_tokens, self.completion_tokens))
    }

    /// Synchronizes session with current chat messages, prompt history, total tokens, and active provider/model.
    pub fn update_from_chat(
        &mut self,
        messages: &[ChatMessage],
        prompt_history: &[String],
        tokens: usize,
        provider: &str,
        model: &str,
    ) {
        self.messages = messages.to_vec();
        self.prompt_history = prompt_history.to_vec();
        self.total_tokens = tokens;
        self.provider = provider.to_string();
        self.model = model.to_string();
        self.updated_at = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();

        // Auto-generate or refine title from the first substantive user prompt
        if is_generic_or_default_title(&self.title) && !messages.is_empty() {
            for msg in messages {
                if msg.role == MessageRole::User && !msg.content.trim().is_empty() {
                    let first_line = msg.content.lines().next().unwrap_or("").trim();
                    let clean = first_line.trim_start_matches(|c: char| !c.is_alphanumeric());
                    let lower = clean.to_lowercase();
                    if !is_generic_or_default_title(&lower) && clean.len() >= 3 {
                        let title: String = clean.chars().take(45).collect();
                        if !title.is_empty() {
                            self.title = title;
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Compacts this session's persisted history (kept for compatibility with
    /// older callers/tests): older turns roll into a structured summary while
    /// the most recent turns stay intact. No longer called on save — the disk
    /// session keeps the FULL history since v0.5.2; compaction applies only to
    /// the LLM context (see `compact_chat_messages`).
    pub fn compact(&mut self) -> bool {
        match compact_chat_messages(&self.messages) {
            CompactedHistory {
                messages,
                summary: Some(summary),
            } => {
                self.compacted_summary = Some(summary);
                self.messages = messages;
                self.updated_at = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
                true
            }
            _ => false,
        }
    }
}

/// Outcome of a history compaction: the rewritten message list (rolling System
/// summary first, recent turns preserved verbatim) plus the summary text when
/// compaction actually applied.
pub struct CompactedHistory {
    pub messages: Vec<ChatMessage>,
    pub summary: Option<String>,
}

/// Compacts a chat history **for LLM context only**: turns older than the last
/// 8 roll into a single System summary message so long conversations stay
/// within a bounded context budget. Histories of 8 messages or fewer are
/// returned unchanged (`summary: None`).
///
/// Pure transformation — it never mutates the live conversation nor the
/// persisted session: the UI keeps scrolling the full history and the session
/// JSON saves every message.
pub fn compact_chat_messages(messages: &[ChatMessage]) -> CompactedHistory {
    if messages.len() <= 8 {
        return CompactedHistory {
            messages: messages.to_vec(),
            summary: None,
        };
    }

    let split_idx = messages.len().saturating_sub(8);
    let older_msgs = &messages[..split_idx];
    let recent_msgs = &messages[split_idx..];

    let mut summary_points = Vec::new();
    for msg in older_msgs {
        let trimmed = msg.content.trim();
        if trimmed.is_empty() {
            continue;
        }
        match msg.role {
            MessageRole::User => {
                if let Some(start) = trimmed.find("💻 `") {
                    let prefix_len = "💻 `".len();
                    let after = &trimmed[start + prefix_len..];
                    if let Some(end) = after.find('`') {
                        let snippet = clean_summary_snippet(&after[..end], 120);
                        summary_points.push(format!("- 💻 Commande exécutée : `{}`", snippet));
                    } else {
                        let snippet = clean_summary_snippet(trimmed, 120);
                        summary_points.push(format!("- 👤 Utilisateur : {}", snippet));
                    }
                } else {
                    let first_line = trimmed.lines().next().unwrap_or(trimmed);
                    let snippet = clean_summary_snippet(first_line, 120);
                    summary_points.push(format!("- 👤 Utilisateur : {}", snippet));
                }
            }
            MessageRole::Assistant => {
                if let Some(start) = trimmed.find("💻 `") {
                    let prefix_len = "💻 `".len();
                    let after = &trimmed[start + prefix_len..];
                    if let Some(end) = after.find('`') {
                        let snippet = clean_summary_snippet(&after[..end], 120);
                        summary_points.push(format!("- 💻 Commande exécutée : `{}`", snippet));
                    } else {
                        let snippet = clean_summary_snippet(trimmed, 120);
                        summary_points.push(format!("- 👻 Résumé assistant : {}", snippet));
                    }
                } else {
                    let first_line = trimmed.lines().next().unwrap_or(trimmed);
                    let snippet = clean_summary_snippet(first_line, 120);
                    summary_points.push(format!("- 👻 Résumé assistant : {}", snippet));
                }
            }
            MessageRole::System => {
                // Accumulate and preserve previous compaction points rather than dropping them
                for l in trimmed.lines() {
                    let line_trim = l.trim();
                    if line_trim.starts_with("- ") {
                        summary_points.push(line_trim.to_string());
                    }
                }
            }
        }
    }

    let summary_text = if summary_points.is_empty() {
        "Contexte précédent archivé et compacté.".to_string()
    } else {
        format!(
            "Contexte précédent compacté :\n{}",
            summary_points.join("\n")
        )
    };

    // Replace older messages with a single summary message, followed by recent messages
    let mut new_messages = Vec::new();
    new_messages.push(ChatMessage {
        role: MessageRole::System,
        content: summary_text.clone(),
        command_proposal: None,
    });
    new_messages.extend_from_slice(recent_msgs);

    CompactedHistory {
        messages: new_messages,
        summary: Some(summary_text),
    }
}

fn is_generic_or_default_title(title: &str) -> bool {
    let clean = title.trim().to_lowercase();
    matches!(
        clean.as_str(),
        "" | "nouvelle session"
            | "new session"
            | "salut"
            | "salut!"
            | "salut !"
            | "bonjour"
            | "bonjour!"
            | "hello"
            | "hello!"
            | "hi"
            | "hi!"
            | "hey"
            | "yo"
            | "test"
            | "test!"
            | "coucou"
    )
}

fn clean_summary_snippet(text: &str, max_chars: usize) -> String {
    let clean = text.trim();
    if clean.chars().count() <= max_chars {
        return clean.to_string();
    }

    let truncated: String = clean.chars().take(max_chars).collect();
    if let Some(last_space) = truncated.rfind(' ') {
        if last_space > max_chars / 2 {
            return format!("{}...", &truncated[..last_space]);
        }
    }
    format!("{}...", truncated)
}

/// Extracts a SSH target from one message's content, or `None`.
///
/// Two conservative signals (no fuzzy guessing):
/// 1. an executed/proposed command message (`💻 \``ssh …```) — the first non-flag
///    token after `ssh` is the target (alias, `host`, or `user@host`);
/// 2. a remote prompt remnant ANYWHERE in the content — classic `user@host:~$`,
///    `user@host:/path$`, or the bracketed zsh style `[user@host:/path] main(3) ±`.
///    The colon must be followed by an absolute path (`/`, `~`) to avoid matching
///    git remotes (`git@host:user/repo.git`) or e-mail addresses.
fn extract_ssh_target(content: &str) -> Option<String> {
    if content.starts_with("💻 `") {
        if let Some(pos) = content.find("ssh ") {
            let rest = &content[pos + 4..];
            for tok in rest.split_whitespace() {
                let tok = tok.trim_matches(|c| c == '`' || c == '"' || c == '\'');
                if tok.is_empty() || tok.starts_with('-') || tok.contains('/') {
                    continue;
                }
                return Some(tok.to_string());
            }
        }
    }

    for line in content.lines() {
        for tok in line.split_whitespace() {
            if !tok.contains('@') || tok.contains("://") {
                continue;
            }
            let tok = tok.trim_start_matches('[').trim_end_matches(']');
            let (user, rest) = tok.split_once('@')?;
            if user.is_empty() {
                continue;
            }
            match rest.split_once(':') {
                // `user@host:/abs/path` or `user@host:~` — prompt-style colon path.
                Some((host, path))
                    if !host.is_empty()
                        && !host.contains('/')
                        && (path.starts_with('/') || path.starts_with('~')) =>
                {
                    return Some(format!("{user}@{host}"));
                }
                // Colon-less prompt tail `user@host$` / `user@host#` (no TLD dot).
                None if rest.ends_with('$') || rest.ends_with('#') => {
                    let host = rest.trim_end_matches(['$', '#']);
                    if !host.is_empty() && !host.contains('.') {
                        return Some(format!("{user}@{host}"));
                    }
                }
                _ => {}
            }
        }
    }
    None
}

#[cfg(test)]
mod compaction_tests {
    use super::{compact_chat_messages, Session};
    use crate::app::{ChatMessage, MessageRole};

    fn msg(content: &str) -> ChatMessage {
        ChatMessage {
            role: MessageRole::User,
            content: content.to_string(),
            command_proposal: None,
        }
    }

    #[test]
    fn short_history_is_returned_unchanged() {
        let input: Vec<ChatMessage> = (0..8).map(|i| msg(&format!("m{}", i))).collect();
        let res = compact_chat_messages(&input);
        assert!(res.summary.is_none());
        assert_eq!(res.messages.len(), 8);
        assert_eq!(res.messages[0].content, "m0");
        assert_eq!(res.messages[7].content, "m7");
    }

    #[test]
    fn long_history_rolls_into_summary_plus_last_eight() {
        let mut input: Vec<ChatMessage> = Vec::new();
        for i in 0..20 {
            input.push(ChatMessage {
                role: if i % 2 == 0 {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                content: format!("message numéro {}", i),
                command_proposal: None,
            });
        }
        let res = compact_chat_messages(&input);
        let summary = res.summary.as_deref().expect("long history must compact");
        assert_eq!(res.messages.len(), 9); // 1 System summary + 8 recent turns
        assert_eq!(res.messages[0].role, MessageRole::System);
        assert!(summary.starts_with("Contexte précédent compacté"));
        // Older turns reduced to summary points, newest 8 preserved verbatim & ordered
        assert!(summary.contains("message numéro 0"));
        assert_eq!(res.messages[1].content, "message numéro 12");
        assert_eq!(res.messages[8].content, "message numéro 19");
        // Original slice untouched (pure transformation)
        assert_eq!(input.len(), 20);
    }

    #[test]
    fn session_compact_keeps_persisted_history_in_sync() {
        let mut session = Session::new("p", "m");
        session.messages = (0..14).map(|i| msg(&format!("m{}", i))).collect();
        assert!(session.compact());
        assert_eq!(session.messages.len(), 9);
        assert_eq!(session.messages[0].role, MessageRole::System);
        // Session::compact no longer feeds the save path — full history is persisted.
        session.messages = (0..11).map(|i| msg(&format!("n{}", i))).collect();
        let full: Vec<ChatMessage> = session.messages.clone();
        let before = full.len();
        let _ = session.compact();
        // compact() is still a pure helper; the save path (save_current_session)
        // no longer calls it — assert the stored full copy is what a save writes.
        assert_eq!(before, 11);
    }
}

#[cfg(test)]
mod ssh_hint_tests {
    use super::{extract_ssh_target, Session};
    use crate::app::{ChatMessage, MessageRole};

    fn msg(content: &str) -> ChatMessage {
        ChatMessage {
            role: MessageRole::User,
            content: content.to_string(),
            command_proposal: None,
        }
    }

    #[test]
    fn serde_backward_compatible_without_ssh_field() {
        // A session JSON written by an older build has no last_ssh_target key.
        let legacy = r#"{"id":"s","title":"t","created_at":"c","updated_at":"u","provider":"p","model":"m","total_tokens":0,"messages":[],"prompt_history":[]}"#;
        let s: Session = serde_json::from_str(legacy).unwrap();
        assert!(s.last_ssh_target.is_none());
    }

    #[test]
    fn infers_target_from_ssh_command_message() {
        let mut s = Session::new("p", "m");
        s.messages.push(msg("💻 `ssh xorne@203.0.113.7 hostname`"));
        s.infer_last_ssh_target();
        assert_eq!(s.last_ssh_target.as_deref(), Some("xorne@203.0.113.7"));
    }

    #[test]
    fn infers_target_from_remote_prompt_remnant() {
        let mut s = Session::new("p", "m");
        s.messages.push(msg(
            "[RÉSULTAT]: total 12\ndrwxr-xr-x 2 xorne xorne\nxorne@vps-prod:~$ ",
        ));
        s.infer_last_ssh_target();
        assert_eq!(s.last_ssh_target.as_deref(), Some("xorne@vps-prod"));
    }

    #[test]
    fn local_only_history_infers_nothing() {
        let mut s = Session::new("p", "m");
        s.messages.push(msg("💻 `ls -la`"));
        s.messages.push(msg("regarde le dossier src/ stp"));
        s.infer_last_ssh_target();
        assert!(s.last_ssh_target.is_none());
    }

    #[test]
    fn prose_mentioning_ssh_does_not_match() {
        assert_eq!(
            extract_ssh_target("je me connecte en ssh sur le serveur"),
            None
        );
    }

    #[test]
    fn infers_target_from_bracketed_zsh_prompt_mid_content() {
        // Real-world case: zsh bracketed prompt `[user@host:/path] main(3) ±` in the
        // MIDDLE of a command-result message (last line is the analysis instruction).
        let mut s = Session::new("p", "m");
        s.messages.push(msg(
            "[RÉSULTAT DE L'EXÉCUTION DE LA COMMANDE 'cat > /home/xorne/recap.txt <<'EOF' …']: ⏎ total 12\n[xorne@prod:/var/www/xorne/resa-prod] main(3) ± ls -la /home/xorne/recap_site_reservations.txt\n-rw-rw-r-- 1 xorne staff 1726\n[Analysez ce résultat et expliquez la situation à l'utilisateur]",
        ));
        s.infer_last_ssh_target();
        assert_eq!(s.last_ssh_target.as_deref(), Some("xorne@prod"));
    }

    #[test]
    fn git_remote_and_emails_do_not_match() {
        assert_eq!(
            extract_ssh_target("push origin git@github.com:user/repo.git"),
            None
        );
        assert_eq!(extract_ssh_target("écris à contact@moondogs.fr stp"), None);
        assert_eq!(
            extract_ssh_target("clone depuis https://token@host.com/user/repo.git"),
            None
        );
    }

    #[test]
    fn colonless_prompt_tail_matches() {
        assert_eq!(
            extract_ssh_target("ls -la\nxorne@prod$ "),
            Some("xorne@prod".to_string())
        );
        assert_eq!(
            extract_ssh_target("xorne@vps# "),
            Some("xorne@vps".to_string())
        );
        // A TLD-looking token without a colon path stays ignored (e-mail safety).
        assert_eq!(extract_ssh_target("xorne@prod.com$ "), None);
    }

    #[test]
    fn newest_message_wins() {
        let mut s = Session::new("p", "m");
        s.messages.push(msg("💻 `ssh old-host pwd`"));
        s.messages.push(msg("💻 `ssh new-host pwd`"));
        s.infer_last_ssh_target();
        assert_eq!(s.last_ssh_target.as_deref(), Some("new-host"));
    }
}
