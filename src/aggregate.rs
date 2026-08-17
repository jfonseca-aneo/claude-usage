use std::collections::HashMap;

use serde::Serialize;

use crate::models::{CostBucket, UsageBucket};

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct TokenTotals {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl TokenTotals {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }

    fn add(&mut self, other: &TokenTotals) {
        self.input += other.input;
        self.output += other.output;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
    }
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub since_label: String,
    pub until_label: String,
    pub org_name: Option<String>,
    pub total_spend_dollars: f64,
    /// (label, dollars), sorted by spend descending.
    pub spend_by_model: Vec<(String, f64)>,
    /// (date, dollars), chronological.
    pub daily_spend: Vec<(String, f64)>,
    pub token_totals: TokenTotals,
    /// (model, totals), sorted by total tokens descending.
    pub tokens_by_model: Vec<(String, TokenTotals)>,
    /// (project, dollars), sorted by spend descending. Empty unless the data
    /// source has a notion of "project" (e.g. local Claude Code logs).
    #[serde(default)]
    pub spend_by_project: Vec<(String, f64)>,
    /// True when `total_spend_dollars` is a locally-computed estimate rather
    /// than authoritative billing data from the Cost Report API.
    #[serde(default)]
    pub is_estimate: bool,
    /// True when some usage came from a model with no known price, so
    /// `total_spend_dollars` understates actual spend.
    #[serde(default)]
    pub has_unpriced_data: bool,
}

/// Cost amounts are decimal strings denominated in the lowest currency unit
/// (cents for USD), e.g. "123.45" == $1.2345. See the Anthropic Cost Report
/// API reference for the `amount` field.
fn cents_str_to_dollars(amount: &str) -> f64 {
    amount.parse::<f64>().unwrap_or(0.0) / 100.0
}

pub fn build_summary(
    since_label: String,
    until_label: String,
    org_name: Option<String>,
    usage: &[UsageBucket],
    cost: &[CostBucket],
) -> Summary {
    let mut total_spend_dollars = 0.0;
    let mut spend_by_label: HashMap<String, f64> = HashMap::new();
    let mut daily_spend: Vec<(String, f64)> = Vec::new();

    for bucket in cost {
        let mut day_total = 0.0;
        for r in &bucket.results {
            let dollars = cents_str_to_dollars(&r.amount);
            day_total += dollars;
            total_spend_dollars += dollars;
            let label = r
                .model
                .clone()
                .or_else(|| r.description.clone())
                .or_else(|| r.cost_type.clone())
                .unwrap_or_else(|| "Other".to_string());
            *spend_by_label.entry(label).or_insert(0.0) += dollars;
        }
        let date = bucket.starting_at.split('T').next().unwrap_or(&bucket.starting_at);
        daily_spend.push((date.to_string(), day_total));
    }

    let mut spend_by_model: Vec<(String, f64)> = spend_by_label.into_iter().collect();
    spend_by_model.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut token_totals = TokenTotals::default();
    let mut tokens_by_label: HashMap<String, TokenTotals> = HashMap::new();

    for bucket in usage {
        for r in &bucket.results {
            let t = TokenTotals {
                input: r.uncached_input_tokens,
                output: r.output_tokens,
                cache_read: r.cache_read_input_tokens,
                cache_write: r.cache_creation.ephemeral_1h_input_tokens
                    + r.cache_creation.ephemeral_5m_input_tokens,
            };
            token_totals.add(&t);
            let label = r.model.clone().unwrap_or_else(|| "Unknown".to_string());
            tokens_by_label.entry(label).or_default().add(&t);
        }
    }

    let mut tokens_by_model: Vec<(String, TokenTotals)> = tokens_by_label.into_iter().collect();
    tokens_by_model.sort_by_key(|(_, t)| std::cmp::Reverse(t.total()));

    Summary {
        since_label,
        until_label,
        org_name,
        total_spend_dollars,
        spend_by_model,
        daily_spend,
        token_totals,
        tokens_by_model,
        spend_by_project: Vec::new(),
        is_estimate: false,
        has_unpriced_data: false,
    }
}
