use anyhow::Result;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::{timeout, Duration};
use tokio_util::sync::CancellationToken;

use super::LlmProvider;
use crate::{
    app::{ChatMessage, MessageRole},
    event::AppEvent,
};

pub struct GeminiProvider {
    base_url: String,
    model: String,
    api_key: String,
    client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(base_url: Option<String>, model: String, api_key: String) -> Self {
        let base_url = base_url
            .unwrap_or_else(|| "https://generativelanguage.googleapis.com".to_string())
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
#[serde(untagged)]
enum GeminiPart<'a> {
    Text { text: &'a str },
    InlineData {
        inline_data: InlineData<'a>,
    },
}

#[derive(Serialize)]
struct InlineData<'a> {
    mime_type: &'a str,
    data: &'a str,
}

#[derive(Serialize)]
struct GeminiContent<'a> {
    role: &'a str,
    parts: Vec<GeminiPart<'a>>,
}

#[derive(Serialize)]
struct GeminiSystemInstruction<'a> {
    parts: Vec<GeminiPart<'a>>,
}

#[derive(Serialize)]
struct GeminiRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction<'a>>,
    contents: Vec<GeminiContent<'a>>,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Deserialize)]
struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: Option<usize>,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: Option<usize>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiCandidateContent>,
}

#[derive(Deserialize)]
struct GeminiCandidateContent {
    parts: Option<Vec<GeminiCandidatePart>>,
}

#[derive(Deserialize)]
struct GeminiCandidatePart {
    text: Option<String>,
    #[serde(default)]
    thought: Option<bool>,
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    async fn stream_chat(
        &self,
        messages: &[ChatMessage],
        system_prompt: &str,
        event_tx: UnboundedSender<AppEvent>,
        cancel: CancellationToken,
    ) -> Result<()> {
        if self.api_key.trim().is_empty() {
            let err =
                "Clé d'API Gemini manquante. Configurez-la avec Ctrl+P ou exportez GEMINI_API_KEY."
                    .to_string();
            let _ = event_tx.send(AppEvent::AgentError(err.clone()));
            anyhow::bail!(err);
        }

        let system_instruction = if !system_prompt.is_empty() {
            Some(GeminiSystemInstruction {
                parts: vec![GeminiPart::Text {
                    text: system_prompt,
                }],
            })
        } else {
            None
        };

        // Owned inline_data payloads per message, aligned with `messages`, so the parts can
        // borrow them while the request body lives. Gemini's `inline_data` is the raw (non
        // data:-URI) base64 body, e.g. `data` = "iVBOR…" with `mime_type` = "image/png".
        let inline_datas: Vec<Vec<(String, String)>> = messages
            .iter()
            .map(|m| {
                m.attachments
                    .iter()
                    .map(|a| (a.mime_type.clone(), a.data_base64.clone()))
                    .collect()
            })
            .collect();

        let mut contents: Vec<GeminiContent> = Vec::new();
        for (idx, msg) in messages.iter().enumerate() {
            if msg.content.is_empty() && msg.attachments.is_empty() {
                // Never send empty turns (the trailing assistant streaming
                // placeholder is dropped upstream, but stay defensive).
                continue;
            }
            let role = match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "model",
                MessageRole::System => "user",
            };
            let mut parts: Vec<GeminiPart> = Vec::new();
            if !msg.content.is_empty() {
                parts.push(GeminiPart::Text { text: &msg.content });
            }
            for (mime, data) in &inline_datas[idx] {
                parts.push(GeminiPart::InlineData {
                    inline_data: InlineData {
                        mime_type: mime,
                        data,
                    },
                });
            }
            if parts.is_empty() {
                continue;
            }
            match contents.last_mut() {
                // Merge consecutive same-role contents (System summaries map to
                // "user" and tool results arrive as back-to-back user turns):
                // keeps the request canonical for the Gemini API.
                Some(last) if last.role == role => {
                    last.parts.push(GeminiPart::Text { text: "\n" });
                    last.parts.extend(parts);
                }
                _ => {
                    contents.push(GeminiContent { role, parts });
                }
            }
        }

        let request_body = GeminiRequest {
            system_instruction,
            contents,
        };

        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
            self.base_url, self.model
        );

        let send_res = timeout(
            Duration::from_secs(45),
            self.client
                .post(&url)
                .header("x-goog-api-key", &self.api_key)
                .json(&request_body)
                .send(),
        )
        .await;

        let response = match send_res {
            Ok(Ok(resp)) => resp,
            Ok(Err(err)) => {
                let err_msg = format!(
                    "Impossible de se connecter à l'API Google Gemini : {}. Vérifiez votre connexion Internet.",
                    err
                );
                let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
                anyhow::bail!(err_msg);
            }
            Err(_) => {
                let err_msg = "Délai d'attente dépassé (timeout 45s) lors de la connexion à Google Gemini API.".to_string();
                let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
                anyhow::bail!(err_msg);
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            let err_msg = format!("Gemini API error (HTTP {}): {}", status, error_text);
            let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
            anyhow::bail!(err_msg);
        }

        let mut event_stream = response.bytes_stream().eventsource();
        let mut prompt_toks = 0usize;
        let mut comp_toks = 0usize;
        let mut bracket = crate::agent::providers::openai::ReasoningBracket::default();

        loop {
            let next_res = tokio::select! {
                _ = cancel.cancelled() => break,
                r = timeout(Duration::from_secs(90), event_stream.next()) => r,
            };

            match next_res {
                Ok(Some(event_res)) => match event_res {
                    Ok(event) => {
                        let data = event.data.trim();
                        if let Ok(parsed) = serde_json::from_str::<GeminiResponse>(data) {
                            // `usageMetadata` is cumulative per stream — keep the latest, emit once at the end.
                            if let Some(usage) = parsed.usage_metadata {
                                prompt_toks = usage.prompt_token_count.unwrap_or(prompt_toks);
                                comp_toks = usage.candidates_token_count.unwrap_or(comp_toks);
                            }

                            if let Some(candidates) = parsed.candidates {
                                for cand in candidates {
                                    if let Some(content) = cand.content {
                                        if let Some(parts) = content.parts {
                                            for part in parts {
                                                if let Some(text) = part.text {
                                                    if !text.is_empty() {
                                                        let is_thought =
                                                            part.thought.unwrap_or(false);
                                                        let folded = if is_thought {
                                                            bracket.on_delta(Some(&text), None)
                                                        } else {
                                                            bracket.on_delta(None, Some(&text))
                                                        };
                                                        if !folded.is_empty() {
                                                            let _ = event_tx.send(
                                                                AppEvent::AgentChunk(folded),
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => {
                        let err_msg = format!("Gemini stream error: {}", err);
                        let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
                        anyhow::bail!(err_msg);
                    }
                },
                Ok(None) => break,
                Err(_) => {
                    let err_msg =
                        "Délai d'inactivité de 90s dépassé sur le flux Gemini (timeout SSE)."
                            .to_string();
                    let _ = event_tx.send(AppEvent::AgentError(err_msg.clone()));
                    anyhow::bail!(err_msg);
                }
            }
        }

        // Flush any trailing thought closure if only reasoning was delivered
        let trailing = bracket.on_delta(None, Some(""));
        if !trailing.is_empty() {
            let _ = event_tx.send(AppEvent::AgentChunk(trailing));
        }

        if prompt_toks > 0 || comp_toks > 0 {
            let _ = event_tx.send(AppEvent::AgentUsage {
                prompt_tokens: prompt_toks,
                completion_tokens: comp_toks,
                exact_speed: None,
            });
        }
        let _ = event_tx.send(AppEvent::AgentDone);
        Ok(())
    }
}
