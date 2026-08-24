use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
};

/// Pricing for a model per 1,000,000 tokens (in USD)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ModelPricing {
    /// Cost per 1 million input (prompt) tokens in USD
    pub prompt: f64,
    /// Cost per 1 million output (completion) tokens in USD
    pub completion: f64,
}

impl ModelPricing {
    pub const fn new(prompt: f64, completion: f64) -> Self {
        Self { prompt, completion }
    }

    pub const fn free() -> Self {
        Self { prompt: 0.0, completion: 0.0 }
    }

    /// Computes total cost for given prompt and completion token counts
    pub fn calculate_cost(&self, prompt_tokens: usize, completion_tokens: usize) -> f64 {
        let p_cost = (prompt_tokens as f64 / 1_000_000.0) * self.prompt;
        let c_cost = (completion_tokens as f64 / 1_000_000.0) * self.completion;
        p_cost + c_cost
    }
}

/// Registry of LLM pricing supporting default rates, remote cache, and user overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingRegistry {
    /// Cached or fetched model rates
    pub models: HashMap<String, ModelPricing>,
    /// User custom overrides from config.toml
    #[serde(skip)]
    pub overrides: HashMap<String, ModelPricing>,
}

impl Default for PricingRegistry {
    fn default() -> Self {
        Self {
            models: Self::builtin_defaults(),
            overrides: HashMap::new(),
        }
    }
}

impl PricingRegistry {
    /// Official primary online pricing URL
    pub const ONLINE_PRICING_URL: &'static str =
        "https://raw.githubusercontent.com/xorne-git/Spiritty/main/assets/pricing.json";

    /// Fallback mirror pricing URL
    pub const FALLBACK_PRICING_URL: &'static str =
        "https://raw.githubusercontent.com/xorne-git/Spiritty/master/assets/pricing.json";

    /// Standard built-in default pricing table for major LLM providers
    pub fn builtin_defaults() -> HashMap<String, ModelPricing> {
        let mut m = HashMap::new();

        // Local Models (Free)
        m.insert("ollama".to_string(), ModelPricing::free());
        m.insert("lmstudio".to_string(), ModelPricing::free());
        m.insert("local".to_string(), ModelPricing::free());

        // DeepSeek
        m.insert("deepseek-chat".to_string(), ModelPricing::new(0.14, 0.28));
        m.insert("deepseek-v3".to_string(), ModelPricing::new(0.14, 0.28));
        m.insert("deepseek-coder".to_string(), ModelPricing::new(0.14, 0.28));
        m.insert("deepseek-reasoner".to_string(), ModelPricing::new(0.55, 2.19));
        m.insert("deepseek-r1".to_string(), ModelPricing::new(0.55, 2.19));

        // OpenAI
        m.insert("gpt-4o".to_string(), ModelPricing::new(2.50, 10.00));
        m.insert("gpt-4o-mini".to_string(), ModelPricing::new(0.15, 0.60));
        m.insert("gpt-4-turbo".to_string(), ModelPricing::new(10.00, 30.00));
        m.insert("gpt-3.5-turbo".to_string(), ModelPricing::new(0.50, 1.50));
        m.insert("o1".to_string(), ModelPricing::new(15.00, 60.00));
        m.insert("o1-mini".to_string(), ModelPricing::new(3.00, 12.00));
        m.insert("o3-mini".to_string(), ModelPricing::new(1.10, 4.40));

        // Anthropic Claude
        m.insert("claude-3-5-sonnet".to_string(), ModelPricing::new(3.00, 15.00));
        m.insert("claude-3-7-sonnet".to_string(), ModelPricing::new(3.00, 15.00));
        m.insert("claude-3-5-haiku".to_string(), ModelPricing::new(0.80, 4.00));
        m.insert("claude-3-haiku".to_string(), ModelPricing::new(0.25, 1.25));
        m.insert("claude-3-opus".to_string(), ModelPricing::new(15.00, 75.00));

        // Google Gemini
        m.insert("gemini-2.0-flash".to_string(), ModelPricing::new(0.10, 0.40));
        m.insert("gemini-1.5-flash".to_string(), ModelPricing::new(0.075, 0.30));
        m.insert("gemini-1.5-pro".to_string(), ModelPricing::new(1.25, 5.00));

        // Grok / xAI
        m.insert("grok-2".to_string(), ModelPricing::new(2.00, 10.00));
        m.insert("grok-2-mini".to_string(), ModelPricing::new(0.50, 2.00));
        m.insert("grok-beta".to_string(), ModelPricing::new(5.00, 15.00));

        // Mistral AI
        m.insert("mistral-large".to_string(), ModelPricing::new(2.00, 6.00));
        m.insert("mistral-small".to_string(), ModelPricing::new(0.20, 0.60));
        m.insert("codestral".to_string(), ModelPricing::new(0.30, 0.90));
        m.insert("open-mistral-nemo".to_string(), ModelPricing::new(0.15, 0.15));

        m
    }

    /// Path to cached pricing JSON file
    pub fn cache_path() -> PathBuf {
        let base_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("spiritty");
        base_dir.join("pricing.json")
    }

    /// Loads pricing registry from built-in defaults, merged with cached file and config overrides
    pub fn load_with_overrides(overrides: HashMap<String, ModelPricing>) -> Self {
        let mut registry = Self::default();

        // 1. Try to load saved pricing.json cache
        let cache_file = Self::cache_path();
        if cache_file.exists() {
            if let Ok(content) = fs::read_to_string(&cache_file) {
                if let Ok(cached_map) = serde_json::from_str::<HashMap<String, ModelPricing>>(&content) {
                    for (k, v) in cached_map {
                        registry.models.insert(k.to_lowercase(), v);
                    }
                }
            }
        }

        // 2. Set user overrides from config.toml
        registry.overrides = overrides;
        registry
    }

    /// Saves the current model pricing cache to ~/.config/spiritty/pricing.json
    pub fn save_cache(&self) -> Result<()> {
        let cache_file = Self::cache_path();
        if let Some(parent) = cache_file.parent() {
            fs::create_dir_all(parent).context("Failed to create config dir for pricing cache")?;
        }
        let json = serde_json::to_string_pretty(&self.models)
            .context("Failed to serialize pricing cache")?;
        fs::write(&cache_file, json).context("Failed to write pricing.json cache")?;
        Ok(())
    }

    /// Fetches online pricing from the community/official endpoint and updates local registry & cache
    pub async fn fetch_online_and_update(&mut self, client: &reqwest::Client) -> Result<usize> {
        let urls = [Self::ONLINE_PRICING_URL, Self::FALLBACK_PRICING_URL];
        let mut fetched_map = None;

        for url in urls {
            let req = client
                .get(url)
                .timeout(std::time::Duration::from_secs(6))
                .header("User-Agent", "Spiritty-Client");

            if let Ok(resp) = req.send().await {
                if resp.status().is_success() {
                    if let Ok(map) = resp.json::<HashMap<String, ModelPricing>>().await {
                        fetched_map = Some(map);
                        break;
                    }
                }
            }
        }

        let map = fetched_map.context("Impossible de récupérer les tarifs en ligne depuis les dépôts officiels")?;
        let count = map.len();

        for (k, v) in map {
            self.models.insert(k.to_lowercase(), v);
        }

        let _ = self.save_cache();
        Ok(count)
    }

    /// Finds matching pricing for a given provider and model name
    pub fn get_pricing(&self, provider: &str, model: &str) -> ModelPricing {
        let prov_clean = provider.trim().to_lowercase();
        let model_clean = model.trim().to_lowercase();

        // An empty model name must not fall through to the substring match below
        // (which would return an arbitrary, non-deterministic entry).
        if model_clean.is_empty() {
            return ModelPricing::free();
        }

        // 1. Check if local provider (always free)
        if prov_clean.contains("ollama") || prov_clean.contains("lmstudio") || prov_clean.contains("local") {
            return ModelPricing::free();
        }

        // 2. Check user explicit overrides first
        if let Some(price) = self.overrides.get(&model_clean) {
            return *price;
        }
        for (k, price) in &self.overrides {
            if model_clean.contains(k) {
                return *price;
            }
        }

        // 3. Exact match in models table
        if let Some(price) = self.models.get(&model_clean) {
            return *price;
        }

        // 4. Prefix / Substring match in models table
        for (key, price) in &self.models {
            if model_clean.contains(key) || key.contains(&model_clean) {
                return *price;
            }
        }

        // 5. Fallback heuristics by provider family
        match prov_clean.as_str() {
            p if p.contains("deepseek") => {
                if model_clean.contains("reasoner") || model_clean.contains("r1") {
                    ModelPricing::new(0.55, 2.19)
                } else {
                    ModelPricing::new(0.14, 0.28)
                }
            }
            p if p.contains("openai") || p.contains("chatgpt") => {
                if model_clean.contains("mini") {
                    ModelPricing::new(0.15, 0.60)
                } else if model_clean.contains("o1") || model_clean.contains("o3") {
                    ModelPricing::new(15.00, 60.00)
                } else {
                    ModelPricing::new(2.50, 10.00)
                }
            }
            p if p.contains("anthropic") || p.contains("claude") => {
                if model_clean.contains("haiku") {
                    ModelPricing::new(0.80, 4.00)
                } else if model_clean.contains("opus") {
                    ModelPricing::new(15.00, 75.00)
                } else {
                    ModelPricing::new(3.00, 15.00)
                }
            }
            p if p.contains("gemini") || p.contains("google") => {
                if model_clean.contains("pro") {
                    ModelPricing::new(1.25, 5.00)
                } else {
                    ModelPricing::new(0.075, 0.30)
                }
            }
            p if p.contains("grok") || p.contains("xai") => ModelPricing::new(2.00, 10.00),
            p if p.contains("mistral") => ModelPricing::new(2.00, 6.00),
            _ => ModelPricing::new(0.50, 1.50),
        }
    }
}
