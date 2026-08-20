use anyhow::{Context, Result};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::timeout;

use crate::{
    app::{ChatMessage, MessageRole},
    event::AppEvent,
};
use super::LlmProvider;

pub struct OpenAiCompatibleProvider {
    name: String,
    base_url: String,
    model: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        name: &str,
        base_url: Option<String>,
        model: String,
        api_key: Option<String>,
    ) -> Self {
        let base_url = base_url
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string())
            .trim_end_matches('/')
            .to_string();

        Self {
            name: name.to_string(),
            base_url,
            model,
            api_key,
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
        }
    }
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
}

#[derive(Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<ChunkChoice>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChunkDelta {
    content: Option<String>,
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    async fn stream_chat(
        &self,
        messages: &[ChatMessage],
        system_prompt: &str,
        event_tx: UnboundedSender<AppEvent>,
    ) -> Result<()> {
        // Validate API Key for cloud providers
        if self.name != "LM Studio" {
            let key_str = self.api_key.as_deref().unwrap_or("").trim();
            if key_str.is_empty() {
                let err = format!(
                    "Clé d'API manquante pour {}. Configurez-la avec Ctrl+P ou exportez la variable d'environnement.",
                    self.name
                );
                let _ = event_tx.send(AppEvent::AgentError(err.clone()));
                anyhow::bail!(err);
            }
        }

        let mut api_messages = Vec::new();

        if !system_prompt.is_empty() {
            api_messages.push(Message {
                role: "system",
                content: system_prompt,
            });
        }

        for msg in messages {
            let role = match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System => "user",
            };
            api_messages.push(Message {
                role,
                content: &msg.content,
            });
        }

        let request_body = ChatCompletionRequest {
            model: &self.model,
            messages: api_messages,
            stream: true,
            max_tokens: Some(8192),
        };

        let url = format!("{}/chat/completions", self.base_url);
        let mut req = self.client.post(&url).json(&request_body);

        if let Some(key) = &self.api_key {
            let clean_key = key.trim();
            if !clean_key.is_empty() {
                req = req.bearer_auth(clean_key);
            }
        }

        let response = req
            .send()
            .await
            .with_context(|| format!("Failed to connect to {} at {}", self.name, url))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            let err_msg = format!("{} error (HTTP {}): {}", self.name, status, error_text);
            let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
            anyhow::bail!(err_msg);
        }

        let mut event_stream = response.bytes_stream().eventsource();

        loop {
            match timeout(Duration::from_secs(25), event_stream.next()).await {
                Ok(Some(event_res)) => match event_res {
                    Ok(event) => {
                        let data = event.data.trim();
                        if data == "[DONE]" {
                            let _ = event_tx.send(AppEvent::AgentDone);
                            return Ok(());
                        }

                        if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                            for choice in chunk.choices {
                                if let Some(content) = choice.delta.content {
                                    if !content.is_empty() {
                                        let _ = event_tx.send(AppEvent::AgentChunk(content));
                                    }
                                }
                                if let Some(ref reason) = choice.finish_reason {
                                    if reason == "length" {
                                        let _ = event_tx.send(AppEvent::AgentChunk(
                                            "\n\n[⚠️ Réponse interrompue : limite de tokens atteinte. Tapez 'continue' pour la suite.]".to_string(),
                                        ));
                                    }
                                    let _ = event_tx.send(AppEvent::AgentDone);
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Err(err) => {
                        let err_msg = format!("SSE stream error: {}", err);
                        let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
                        anyhow::bail!(err_msg);
                    }
                },
                Ok(None) => {
                    break;
                }
                Err(_) => {
                    let err_msg = "Délai d'inactivité de 25s dépassé sur le flux du modèle (timeout SSE).".to_string();
                    let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
                    anyhow::bail!(err_msg);
                }
            }
        }

        let _ = event_tx.send(AppEvent::AgentDone);
        Ok(())
    }
}
