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

use crate::config::ReasoningEffort;

pub struct GeminiProvider {
    base_url: String,
    model: String,
    api_key: String,
    reasoning_effort: ReasoningEffort,
    client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(
        base_url: Option<String>,
        model: String,
        api_key: String,
        reasoning_effort: ReasoningEffort,
    ) -> Self {
        let base_url = base_url
            .unwrap_or_else(|| "https://generativelanguage.googleapis.com".to_string())
            .trim_end_matches('/')
            .to_string();

        Self {
            base_url,
            model,
            api_key,
            reasoning_effort,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum GeminiPart<'a> {
    Text { text: &'a str },
    InlineData { inline_data: InlineData<'a> },
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
struct GeminiThinkingConfig {
    #[serde(rename = "thinkingBudget", skip_serializing_if = "Option::is_none")]
    thinking_budget: Option<i32>,
    #[serde(rename = "includeThoughts", skip_serializing_if = "Option::is_none")]
    include_thoughts: Option<bool>,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    #[serde(rename = "thinkingConfig", skip_serializing_if = "Option::is_none")]
    thinking_config: Option<GeminiThinkingConfig>,
}

#[derive(Serialize)]
struct GeminiRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction<'a>>,
    contents: Vec<GeminiContent<'a>>,
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
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

        let generation_config = match self.reasoning_effort {
            ReasoningEffort::Default => Some(GeminiGenerationConfig {
                thinking_config: Some(GeminiThinkingConfig {
                    thinking_budget: None,
                    include_thoughts: Some(true),
                }),
            }),
            ReasoningEffort::Off => Some(GeminiGenerationConfig {
                thinking_config: Some(GeminiThinkingConfig {
                    thinking_budget: Some(0),
                    include_thoughts: None,
                }),
            }),
            ReasoningEffort::Low => Some(GeminiGenerationConfig {
                thinking_config: Some(GeminiThinkingConfig {
                    thinking_budget: Some(1024),
                    include_thoughts: Some(true),
                }),
            }),
            ReasoningEffort::Medium => Some(GeminiGenerationConfig {
                thinking_config: Some(GeminiThinkingConfig {
                    thinking_budget: Some(4096),
                    include_thoughts: Some(true),
                }),
            }),
            ReasoningEffort::High => Some(GeminiGenerationConfig {
                thinking_config: Some(GeminiThinkingConfig {
                    thinking_budget: Some(16384),
                    include_thoughts: Some(true),
                }),
            }),
        };

        let request_body = GeminiRequest {
            system_instruction,
            contents,
            generation_config,
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
                                                            let _ = event_tx
                                                                .send(AppEvent::AgentChunk(folded));
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
        if let Some(close_tag) = bracket.finish() {
            let _ = event_tx.send(AppEvent::AgentChunk(close_tag.to_string()));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_thinking_config_serialization() {
        let medium_cfg = GeminiGenerationConfig {
            thinking_config: Some(GeminiThinkingConfig {
                thinking_budget: Some(4096),
                include_thoughts: Some(true),
            }),
        };
        let json = serde_json::to_string(&medium_cfg).expect("serialize medium");
        assert!(json.contains("\"thinkingBudget\":4096"));
        assert!(json.contains("\"includeThoughts\":true"));

        let default_cfg = GeminiGenerationConfig {
            thinking_config: Some(GeminiThinkingConfig {
                thinking_budget: None,
                include_thoughts: Some(true),
            }),
        };
        let json_default = serde_json::to_string(&default_cfg).expect("serialize default");
        assert!(!json_default.contains("thinkingBudget"));
        assert!(json_default.contains("\"includeThoughts\":true"));

        let off_cfg = GeminiGenerationConfig {
            thinking_config: Some(GeminiThinkingConfig {
                thinking_budget: Some(0),
                include_thoughts: None,
            }),
        };
        let json_off = serde_json::to_string(&off_cfg).expect("serialize off");
        assert!(json_off.contains("\"thinkingBudget\":0"));
        assert!(!json_off.contains("includeThoughts"));
    }

    #[test]
    fn test_gemini_candidate_part_thought_deserialization() {
        let raw_thought = r#"{"text": "I am thinking", "thought": true}"#;
        let part: GeminiCandidatePart =
            serde_json::from_str(raw_thought).expect("deserialize thought");
        assert_eq!(part.thought, Some(true));
        assert_eq!(part.text.as_deref(), Some("I am thinking"));

        let raw_content = r#"{"text": "Final answer"}"#;
        let part_content: GeminiCandidatePart =
            serde_json::from_str(raw_content).expect("deserialize content");
        assert_eq!(part_content.thought, None);
        assert_eq!(part_content.text.as_deref(), Some("Final answer"));
    }
}
