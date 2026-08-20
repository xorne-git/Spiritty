use spiritty::config::{Config, ProviderType};

#[test]
fn test_config_defaults_and_providers() {
    let config = Config::default();
    assert_eq!(config.default_provider, ProviderType::Ollama);

    // Verify all 7 providers are present in default config
    assert!(config.providers.contains_key("ollama"));
    assert!(config.providers.contains_key("lmstudio"));
    assert!(config.providers.contains_key("gemini"));
    assert!(config.providers.contains_key("grok"));
    assert!(config.providers.contains_key("deepseek"));
    assert!(config.providers.contains_key("openai"));
    assert!(config.providers.contains_key("anthropic"));

    // Verify LM Studio default URL and model
    let lmstudio = config.providers.get("lmstudio").unwrap();
    assert_eq!(lmstudio.base_url.as_deref(), Some("http://localhost:1234/v1"));

    // Verify Grok default URL
    let grok = config.providers.get("grok").unwrap();
    assert_eq!(grok.base_url.as_deref(), Some("https://api.x.ai/v1"));
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
    }
    let resolved_fallback = Config::resolve_api_key_for_provider(ProviderType::DeepSeek, None);
    assert_eq!(resolved_fallback, Some("sk-deepseek-test".to_string()));
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
    };

    let mut config = Config::default();
    config.system_prompt = Some("Custom Spiritty Prompt with {sys_info}".to_string());

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
    assert_eq!(cfg_true.auto_approve, AutoApproveLevel::Yolo);

    let cfg_false: Config = toml::from_str("auto_approve = false").unwrap();
    assert_eq!(cfg_false.auto_approve, AutoApproveLevel::Off);
}

