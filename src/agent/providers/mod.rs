pub mod anthropic;
pub mod gemini;
pub mod ollama;
pub mod openai;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

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
        cancel: CancellationToken,
    ) -> Result<()>;
}

/// Creates the active LLM provider instance from the configuration.
pub fn create_provider(config: &Config) -> Box<dyn LlmProvider> {
    let provider_type = config.default_provider;
    let provider_cfg = config.get_active_provider_config();
    let api_key =
        Config::resolve_api_key_for_provider(provider_type, provider_cfg.api_key.as_deref());
    let reasoning_effort = provider_cfg.reasoning_effort;

    match provider_type {
        ProviderType::Ollama => Box::new(OllamaProvider::new(
            provider_cfg.base_url,
            provider_cfg.model,
        )),
        ProviderType::LmStudio => Box::new(OpenAiCompatibleProvider::new(
            "LM Studio",
            provider_cfg
                .base_url
                .or_else(|| Some("http://localhost:1234/v1".to_string())),
            provider_cfg.model,
            api_key,
            reasoning_effort,
        )),
        ProviderType::Gemini => Box::new(GeminiProvider::new(
            provider_cfg.base_url,
            provider_cfg.model,
            api_key.unwrap_or_default(),
            reasoning_effort,
        )),
        ProviderType::Grok => Box::new(OpenAiCompatibleProvider::new(
            "Grok (xAI)",
            provider_cfg
                .base_url
                .or_else(|| Some("https://api.x.ai/v1".to_string())),
            provider_cfg.model,
            api_key,
            reasoning_effort,
        )),
        ProviderType::DeepSeek => Box::new(OpenAiCompatibleProvider::new(
            "DeepSeek",
            provider_cfg
                .base_url
                .or_else(|| Some("https://api.deepseek.com/v1".to_string())),
            provider_cfg.model,
            api_key,
            reasoning_effort,
        )),
        ProviderType::Zai => Box::new(OpenAiCompatibleProvider::new(
            "Z.ai",
            provider_cfg
                .base_url
                .or_else(|| Some("https://api.z.ai/api/paas/v4".to_string())),
            provider_cfg.model,
            api_key,
            reasoning_effort,
        )),
        ProviderType::OpenAI => Box::new(OpenAiCompatibleProvider::new(
            "OpenAI",
            provider_cfg
                .base_url
                .or_else(|| Some("https://api.openai.com/v1".to_string())),
            provider_cfg.model,
            api_key,
            reasoning_effort,
        )),
        ProviderType::Anthropic => Box::new(AnthropicProvider::new(
            provider_cfg.base_url,
            provider_cfg.model,
            api_key.unwrap_or_default(),
            reasoning_effort,
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

#[derive(serde::Deserialize)]
struct GeminiModelsResponse {
    #[serde(default)]
    models: Vec<GeminiModelItem>,
}

#[derive(serde::Deserialize)]
struct GeminiModelItem {
    name: String,
    #[serde(rename = "supportedGenerationMethods", default)]
    supported_generation_methods: Vec<String>,
}

#[derive(serde::Deserialize)]
struct AnthropicModelsResponse {
    #[serde(default)]
    data: Vec<AnthropicModelItem>,
}

#[derive(serde::Deserialize)]
struct AnthropicModelItem {
    id: String,
}

/// Asynchronously probes available models from local (Ollama, LM Studio) or remote provider endpoints.
pub async fn fetch_available_models(
    provider: ProviderType,
    custom_base_url: Option<&str>,
    api_key: Option<&str>,
) -> Result<Vec<String>, String> {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => return Err(format!("Client error: {}", e)),
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
                        return Ok(names);
                    }
                }
            }

            // 2. Try Ollama OpenAI-compatible /v1/models endpoint
            let models_url = format!("{}/v1/models", base);
            match client.get(&models_url).send().await {
                Ok(res) => {
                    if let Ok(models) = res.json::<OpenAiModelsResponse>().await {
                        let names: Vec<String> = models.data.into_iter().map(|m| m.id).collect();
                        if !names.is_empty() {
                            return Ok(names);
                        }
                    }
                    Err("Aucun modèle trouvé dans Ollama".to_string())
                }
                Err(e) => Err(format!("Connexion impossible à Ollama ({})", e)),
            }
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

            match client.get(&models_url).send().await {
                Ok(res) => {
                    if let Ok(models) = res.json::<OpenAiModelsResponse>().await {
                        let names: Vec<String> = models
                            .data
                            .into_iter()
                            .map(|m| m.id)
                            .filter(|id| !id.contains("embedding") && !id.contains("embed-text"))
                            .collect();
                        if !names.is_empty() {
                            return Ok(names);
                        }
                    }
                    Err("Aucun modèle chargé dans LM Studio".to_string())
                }
                Err(e) => Err(format!("Connexion impossible à LM Studio ({})", e)),
            }
        }
        ProviderType::Gemini => {
            let key = match api_key {
                Some(k) if !k.trim().is_empty() => k.trim(),
                _ => return Err("missing_key".to_string()),
            };
            let base = custom_base_url
                .unwrap_or("https://generativelanguage.googleapis.com")
                .trim_end_matches('/');
            let url = format!("{}/v1beta/models?key={}", base, key);

            match client.get(&url).send().await {
                Ok(res) => {
                    if !res.status().is_success() {
                        return Err(format!("HTTP {}", res.status()));
                    }
                    if let Ok(body) = res.json::<GeminiModelsResponse>().await {
                        let mut names: Vec<String> = body
                            .models
                            .into_iter()
                            .filter(|m| {
                                m.supported_generation_methods
                                    .iter()
                                    .any(|method| method == "generateContent")
                            })
                            .map(|m| {
                                m.name
                                    .strip_prefix("models/")
                                    .unwrap_or(&m.name)
                                    .to_string()
                            })
                            .filter(|name| {
                                !name.contains("embedding")
                                    && !name.contains("aqa")
                                    && !name.contains("learnlm")
                            })
                            .collect();

                        // Sort newest first
                        names.sort_by(|a, b| {
                            let score = |s: &str| -> i32 {
                                if s.contains("3.8") {
                                    10
                                } else if s.contains("3.7") {
                                    9
                                } else if s.contains("3.1") {
                                    8
                                } else if s.contains("3.6") {
                                    7
                                } else if s.contains("2.5") {
                                    6
                                } else if s.contains("2.0") {
                                    5
                                } else if s.contains("1.5") {
                                    4
                                } else {
                                    1
                                }
                            };
                            score(b).cmp(&score(a)).then_with(|| a.cmp(b))
                        });

                        if !names.is_empty() {
                            return Ok(names);
                        }
                    }
                    Err("Aucun modèle Gemini trouvé".to_string())
                }
                Err(e) => Err(format!("Erreur réseau Google Gemini ({})", e)),
            }
        }
        ProviderType::Anthropic => {
            let key = match api_key {
                Some(k) if !k.trim().is_empty() => k.trim(),
                _ => return Err("missing_key".to_string()),
            };
            let base = custom_base_url
                .unwrap_or("https://api.anthropic.com")
                .trim_end_matches('/');
            let url = format!("{}/v1/models", base);

            match client
                .get(&url)
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .send()
                .await
            {
                Ok(res) => {
                    if !res.status().is_success() {
                        return Err(format!("HTTP {}", res.status()));
                    }
                    if let Ok(body) = res.json::<AnthropicModelsResponse>().await {
                        let mut names: Vec<String> = body.data.into_iter().map(|m| m.id).collect();
                        names.sort_by(|a, b| {
                            let score = |s: &str| -> i32 {
                                if s.contains("sonnet") {
                                    5
                                } else if s.contains("opus") {
                                    4
                                } else if s.contains("haiku") {
                                    3
                                } else {
                                    1
                                }
                            };
                            score(b).cmp(&score(a)).then_with(|| b.cmp(a))
                        });
                        if !names.is_empty() {
                            return Ok(names);
                        }
                    }
                    Err("Aucun modèle Anthropic trouvé".to_string())
                }
                Err(e) => Err(format!("Erreur réseau Anthropic ({})", e)),
            }
        }
        ProviderType::OpenAI | ProviderType::Grok | ProviderType::DeepSeek | ProviderType::Zai => {
            let key = match api_key {
                Some(k) if !k.trim().is_empty() => k.trim(),
                _ => return Err("missing_key".to_string()),
            };
            if let Some(base) = custom_base_url.or_else(|| provider.default_base_url()) {
                let base_trimmed = base.trim_end_matches('/');
                let models_url = if base_trimmed.ends_with("/v1") || base_trimmed.ends_with("/v4") {
                    format!("{}/models", base_trimmed)
                } else {
                    format!("{}/v1/models", base_trimmed)
                };

                match client
                    .get(&models_url)
                    .header("Authorization", format!("Bearer {}", key))
                    .send()
                    .await
                {
                    Ok(res) => {
                        if !res.status().is_success() {
                            return Err(format!("HTTP {}", res.status()));
                        }
                        if let Ok(models) = res.json::<OpenAiModelsResponse>().await {
                            let names: Vec<String> = models
                                .data
                                .into_iter()
                                .map(|m| m.id)
                                .filter(|id| {
                                    !id.contains("embedding")
                                        && !id.contains("whisper")
                                        && !id.contains("dall-e")
                                        && !id.contains("tts")
                                        && !id.contains("babbage")
                                        && !id.contains("davinci")
                                })
                                .collect();
                            if !names.is_empty() {
                                return Ok(names);
                            }
                        }
                        Err("Aucun modèle trouvé".to_string())
                    }
                    Err(e) => Err(format!("Erreur réseau ({})", e)),
                }
            } else {
                Err("URL de base manquante".to_string())
            }
        }
    }
}
