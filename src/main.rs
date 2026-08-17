mod aggregate;
mod cli;
mod client;
mod local_usage;
mod models;
mod pricing;
mod render_html;
mod render_table;
mod time_range;

use std::fs;
use std::process::Command as ProcessCommand;

use anyhow::{bail, Result};
use clap::Parser;

use aggregate::build_summary;
use cli::{Cli, Command};
use client::AdminClient;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Code { claude_dir: None });

    if let Command::Code { claude_dir } = command {
        let range = time_range::resolve(
            cli.range.days,
            cli.range.since.as_deref(),
            cli.range.until.as_deref(),
        )?;
        return run_code(&range, claude_dir, cli.json, cli.html);
    }

    let api_key = cli.api_key.clone().unwrap_or_default();
    if api_key.is_empty() {
        bail!(
            "no Admin API key provided.\n\n\
             Pass one with --api-key, or set ANTHROPIC_ADMIN_KEY.\n\
             Admin API keys start with sk-ant-admin... and are created by an\n\
             organization admin at https://platform.claude.com — see\n\
             https://platform.claude.com/docs/en/manage-claude/admin-api-keys\n\n\
             No admin key? Run `claude-usage code` instead — it reads local Claude\n\
             Code session logs and needs no credentials at all."
        );
    }
    if !api_key.starts_with("sk-ant-admin") {
        eprintln!(
            "warning: this key doesn't look like an Admin API key (expected a \
             sk-ant-admin... key). Usage/cost endpoints will reject a regular API key."
        );
    }

    let range = time_range::resolve(
        cli.range.days,
        cli.range.since.as_deref(),
        cli.range.until.as_deref(),
    )?;

    let client = AdminClient::new(api_key)?;

    match command {
        Command::Summary => run_summary(&client, &range, cli.json, cli.html),
        Command::Code { .. } => unreachable!("handled above"),
        Command::Usage { group_by, bucket_width } => {
            if cli.html.is_some() {
                eprintln!("note: --html is only supported for the summary report; ignoring it.");
            }
            let buckets = client.usage_report(
                &range.starting_at,
                &range.ending_at,
                &bucket_width,
                &group_by.iter().map(String::as_str).collect::<Vec<_>>(),
            )?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&buckets)?);
            } else {
                render_table::print_usage_raw(&buckets, &group_by);
            }
            Ok(())
        }
        Command::Cost { group_by } => {
            if cli.html.is_some() {
                eprintln!("note: --html is only supported for the summary report; ignoring it.");
            }
            let buckets = client.cost_report(
                &range.starting_at,
                &range.ending_at,
                &group_by.iter().map(String::as_str).collect::<Vec<_>>(),
            )?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&buckets)?);
            } else {
                render_table::print_cost_raw(&buckets, &group_by);
            }
            Ok(())
        }
    }
}

fn run_summary(
    client: &AdminClient,
    range: &time_range::Range,
    json: bool,
    html: Option<std::path::PathBuf>,
) -> Result<()> {
    let org_name = client.org_info().ok().map(|o| o.name);

    let usage = client.usage_report(&range.starting_at, &range.ending_at, "1d", &["model"])?;
    let cost = client.cost_report(&range.starting_at, &range.ending_at, &["description"])?;

    let summary = build_summary(
        range.since_label.clone(),
        range.until_label.clone(),
        org_name,
        &usage,
        &cost,
    );

    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        render_table::print_summary(&summary);
    }

    if let Some(path) = html {
        let path = ensure_html_extension(path);
        let doc = render_html::render(&summary);
        fs::write(&path, doc)?;
        println!("\nWrote HTML report to {}", path.display());
        open_in_browser(&path)?;
    }

    Ok(())
}

fn run_code(
    range: &time_range::Range,
    claude_dir: Option<std::path::PathBuf>,
    json: bool,
    html: Option<std::path::PathBuf>,
) -> Result<()> {
    let dir = match claude_dir {
        Some(d) => d,
        None => local_usage::claude_dir()?,
    };
    let projects_dir = dir.join("projects");

    let records = local_usage::load_assistant_records(&projects_dir)?;

    let since: chrono::DateTime<chrono::Utc> = range.starting_at.parse()?;
    let until: chrono::DateTime<chrono::Utc> = range.ending_at.parse()?;
    let in_range: Vec<_> = records
        .into_iter()
        .filter(|r| r.timestamp >= since && r.timestamp < until)
        .collect();

    if in_range.is_empty() {
        eprintln!(
            "note: no Claude Code activity found in {} to {} under {}",
            range.since_label,
            range.until_label,
            projects_dir.display()
        );
    }

    let summary = local_usage::build_local_summary(
        range.since_label.clone(),
        range.until_label.clone(),
        &in_range,
    );

    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!(
            "{} assistant turns across {} project{}\n",
            in_range.len(),
            summary.spend_by_project.len(),
            if summary.spend_by_project.len() == 1 { "" } else { "s" }
        );
        render_table::print_summary(&summary);
        if summary.has_unpriced_data {
            println!(
                "\nnote: some usage came from a model with no known price and isn't \
                 included in the total — token counts above are still complete."
            );
        }
    }

    if let Some(path) = html {
        let path = ensure_html_extension(path);
        let doc = render_html::render(&summary);
        fs::write(&path, doc)?;
        println!("\nWrote HTML report to {}", path.display());
        open_in_browser(&path)?;
    }

    Ok(())
}

/// Appends `.html` when the given path has no extension at all, so
/// `--html report` behaves like `--html report.html`. A path that already
/// has some extension (even a non-html one) is left untouched.
fn ensure_html_extension(path: std::path::PathBuf) -> std::path::PathBuf {
    if path.extension().is_none() {
        path.with_extension("html")
    } else {
        path
    }
}

fn open_in_browser(path: &std::path::Path) -> Result<()> {
    let abs = fs::canonicalize(path)?;
    #[cfg(target_os = "macos")]
    let status = ProcessCommand::new("open").arg(&abs).status();
    #[cfg(target_os = "linux")]
    let status = ProcessCommand::new("xdg-open").arg(&abs).status();
    #[cfg(target_os = "windows")]
    let status = ProcessCommand::new("cmd").args(["/C", "start", ""]).arg(&abs).status();
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let status: std::io::Result<std::process::ExitStatus> =
        Err(std::io::Error::other("unsupported platform for opening a browser"));

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => bail!("failed to open browser (exit code {:?})", s.code()),
        Err(e) => bail!("failed to open browser: {e}"),
    }
}
