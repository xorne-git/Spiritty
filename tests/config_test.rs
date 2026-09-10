use spiritty::config::{Config, ProviderType};

#[test]
fn test_config_defaults_and_providers() {
    let config = Config::default();
    assert_eq!(config.default_provider, ProviderType::Ollama);

    // Verify all 8 providers are present in default config
    assert!(config.providers.contains_key("ollama"));
    assert!(config.providers.contains_key("lmstudio"));
    assert!(config.providers.contains_key("gemini"));
    assert!(config.providers.contains_key("grok"));
    assert!(config.providers.contains_key("deepseek"));
    assert!(config.providers.contains_key("zai"));
    assert!(config.providers.contains_key("openai"));
    assert!(config.providers.contains_key("anthropic"));

    // Verify LM Studio default URL and model
    let lmstudio = config.providers.get("lmstudio").unwrap();
    assert_eq!(
        lmstudio.base_url.as_deref(),
        Some("http://localhost:1234/v1")
    );

    // Verify Grok default URL
    let grok = config.providers.get("grok").unwrap();
    assert_eq!(grok.base_url.as_deref(), Some("https://api.x.ai/v1"));

    // Verify Z.ai default URL and model
    let zai = config.providers.get("zai").unwrap();
    assert_eq!(
        zai.base_url.as_deref(),
        Some("https://api.z.ai/api/paas/v4")
    );
    assert_eq!(zai.model, "glm-5.3");
}

#[test]
fn test_toml_serialization() {
    let config = Config::default();
    let toml_str = toml::to_string_pretty(&config).expect("Failed to serialize config");
    assert!(toml_str.contains("default_provider = \"ollama\""));
    assert!(toml_str.contains("[providers.lmstudio]"));
    assert!(toml_str.contains("[providers.grok]"));

    let deserialized: Config = toml::from_str(&toml_str).expect("Failed to deserialize config");
    assert_eq!(deserialized.default_provider, ProviderType::Ollama);
}

#[test]
fn test_api_key_resolution() {
    unsafe {
        std::env::set_var("TEST_SPIRITTY_KEY", "secret-test-key-12345");
    }

    // Direct key
    let resolved_direct = Config::resolve_api_key(Some("my-custom-key"));
    assert_eq!(resolved_direct, Some("my-custom-key".to_string()));

    // Env prefixed
    let resolved_env = Config::resolve_api_key(Some("ENV:TEST_SPIRITTY_KEY"));
    assert_eq!(resolved_env, Some("secret-test-key-12345".to_string()));

    // Provider fallback
    unsafe {
        std::env::set_var("DEEPSEEK_API_KEY", "sk-deepseek-test");
        std::env::set_var("ZAI_API_KEY", "sk-zai-test");
    }
    let resolved_fallback = Config::resolve_api_key_for_provider(ProviderType::DeepSeek, None);
    assert_eq!(resolved_fallback, Some("sk-deepseek-test".to_string()));

    let resolved_zai = Config::resolve_api_key_for_provider(ProviderType::Zai, None);
    assert_eq!(resolved_zai, Some("sk-zai-test".to_string()));
}

#[test]
fn test_custom_system_prompt() {
    use spiritty::agent::prompt::build_system_prompt;
    use spiritty::i18n::Language;
    use spiritty::system::SystemContext;

    let sys = SystemContext {
        os_name: "Linux".to_string(),
        distro: "CachyOS".to_string(),
        kernel: "6.12".to_string(),
        shell: "/usr/bin/fish".to_string(),
        terminal_emulator: "Ghostty v1.3.1".to_string(),
        package_managers: vec!["pacman".to_string()],
        desktop_env: Some("niri".to_string()),
        active_session: spiritty::system::ActiveSession::Local {
            foreground_process: None,
        },
        active_remote_profile: None,
        current_dir: Some("~/Projets/Spiritty".to_string()),
        git_branch: Some("main".to_string()),
    };

    let config = Config {
        system_prompt: Some("Custom Spiritty Prompt with {sys_info}".to_string()),
        ..Config::default()
    };

    let prompt = build_system_prompt(Language::Fr, &sys, &config);
    assert!(prompt.starts_with("Custom Spiritty Prompt with"));
    assert!(prompt.contains("CachyOS"));
    assert!(prompt.contains("pacman"));
}

#[test]
fn test_auto_approve_deserialization() {
    use spiritty::config::AutoApproveLevel;

    // Test default
    let cfg_default: Config = toml::from_str("").unwrap();
    assert_eq!(cfg_default.auto_approve, AutoApproveLevel::Safe);

    // Test string variants
    let cfg_safe: Config = toml::from_str("auto_approve = \"safe\"").unwrap();
    assert_eq!(cfg_safe.auto_approve, AutoApproveLevel::Safe);

    let cfg_sudo: Config = toml::from_str("auto_approve = \"sudo\"").unwrap();
    assert_eq!(cfg_sudo.auto_approve, AutoApproveLevel::Sudo);

    let cfg_yolo: Config = toml::from_str("auto_approve = \"yolo\"").unwrap();
    assert_eq!(cfg_yolo.auto_approve, AutoApproveLevel::Yolo);

    let cfg_off: Config = toml::from_str("auto_approve = \"off\"").unwrap();
    assert_eq!(cfg_off.auto_approve, AutoApproveLevel::Off);

    // Test bool backwards compatibility
    let cfg_true: Config = toml::from_str("auto_approve = true").unwrap();
    assert_eq!(cfg_true.auto_approve, AutoApproveLevel::Safe);

    let cfg_false: Config = toml::from_str("auto_approve = false").unwrap();
    assert_eq!(cfg_false.auto_approve, AutoApproveLevel::Off);
}

#[test]
fn test_split_ratio_and_theme_persistence() {
    use spiritty::ui::theme::ThemeId;

    // Test default
    let cfg_default = Config::default();
    assert_eq!(cfg_default.get_split_ratio(), 50);
    assert_eq!(cfg_default.get_theme(), "spiritty_dark");

    // Test custom toml serialization / deserialization
    let toml_data = r#"
split_ratio = 42
theme = "tokyo_night"
"#;
    let cfg: Config = toml::from_str(toml_data).unwrap();
    assert_eq!(cfg.get_split_ratio(), 42);
    assert_eq!(cfg.get_theme(), "tokyo_night");
    assert_eq!(
        ThemeId::parse_or_default(&cfg.get_theme()),
        ThemeId::TokyoNight
    );

    // Test clamp on split_ratio
    let clamped_low: Config = toml::from_str("split_ratio = 5").unwrap();
    assert_eq!(clamped_low.get_split_ratio(), 15);

    let clamped_high: Config = toml::from_str("split_ratio = 99").unwrap();
    assert_eq!(clamped_high.get_split_ratio(), 85);
}

#[test]
fn test_gemini_models_and_config_merging() {
    // 1. Verify ProviderType::Gemini defaults
    assert_eq!(ProviderType::Gemini.default_model(), "gemini-3.8-flash");
    assert!(ProviderType::Gemini
        .popular_models()
        .contains(&"gemini-3.8-flash"));

    // 2. Simulate existing config that only had gemini-3.7-flash
    let legacy_toml = r#"
default_provider = "gemini"

[providers.gemini]
model = "gemini-3.7-flash"
models = ["gemini-3.7-flash", "gemini-2.0-flash"]
"#;
    let mut cfg: Config = toml::from_str(legacy_toml).unwrap();

    // Replicate the load-time popular models synchronization
    for p in ProviderType::all() {
        let key = p.key_str();
        let default_models: Vec<String> =
            p.popular_models().iter().map(|s| s.to_string()).collect();
        if let Some(p_cfg) = cfg.providers.get_mut(key) {
            for dm in default_models.iter().rev() {
                if !p_cfg.models.contains(dm) {
                    p_cfg.models.insert(0, dm.clone());
                }
            }
        }
    }

    let gemini_cfg = cfg.providers.get("gemini").unwrap();
    assert!(gemini_cfg.models.contains(&"gemini-3.8-flash".to_string()));
    assert_eq!(gemini_cfg.models[0], "gemini-3.8-flash");
    assert_eq!(gemini_cfg.model, "gemini-3.7-flash"); // User's chosen active model is not clobbered
}

#[test]
fn test_reasoning_effort_configuration_and_serialization() {
    use spiritty::config::ReasoningEffort;

    // 1. Next & Prev cycle
    let eff = ReasoningEffort::Default;
    assert_eq!(eff.next(), ReasoningEffort::Off);
    assert_eq!(eff.next().next(), ReasoningEffort::Low);
    assert_eq!(eff.next().next().next(), ReasoningEffort::Medium);
    assert_eq!(eff.next().next().next().next(), ReasoningEffort::High);
    assert_eq!(
        eff.next().next().next().next().next(),
        ReasoningEffort::Default
    );

    assert_eq!(eff.prev(), ReasoningEffort::High);
    assert_eq!(eff.prev().prev(), ReasoningEffort::Medium);
    assert_eq!(eff.prev().prev().prev(), ReasoningEffort::Low);
    assert_eq!(eff.prev().prev().prev().prev(), ReasoningEffort::Off);
    assert_eq!(
        eff.prev().prev().prev().prev().prev(),
        ReasoningEffort::Default
    );

    // 2. Deserialization from TOML
    let toml_with_effort = r#"
[providers.gemini]
model = "gemini-3.8-flash"
models = ["gemini-3.8-flash"]
reasoning_effort = "medium"
"#;
    let cfg: Config = toml::from_str(toml_with_effort).unwrap();
    let gemini = cfg.providers.get("gemini").unwrap();
    assert_eq!(gemini.reasoning_effort, ReasoningEffort::Medium);

    // 3. Absent defaults to Default
    let toml_absent = r#"
[providers.gemini]
model = "gemini-3.8-flash"
models = ["gemini-3.8-flash"]
"#;
    let cfg_absent: Config = toml::from_str(toml_absent).unwrap();
    let gemini_absent = cfg_absent.providers.get("gemini").unwrap();
    assert_eq!(gemini_absent.reasoning_effort, ReasoningEffort::Default);

    // 4. Serialization skips Default but includes non-default
    let serialized_absent = toml::to_string(&cfg_absent).unwrap();
    assert!(!serialized_absent.contains("reasoning_effort"));

    let serialized_with_effort = toml::to_string(&cfg).unwrap();
    assert!(serialized_with_effort.contains("reasoning_effort = \"medium\""));
}
