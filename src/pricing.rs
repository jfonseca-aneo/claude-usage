use chrono::{DateTime, Utc};

/// USD per million tokens.
#[derive(Debug, Clone, Copy)]
pub struct Rates {
    pub input: f64,
    pub output: f64,
    /// Cache-read rate when it isn't the usual 0.1x input (e.g. Opus 5.5).
    pub cache_read: Option<f64>,
}

const fn rates(input: f64, output: f64) -> Rates {
    Rates { input, output, cache_read: None }
}

impl Rates {
    /// Cache writes cost 1.25x the input rate for a 5-minute TTL entry,
    /// 2x for a 1-hour TTL entry; cache reads cost 0.1x. See Anthropic's
    /// prompt-caching pricing. Some models override the cache-read rate.
    fn cache_write_5m(&self) -> f64 {
        self.input * 1.25
    }
    fn cache_write_1h(&self) -> f64 {
        self.input * 2.0
    }
    fn cache_read(&self) -> f64 {
        self.cache_read.unwrap_or(self.input * 0.1)
    }
}

/// Claude Sonnet 5 introductory pricing applies through this date (inclusive).
const SONNET_5_INTRO_CUTOFF: &str = "2026-08-31";

/// Looks up USD/MTok rates for a model at a given point in time. Returns
/// `None` for models with no known price (new/unreleased models, or ones
/// this table hasn't been updated for) rather than guessing.
pub fn rates_for(model: &str, at: DateTime<Utc>) -> Option<Rates> {
    match model {
        "claude-fable-5-1" | "claude-mythos-5-1" => {
            Some(Rates { cache_read: Some(0.25), ..rates(10.0, 50.0) })
        }
        "claude-fable-5" | "claude-mythos-5" => Some(rates(10.0, 50.0)),
        "claude-opus-5-5" => Some(Rates { cache_read: Some(0.20), ..rates(4.0, 20.0) }),
        "claude-opus-5" => Some(rates(5.0, 25.0)),
        "claude-opus-4-8" | "claude-opus-4-7" | "claude-opus-4-6" => {
            Some(rates(5.0, 25.0))
        }
        "claude-sonnet-5-5" => Some(rates(2.0, 10.0)),
        "claude-sonnet-5" => {
            let cutoff: DateTime<Utc> = format!("{SONNET_5_INTRO_CUTOFF}T23:59:59Z")
                .parse()
                .expect("valid cutoff constant");
            if at <= cutoff {
                Some(rates(2.0, 10.0))
            } else {
                Some(rates(3.0, 15.0))
            }
        }
        "claude-sonnet-4-6" => Some(rates(3.0, 15.0)),
        m if m.starts_with("claude-haiku-4-5") => Some(rates(1.0, 5.0)),
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
