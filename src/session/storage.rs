use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use super::Session;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHeader {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub provider: String,
    pub model: String,
    pub message_count: usize,
    pub total_tokens: usize,
}

pub struct SessionStorage;

impl SessionStorage {
    pub fn sessions_dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Could not find standard config directory (~/.config)")?
            .join("spiritty")
            .join("sessions");

        if !dir.exists() {
            fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create sessions directory at {:?}", dir))?;
        }

        Ok(dir)
    }

    pub fn list_sessions() -> Result<Vec<SessionHeader>> {
        let dir = Self::sessions_dir()?;
        let mut headers = Vec::new();

        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map(|e| e == "json").unwrap_or(false) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(session) = serde_json::from_str::<Session>(&content) {
                            if session.messages.is_empty() {
                                continue;
                            }
                            headers.push(SessionHeader {
                                id: session.id,
                                title: session.title,
                                created_at: session.created_at,
                                updated_at: session.updated_at,
                                provider: session.provider,
                                model: session.model,
                                message_count: session.messages.len(),
                                total_tokens: session.total_tokens,
                            });
                        }
                    }
                }
            }
        }

        // Sort descending by updated_at (most recent first)
        headers.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(headers)
    }

    pub fn load(id: &str) -> Result<Session> {
        let dir = Self::sessions_dir()?;
        let file_path = dir.join(format!("{}.json", id));
        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read session file at {:?}", file_path))?;
        let session = serde_json::from_str::<Session>(&content)
            .with_context(|| format!("Failed to parse session JSON from {:?}", file_path))?;
        Ok(session)
    }

    pub fn save(session: &Session) -> Result<()> {
        let dir = Self::sessions_dir()?;
        let file_path = dir.join(format!("{}.json", session.id));
        let json = serde_json::to_string_pretty(session)
            .context("Failed to serialize session to JSON")?;
        fs::write(&file_path, json)
            .with_context(|| format!("Failed to write session file to {:?}", file_path))?;
        Ok(())
    }

    pub fn delete(id: &str) -> Result<()> {
        let dir = Self::sessions_dir()?;
        let file_path = dir.join(format!("{}.json", id));
        if file_path.exists() {
            fs::remove_file(&file_path)
                .with_context(|| format!("Failed to delete session file at {:?}", file_path))?;
        }
        Ok(())
    }
}
