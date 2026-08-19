use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::aggregate::{Summary, TokenTotals};
use crate::pricing::{self, UsageForPricing};

/// Resolves Claude Code's local data directory: `$CLAUDE_CONFIG_DIR` if set,
/// otherwise `~/.claude`.
pub fn claude_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .context("could not determine home directory (HOME/USERPROFILE unset)")?;
    Ok(PathBuf::from(home).join(".claude"))
}

fn find_jsonl_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return Ok(()), // skip unreadable subdirs rather than failing the whole scan
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            find_jsonl_files(&path, out)?;
        } else if file_type.is_file() && path.extension().is_some_and(|e| e == "jsonl") {
            out.push(path);
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct RawUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation: Option<RawCacheCreation>,
}

#[derive(Debug, Deserialize, Default)]
struct RawCacheCreation {
    #[serde(default)]
    ephemeral_1h_input_tokens: u64,
    #[serde(default)]
    ephemeral_5m_input_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct RawMessage {
    id: String,
    model: String,
    #[serde(default)]
    usage: Option<RawUsage>,
}

#[derive(Debug, Deserialize)]
struct RawRecord {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    message: Option<RawMessage>,
    #[serde(default)]
    timestamp: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AssistantRecord {
    pub timestamp: DateTime<Utc>,
    pub model: String,
    pub project: String,
    pub tokens: TokenTotals,
    /// Split cache-write tokens, needed for accurate per-TTL cost.
    pub cache_5m: u64,
    pub cache_1h: u64,
}

/// Loads and deduplicates every assistant turn from Claude Code's local
/// session logs under `root` (typically `~/.claude/projects`).
///
/// Claude Code streams progressively-updated snapshots of an in-flight
/// assistant message into the same JSONL file — the same `message.id`
/// appears multiple times with growing `usage` counts as the response
/// streams in. Keeping only the *last* occurrence per id (later lines are
/// always a superset of earlier ones) gives the correct final token count.
pub fn load_assistant_records(root: &Path) -> Result<Vec<AssistantRecord>> {
    let mut files = Vec::new();
    find_jsonl_files(root, &mut files)
        .with_context(|| format!("scanning {} for session logs", root.display()))?;

    if files.is_empty() {
        bail!(
            "no Claude Code session logs found under {}\n\n\
             This reads local logs written by the `claude` CLI itself — if you haven't\n\
             used Claude Code on this machine (or set a custom CLAUDE_CONFIG_DIR), there's\n\
             nothing to read yet.",
            root.display()
        );
    }

    let mut by_id: HashMap<String, AssistantRecord> = HashMap::new();

    for file in files {
        let content = match fs::read_to_string(&file) {
            Ok(c) => c,
            Err(_) => continue, // unreadable/binary/partially-written file: skip
        };
        let project = project_label(&file, root);

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let raw: RawRecord = match serde_json::from_str(line) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if raw.kind != "assistant" {
                continue;
            }
            let Some(message) = raw.message else { continue };
            if message.model == "<synthetic>" {
                continue;
            }
            let Some(usage) = message.usage else { continue };
            let Some(ts) = raw.timestamp.as_deref() else { continue };
            let Ok(timestamp) = DateTime::parse_from_rfc3339(ts) else { continue };
            let timestamp = timestamp.with_timezone(&Utc);

            let cache = usage.cache_creation.unwrap_or_default();
            let record = AssistantRecord {
                timestamp,
                model: message.model,
                project: project.clone(),
                tokens: TokenTotals {
                    input: usage.input_tokens,
                    output: usage.output_tokens,
                    cache_read: usage.cache_read_input_tokens,
                    cache_write: cache.ephemeral_1h_input_tokens + cache.ephemeral_5m_input_tokens,
                },
                cache_5m: cache.ephemeral_5m_input_tokens,
                cache_1h: cache.ephemeral_1h_input_tokens,
            };

            by_id.insert(message.id, record);
        }
    }

    Ok(by_id.into_values().collect())
}

/// Best-effort project label from a project directory name under
/// `~/.claude/projects/`, e.g. `-home-jose-repos-ns-fd-rust` -> `ns-fd-rust`.
/// Falls back to the raw slug when it doesn't match the common
/// `-home-<user>[-repos]-<name>` shape Claude Code produces.
fn project_label(file: &Path, root: &Path) -> String {
    let slug = file
        .strip_prefix(root)
        .ok()
        .and_then(|rel| rel.components().next())
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_string());
    shorten_project_slug(&slug)
}

fn shorten_project_slug(slug: &str) -> String {
    let parts: Vec<&str> = slug.split('-').filter(|s| !s.is_empty()).collect();
    if parts.first() != Some(&"home") || parts.len() < 3 {
        return slug.to_string();
    }
    // Skip "home" and the username; also skip a following "repos" segment.
    let mut idx = 2;
    if parts.get(idx) == Some(&"repos") {
        idx += 1;
    }
    if idx < parts.len() {
        parts[idx..].join("-")
    } else {
        slug.to_string()
    }
}

pub fn build_local_summary(
    since_label: String,
    until_label: String,
    records: &[AssistantRecord],
) -> Summary {
    let mut total_spend_dollars = 0.0;
    let mut has_unpriced_data = false;
    let mut spend_by_label: HashMap<String, f64> = HashMap::new();
    let mut spend_by_project: HashMap<String, f64> = HashMap::new();
    let mut daily_spend: HashMap<String, f64> = HashMap::new();
    let mut daily_project_spend: HashMap<String, HashMap<String, f64>> = HashMap::new();
    let mut token_totals = TokenTotals::default();
    let mut tokens_by_label: HashMap<String, TokenTotals> = HashMap::new();

    for r in records {
        token_totals.input += r.tokens.input;
        token_totals.output += r.tokens.output;
        token_totals.cache_read += r.tokens.cache_read;
        token_totals.cache_write += r.tokens.cache_write;
        let entry = tokens_by_label.entry(r.model.clone()).or_default();
        entry.input += r.tokens.input;
        entry.output += r.tokens.output;
        entry.cache_read += r.tokens.cache_read;
        entry.cache_write += r.tokens.cache_write;

        let cost = pricing::cost_dollars(
            &r.model,
            r.timestamp,
            &UsageForPricing {
                uncached_input_tokens: r.tokens.input,
                output_tokens: r.tokens.output,
                cache_read_input_tokens: r.tokens.cache_read,
                cache_creation_5m_tokens: r.cache_5m,
                cache_creation_1h_tokens: r.cache_1h,
            },
        );
        match cost {
            Some(dollars) => {
                total_spend_dollars += dollars;
                *spend_by_label.entry(r.model.clone()).or_insert(0.0) += dollars;
                *spend_by_project.entry(r.project.clone()).or_insert(0.0) += dollars;
                let date = r.timestamp.format("%Y-%m-%d").to_string();
                *daily_spend.entry(date.clone()).or_insert(0.0) += dollars;
                *daily_project_spend
                    .entry(date)
                    .or_default()
                    .entry(r.project.clone())
                    .or_insert(0.0) += dollars;
            }
            None => has_unpriced_data = true,
        }
    }

    let mut spend_by_model: Vec<(String, f64)> = spend_by_label.into_iter().collect();
    spend_by_model.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut spend_by_project: Vec<(String, f64)> = spend_by_project.into_iter().collect();
    spend_by_project.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut tokens_by_model: Vec<(String, TokenTotals)> = tokens_by_label.into_iter().collect();
    tokens_by_model.sort_by_key(|(_, t)| std::cmp::Reverse(t.total()));

    let mut daily_spend: Vec<(String, f64)> = daily_spend.into_iter().collect();
    daily_spend.sort_by(|a, b| a.0.cmp(&b.0));

    let daily_spend_by_project: Vec<(String, Vec<(String, f64)>)> = daily_spend
        .iter()
        .map(|(date, _)| {
            let mut entries: Vec<(String, f64)> = daily_project_spend
                .get(date)
                .map(|by_project| by_project.iter().map(|(p, v)| (p.clone(), *v)).collect())
                .unwrap_or_default();
            entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            (date.clone(), entries)
        })
        .collect();

    Summary {
        since_label,
        until_label,
        org_name: Some("Claude Code (local usage)".to_string()),
        total_spend_dollars,
        spend_by_model,
        daily_spend,
        token_totals,
        tokens_by_model,
        spend_by_project,
        daily_spend_by_project,
        is_estimate: true,
        has_unpriced_data,
    }
}
