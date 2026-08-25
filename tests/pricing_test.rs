use spiritty::{
    config::Config,
    pricing::{ModelPricing, PricingRegistry},
    session::Session,
};
use std::collections::HashMap;

#[test]
fn test_model_pricing_cost_calculation() {
    let pricing = ModelPricing::new(2.50, 10.00); // e.g. GPT-4o
    // 1,000 prompt tokens = $0.0025, 500 completion tokens = $0.0050 => Total = $0.0075
    let cost = pricing.calculate_cost(1_000, 500);
    assert!((cost - 0.0075).abs() < 1e-6);

    let free = ModelPricing::free();
    assert_eq!(free.calculate_cost(100_000, 50_000), 0.0);
}

#[test]
fn test_pricing_registry_builtin_lookups() {
    let registry = PricingRegistry::default();

    // Local
    assert_eq!(registry.get_pricing("ollama", "qwen2.5-coder"), ModelPricing::free());
    assert_eq!(registry.get_pricing("lmstudio", "deepseek-r1"), ModelPricing::free());

    // DeepSeek (base peak rates)
    let ds_chat = registry.get_pricing_at("deepseek", "deepseek-chat", None);
    assert_eq!(ds_chat, ModelPricing::new(0.14, 0.28));

    let ds_r1 = registry.get_pricing_at("deepseek", "deepseek-reasoner", None);
    assert_eq!(ds_r1, ModelPricing::new(0.55, 2.19));

    // OpenAI
    let gpt4o = registry.get_pricing("openai", "gpt-4o");
    assert_eq!(gpt4o, ModelPricing::new(2.50, 10.00));

    let gpt4o_mini = registry.get_pricing("openai", "gpt-4o-mini");
    assert_eq!(gpt4o_mini, ModelPricing::new(0.15, 0.60));

    // Anthropic
    let sonnet = registry.get_pricing("anthropic", "claude-3-5-sonnet-20241022");
    assert_eq!(sonnet, ModelPricing::new(3.00, 15.00));

    // Gemini
    let gemini_flash = registry.get_pricing("gemini", "gemini-1.5-flash");
    assert_eq!(gemini_flash, ModelPricing::new(0.075, 0.30));

    let gemini_pro = registry.get_pricing("gemini", "gemini-1.5-pro");
    assert_eq!(gemini_pro, ModelPricing::new(1.25, 5.00));
}

#[test]
fn test_pricing_registry_user_overrides_priority() {
    let mut overrides = HashMap::new();
    // User overrides deepseek-chat with a discounted enterprise price
    overrides.insert("deepseek-chat".to_string(), ModelPricing::new(0.05, 0.10));
    // User adds custom proprietary model
    overrides.insert("my-corp-llm".to_string(), ModelPricing::new(1.20, 2.40));

    let registry = PricingRegistry::load_with_overrides(overrides);

    // Overridden price must take precedence over builtin
    let p_ds = registry.get_pricing("deepseek", "deepseek-chat");
    assert_eq!(p_ds, ModelPricing::new(0.05, 0.10));

    // Custom model must be found
    let p_corp = registry.get_pricing("openai", "my-corp-llm-v2");
    assert_eq!(p_corp, ModelPricing::new(1.20, 2.40));

    // Non-overridden model still uses standard default
    let p_gpt = registry.get_pricing("openai", "gpt-4o");
    assert_eq!(p_gpt, ModelPricing::new(2.50, 10.00));
}

#[test]
fn test_pricing_toml_deserialization_in_config() {
    let toml_str = r#"
        default_provider = "deepseek"

        [pricing."deepseek-chat"]
        prompt = 0.10
        completion = 0.20

        [pricing."gpt-4o"]
        prompt = 2.00
        completion = 8.00
    "#;

    let config: Config = toml::from_str(toml_str).expect("Valid TOML with [pricing] section");
    assert_eq!(config.pricing.len(), 2);
    assert_eq!(config.pricing.get("deepseek-chat"), Some(&ModelPricing::new(0.10, 0.20)));
    assert_eq!(config.pricing.get("gpt-4o"), Some(&ModelPricing::new(2.00, 8.00)));

    let registry = PricingRegistry::load_with_overrides(config.pricing);
    assert_eq!(registry.get_pricing("deepseek", "deepseek-chat"), ModelPricing::new(0.10, 0.20));
}

#[test]
fn test_session_cost_with_dynamic_registry() {
    let mut session = Session::new("OpenAI", "gpt-4o");
    session.prompt_tokens = 2_000_000;      // 2M tokens * $2.50 = $5.00
    session.completion_tokens = 1_000_000;  // 1M tokens * $10.00 = $10.00

    let default_cost = session.estimated_cost_usd();
    assert!((default_cost - 15.00).abs() < 1e-6);

    // With custom discount registry
    let mut overrides = HashMap::new();
    overrides.insert("gpt-4o".to_string(), ModelPricing::new(1.00, 4.00));
    let custom_registry = PricingRegistry::load_with_overrides(overrides);

    // 2M * 1.00 + 1M * 4.00 = $6.00
    let discounted_cost = session.estimated_cost_with_pricing(&custom_registry);
    assert!((discounted_cost - 6.00).abs() < 1e-6);
}

#[test]
fn test_deepseek_offpeak_dynamic_pricing() {
    use chrono::{TimeZone, Utc};
    use spiritty::pricing::is_deepseek_offpeak;

    let registry = PricingRegistry::default();

    // 1. Weekday Peak: Tuesday at 02:30 UTC (10:30 CST / 04:30 France) -> standard peak prices
    let tuesday_peak = Utc.with_ymd_and_hms(2026, 8, 25, 2, 30, 0).unwrap();
    assert!(!is_deepseek_offpeak(tuesday_peak));
    let price_peak = registry.get_pricing_at("deepseek", "deepseek-chat", Some(tuesday_peak));
    assert_eq!(price_peak, ModelPricing::new(0.14, 0.28));

    // 2. Weekday Afternoon Off-Peak: Tuesday at 12:30 UTC (20:30 CST / 14:30 France) -> 50% discount
    let tuesday_afternoon = Utc.with_ymd_and_hms(2026, 8, 25, 12, 30, 0).unwrap();
    assert!(is_deepseek_offpeak(tuesday_afternoon));
    let price_afternoon = registry.get_pricing_at("deepseek", "deepseek-chat", Some(tuesday_afternoon));
    assert_eq!(price_afternoon, ModelPricing::new(0.07, 0.14));

    // 3. Weekend (All Day): Sunday at 06:00 UTC (14:00 CST) -> 50% discount
    let sunday_offpeak = Utc.with_ymd_and_hms(2026, 8, 23, 6, 0, 0).unwrap();
    assert!(is_deepseek_offpeak(sunday_offpeak));
    let price_sunday = registry.get_pricing_at("deepseek", "deepseek-reasoner", Some(sunday_offpeak));
    assert_eq!(price_sunday, ModelPricing::new(0.275, 1.095));

    // 4. Saturday (All Day): Saturday at 12:00 UTC (20:00 CST) -> 50% discount
    let saturday_offpeak = Utc.with_ymd_and_hms(2026, 8, 22, 12, 0, 0).unwrap();
    assert!(is_deepseek_offpeak(saturday_offpeak));
    let price_saturday = registry.get_pricing_at("deepseek", "deepseek-chat", Some(saturday_offpeak));
    assert_eq!(price_saturday, ModelPricing::new(0.07, 0.14));
}
