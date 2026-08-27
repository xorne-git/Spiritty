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

    /// Compacts older conversation history into a structured summary to save context tokens,
    /// while keeping the most recent turns intact for ongoing interaction.
    pub fn compact(&mut self) -> bool {
        if self.messages.len() <= 8 {
            return false;
        }

        let split_idx = self.messages.len().saturating_sub(8);
        let older_msgs = &self.messages[..split_idx];
        let recent_msgs = &self.messages[split_idx..];

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

        self.compacted_summary = Some(summary_text.clone());

        // Replace older messages with a single summary message, followed by recent messages
        let mut new_messages = Vec::new();
        new_messages.push(ChatMessage {
            role: MessageRole::System,
            content: summary_text,
            command_proposal: None,
        });
        new_messages.extend_from_slice(recent_msgs);

        self.messages = new_messages;
        self.updated_at = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        true
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
