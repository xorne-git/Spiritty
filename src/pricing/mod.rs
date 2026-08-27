use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

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
        Self {
            prompt: 0.0,
            completion: 0.0,
        }
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
        m.insert(
            "deepseek-reasoner".to_string(),
            ModelPricing::new(0.55, 2.19),
        );
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
        m.insert(
            "claude-3-5-sonnet".to_string(),
            ModelPricing::new(3.00, 15.00),
        );
        m.insert(
            "claude-3-7-sonnet".to_string(),
            ModelPricing::new(3.00, 15.00),
        );
        m.insert(
            "claude-3-5-haiku".to_string(),
            ModelPricing::new(0.80, 4.00),
        );
        m.insert("claude-3-haiku".to_string(), ModelPricing::new(0.25, 1.25));
        m.insert("claude-3-opus".to_string(), ModelPricing::new(15.00, 75.00));

        // Google Gemini
        m.insert(
            "gemini-2.0-flash".to_string(),
            ModelPricing::new(0.10, 0.40),
        );
        m.insert(
            "gemini-1.5-flash".to_string(),
            ModelPricing::new(0.075, 0.30),
        );
        m.insert("gemini-1.5-pro".to_string(), ModelPricing::new(1.25, 5.00));

        // Grok / xAI
        m.insert("grok-2".to_string(), ModelPricing::new(2.00, 10.00));
        m.insert("grok-2-mini".to_string(), ModelPricing::new(0.50, 2.00));
        m.insert("grok-beta".to_string(), ModelPricing::new(5.00, 15.00));

        // Mistral AI
        m.insert("mistral-large".to_string(), ModelPricing::new(2.00, 6.00));
        m.insert("mistral-small".to_string(), ModelPricing::new(0.20, 0.60));
        m.insert("codestral".to_string(), ModelPricing::new(0.30, 0.90));
        m.insert(
            "open-mistral-nemo".to_string(),
            ModelPricing::new(0.15, 0.15),
        );

        // Z.ai (GLM) - https://docs.z.ai/guides/overview/pricing
        m.insert("glm-5.3".to_string(), ModelPricing::new(1.40, 4.40));
        m.insert("glm-5.3-flash".to_string(), ModelPricing::new(0.075, 0.25));
        m.insert("glm-5.2".to_string(), ModelPricing::new(1.40, 4.40));
        m.insert("glm-5.1".to_string(), ModelPricing::new(1.40, 4.40));
        m.insert("glm-5".to_string(), ModelPricing::new(1.00, 3.20));
        m.insert("glm-5-turbo".to_string(), ModelPricing::new(1.20, 4.00));
        m.insert("glm-4.7".to_string(), ModelPricing::new(0.60, 2.20));
        m.insert("glm-4.7-flash".to_string(), ModelPricing::free());
        m.insert("glm-4.7-flashx".to_string(), ModelPricing::new(0.07, 0.40));
        m.insert("glm-4.6".to_string(), ModelPricing::new(0.60, 2.20));
        m.insert("glm-4.5".to_string(), ModelPricing::new(0.60, 2.20));
        m.insert("glm-4.5-x".to_string(), ModelPricing::new(2.20, 8.90));
        m.insert("glm-4.5-air".to_string(), ModelPricing::new(0.20, 1.10));
        m.insert("glm-4.5-airx".to_string(), ModelPricing::new(1.10, 4.50));
        m.insert("glm-4.5-flash".to_string(), ModelPricing::free());
        m.insert("glm-4-flash".to_string(), ModelPricing::free());
        m.insert("glm-4-plus".to_string(), ModelPricing::new(0.60, 2.20));
        m.insert("glm-4-air".to_string(), ModelPricing::new(0.20, 1.10));
        m.insert("glm-4-long".to_string(), ModelPricing::new(0.60, 2.20));
        m.insert("glm-4".to_string(), ModelPricing::new(0.60, 2.20));
        m.insert("glm-5v-turbo".to_string(), ModelPricing::new(1.20, 4.00));
        m.insert("glm-4.6v".to_string(), ModelPricing::new(0.30, 0.90));
        m.insert("glm-ocr".to_string(), ModelPricing::new(0.03, 0.03));
        m.insert("glm-4.6v-flashx".to_string(), ModelPricing::new(0.04, 0.40));
        m.insert("glm-4.5v".to_string(), ModelPricing::new(0.60, 1.80));
        m.insert("glm-4.6v-flash".to_string(), ModelPricing::free());

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
                if let Ok(cached_map) =
                    serde_json::from_str::<HashMap<String, ModelPricing>>(&content)
                {
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
        crate::config::restrict_file_permissions(&cache_file);
        Ok(())
    }

    /// Fetches online pricing from multiple endpoints (union merge) and updates the local
    /// registry & cache. Both the primary and the mirror listings are consulted so entries for
    /// every provider are checked, not just whichever source answers first.
    pub async fn fetch_online_and_update(&mut self, client: &reqwest::Client) -> Result<usize> {
        let urls = [Self::ONLINE_PRICING_URL, Self::FALLBACK_PRICING_URL];
        let mut merged: HashMap<String, ModelPricing> = HashMap::new();
        let mut fetched_any = false;

        for url in urls {
            let req = client
                .get(url)
                .timeout(std::time::Duration::from_secs(6))
                .header("User-Agent", "Spiritty-Client");

            if let Ok(resp) = req.send().await {
                if resp.status().is_success() {
                    if let Ok(map) = resp.json::<HashMap<String, ModelPricing>>().await {
                        fetched_any = true;
                        for (k, v) in map {
                            merged.entry(k.to_lowercase()).or_insert(v);
                        }
                    }
                }
            }
        }

        anyhow::ensure!(
            fetched_any && !merged.is_empty(),
            "Impossible de récupérer les tarifs en ligne depuis les dépôts officiels"
        );

        let count = merged.len();
        for (k, v) in merged {
            self.models.insert(k.to_lowercase(), v);
        }

        let _ = self.save_cache();
        Ok(count)
    }

    /// Finds matching pricing for a given provider and model name at the current time
    /// (accounting for dynamic off-peak discounts). Returns `None` when no tariff is actually
    /// configured for this specific model — callers must then hide the cost rather than guess.
    pub fn get_pricing(&self, provider: &str, model: &str) -> Option<ModelPricing> {
        self.get_pricing_at(provider, model, Some(chrono::Utc::now()))
    }

    /// Finds matching pricing with an explicit optional timestamp (for deterministic testing &
    /// historical calculation). Resolution order (strict, never invents a rate):
    /// 1. Local providers are always free.
    /// 2. Explicit `[pricing]` overrides from config.toml (user-defined, trusted).
    /// 3. EXACT match in the models table (online cache or built-ins).
    /// 4. Exact match after stripping a dated-snapshot suffix (e.g. `-20241022`, `-0324`).
    /// 5. Documented official DeepSeek family rates (the only provider whose lineup pricing
    ///    is published per family: chat/coder/v3 → low rate, reasoner/r1 → reasoning rate).
    ///
    /// Anything else returns `None`.
    pub fn get_pricing_at(
        &self,
        provider: &str,
        model: &str,
        now: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Option<ModelPricing> {
        let prov_clean = provider.trim().to_lowercase();
        let model_clean = model.trim().to_lowercase();

        // An empty model name has no corresponding tariff.
        if model_clean.is_empty() {
            return None;
        }

        // 1. Check if local provider (always free)
        if prov_clean.contains("ollama")
            || prov_clean.contains("lmstudio")
            || prov_clean.contains("local")
        {
            return Some(ModelPricing::free());
        }

        // 2. Check user explicit overrides first (verbatim, no time adjustments)
        if let Some(price) = self.overrides.get(&model_clean) {
            return Some(*price);
        }
        for (k, price) in &self.overrides {
            if !k.is_empty() && (model_clean.contains(k.as_str()) || k.contains(&model_clean)) {
                return Some(*price);
            }
        }

        // 3. Exact match in models table — a similar-looking name may denote a different
        //    tier with a very different price (e.g. flash vs flash-lite, o1 vs o1-mini),
        //    so substring matching is deliberately NOT used here.
        let mut base_price = self.models.get(&model_clean).copied();

        // 4. Exact match ignoring a dated snapshot suffix (`-20241022`, `-0618`, …):
        //    providers publish identical rates for dated variants of the same model.
        if base_price.is_none() {
            base_price = self.models_snapshot_trimmed_lookup(&model_clean);
        }

        // 5. Official documented DeepSeek family pricing (per line-up, not per snapshot).
        let is_deepseek_family =
            prov_clean.contains("deepseek") || model_clean.contains("deepseek");
        if base_price.is_none() && is_deepseek_family {
            if model_clean.contains("reasoner") || model_clean.contains("r1") {
                base_price = Some(ModelPricing::new(0.55, 2.19));
            } else {
                base_price = Some(ModelPricing::new(0.14, 0.28));
            }
        }

        let mut base_price = base_price?;

        // 6. Dynamic DeepSeek Peak / Off-Peak adjustment (50% discount during off-peak and weekends)
        if is_deepseek_family {
            if let Some(dt) = now {
                if is_deepseek_offpeak(dt) {
                    base_price =
                        ModelPricing::new(base_price.prompt * 0.5, base_price.completion * 0.5);
                }
            }
        }

        Some(base_price)
    }

    /// Looks up the model ignoring a trailing dated snapshot segment (e.g.
    /// `claude-3-5-sonnet-20241022` → `claude-3-5-sonnet`). Only strips tokens that cannot
    /// denote a distinct pricing tier (pure dates/version numbers), never words like `mini`
    /// or `lite` that identify cheaper variants.
    fn models_snapshot_trimmed_lookup(&self, model_clean: &str) -> Option<ModelPricing> {
        let mut candidate = model_clean.to_string();
        for _ in 0..2 {
            let idx = candidate.rfind('-')?;
            let tail = &candidate[idx + 1..];
            let is_date_like =
                !tail.is_empty() && (tail.bytes().all(|b| b.is_ascii_digit())) && tail.len() >= 3;
            if !is_date_like {
                break;
            }
            candidate.truncate(idx);
            if let Some(price) = self.models.get(&candidate).copied() {
                return Some(price);
            }
        }
        None
    }
}

/// Returns true if the given UTC timestamp falls in DeepSeek's Off-Peak billing window.
/// Official DeepSeek rules (api-docs.deepseek.com/quick_start/pricing):
/// - Weekends: All day Saturday and Sunday are Off-Peak (-50%).
/// - Weekdays (Monday to Friday):
///     - Peak hours: 01:00 – 04:00 UTC (09:00–12:00 CST) and 06:00 – 10:00 UTC (14:00–18:00 CST).
///     - Off-Peak hours: All other times (00:00–01:00 UTC, 04:00–06:00 UTC, 10:00–24:00 UTC).
pub fn is_deepseek_offpeak(now_utc: chrono::DateTime<chrono::Utc>) -> bool {
    use chrono::{Datelike, Timelike, Weekday};

    let weekday = now_utc.weekday();

    // All weekend (Saturday & Sunday full day) is Off-Peak
    if weekday == Weekday::Sat || weekday == Weekday::Sun {
        return true;
    }

    let hour = now_utc.hour();
    let min = now_utc.minute();
    let mins_utc = hour * 60 + min;

    // Peak window 1: 01:00 – 04:00 UTC (60 to 240 mins)
    let is_peak_morning = (60..240).contains(&mins_utc);
    // Peak window 2: 06:00 – 10:00 UTC (360 to 600 mins)
    let is_peak_afternoon = (360..600).contains(&mins_utc);

    // Everything outside peak windows is Off-Peak
    !(is_peak_morning || is_peak_afternoon)
}
