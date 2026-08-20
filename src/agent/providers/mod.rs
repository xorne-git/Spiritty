pub mod anthropic;
pub mod gemini;
pub mod ollama;
pub mod openai;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::ChatMessage,
    config::{Config, ProviderType},
    event::AppEvent,
};
use anthropic::AnthropicProvider;
use gemini::GeminiProvider;
use ollama::OllamaProvider;
use openai::OpenAiCompatibleProvider;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn stream_chat(
        &self,
        messages: &[ChatMessage],
        system_prompt: &str,
        event_tx: UnboundedSender<AppEvent>,
    ) -> Result<()>;
}

/// Creates the active LLM provider instance from the configuration.
pub fn create_provider(config: &Config) -> Box<dyn LlmProvider> {
    let provider_type = config.default_provider;
    let provider_cfg = config.get_active_provider_config();
    let api_key = Config::resolve_api_key_for_provider(provider_type, provider_cfg.api_key.as_deref());

    match provider_type {
        ProviderType::Ollama => Box::new(OllamaProvider::new(
            provider_cfg.base_url,
            provider_cfg.model,
        )),
        ProviderType::LmStudio => Box::new(OpenAiCompatibleProvider::new(
            "LM Studio",
            provider_cfg.base_url.or_else(|| Some("http://localhost:1234/v1".to_string())),
            provider_cfg.model,
            api_key,
        )),
        ProviderType::Gemini => Box::new(GeminiProvider::new(
            provider_cfg.base_url,
            provider_cfg.model,
            api_key.unwrap_or_default(),
        )),
        ProviderType::Grok => Box::new(OpenAiCompatibleProvider::new(
            "Grok (xAI)",
            provider_cfg.base_url.or_else(|| Some("https://api.x.ai/v1".to_string())),
            provider_cfg.model,
            api_key,
        )),
        ProviderType::DeepSeek => Box::new(OpenAiCompatibleProvider::new(
            "DeepSeek",
            provider_cfg.base_url.or_else(|| Some("https://api.deepseek.com/v1".to_string())),
            provider_cfg.model,
            api_key,
        )),
        ProviderType::OpenAI => Box::new(OpenAiCompatibleProvider::new(
            "OpenAI",
            provider_cfg.base_url.or_else(|| Some("https://api.openai.com/v1".to_string())),
            provider_cfg.model,
            api_key,
        )),
        ProviderType::Anthropic => Box::new(AnthropicProvider::new(
            provider_cfg.base_url,
            provider_cfg.model,
            api_key.unwrap_or_default(),
        )),
    }
}

use std::time::Duration;

#[derive(serde::Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaTagItem>,
}

#[derive(serde::Deserialize)]
struct OllamaTagItem {
    name: String,
}

#[derive(serde::Deserialize)]
struct OpenAiModelsResponse {
    #[serde(default)]
    data: Vec<OpenAiModelItem>,
}

#[derive(serde::Deserialize)]
struct OpenAiModelItem {
    id: String,
}

/// Asynchronously probes available models from local (Ollama, LM Studio) or remote provider endpoints.
pub async fn fetch_available_models(
    provider: ProviderType,
    custom_base_url: Option<&str>,
    api_key: Option<&str>,
) -> Vec<String> {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    match provider {
        ProviderType::Ollama => {
            let base = custom_base_url
                .unwrap_or("http://localhost:11434")
                .trim_end_matches('/')
                .trim_end_matches("/v1");

            // 1. Try Ollama Native tags endpoint (/api/tags)
            let tags_url = format!("{}/api/tags", base);
            if let Ok(res) = client.get(&tags_url).send().await {
                if let Ok(tags) = res.json::<OllamaTagsResponse>().await {
                    let names: Vec<String> = tags.models.into_iter().map(|m| m.name).collect();
                    if !names.is_empty() {
                        return names;
                    }
                }
            }

            // 2. Try Ollama OpenAI-compatible /v1/models endpoint
            let models_url = format!("{}/v1/models", base);
            if let Ok(res) = client.get(&models_url).send().await {
                if let Ok(models) = res.json::<OpenAiModelsResponse>().await {
                    let names: Vec<String> = models.data.into_iter().map(|m| m.id).collect();
                    if !names.is_empty() {
                        return names;
                    }
                }
            }
            Vec::new()
        }
        ProviderType::LmStudio => {
            let base = custom_base_url
                .unwrap_or("http://localhost:1234/v1")
                .trim_end_matches('/');

            let models_url = if base.ends_with("/v1") {
                format!("{}/models", base)
            } else {
                format!("{}/v1/models", base)
            };

            if let Ok(res) = client.get(&models_url).send().await {
                if let Ok(models) = res.json::<OpenAiModelsResponse>().await {
                    let names: Vec<String> = models
                        .data
                        .into_iter()
                        .map(|m| m.id)
                        .filter(|id| !id.contains("embedding") && !id.contains("embed-text"))
                        .collect();
                    if !names.is_empty() {
                        return names;
                    }
                }
            }
            Vec::new()
        }
        ProviderType::OpenAI | ProviderType::Grok | ProviderType::DeepSeek => {
            if let Some(base) = custom_base_url.or_else(|| provider.default_base_url()) {
                let base_trimmed = base.trim_end_matches('/');
                let models_url = if base_trimmed.ends_with("/v1") {
                    format!("{}/models", base_trimmed)
                } else {
                    format!("{}/v1/models", base_trimmed)
                };

                let mut req = client.get(&models_url);
                if let Some(key) = api_key {
                    if !key.is_empty() {
                        req = req.header("Authorization", format!("Bearer {}", key));
                    }
                }
                if let Ok(res) = req.send().await {
                    if let Ok(models) = res.json::<OpenAiModelsResponse>().await {
                        let names: Vec<String> = models
                            .data
                            .into_iter()
                            .map(|m| m.id)
                            .filter(|id| !id.contains("embedding") && !id.contains("whisper") && !id.contains("dall-e") && !id.contains("tts"))
                            .collect();
                        if !names.is_empty() {
                            return names;
                        }
                    }
                }
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}
