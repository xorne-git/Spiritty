use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, fs,
    path::PathBuf,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    #[default]
    Ollama,
    #[serde(rename = "lmstudio")]
    LmStudio,
    Gemini,
    Grok,
    DeepSeek,
    OpenAI,
    Anthropic,
}

impl ProviderType {
    pub fn all() -> &'static [ProviderType] {
        &[
            ProviderType::Ollama,
            ProviderType::LmStudio,
            ProviderType::Gemini,
            ProviderType::Grok,
            ProviderType::DeepSeek,
            ProviderType::OpenAI,
            ProviderType::Anthropic,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ProviderType::Ollama => "Ollama (Local)",
            ProviderType::LmStudio => "LM Studio (Local)",
            ProviderType::Gemini => "Google Gemini",
            ProviderType::Grok => "Grok (xAI)",
            ProviderType::DeepSeek => "DeepSeek",
            ProviderType::OpenAI => "OpenAI",
            ProviderType::Anthropic => "Anthropic Claude",
        }
    }

    pub fn key_str(&self) -> &'static str {
        match self {
            ProviderType::Ollama => "ollama",
            ProviderType::LmStudio => "lmstudio",
            ProviderType::Gemini => "gemini",
            ProviderType::Grok => "grok",
            ProviderType::DeepSeek => "deepseek",
            ProviderType::OpenAI => "openai",
            ProviderType::Anthropic => "anthropic",
        }
    }

    pub fn default_model(&self) -> &'static str {
        match self {
            ProviderType::Ollama => "qwen2.5-coder:latest",
            ProviderType::LmStudio => "local-model",
            ProviderType::Gemini => "gemini-3.7-flash",
            ProviderType::Grok => "grok-4.6",
            ProviderType::DeepSeek => "deepseek-v4-pro",
            ProviderType::OpenAI => "gpt-5.6-sol",
            ProviderType::Anthropic => "claude-sonnet-5",
        }
    }

    pub fn popular_models(&self) -> &'static [&'static str] {
        match self {
            ProviderType::Ollama => &[
                "qwen2.5-coder:latest",
                "qwen2.5-coder:7b",
                "qwen2.5-coder:14b",
                "qwen2.5-coder:32b",
                "deepseek-r1:latest",
                "deepseek-r1:7b",
                "deepseek-r1:14b",
                "llama3.3:latest",
                "mistral-small:latest",
                "phi4:latest",
                "starcoder2:latest",
            ],
            ProviderType::LmStudio => &[
                "local-model",
                "qwen2.5-coder-7b-instruct",
                "deepseek-r1-distill-qwen-7b",
                "llama-3.3-70b-instruct",
                "mistral-small-instruct",
            ],
            ProviderType::Gemini => &[
                "gemini-3.7-flash",
                "gemini-3.1-pro",
                "gemini-3.6-flash",
                "gemini-2.5-flash",
                "gemini-2.5-pro",
                "gemini-2.0-flash",
            ],
            ProviderType::Grok => &[
                "grok-4.6",
                "grok-latest",
                "grok-2-latest",
                "grok-beta",
            ],
            ProviderType::DeepSeek => &[
                "deepseek-v4-pro",
                "deepseek-v4-flash",
            ],
            ProviderType::OpenAI => &[
                "gpt-5.6-sol",
                "gpt-5.6-terra",
                "gpt-5.5-pro",
                "gpt-4.5-preview",
                "gpt-4o",
                "gpt-4o-mini",
                "o3-mini",
                "o1",
            ],
            ProviderType::Anthropic => &[
                "claude-opus-5",
                "claude-sonnet-5",
                "claude-3-7-sonnet-20250219",
                "claude-3-5-sonnet-20241022",
                "claude-3-5-haiku-20241022",
            ],
        }
    }

    pub fn default_base_url(&self) -> Option<&'static str> {
        match self {
            ProviderType::Ollama => Some("http://localhost:11434"),
            ProviderType::LmStudio => Some("http://localhost:1234/v1"),
            ProviderType::Gemini => None,
            ProviderType::Grok => Some("https://api.x.ai/v1"),
            ProviderType::DeepSeek => Some("https://api.deepseek.com/v1"),
            ProviderType::OpenAI => Some("https://api.openai.com/v1"),
            ProviderType::Anthropic => None,
        }
    }

    pub fn default_env_var(&self) -> Option<&'static str> {
        match self {
            ProviderType::Ollama | ProviderType::LmStudio => None,
            ProviderType::Gemini => Some("GEMINI_API_KEY"),
            ProviderType::Grok => Some("XAI_API_KEY"),
            ProviderType::DeepSeek => Some("DEEPSEEK_API_KEY"),
            ProviderType::OpenAI => Some("OPENAI_API_KEY"),
            ProviderType::Anthropic => Some("ANTHROPIC_API_KEY"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub model: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<usize>,
}

use crate::i18n::Language;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebSearchConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub searxng_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brave_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tavily_api_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AutoApproveLevel {
    Off,
    #[default]
    Safe,
    Sudo,
    Yolo,
}

impl AutoApproveLevel {
    pub fn next(&self) -> Self {
        match self {
            AutoApproveLevel::Safe => AutoApproveLevel::Sudo,
            AutoApproveLevel::Sudo => AutoApproveLevel::Yolo,
            AutoApproveLevel::Yolo => AutoApproveLevel::Off,
            AutoApproveLevel::Off => AutoApproveLevel::Safe,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            AutoApproveLevel::Safe => AutoApproveLevel::Off,
            AutoApproveLevel::Sudo => AutoApproveLevel::Safe,
            AutoApproveLevel::Yolo => AutoApproveLevel::Sudo,
            AutoApproveLevel::Off => AutoApproveLevel::Yolo,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            AutoApproveLevel::Safe => "Safe",
            AutoApproveLevel::Sudo => "Sudo",
            AutoApproveLevel::Yolo => "YOLO",
            AutoApproveLevel::Off => "Off",
        }
    }

    pub fn description(&self, lang: crate::i18n::Language) -> &'static str {
        match (self, lang) {
            (AutoApproveLevel::Safe, crate::i18n::Language::Fr) => "Lecture seule auto",
            (AutoApproveLevel::Safe, crate::i18n::Language::En) => "Auto read-only",
            (AutoApproveLevel::Sudo, crate::i18n::Language::Fr) => "Standard (prompts sudo)",
            (AutoApproveLevel::Sudo, crate::i18n::Language::En) => "Standard (prompts sudo)",
            (AutoApproveLevel::Yolo, crate::i18n::Language::Fr) => "Auto-pilote total (YOLO)",
            (AutoApproveLevel::Yolo, crate::i18n::Language::En) => "Full auto-pilot (YOLO)",
            (AutoApproveLevel::Off, crate::i18n::Language::Fr) => "Validation manuelle",
            (AutoApproveLevel::Off, crate::i18n::Language::En) => "Manual validation",
        }
    }
}

impl<'de> Deserialize<'de> for AutoApproveLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AutoApproveVisitor;

        impl<'de> serde::de::Visitor<'de> for AutoApproveVisitor {
            type Value = AutoApproveLevel;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a boolean or a string: 'off', 'safe', 'sudo', 'yolo'")
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v {
                    Ok(AutoApproveLevel::Yolo)
                } else {
                    Ok(AutoApproveLevel::Off)
                }
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v.to_lowercase().as_str() {
                    "off" | "false" | "none" | "disabled" => Ok(AutoApproveLevel::Off),
                    "safe" | "read_only" | "readonly" => Ok(AutoApproveLevel::Safe),
                    "sudo" | "standard" | "write" => Ok(AutoApproveLevel::Sudo),
                    "yolo" | "all" | "true" | "auto" => Ok(AutoApproveLevel::Yolo),
                    _ => Ok(AutoApproveLevel::Safe),
                }
            }
        }

        deserializer.deserialize_any(AutoApproveVisitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default)]
    pub auto_approve: AutoApproveLevel,
    #[serde(default)]
    pub default_provider: ProviderType,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub web_search: WebSearchConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt_file: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut providers = HashMap::new();

        for p in ProviderType::all() {
            let api_key = p.default_env_var().map(|env| format!("ENV:{}", env));
            let models: Vec<String> = p.popular_models().iter().map(|s| s.to_string()).collect();
            providers.insert(
                p.key_str().to_string(),
                ProviderConfig {
                    model: p.default_model().to_string(),
                    models,
                    base_url: p.default_base_url().map(|u| u.to_string()),
                    api_key,
                    context_window: None,
                },
            );
        }

        Self {
            language: None,
            auto_approve: AutoApproveLevel::Safe,
            default_provider: ProviderType::Ollama,
            providers,
            web_search: WebSearchConfig::default(),
            system_prompt: None,
            system_prompt_file: None,
        }
    }
}

impl Config {
    pub fn get_language(&self) -> Language {
        if let Some(ref lang_str) = self.language {
            if let Some(lang) = Language::from_code(lang_str) {
                return lang;
            }
        }
        Language::detect_system()
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not find standard config directory (~/.config)")?;
        Ok(config_dir.join("spiritty").join("config.toml"))
    }

    pub fn prompt_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not find standard config directory (~/.config)")?;
        Ok(config_dir.join("spiritty").join("system_prompt.md"))
    }

    pub fn load_shell_env() {
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        if let Ok(output) = std::process::Command::new(&shell)
            .args(["-l", "-c", "env"])
            .output()
        {
            if output.status.success() {
                if let Ok(text) = String::from_utf8(output.stdout) {
                    for line in text.lines() {
                        if let Some((k, v)) = line.split_once('=') {
                            let key = k.trim();
                            let val = v.trim();
                            if !key.is_empty() && env::var(key).is_err() {
                                unsafe {
                                    env::set_var(key, val);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn load() -> Self {
        Self::load_shell_env();
        if let Ok(path) = Self::config_path() {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(mut config) = toml::from_str::<Config>(&content) {
                        // Ensure all providers have valid configs
                        for p in ProviderType::all() {
                            let key = p.key_str();
                            let default_models: Vec<String> = p.popular_models().iter().map(|s| s.to_string()).collect();

                            if let Some(p_cfg) = config.providers.get_mut(key) {
                                // Fix obsolete or empty model names
                                if p_cfg.model == "deepseek-chat"
                                    || p_cfg.model == "deepseek-reasoner"
                                    || p_cfg.model == "deepseek-v4-pr"
                                    || p_cfg.model.is_empty()
                                {
                                    p_cfg.model = p.default_model().to_string();
                                }

                                // If model list is empty, initialize with defaults
                                if p_cfg.models.is_empty() {
                                    p_cfg.models = default_models;
                                } else {
                                    // Clean up obsolete model names
                                    p_cfg.models.retain(|m| m != "deepseek-chat" && m != "deepseek-reasoner" && m != "deepseek-v4-pr");
                                }
                            } else {
                                let api_key = p.default_env_var().map(|env| format!("ENV:{}", env));
                                config.providers.insert(
                                    key.to_string(),
                                    ProviderConfig {
                                        model: p.default_model().to_string(),
                                        models: default_models,
                                        base_url: p.default_base_url().map(|u| u.to_string()),
                                        api_key,
                                        context_window: None,
                                    },
                                );
                            }
                        }
                        Self::ensure_default_prompt_file();
                        return config;
                    }
                }
            }
        }

        // Generate and persist default config
        let default_config = Self::default();
        let _ = default_config.save();
        Self::ensure_default_prompt_file();
        default_config
    }

    pub fn ensure_default_prompt_file() {
        if let Ok(prompt_file) = Self::prompt_path() {
            if !prompt_file.exists() {
                if let Some(parent) = prompt_file.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let default_content = r#"Vous êtes Spiritty, un assistant IA expert en terminal Linux/macOS, DevOps et administration système.
Vous êtes connecté directement au shell de l'utilisateur.

{sys_info}

FONCTIONNEMENT & OUTILS :

1. INSPECTION SYSTÈME (pour lire des logs, vérifier l'état des services, fichiers, etc.) :
Écrivez UNIQUEMENT le bloc suivant pour que Spiritty exécute la commande et vous renvoie les vraies données :
```tool:run_command
votre_commande_d_inspection
```

2. PROPOSITION DE COMMANDE (pour suggérer une action ou configuration à l'utilisateur) :
Écrivez la commande dans un bloc bash standard :
```bash
votre_commande_proposee
```

3. RECHERCHE WEB :
```tool:web_search
mots cles de recherche
```

EXEMPLES D'INTERACTION :

Exemple 1 — L'utilisateur demande une information ou un diagnostic :
Utilisateur : "Quels services utilisateur tournent actuellement ?"
Assistant :
```tool:run_command
systemctl --user list-units --type=service --state=running
```

Exemple 2 — L'utilisateur demande comment faire une action ou réparer :
Utilisateur : "Comment arrêter le service bluetooth ?"
Assistant :
Vous pouvez arrêter le service Bluetooth avec la commande suivante :
```bash
sudo systemctl stop bluetooth.service
```

RÈGLES IMPORTANTES :
- Ne simulez jamais de faux résultats de commandes. Attendez les vraies données de ```tool:run_command```.
- Ne répétez jamais une inspection déjà faite au tour précédent.
- Répondez toujours en français, de manière concise, structurée et factuelle.
"#;
                let _ = fs::write(&prompt_file, default_content);
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {:?}", parent))?;
        }

        let toml_str = toml::to_string_pretty(self)
            .context("Failed to serialize config to TOML")?;
        fs::write(&path, toml_str)
            .with_context(|| format!("Failed to write config file {:?}", path))?;
        Ok(())
    }

    pub fn get_active_provider_config(&self) -> ProviderConfig {
        let key = self.default_provider.key_str();
        self.providers.get(key).cloned().unwrap_or_else(|| ProviderConfig {
            model: self.default_provider.default_model().to_string(),
            models: self.default_provider.popular_models().iter().map(|s| s.to_string()).collect(),
            base_url: self.default_provider.default_base_url().map(|s| s.to_string()),
            api_key: self.default_provider.default_env_var().map(|env| format!("ENV:{}", env)),
            context_window: None,
        })
    }

    pub fn get_models_for_provider(&self, provider: ProviderType) -> Vec<String> {
        let key = provider.key_str();
        if let Some(p_cfg) = self.providers.get(key) {
            if !p_cfg.models.is_empty() {
                let mut list = p_cfg.models.clone();
                if !p_cfg.model.is_empty() && !list.contains(&p_cfg.model) {
                    list.push(p_cfg.model.clone());
                }
                return list;
            }
        }
        provider.popular_models().iter().map(|s| s.to_string()).collect()
    }

    pub fn resolve_api_key(raw_key: Option<&str>) -> Option<String> {
        let key = raw_key?.trim();
        if key.is_empty() {
            return None;
        }

        if let Some(env_name) = key.strip_prefix("ENV:") {
            Self::get_env_var(env_name.trim())
        } else {
            Some(key.to_string())
        }
    }

    pub fn resolve_api_key_for_provider(provider: ProviderType, raw_key: Option<&str>) -> Option<String> {
        // 1. If explicit key is set in config
        if let Some(key) = raw_key {
            let trimmed = key.trim();
            if !trimmed.is_empty() {
                if let Some(env_name) = trimmed.strip_prefix("ENV:") {
                    if let Some(val) = Self::get_env_var(env_name.trim()) {
                        return Some(val);
                    }
                } else {
                    return Some(trimmed.to_string());
                }
            }
        }

        // 2. Fallback to standard provider default env var
        if let Some(default_env) = provider.default_env_var() {
            if let Some(val) = Self::get_env_var(default_env) {
                return Some(val);
            }
        }

        None
    }

    pub fn get_env_var(name: &str) -> Option<String> {
        // Check current process environment
        if let Ok(val) = env::var(name) {
            let trimmed = val.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }

        // Probe default login shell (captures fish / zsh / bash export variables)
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        if let Ok(output) = std::process::Command::new(&shell)
            .args(["-l", "-c", &format!("echo -n \"${}\"", name)])
            .output()
        {
            if output.status.success() {
                let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !val.is_empty() {
                    unsafe {
                        env::set_var(name, &val);
                    }
                    return Some(val);
                }
            }
        }

        None
    }
}
