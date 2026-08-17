use chrono::{DateTime, Utc};

/// USD per million tokens.
#[derive(Debug, Clone, Copy)]
pub struct Rates {
    pub input: f64,
    pub output: f64,
}

impl Rates {
    /// Cache writes cost 1.25x the input rate for a 5-minute TTL entry,
    /// 2x for a 1-hour TTL entry; cache reads cost 0.1x. See Anthropic's
    /// prompt-caching pricing.
    fn cache_write_5m(&self) -> f64 {
        self.input * 1.25
    }
    fn cache_write_1h(&self) -> f64 {
        self.input * 2.0
    }
    fn cache_read(&self) -> f64 {
        self.input * 0.1
    }
}

/// Claude Sonnet 5 introductory pricing applies through this date (inclusive).
const SONNET_5_INTRO_CUTOFF: &str = "2026-08-31";

/// Looks up USD/MTok rates for a model at a given point in time. Returns
/// `None` for models with no known price (new/unreleased models, or ones
/// this table hasn't been updated for) rather than guessing.
pub fn rates_for(model: &str, at: DateTime<Utc>) -> Option<Rates> {
    match model {
        "claude-fable-5" | "claude-mythos-5" => Some(Rates { input: 10.0, output: 50.0 }),
        "claude-opus-5" => Some(Rates { input: 5.0, output: 25.0 }),
        "claude-opus-4-8" | "claude-opus-4-7" | "claude-opus-4-6" => {
            Some(Rates { input: 5.0, output: 25.0 })
        }
        "claude-sonnet-5" => {
            let cutoff: DateTime<Utc> = format!("{SONNET_5_INTRO_CUTOFF}T23:59:59Z")
                .parse()
                .expect("valid cutoff constant");
            if at <= cutoff {
                Some(Rates { input: 2.0, output: 10.0 })
            } else {
                Some(Rates { input: 3.0, output: 15.0 })
            }
        }
        "claude-sonnet-4-6" => Some(Rates { input: 3.0, output: 15.0 }),
        m if m.starts_with("claude-haiku-4-5") => Some(Rates { input: 1.0, output: 5.0 }),
        _ => None,
    }
}

pub struct UsageForPricing {
    pub uncached_input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_5m_tokens: u64,
    pub cache_creation_1h_tokens: u64,
}

/// Returns `None` if the model has no known price.
pub fn cost_dollars(model: &str, at: DateTime<Utc>, usage: &UsageForPricing) -> Option<f64> {
    let rates = rates_for(model, at)?;
    let dollars = (usage.uncached_input_tokens as f64 * rates.input
        + usage.output_tokens as f64 * rates.output
        + usage.cache_read_input_tokens as f64 * rates.cache_read()
        + usage.cache_creation_5m_tokens as f64 * rates.cache_write_5m()
        + usage.cache_creation_1h_tokens as f64 * rates.cache_write_1h())
        / 1_000_000.0;
    Some(dollars)
}
