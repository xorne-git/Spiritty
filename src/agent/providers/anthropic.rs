use anyhow::{Context, Result};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::{ChatMessage, MessageRole},
    event::AppEvent,
};
use super::LlmProvider;

pub struct AnthropicProvider {
    base_url: String,
    model: String,
    api_key: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(base_url: Option<String>, model: String, api_key: String) -> Self {
        let base_url = base_url
            .unwrap_or_else(|| "https://api.anthropic.com".to_string())
            .trim_end_matches('/')
            .to_string();

        Self {
            base_url,
            model,
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    #[serde(skip_serializing_if = "str::is_empty")]
    system: &'a str,
    messages: Vec<AnthropicMessage<'a>>,
    stream: bool,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum AnthropicEvent {
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { delta: ContentDelta },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ContentDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(other)]
    Other,
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn stream_chat(
        &self,
        messages: &[ChatMessage],
        system_prompt: &str,
        event_tx: UnboundedSender<AppEvent>,
    ) -> Result<()> {
        if self.api_key.trim().is_empty() {
            let err = "Clé d'API Anthropic manquante. Configurez-la avec Ctrl+P ou exportez ANTHROPIC_API_KEY.".to_string();
            let _ = event_tx.send(AppEvent::AgentError(err.clone()));
            anyhow::bail!(err);
        }

        let mut api_messages = Vec::new();
        for msg in messages {
            let role = match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System => "user",
            };
            api_messages.push(AnthropicMessage {
                role,
                content: &msg.content,
            });
        }

        let request_body = AnthropicRequest {
            model: &self.model,
            max_tokens: 4096,
            system: system_prompt,
            messages: api_messages,
            stream: true,
        };

        let url = format!("{}/v1/messages", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .with_context(|| format!("Failed to connect to Anthropic API at {}", url))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            let err_msg = format!("Anthropic API error (HTTP {}): {}", status, error_text);
            let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
            anyhow::bail!(err_msg);
        }

        let mut event_stream = response.bytes_stream().eventsource();

        while let Some(event_res) = event_stream.next().await {
            match event_res {
                Ok(event) => {
                    let data = event.data.trim();
                    if let Ok(parsed) = serde_json::from_str::<AnthropicEvent>(data) {
                        match parsed {
                            AnthropicEvent::ContentBlockDelta { delta } => {
                                if let ContentDelta::TextDelta { text } = delta {
                                    if !text.is_empty() {
                                        let _ = event_tx.send(AppEvent::AgentChunk(text));
                                    }
                                }
                            }
                            AnthropicEvent::MessageStop => {
                                let _ = event_tx.send(AppEvent::AgentDone);
                                return Ok(());
                            }
                            AnthropicEvent::Other => {}
                        }
                    }
                }
                Err(err) => {
                    let err_msg = format!("Anthropic stream error: {}", err);
                    let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
                    anyhow::bail!(err_msg);
                }
            }
        }

        let _ = event_tx.send(AppEvent::AgentDone);
        Ok(())
    }
}
