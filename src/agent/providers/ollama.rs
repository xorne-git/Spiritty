use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use super::LlmProvider;
use crate::{
    app::{ChatMessage, MessageRole},
    event::AppEvent,
};

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
    prompt_eval_count: Option<usize>,
    eval_count: Option<usize>,
    eval_duration: Option<u64>,
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
        cancel: CancellationToken,
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
        let send_res = timeout(
            Duration::from_secs(45),
            self.client.post(&url).json(&request_body).send(),
        )
        .await;

        let response = match send_res {
            Ok(Ok(resp)) => resp,
            Ok(Err(err)) => {
                let err_msg = format!(
                    "Impossible de joindre Ollama sur {} : {}. Vérifiez qu'Ollama est bien lancé ('ollama serve').",
                    url, err
                );
                let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
                anyhow::bail!(err_msg);
            }
            Err(_) => {
                let err_msg = format!(
                    "Délai d'attente dépassé (timeout 45s) pour joindre Ollama sur {}.",
                    url
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
        // Accumulate raw bytes so a UTF-8 character split across TCP chunks is never dropped.
        let mut buffer: Vec<u8> = Vec::new();

        loop {
            let next_res = tokio::select! {
                _ = cancel.cancelled() => break,
                r = timeout(Duration::from_secs(90), stream.next()) => r,
            };

            let chunk_res = match next_res {
                Ok(Some(res)) => res,
                Ok(None) => break,
                Err(_) => {
                    let err_msg =
                        "Délai d'inactivité de 90s dépassé sur le flux Ollama (timeout SSE)."
                            .to_string();
                    let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
                    anyhow::bail!(err_msg);
                }
            };

            match chunk_res {
                Ok(bytes) => {
                    buffer.extend_from_slice(&bytes);

                    while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                        let line_bytes: Vec<u8> = buffer.drain(..=pos).collect();
                        let line = String::from_utf8_lossy(&line_bytes);
                        let line = line.trim();

                        if line.is_empty() {
                            continue;
                        }

                        // Check for error frames first (Ollama reports errors with an "error" field).
                        if let Ok(val) = serde_json::from_str::<Value>(line) {
                            if let Some(err) = val.get("error").and_then(|e| e.as_str()) {
                                let _ = event_tx.send(AppEvent::AgentError(err.to_string()));
                                anyhow::bail!("Ollama error: {}", err);
                            }
                        }

                        if let Ok(chunk) = serde_json::from_str::<OllamaChunk>(line) {
                            if let Some(msg) = chunk.message {
                                if let Some(content) = msg.content {
                                    if !content.is_empty() {
                                        let _ = event_tx.send(AppEvent::AgentChunk(content));
                                    }
                                }
                            }
                            if chunk.done.unwrap_or(false) {
                                if let Some(eval_cnt) = chunk.eval_count {
                                    let prompt_cnt = chunk.prompt_eval_count.unwrap_or(0);
                                    let speed = chunk.eval_duration.and_then(|dur_ns| {
                                        if dur_ns > 0 {
                                            Some(
                                                eval_cnt as f64 / (dur_ns as f64 / 1_000_000_000.0),
                                            )
                                        } else {
                                            None
                                        }
                                    });
                                    let _ = event_tx.send(AppEvent::AgentUsage {
                                        prompt_tokens: prompt_cnt,
                                        completion_tokens: eval_cnt,
                                        exact_speed: speed,
                                    });
                                }
                                let _ = event_tx.send(AppEvent::AgentDone);
                                return Ok(());
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
