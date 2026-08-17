use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "claude-usage",
    version,
    about = "View your Anthropic API credit usage and cost from the terminal, or export it as an HTML report",
    long_about = "View your Anthropic API credit usage and cost from the terminal, or export it as an HTML report.\n\n\
Requires an Admin API key (starts with sk-ant-admin...) for your Anthropic\n\
organization — see https://platform.claude.com/docs/en/manage-claude/admin-api-keys.\n\
Pass it with --api-key or the ANTHROPIC_ADMIN_KEY environment variable."
)]
pub struct Cli {
    #[command(flatten)]
    pub range: RangeArgs,

    /// Admin API key (starts with sk-ant-admin...).
    #[arg(long, env = "ANTHROPIC_ADMIN_KEY", global = true, hide_env_values = true)]
    pub api_key: Option<String>,

    /// Write a self-contained HTML report to this file. If FILE is omitted
    /// (or passed as "-"), a name is generated from a random UUID.
    #[arg(long, value_name = "FILE", global = true, num_args(0..=1), default_missing_value = "-")]
    pub html: Option<PathBuf>,

    /// Print machine-readable JSON instead of a table.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Args, Clone)]
pub struct RangeArgs {
    /// Show the last N days. Ignored if --since is set. [default: 7]
    #[arg(long, global = true)]
    pub days: Option<i64>,

    /// Start of the range: YYYY-MM-DD or an RFC 3339 timestamp.
    #[arg(long, global = true)]
    pub since: Option<String>,

    /// End of the range: YYYY-MM-DD or an RFC 3339 timestamp. [default: now]
    #[arg(long, global = true)]
    pub until: Option<String>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Combined spend + token summary.
    Summary,
    /// Raw token usage report, grouped by dimension(s) of your choice.
    Usage {
        /// Comma-separated: model,workspace_id,api_key_id,service_tier,context_window,account_id,service_account_id,inference_geo
        #[arg(long, value_delimiter = ',', default_value = "model")]
        group_by: Vec<String>,
        /// Time bucket size.
        #[arg(long, default_value = "1d", value_parser = ["1d", "1h", "1m"])]
        bucket_width: String,
    },
    /// Raw cost report, grouped by dimension(s) of your choice.
    Cost {
        /// Comma-separated: description,workspace_id
        #[arg(long, value_delimiter = ',', default_value = "description")]
        group_by: Vec<String>,
    },
    /// Estimated usage from local Claude Code session logs (default command) — no API key needed.
    ///
    /// Reads the JSONL transcripts the `claude` CLI already writes to
    /// ~/.claude/projects (or $CLAUDE_CONFIG_DIR) and estimates cost from
    /// published per-model pricing. This is a local estimate, not
    /// authoritative billing data — use `summary` for that.
    Code {
        /// Path to the Claude Code data directory. Defaults to
        /// $CLAUDE_CONFIG_DIR, or ~/.claude if unset.
        #[arg(long)]
        claude_dir: Option<std::path::PathBuf>,
    },
}
