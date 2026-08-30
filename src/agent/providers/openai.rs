use anyhow::Result;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use super::LlmProvider;
use crate::{
    app::{ChatMessage, MessageRole},
    event::AppEvent,
};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<Vec<ContentPart<'a>>>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ContentPart<'a> {
    Text { r#type: &'a str, text: &'a str },
    ImageUrl {
        r#type: &'a str,
        image_url: ImageUrl<'a>,
    },
}

#[derive(Serialize)]
struct ImageUrl<'a> {
    url: &'a str,
}

#[derive(Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: Option<usize>,
    completion_tokens: Option<usize>,
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
    /// DeepSeek/GLM/Z.ai-style streamed reasoning (the visible "thinking"): arrives in
    /// this SEPARATE field, NOT in `content`. Captured and re-wrapped in a
    /// `<think>…</think>` block so the chat panel's reasoning parser can display it.
    #[serde(default)]
    reasoning_content: Option<String>,
}

/// Streaming state machine that folds `reasoning_content` deltas into the plain
/// content stream as a `<think>…</think>` block (which the UI already parses):
/// opens `<think>` on the first reasoning chunk, closes it just before the first
/// real content chunk, and drops any stray reasoning that arrives after the answer
/// started (interleaving would otherwise leak raw tags into the visible response).
#[derive(Default)]
pub(crate) struct ReasoningBracket {
    in_reasoning: bool,
    closed: bool,
    /// True once visible answer content has been emitted: reasoning arriving after
    /// this point is dropped rather than wrapped (no raw tags in the response).
    content_started: bool,
}

impl ReasoningBracket {
    /// Returns the text to append to the visible content stream for this delta.
    pub(crate) fn on_delta(&mut self, reasoning: Option<&str>, content: Option<&str>) -> String {
        let mut out = String::new();
        if !self.closed {
            if let Some(rc) = reasoning.filter(|r| !r.is_empty()) {
                if !self.in_reasoning && !self.content_started {
                    out.push_str("<think>");
                    self.in_reasoning = true;
                }
                if self.in_reasoning {
                    out.push_str(rc);
                }
            }
            if let Some(c) = content.filter(|c| !c.is_empty()) {
                if self.in_reasoning {
                    out.push_str("</think>");
                    self.in_reasoning = false;
                }
                self.closed = true;
                self.content_started = true;
                out.push_str(c);
            }
        } else if let Some(c) = content.filter(|c| !c.is_empty()) {
            out.push_str(c);
        }
        out
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    async fn stream_chat(
        &self,
        messages: &[ChatMessage],
        system_prompt: &str,
        event_tx: UnboundedSender<AppEvent>,
        cancel: CancellationToken,
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
                content: Some(vec![ContentPart::Text {
                    r#type: "text",
                    text: system_prompt,
                }]),
            });
        }

        // Owned data-URIs of every attachment, aligned per-message with `messages`. They
        // must outlive the request body (which borrows them), so they are precomputed
        // rather than produced inline (a temporary would be dropped).
        let message_uris: Vec<Vec<String>> = messages
            .iter()
            .map(|m| m.attachments.iter().map(|a| a.data_uri()).collect())
            .collect();

        let mut api_messages = Vec::new();
        for (idx, msg) in messages.iter().enumerate() {
            let role = match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System => "user",
            };
            if msg.attachments.is_empty() {
                api_messages.push(Message {
                    role,
                    content: Some(vec![ContentPart::Text {
                        r#type: "text",
                        text: &msg.content,
                    }]),
                });
            } else {
                // Vision turn: text + one image part per attachment (data: URI). The text
                // part is retained so an image-only attach still carries an empty text block
                // (the API refuses an image-only turn for some providers).
                let mut parts = Vec::with_capacity(msg.attachments.len() + 1);
                parts.push(ContentPart::Text {
                    r#type: "text",
                    text: &msg.content,
                });
                for uri in &message_uris[idx] {
                    parts.push(ContentPart::ImageUrl {
                        r#type: "image_url",
                        image_url: ImageUrl { url: uri },
                    });
                }
                api_messages.push(Message {
                    role,
                    content: Some(parts),
                });
            }
        }

        let request_body = ChatCompletionRequest {
            model: &self.model,
            messages: api_messages,
            stream: true,
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
            max_tokens: None,
            temperature: Some(0.2),
        };

        let url = format!("{}/chat/completions", self.base_url);
        let mut req = self.client.post(&url).json(&request_body);

        if let Some(key) = &self.api_key {
            let clean_key = key.trim();
            if !clean_key.is_empty() {
                req = req.bearer_auth(clean_key);
            }
        }

        let send_res = timeout(Duration::from_secs(12), req.send()).await;

        let response = match send_res {
            Ok(Ok(resp)) => resp,
            Ok(Err(err)) => {
                let err_msg = if self.name == "LM Studio" {
                    format!(
                        "Impossible de se connecter à LM Studio sur {} ({}). Vérifiez que LM Studio est lancé et que le serveur local est actif ('Start Server').",
                        self.base_url, err
                    )
                } else {
                    format!(
                        "Échec de connexion vers {} sur {} : {}.",
                        self.name, self.base_url, err
                    )
                };
                let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
                anyhow::bail!(err_msg);
            }
            Err(_) => {
                let err_msg = format!(
                    "Délai d'attente dépassé (timeout 12s) lors de la connexion à {} sur {}.",
                    self.name, self.base_url
                );
                let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
                anyhow::bail!(err_msg);
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            let err_msg = format!("{} error (HTTP {}): {}", self.name, status, error_text);
            let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
            anyhow::bail!(err_msg);
        }

        let mut event_stream = response.bytes_stream().eventsource();

        // Folds reasoning_content deltas into a <think>…</think> block in the stream.
        let mut reasoning_bracket = ReasoningBracket::default();

        loop {
            let next_res = tokio::select! {
                _ = cancel.cancelled() => break,
                r = timeout(Duration::from_secs(25), event_stream.next()) => r,
            };

            match next_res {
                Ok(Some(event_res)) => match event_res {
                    Ok(event) => {
                        let data = event.data.trim();
                        if data == "[DONE]" {
                            let _ = event_tx.send(AppEvent::AgentDone);
                            return Ok(());
                        }

                        if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                            if let Some(usage) = chunk.usage {
                                let prompt_toks = usage.prompt_tokens.unwrap_or(0);
                                let comp_toks = usage.completion_tokens.unwrap_or(0);
                                let _ = event_tx.send(AppEvent::AgentUsage {
                                    prompt_tokens: prompt_toks,
                                    completion_tokens: comp_toks,
                                    exact_speed: None,
                                });
                            }

                            for choice in chunk.choices {
                                let emitted = reasoning_bracket.on_delta(
                                    choice.delta.reasoning_content.as_deref(),
                                    choice.delta.content.as_deref(),
                                );
                                if !emitted.is_empty() {
                                    let _ = event_tx.send(AppEvent::AgentChunk(emitted));
                                }
                                if let Some(ref reason) = choice.finish_reason {
                                    if reason == "length" {
                                        let _ = event_tx.send(AppEvent::AgentChunk(
                                            "\n\n[⚠️ Réponse interrompue : limite de tokens atteinte. Tapez 'continue' pour la suite.]".to_string(),
                                        ));
                                    }
                                    // NOTE: do NOT return here — the final usage chunk
                                    // (`stream_options.include_usage`) may still be emitted after
                                    // `finish_reason` and must be captured before `[DONE]`.
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
                    let err_msg =
                        "Délai d'inactivité de 25s dépassé sur le flux du modèle (timeout SSE)."
                            .to_string();
                    let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
                    anyhow::bail!(err_msg);
                }
            }
        }

        let _ = event_tx.send(AppEvent::AgentDone);
        Ok(())
    }
}

#[cfg(test)]
mod reasoning_bracket_tests {
    use super::ReasoningBracket;

    #[test]
    fn wraps_reasoning_then_content_in_think_block() {
        let mut b = ReasoningBracket::default();
        assert_eq!(
            b.on_delta(Some("Je réfléchis"), None),
            "<think>Je réfléchis"
        );
        assert_eq!(b.on_delta(Some(" profondément."), None), " profondément.");
        assert_eq!(b.on_delta(None, Some("Voici la")), "</think>Voici la");
        assert_eq!(b.on_delta(None, Some(" réponse.")), " réponse.");
    }

    #[test]
    fn same_delta_reasoning_and_content() {
        let mut b = ReasoningBracket::default();
        assert_eq!(
            b.on_delta(Some("pense"), Some("réponds")),
            "<think>pense</think>réponds"
        );
    }

    #[test]
    fn empty_reasoning_chunks_open_nothing_and_stray_reasoning_dropped() {
        let mut b = ReasoningBracket::default();
        assert_eq!(b.on_delta(Some(""), None), "");
        assert_eq!(b.on_delta(None, Some("direct")), "direct");
        assert_eq!(b.on_delta(Some("stray after answer"), Some("")), "");
        assert_eq!(b.on_delta(None, Some("suite")), "suite");
    }
}

#[cfg(test)]
mod vision_payload_tests {
    use crate::app::MessageAttachment;
    use super::{ContentPart, ImageUrl, Message};

    #[test]
    fn plain_text_message_is_single_text_part() {
        let msg = Message {
            role: "user",
            content: Some(vec![ContentPart::Text {
                r#type: "text",
                text: "hello",
            }]),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "user");
        assert_eq!(json["content"][0]["type"], "text");
        assert_eq!(json["content"][0]["text"], "hello");
    }

    #[test]
    fn image_message_emits_image_url_data_uri_part() {
        let att = MessageAttachment {
            mime_type: "image/png".to_string(),
            data_base64: "iVBORw0KGgo".to_string(),
        };
        let uri = att.data_uri();
        assert_eq!(uri, "data:image/png;base64,iVBORw0KGgo");

        let msg = Message {
            role: "user",
            content: Some(vec![
                ContentPart::Text { r#type: "text", text: "" },
                ContentPart::ImageUrl {
                    r#type: "image_url",
                    image_url: ImageUrl { url: &uri },
                },
            ]),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["content"][1]["type"], "image_url");
        assert_eq!(json["content"][1]["image_url"]["url"], uri);
    }

    #[test]
    fn data_uri_roundtrip_of_attachment() {
        let att = MessageAttachment {
            mime_type: "image/jpeg".to_string(),
            data_base64: "abc123".to_string(),
        };
        assert_eq!(att.data_uri(), "data:image/jpeg;base64,abc123");
        // Ensure `data:` URI form is what the MessageAttachment API promises.
        assert!(att.data_uri().starts_with("data:"));
    }

    #[test]
    fn message_attachment_serializes_verbatim() {
        let att = MessageAttachment {
            mime_type: "image/png".to_string(),
            data_base64: "iVBORw0KGgo".to_string(),
        };
        let json = serde_json::to_value(&att).unwrap();
        assert_eq!(json["mime_type"], "image/png");
        assert_eq!(json["data_base64"], "iVBORw0KGgo");
    }
}
