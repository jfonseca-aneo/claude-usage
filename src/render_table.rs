use comfy_table::presets::UTF8_FULL_CONDENSED;
use comfy_table::{Cell, CellAlignment, ContentArrangement, Table};

use crate::aggregate::Summary;
use crate::models::{CostBucket, UsageBucket};

const SPARK_LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

fn sparkline(values: &[f64]) -> String {
    let max = values.iter().cloned().fold(0.0_f64, f64::max);
    if max <= 0.0 {
        return values.iter().map(|_| SPARK_LEVELS[0]).collect();
    }
    values
        .iter()
        .map(|v| {
            let idx = ((v / max) * (SPARK_LEVELS.len() as f64 - 1.0)).round() as usize;
            SPARK_LEVELS[idx.min(SPARK_LEVELS.len() - 1)]
        })
        .collect()
}

fn fmt_usd(v: f64) -> String {
    format!("${v:.2}")
}

fn fmt_tokens(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

fn base_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

pub fn print_summary(summary: &Summary) {
    println!(
        "{}  {} → {}",
        summary.org_name.as_deref().unwrap_or("Anthropic API usage"),
        summary.since_label,
        summary.until_label,
    );
    println!();
    let spend_label = if summary.is_estimate { "  Est. spend:   " } else { "  Total spend:  " };
    println!("{}  {}", spend_label, fmt_usd(summary.total_spend_dollars));
    println!(
        "  Total tokens:   {} (in: {}, out: {}, cache read: {}, cache write: {})",
        fmt_tokens(summary.token_totals.total()),
        fmt_tokens(summary.token_totals.input),
        fmt_tokens(summary.token_totals.output),
        fmt_tokens(summary.token_totals.cache_read),
        fmt_tokens(summary.token_totals.cache_write),
    );

    if !summary.daily_spend.is_empty() {
        let values: Vec<f64> = summary.daily_spend.iter().map(|(_, v)| *v).collect();
        println!(
            "  Daily spend:    {}  ({} → {})",
            sparkline(&values),
            fmt_usd(*values.first().unwrap_or(&0.0)),
            fmt_usd(*values.last().unwrap_or(&0.0)),
        );
    }
    println!();

    if !summary.spend_by_model.is_empty() {
        println!("Spend by model");
        let mut table = base_table();
        table.set_header(vec!["Model", "Spend", "% of total"]);
        for (label, amount) in &summary.spend_by_model {
            let pct = if summary.total_spend_dollars > 0.0 {
                amount / summary.total_spend_dollars * 100.0
            } else {
                0.0
            };
            table.add_row(vec![
                Cell::new(label),
                Cell::new(fmt_usd(*amount)).set_alignment(CellAlignment::Right),
                Cell::new(format!("{pct:.1}%")).set_alignment(CellAlignment::Right),
            ]);
        }
        println!("{table}");
        println!();
    }

    if !summary.spend_by_project.is_empty() {
        println!("Spend by project");
        let mut table = base_table();
        table.set_header(vec!["Project", "Spend", "% of total"]);
        for (label, amount) in &summary.spend_by_project {
            let pct = if summary.total_spend_dollars > 0.0 {
                amount / summary.total_spend_dollars * 100.0
            } else {
                0.0
            };
            table.add_row(vec![
                Cell::new(label),
                Cell::new(fmt_usd(*amount)).set_alignment(CellAlignment::Right),
                Cell::new(format!("{pct:.1}%")).set_alignment(CellAlignment::Right),
            ]);
        }
        println!("{table}");
        println!();
    }

    if !summary.tokens_by_model.is_empty() {
        println!("Tokens by model");
        let mut table = base_table();
        table.set_header(vec!["Model", "Input", "Output", "Cache read", "Cache write", "Total"]);
        for (model, t) in &summary.tokens_by_model {
            table.add_row(vec![
                Cell::new(model),
                Cell::new(fmt_tokens(t.input)).set_alignment(CellAlignment::Right),
                Cell::new(fmt_tokens(t.output)).set_alignment(CellAlignment::Right),
                Cell::new(fmt_tokens(t.cache_read)).set_alignment(CellAlignment::Right),
                Cell::new(fmt_tokens(t.cache_write)).set_alignment(CellAlignment::Right),
                Cell::new(fmt_tokens(t.total())).set_alignment(CellAlignment::Right),
            ]);
        }
        println!("{table}");
    }
}

pub fn print_usage_raw(buckets: &[UsageBucket], group_by: &[String]) {
    let mut table = base_table();
    let mut header = vec!["Bucket start".to_string()];
    header.extend(group_by.iter().cloned());
    header.extend(
        ["Input", "Output", "Cache read", "Cache write", "Total"]
            .iter()
            .map(|s| s.to_string()),
    );
    table.set_header(header);

    for bucket in buckets {
        for r in &bucket.results {
            let mut row = vec![Cell::new(&bucket.starting_at)];
            for g in group_by {
                let v = match g.as_str() {
                    "model" => r.model.clone(),
                    "workspace_id" => r.workspace_id.clone(),
                    "api_key_id" => r.api_key_id.clone(),
                    "service_tier" => r.service_tier.clone(),
                    _ => None,
                };
                row.push(Cell::new(v.unwrap_or_else(|| "—".to_string())));
            }
            let cache_write =
                r.cache_creation.ephemeral_1h_input_tokens + r.cache_creation.ephemeral_5m_input_tokens;
            let total = r.uncached_input_tokens + r.output_tokens + r.cache_read_input_tokens + cache_write;
            row.push(Cell::new(fmt_tokens(r.uncached_input_tokens)).set_alignment(CellAlignment::Right));
            row.push(Cell::new(fmt_tokens(r.output_tokens)).set_alignment(CellAlignment::Right));
            row.push(Cell::new(fmt_tokens(r.cache_read_input_tokens)).set_alignment(CellAlignment::Right));
            row.push(Cell::new(fmt_tokens(cache_write)).set_alignment(CellAlignment::Right));
            row.push(Cell::new(fmt_tokens(total)).set_alignment(CellAlignment::Right));
            table.add_row(row);
        }
    }
    println!("{table}");
}

pub fn print_cost_raw(buckets: &[CostBucket], group_by: &[String]) {
    let mut table = base_table();
    let mut header = vec!["Bucket start".to_string()];
    header.extend(group_by.iter().cloned());
    header.push("Amount".to_string());
    table.set_header(header);

    for bucket in buckets {
        for r in &bucket.results {
            let mut row = vec![Cell::new(&bucket.starting_at)];
            for g in group_by {
                let v = match g.as_str() {
                    "description" => r.description.clone(),
                    "workspace_id" => r.workspace_id.clone(),
                    _ => None,
                };
                row.push(Cell::new(v.unwrap_or_else(|| "—".to_string())));
            }
            let dollars = r.amount.parse::<f64>().unwrap_or(0.0) / 100.0;
            row.push(Cell::new(fmt_usd(dollars)).set_alignment(CellAlignment::Right));
            table.add_row(row);
        }
    }
    println!("{table}");
}
