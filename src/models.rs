use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct OrgInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UsageReport {
    pub data: Vec<UsageBucket>,
    pub has_more: bool,
    pub next_page: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UsageBucket {
    pub starting_at: String,
    #[allow(dead_code)]
    pub ending_at: String,
    pub results: Vec<UsageResult>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct CacheCreation {
    #[serde(default)]
    pub ephemeral_1h_input_tokens: u64,
    #[serde(default)]
    pub ephemeral_5m_input_tokens: u64,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct ServerToolUse {
    #[serde(default)]
    #[allow(dead_code)]
    pub web_search_requests: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UsageResult {
    #[allow(dead_code)]
    pub account_id: Option<String>,
    pub api_key_id: Option<String>,
    #[serde(default)]
    pub cache_creation: CacheCreation,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[allow(dead_code)]
    pub context_window: Option<String>,
    #[allow(dead_code)]
    pub inference_geo: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    #[allow(dead_code)]
    pub server_tool_use: ServerToolUse,
    #[allow(dead_code)]
    pub service_account_id: Option<String>,
    pub service_tier: Option<String>,
    #[serde(default)]
    pub uncached_input_tokens: u64,
    pub workspace_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CostReport {
    pub data: Vec<CostBucket>,
    pub has_more: bool,
    pub next_page: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CostBucket {
    pub starting_at: String,
    #[allow(dead_code)]
    pub ending_at: String,
    pub results: Vec<CostResult>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CostResult {
    /// Decimal string, in the *lowest currency unit* (cents for USD).
    /// e.g. "123.45" means $1.2345.
    pub amount: String,
    #[allow(dead_code)]
    pub context_window: Option<String>,
    pub cost_type: Option<String>,
    #[allow(dead_code)]
    pub currency: String,
    pub description: Option<String>,
    #[allow(dead_code)]
    pub inference_geo: Option<String>,
    pub model: Option<String>,
    #[allow(dead_code)]
    pub service_tier: Option<String>,
    #[allow(dead_code)]
    pub token_type: Option<String>,
    pub workspace_id: Option<String>,
}
