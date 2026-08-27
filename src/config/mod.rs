use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs, path::PathBuf};

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

    pub fn from_key(s: &str) -> Option<ProviderType> {
        let trimmed = s.to_lowercase().trim().to_string();
        if trimmed.contains("ollama") {
            Some(ProviderType::Ollama)
        } else if trimmed.contains("lmstudio")
            || trimmed.contains("lm_studio")
            || trimmed.contains("lm-studio")
        {
            Some(ProviderType::LmStudio)
        } else if trimmed.contains("gemini") || trimmed.contains("google") {
            Some(ProviderType::Gemini)
        } else if trimmed.contains("grok") || trimmed.contains("xai") {
            Some(ProviderType::Grok)
        } else if trimmed.contains("deepseek") {
            Some(ProviderType::DeepSeek)
        } else if trimmed.contains("openai")
            || trimmed.contains("chatgpt")
            || trimmed.contains("gpt")
        {
            Some(ProviderType::OpenAI)
        } else if trimmed.contains("anthropic") || trimmed.contains("claude") {
            Some(ProviderType::Anthropic)
        } else {
            None
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
            ProviderType::Grok => &["grok-4.6", "grok-latest", "grok-2-latest", "grok-beta"],
            ProviderType::DeepSeek => &["deepseek-v4-pro", "deepseek-v4-flash"],
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    #[serde(default = "default_mcp_enabled")]
    pub enabled: bool,
}

fn default_mcp_enabled() -> bool {
    true
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
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub pricing: HashMap<String, crate::pricing::ModelPricing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split_ratio: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_dir: Option<String>,
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
            mcp_servers: HashMap::new(),
            pricing: HashMap::new(),
            system_prompt: None,
            system_prompt_file: None,
            split_ratio: Some(50),
            theme: Some("spiritty_dark".to_string()),
            export_dir: None,
        }
    }
}

impl Config {
    pub fn get_split_ratio(&self) -> u16 {
        self.split_ratio.unwrap_or(50).clamp(15, 85)
    }

    pub fn get_theme(&self) -> String {
        self.theme
            .clone()
            .unwrap_or_else(|| "spiritty_dark".to_string())
    }

    pub fn get_language(&self) -> Language {
        if let Some(ref lang_str) = self.language {
            if let Some(lang) = Language::from_code(lang_str) {
                return lang;
            }
        }
        Language::detect_system()
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir =
            dirs::config_dir().context("Could not find standard config directory (~/.config)")?;
        Ok(config_dir.join("spiritty").join("config.toml"))
    }

    pub fn prompt_path() -> Result<PathBuf> {
        let config_dir =
            dirs::config_dir().context("Could not find standard config directory (~/.config)")?;
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
                            let default_models: Vec<String> =
                                p.popular_models().iter().map(|s| s.to_string()).collect();

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
                                    p_cfg.models.retain(|m| {
                                        m != "deepseek-chat"
                                            && m != "deepseek-reasoner"
                                            && m != "deepseek-v4-pr"
                                    });
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
                let default_content = r#"You are Spiritty, an expert AI terminal companion for Linux/macOS, DevOps, and system administration.
You are assisting the user who is actively working in a live terminal on the right split screen.

{sys_info}

WORKFLOW & DUAL EXECUTION PARADIGM:

1. DIRECT TOOL EXECUTION (`tool:run_command`) — FOR INVESTIGATIONS, DIAGNOSTICS & USER-APPROVED ACTIONS:
Whenever you need to inspect the system, check files/backups/directories, query Docker containers, inspect systemd services, read logs, OR whenever the user confirms or gives approval (e.g., "oui", "vas-y", "fais-le", "ok", "go", "continue", "lance", "vérifie", "le backup est fini"):
DO NOT just output a proposal card. Instead, DIRECTLY EXECUTE the command with:
```tool:run_command
your_command_to_execute
```
Spiritty executes this command live in the terminal (auto-approving safe inspections or requesting approval according to the security policy), captures the output, and returns the result to you in the next turn so you can analyze it immediately.

2. COMMAND PROPOSALS & ACTION CARDS (`bash` code blocks) — FOR USER-DRIVEN COMMANDS & SCRIPTS:
Whenever you suggest a script, multi-step plan, configuration edit, or command for the user to review and run at their own pace:
Format each executable command inside a standard markdown bash code block:
```bash
your_command_here
```
Spiritty parses this block into an interactive action card with safety badges (🟢 Safe / 🟡 Sudo / 🔴 Risky) and an `Alt + 1..9` shortcut button.

3. WEB SEARCH (`tool:web_search`):
If you need online manuals, package repositories, or external documentation:
```tool:web_search
search keywords
```

INTERACTION EXAMPLES:

Example 1 — User asks to inspect or verify something:
User: "ok le backup est fini, vérifie que tout est bon"
Assistant:
Je vérifie l'intégrité et la taille des fichiers de sauvegarde :
```tool:run_command
ls -lah /home/xorne/filerise-backup-* 2>/dev/null && du -sh /home/xorne/filerise-backup-*/* 2>/dev/null
```

Example 2 — User approves a proposed action:
User: "oui vas y"
Assistant:
Je récupère la configuration et les variables d'environnement du conteneur :
```tool:run_command
docker inspect filerise --format '{{range .Config.Env}}{{println .}}{{end}}'
```

IMPORTANT RULES:
- Always be structured, concise, factual, and direct.
- When the user asks you to check, diagnose, or says "oui / vas-y / continue / fais-le", use ```tool:run_command``` so the user doesn't have to manually press Alt+1.
- All commands execute in a standard Bash/POSIX subshell. All proposed commands must strictly be valid Bash/POSIX syntax. Never use Fish-specific syntax (no `set -l`, no `begin...end`, no `(cmd)` for evaluation), even if the user's interactive shell is Fish.
- NEVER put angle-bracket placeholders like `<PID>`, `<service>`, `<package>`, or `<path>` inside commands. Always provide concrete, usable commands.
- ALL shell commands must ALWAYS be enclosed inside triple backticks (`tool:run_command` or `bash`). NEVER write bare shell commands in raw text without code blocks.
- When root or elevated privileges are required, use `sudo <command>` directly. NEVER use `sudo -n` (the terminal is live and interactive, allowing the user to enter their sudo password directly).
- CRITICAL: NEVER announce that you are running or checking something (e.g. "Je lance...", "Vérifions...", "Voici la commande...") without IMMEDIATELY outputting the ```tool:run_command``` or ```bash``` code block in the exact same response! Every announced action MUST have its executable block right below.
- Propose direct, clean, human-readable commands (e.g. `cat ...`, `ls -la`, `curl ...`, `docker ps`). NEVER wrap your proposed commands in `bash -c '...'` and NEVER create temporary execution scripts in `/tmp` unless the user explicitly asks for a script file.
- COMMUNICATION LANGUAGE: Always communicate, explain, and respond to the user in French, in a clear, concise, structured, and factual tone.
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

        let toml_str =
            toml::to_string_pretty(self).context("Failed to serialize config to TOML")?;
        fs::write(&path, toml_str)
            .with_context(|| format!("Failed to write config file {:?}", path))?;
        restrict_file_permissions(&path);
        Ok(())
    }

    pub fn get_active_provider_config(&self) -> ProviderConfig {
        let key = self.default_provider.key_str();
        self.providers
            .get(key)
            .cloned()
            .unwrap_or_else(|| ProviderConfig {
                model: self.default_provider.default_model().to_string(),
                models: self
                    .default_provider
                    .popular_models()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                base_url: self
                    .default_provider
                    .default_base_url()
                    .map(|s| s.to_string()),
                api_key: self
                    .default_provider
                    .default_env_var()
                    .map(|env| format!("ENV:{}", env)),
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
        provider
            .popular_models()
            .iter()
            .map(|s| s.to_string())
            .collect()
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

    pub fn resolve_api_key_for_provider(
        provider: ProviderType,
        raw_key: Option<&str>,
    ) -> Option<String> {
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

        // Consult the process-level probe cache first: hitting the login shell from the
        // UI thread on every provider creation used to block keydown handling for tens
        // of milliseconds (and indefinitely under a slow shell init).
        if let Some(cached) = shell_env_cache()
            .lock()
            .ok()
            .and_then(|c| c.get(name).cloned())
        {
            return cached;
        }

        // Probe default login shell (captures fish / zsh / bash export variables)
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut probed: Option<Option<String>> = None;
        if let Ok(output) = std::process::Command::new(&shell)
            .args(["-l", "-c", &format!("echo -n \"${}\"", name)])
            .output()
        {
            if output.status.success() {
                let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let found = (!val.is_empty()).then_some(val);
                if let Some(v) = &found {
                    // SAFETY: single-threaded startup probe, before TUI tasks read this var.
                    unsafe {
                        env::set_var(name, v);
                    }
                }
                probed = Some(found);
            }
        }
        let resolved = probed.unwrap_or(None);
        if let Ok(mut cache) = shell_env_cache().lock() {
            cache.insert(name.to_string(), resolved.clone());
        }
        resolved
    }
}

/// Process-level memoization of login-shell environment probes, so each variable is only
/// ever probed once per run regardless of how often a provider is re-created.
fn shell_env_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, Option<String>>>
{
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Option<String>>>,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Restricts an app-owned state file to owner-only access. Config files may hold API keys,
/// session archives hold conversation history — all belong in private storage.
#[cfg(unix)]
pub fn restrict_file_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(err) = fs::set_permissions(path, fs::Permissions::from_mode(0o600)) {
        tracing::warn!("Failed to restrict permissions on {:?}: {}", path, err);
    }
}

#[cfg(not(unix))]
pub fn restrict_file_permissions(_path: &std::path::Path) {}
