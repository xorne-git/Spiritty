use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::{ChatMessage, MessageRole},
    event::AppEvent,
};
use super::LlmProvider;

pub struct OllamaProvider {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(base_url: Option<String>, model: String) -> Self {
        let base_url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Serialize)]
struct OllamaMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage<'a>>,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaChunk {
    message: Option<OllamaChunkMessage>,
    done: Option<bool>,
}

#[derive(Deserialize)]
struct OllamaChunkMessage {
    content: Option<String>,
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    async fn stream_chat(
        &self,
        messages: &[ChatMessage],
        system_prompt: &str,
        event_tx: UnboundedSender<AppEvent>,
    ) -> Result<()> {
        let mut api_messages = Vec::new();

        // 1. System Prompt
        if !system_prompt.is_empty() {
            api_messages.push(OllamaMessage {
                role: "system",
                content: system_prompt,
            });
        }

        // 2. Chat history
        for msg in messages {
            let role = match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System => "user",
            };
            api_messages.push(OllamaMessage {
                role,
                content: &msg.content,
            });
        }

        let request_body = OllamaRequest {
            model: &self.model,
            messages: api_messages,
            stream: true,
        };

        let url = format!("{}/api/chat", self.base_url);
        let send_res = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await;

        let response = match send_res {
            Ok(resp) => resp,
            Err(err) => {
                let err_msg = format!(
                    "Impossible de joindre Ollama sur {} : {}. Vérifiez qu'Ollama est bien lancé ('ollama serve').",
                    url, err
                );
                let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
                anyhow::bail!(err_msg);
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            let err_msg = format!("Ollama error (HTTP {}): {}", status, error_text);
            let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
            anyhow::bail!(err_msg);
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_res) = stream.next().await {
            match chunk_res {
                Ok(bytes) => {
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        buffer.push_str(text);

                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].trim().to_string();
                            buffer.drain(..=pos);

                            if line.is_empty() {
                                continue;
                            }

                            if let Ok(chunk) = serde_json::from_str::<OllamaChunk>(&line) {
                                if let Some(msg) = chunk.message {
                                    if let Some(content) = msg.content {
                                        if !content.is_empty() {
                                            let _ = event_tx.send(AppEvent::AgentChunk(content));
                                        }
                                    }
                                }
                                if chunk.done.unwrap_or(false) {
                                    let _ = event_tx.send(AppEvent::AgentDone);
                                    return Ok(());
                                }
                            } else if let Ok(val) = serde_json::from_str::<Value>(&line) {
                                if let Some(err) = val.get("error").and_then(|e| e.as_str()) {
                                    let _ = event_tx.send(AppEvent::AgentError(err.to_string()));
                                    anyhow::bail!("Ollama error: {}", err);
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    let err_msg = format!("Ollama stream read error: {}", err);
                    let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
                    anyhow::bail!(err_msg);
                }
            }
        }

        let _ = event_tx.send(AppEvent::AgentDone);
        Ok(())
    }
}
