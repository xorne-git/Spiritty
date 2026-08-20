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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub compacted_summary: Option<String>,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub prompt_history: Vec<String>,
}

use std::sync::atomic::{AtomicUsize, Ordering};
static SESSION_SEQ: AtomicUsize = AtomicUsize::new(1);

impl Session {
    pub fn new(provider: &str, model: &str) -> Self {
        let seq = SESSION_SEQ.fetch_add(1, Ordering::Relaxed);
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        let timestamp_id = format!("sess_{}_{:04}", chrono::Local::now().format("%Y%m%d_%H%M%S"), seq % 10000);

        Self {
            id: timestamp_id,
            title: "Nouvelle session".to_string(),
            created_at: now.clone(),
            updated_at: now,
            provider: provider.to_string(),
            model: model.to_string(),
            total_tokens: 0,
            compacted_summary: None,
            messages: Vec::new(),
            prompt_history: Vec::new(),
        }
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
            format!("Contexte précédent compacté :\n{}", summary_points.join("\n"))
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
