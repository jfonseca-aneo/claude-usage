use std::fmt::Write as _;

use chrono::{NaiveDate, Utc};

use crate::aggregate::Summary;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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

/// Categorical palette slots (validated for CVD-safety in this fixed order —
/// see the dataviz skill's `references/palette.md`). Projects beyond this
/// count fold into a shared "Other" bucket rather than generating a new hue.
const MAX_CHART_PROJECTS: usize = 8;

/// Reduces `spend_by_project` (already sorted by spend descending) to the
/// fixed set of project names the chart has color slots for, folding any
/// overflow into a single "Other" bucket.
fn chart_project_order(spend_by_project: &[(String, f64)]) -> Vec<String> {
    if spend_by_project.len() <= MAX_CHART_PROJECTS {
        spend_by_project.iter().map(|(p, _)| p.clone()).collect()
    } else {
        let mut order: Vec<String> = spend_by_project[..MAX_CHART_PROJECTS - 1]
            .iter()
            .map(|(p, _)| p.clone())
            .collect();
        order.push("Other".to_string());
        order
    }
}

/// Aligns each day's per-project breakdown to `project_order`, folding any
/// project not in that order into the trailing "Other" slot (if present).
fn align_daily_segments(
    daily_by_project: &[(String, Vec<(String, f64)>)],
    project_order: &[String],
) -> Vec<Vec<f64>> {
    let other_idx = project_order.iter().position(|p| p == "Other");
    daily_by_project
        .iter()
        .map(|(_, entries)| {
            let mut amounts = vec![0.0; project_order.len()];
            for (name, amount) in entries {
                let idx = project_order.iter().position(|p| p == name).or(other_idx);
                if let Some(idx) = idx {
                    amounts[idx] += amount;
                }
            }
            amounts
        })
        .collect()
}

/// Renders the daily-spend trend as an SVG bar chart. With a project
/// breakdown it draws stacked, per-project-colored segments (2px-gapped, 4px
/// rounded stack top); otherwise a single sequential-blue bar per day. Either
/// way: 2px gaps between day-bars anchored to a shared baseline, and a native
/// <title> tooltip per bar/segment as the hover layer.
fn daily_spend_chart(
    daily: &[(String, f64)],
    segments: &[Vec<f64>],
    project_order: &[String],
    width: f64,
    height: f64,
) -> String {
    if daily.is_empty() {
        return String::new();
    }
    let left_pad = 4.0;
    let right_pad = 4.0;
    let top_pad = 16.0;
    let bottom_pad = 24.0;
    let chart_h = height - top_pad - bottom_pad;
    let usable_w = width - left_pad - right_pad;

    let max_val = daily.iter().map(|(_, v)| *v).fold(0.0_f64, f64::max).max(0.000_001);
    let n = daily.len();
    let gap = 2.0_f64;
    let bar_w = ((usable_w - gap * (n as f64 - 1.0)) / n as f64).max(1.0);
    let radius: f64 = 4.0_f64.min(bar_w / 2.0);
    let seg_gap: f64 = 2.0_f64.min(chart_h / 20.0);

    let by_project = segments.len() == daily.len() && project_order.len() > 1;

    // Show at most ~10 x-axis labels, evenly spaced.
    let label_stride = (n as f64 / 10.0).ceil().max(1.0) as usize;

    let mut svg = String::new();
    let _ = write!(
        svg,
        r#"<svg viewBox="0 0 {width} {height}" width="100%" height="{height}" role="img" aria-label="Daily spend trend" preserveAspectRatio="none" class="chart-svg">"#
    );

    // Baseline
    let baseline_y = top_pad + chart_h;
    let _ = write!(
        svg,
        r#"<line x1="{left_pad}" y1="{baseline_y}" x2="{w}" y2="{baseline_y}" class="baseline" />"#,
        w = width - right_pad
    );

    for (i, (date, amount)) in daily.iter().enumerate() {
        let x = left_pad + i as f64 * (bar_w + gap);

        if by_project {
            let amounts = &segments[i];
            let nonzero: Vec<usize> =
                amounts.iter().enumerate().filter(|(_, a)| **a > 0.0).map(|(idx, _)| idx).collect();
            let mut cum_h = 0.0_f64;
            for (seg_pos, &pi) in nonzero.iter().enumerate() {
                let amt = amounts[pi];
                let raw_h = (amt / max_val) * chart_h;
                let is_top = seg_pos == nonzero.len() - 1;
                let draw_h = (raw_h - if is_top { 0.0 } else { seg_gap }).max(0.5);
                let seg_bottom_y = baseline_y - cum_h;
                let seg_y = seg_bottom_y - draw_h;
                let title =
                    format!("{}: {} — {}", esc(date), esc(&project_order[pi]), esc(&fmt_usd(amt)));
                let fill = format!("var(--cat-{})", pi + 1);

                let _ = write!(
                    svg,
                    r#"<g class="bar-group"><rect x="{x:.2}" y="{seg_y:.2}" width="{bar_w:.2}" height="{draw_h:.2}" rx="{r:.1}" ry="{r:.1}" class="bar" style="fill:{fill}" />"#,
                    r = if is_top { radius } else { 0.0 }
                );
                if is_top && draw_h > radius {
                    let _ = write!(
                        svg,
                        r#"<rect x="{x:.2}" y="{sy:.2}" width="{bar_w:.2}" height="{radius:.1}" class="bar" style="fill:{fill}" />"#,
                        sy = seg_bottom_y - radius
                    );
                }
                let _ = write!(svg, r#"<title>{title}</title></g>"#);

                cum_h += raw_h;
            }
        } else {
            let bar_h = ((amount / max_val) * chart_h).max(1.0);
            let y = top_pad + (chart_h - bar_h);
            let title = format!("{}: {}", esc(date), esc(&fmt_usd(*amount)));

            let _ = write!(
                svg,
                r#"<g class="bar-group"><rect x="{x:.2}" y="{y:.2}" width="{bar_w:.2}" height="{bar_h:.2}" rx="{radius:.1}" ry="{radius:.1}" class="bar" />"#
            );
            // Square off the bottom corners so only the top is rounded (a
            // sharp rect over the lower slice of the rounded one).
            if bar_h > radius {
                let _ = write!(
                    svg,
                    r#"<rect x="{x:.2}" y="{sy:.2}" width="{bar_w:.2}" height="{radius:.1}" class="bar" />"#,
                    sy = baseline_y - radius
                );
            }
            let _ = write!(svg, r#"<title>{title}</title></g>"#);
        }

        if i % label_stride == 0 || i == n - 1 {
            let label_x = x + bar_w / 2.0;
            let short = date.get(5..10).unwrap_or(date);
            let _ = write!(
                svg,
                r#"<text x="{label_x:.2}" y="{ly:.2}" class="axis-label" text-anchor="middle">{short}</text>"#,
                ly = height - 6.0
            );
        }
    }

    svg.push_str("</svg>");
    svg
}

fn chart_legend(project_order: &[String]) -> String {
    if project_order.len() < 2 {
        return String::new();
    }
    let mut items = String::new();
    for (i, name) in project_order.iter().enumerate() {
        let _ = write!(
            items,
            r#"<span class="legend-item"><span class="legend-swatch" style="background:var(--cat-{idx})"></span>{name}</span>"#,
            idx = i + 1,
            name = esc(name),
        );
    }
    format!(r#"<div class="chart-legend">{items}</div>"#)
}

fn kpi_tile(label: &str, value: String, sub: Option<String>) -> String {
    let sub_html = sub
        .map(|s| format!(r#"<div class="kpi-sub">{}</div>"#, esc(&s)))
        .unwrap_or_default();
    format!(
        r#"<div class="kpi"><div class="kpi-label">{label}</div><div class="kpi-value">{value}</div>{sub_html}</div>"#,
        label = esc(label)
    )
}

fn spend_table(items: &[(String, f64)], total: f64, first_col: &str) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut rows = String::new();
    for (label, amount) in items {
        let pct = if total > 0.0 { amount / total * 100.0 } else { 0.0 };
        let _ = write!(
            rows,
            r#"<tr><td>{label}</td><td class="num">{amount}</td><td class="num">{pct:.1}%</td></tr>"#,
            label = esc(label),
            amount = esc(&fmt_usd(*amount)),
        );
    }
    format!(
        r#"<table><thead><tr><th>{first_col}</th><th class="num">Spend</th><th class="num">% of total</th></tr></thead><tbody>{rows}</tbody></table>"#,
        first_col = esc(first_col),
    )
}

fn tokens_by_model_table(summary: &Summary) -> String {
    if summary.tokens_by_model.is_empty() {
        return String::new();
    }
    let mut rows = String::new();
    for (model, t) in &summary.tokens_by_model {
        let _ = write!(
            rows,
            r#"<tr><td>{model}</td><td class="num">{input}</td><td class="num">{output}</td><td class="num">{cread}</td><td class="num">{cwrite}</td><td class="num">{total}</td></tr>"#,
            model = esc(model),
            input = fmt_tokens(t.input),
            output = fmt_tokens(t.output),
            cread = fmt_tokens(t.cache_read),
            cwrite = fmt_tokens(t.cache_write),
            total = fmt_tokens(t.total()),
        );
    }
    format!(
        r#"<table><thead><tr><th>Model</th><th class="num">Input</th><th class="num">Output</th><th class="num">Cache read</th><th class="num">Cache write</th><th class="num">Total</th></tr></thead><tbody>{rows}</tbody></table>"#
    )
}

pub fn render(summary: &Summary) -> String {
    let title = format!(
        "Claude API usage — {} to {}",
        summary.since_label, summary.until_label
    );
    let org = summary.org_name.as_deref().unwrap_or("Anthropic API organization");
    let generated = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();

    // The report's date range, not the number of days that happen to have
    // spend recorded — a quiet day in the middle of the range shouldn't
    // shrink "days covered".
    let days_covered = match (
        NaiveDate::parse_from_str(&summary.since_label, "%Y-%m-%d"),
        NaiveDate::parse_from_str(&summary.until_label, "%Y-%m-%d"),
    ) {
        (Ok(since), Ok(until)) => (until - since).num_days() + 1,
        _ => summary.daily_spend.len() as i64,
    }
    .max(1);
    let avg_daily = summary.total_spend_dollars / days_covered as f64;

    let spend_kpi_label = if summary.is_estimate { "Est. spend" } else { "Total spend" };
    let kpis = format!(
        r#"<div class="kpis">{}{}{}{}</div>"#,
        kpi_tile(spend_kpi_label, fmt_usd(summary.total_spend_dollars), Some(format!(
            "{} to {}",
            summary.since_label, summary.until_label
        ))),
        kpi_tile("Total tokens", fmt_tokens(summary.token_totals.total()), Some(format!(
            "in {} · out {}",
            fmt_tokens(summary.token_totals.input),
            fmt_tokens(summary.token_totals.output)
        ))),
        kpi_tile("Avg daily spend", fmt_usd(avg_daily), None),
        kpi_tile("Days covered", days_covered.to_string(), None),
    );

    let project_order = chart_project_order(&summary.spend_by_project);
    let segments = align_daily_segments(&summary.daily_spend_by_project, &project_order);
    let chart = daily_spend_chart(&summary.daily_spend, &segments, &project_order, 920.0, 220.0);
    let legend = chart_legend(&project_order);
    let chart_section = if chart.is_empty() {
        String::new()
    } else {
        format!(
            r#"<section class="card"><h2>Daily spend</h2><div class="chart-wrap">{chart}</div>{legend}</section>"#
        )
    };

    let model_spend_table = spend_table(&summary.spend_by_model, summary.total_spend_dollars, "Model");
    let spend_section = if model_spend_table.is_empty() {
        String::new()
    } else {
        format!(r#"<section class="card"><h2>Spend by model</h2>{model_spend_table}</section>"#)
    };

    let project_spend_table = spend_table(&summary.spend_by_project, summary.total_spend_dollars, "Project");
    let project_section = if project_spend_table.is_empty() {
        String::new()
    } else {
        format!(r#"<section class="card"><h2>Spend by project</h2>{project_spend_table}</section>"#)
    };

    let tokens_table = tokens_by_model_table(summary);
    let tokens_section = if tokens_table.is_empty() {
        String::new()
    } else {
        format!(r#"<section class="card"><h2>Tokens by model</h2>{tokens_table}</section>"#)
    };

    let mut footer_notes = String::from("Generated by claude-usage.");
    if summary.is_estimate {
        footer_notes.push_str(
            " Spend is a local estimate from Claude Code session logs and published \
             per-model pricing, not authoritative billing data.",
        );
    } else {
        footer_notes.push_str(" Data from the Anthropic Usage &amp; Cost Admin API.");
    }
    if summary.has_unpriced_data {
        footer_notes.push_str(
            " Some usage came from a model with no known price and isn't included in the total.",
        );
    }

    format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
:root {{
  color-scheme: light;
  --surface-1:      #fcfcfb;
  --page:           #f9f9f7;
  --text-primary:   #0b0b0b;
  --text-secondary: #52514e;
  --text-muted:     #898781;
  --gridline:       #e1e0d9;
  --baseline:       #c3c2b7;
  --border:         rgba(11,11,11,0.10);
  --series-1:       #2a78d6;
  --cat-1: #2a78d6; --cat-2: #eb6834; --cat-3: #1baf7a; --cat-4: #eda100;
  --cat-5: #e87ba4; --cat-6: #008300; --cat-7: #4a3aa7; --cat-8: #e34948;
}}
@media (prefers-color-scheme: dark) {{
  :root:not([data-theme="light"]) {{
    color-scheme: dark;
    --surface-1:      #1a1a19;
    --page:           #0d0d0d;
    --text-primary:   #ffffff;
    --text-secondary: #c3c2b7;
    --text-muted:     #898781;
    --gridline:       #2c2c2a;
    --baseline:       #383835;
    --border:         rgba(255,255,255,0.10);
    --series-1:       #3987e5;
    --cat-1: #3987e5; --cat-2: #d95926; --cat-3: #199e70; --cat-4: #c98500;
    --cat-5: #d55181; --cat-6: #008300; --cat-7: #9085e9; --cat-8: #e66767;
  }}
}}
:root[data-theme="dark"] {{
  color-scheme: dark;
  --surface-1:      #1a1a19;
  --page:           #0d0d0d;
  --text-primary:   #ffffff;
  --text-secondary: #c3c2b7;
  --text-muted:     #898781;
  --gridline:       #2c2c2a;
  --baseline:       #383835;
  --border:         rgba(255,255,255,0.10);
  --series-1:       #3987e5;
  --cat-1: #3987e5; --cat-2: #d95926; --cat-3: #199e70; --cat-4: #c98500;
  --cat-5: #d55181; --cat-6: #008300; --cat-7: #9085e9; --cat-8: #e66767;
}}
* {{ box-sizing: border-box; }}
body {{
  margin: 0;
  padding: 32px 20px 64px;
  background: var(--page);
  color: var(--text-primary);
  font: 15px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif;
}}
.wrap {{ max-width: 960px; margin: 0 auto; }}
header {{ margin-bottom: 24px; }}
header .org {{ font-size: 22px; font-weight: 600; }}
header .range {{ color: var(--text-secondary); margin-top: 2px; }}
.kpis {{
  display: grid;
  grid-template-columns: repeat(4, minmax(0,1fr));
  gap: 12px;
  margin-bottom: 20px;
}}
.kpi {{
  background: var(--surface-1);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 14px 16px;
}}
.kpi-label {{ font-size: 12px; color: var(--text-muted); text-transform: uppercase; letter-spacing: .04em; }}
.kpi-value {{ font-size: 26px; font-weight: 600; margin-top: 4px; }}
.kpi-sub {{ font-size: 12px; color: var(--text-secondary); margin-top: 4px; }}
.card {{
  background: var(--surface-1);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 18px 20px;
  margin-bottom: 16px;
}}
.card h2 {{ font-size: 14px; font-weight: 600; margin: 0 0 12px; color: var(--text-secondary); }}
.chart-wrap {{ width: 100%; }}
.chart-svg {{ display: block; width: 100%; height: auto; }}
.bar {{ fill: var(--series-1); }}
.bar-group:hover .bar {{ filter: brightness(0.85); }}
.baseline {{ stroke: var(--baseline); stroke-width: 1; }}
.axis-label {{ fill: var(--text-muted); font-size: 10px; font-variant-numeric: tabular-nums; }}
.chart-legend {{ display: flex; flex-wrap: wrap; gap: 6px 16px; margin-top: 12px; }}
.legend-item {{ display: inline-flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text-secondary); }}
.legend-swatch {{ width: 10px; height: 10px; border-radius: 2px; flex: none; }}
table {{ width: 100%; border-collapse: collapse; font-variant-numeric: tabular-nums; }}
th, td {{ text-align: left; padding: 8px 10px; border-bottom: 1px solid var(--gridline); font-size: 13.5px; }}
th {{ color: var(--text-muted); font-weight: 500; font-size: 12px; text-transform: uppercase; letter-spacing: .03em; }}
td.num, th.num {{ text-align: right; }}
tbody tr:last-child td {{ border-bottom: none; }}
footer {{ color: var(--text-muted); font-size: 12px; margin-top: 24px; }}
@media (max-width: 640px) {{
  .kpis {{ grid-template-columns: repeat(2, minmax(0,1fr)); }}
}}
</style>
</head>
<body>
<div class="wrap">
<header>
  <div class="org">{org}</div>
  <div class="range">{since} → {until} · generated {generated}</div>
</header>
{kpis}
{chart_section}
{spend_section}
{project_section}
{tokens_section}
<footer>{footer_notes}</footer>
</div>
</body>
</html>
"##,
        title = esc(&title),
        org = esc(org),
        since = summary.since_label,
        until = summary.until_label,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::TokenTotals;

    fn sample_summary() -> Summary {
        Summary {
            since_label: "2026-08-10".into(),
            until_label: "2026-08-16".into(),
            org_name: Some("Acme, Inc.".into()),
            total_spend_dollars: 128.42,
            spend_by_model: vec![
                ("claude-opus-4-6".into(), 89.10),
                ("claude-sonnet-4-6".into(), 32.50),
                ("Web Search Usage".into(), 6.82),
            ],
            daily_spend: vec![
                ("2026-08-10".into(), 12.0),
                ("2026-08-11".into(), 18.5),
                ("2026-08-12".into(), 9.2),
                ("2026-08-13".into(), 22.0),
                ("2026-08-14".into(), 30.1),
                ("2026-08-15".into(), 14.4),
                ("2026-08-16".into(), 21.32),
            ],
            token_totals: TokenTotals {
                input: 4_200_000,
                output: 980_000,
                cache_read: 12_500_000,
                cache_write: 640_000,
            },
            tokens_by_model: vec![
                (
                    "claude-opus-4-6".into(),
                    TokenTotals { input: 2_800_000, output: 600_000, cache_read: 9_000_000, cache_write: 400_000 },
                ),
                (
                    "claude-sonnet-4-6".into(),
                    TokenTotals { input: 1_400_000, output: 380_000, cache_read: 3_500_000, cache_write: 240_000 },
                ),
            ],
            spend_by_project: vec![
                ("ns-fd-rust".into(), 80.0),
                ("intro-hpc-cloud".into(), 48.42),
            ],
            daily_spend_by_project: vec![
                ("2026-08-10".into(), vec![("ns-fd-rust".into(), 8.0), ("intro-hpc-cloud".into(), 4.0)]),
                ("2026-08-11".into(), vec![("ns-fd-rust".into(), 12.5), ("intro-hpc-cloud".into(), 6.0)]),
                ("2026-08-12".into(), vec![("ns-fd-rust".into(), 9.2)]),
                ("2026-08-13".into(), vec![("intro-hpc-cloud".into(), 22.0)]),
                ("2026-08-14".into(), vec![("ns-fd-rust".into(), 18.1), ("intro-hpc-cloud".into(), 12.0)]),
                ("2026-08-15".into(), vec![("ns-fd-rust".into(), 14.4)]),
                ("2026-08-16".into(), vec![("ns-fd-rust".into(), 17.82), ("intro-hpc-cloud".into(), 3.5)]),
            ],
            is_estimate: false,
            has_unpriced_data: false,
        }
    }

    #[test]
    fn renders_expected_sections() {
        let html = render(&sample_summary());
        assert!(html.contains("<svg"));
        assert!(html.contains("Acme, Inc."));
        assert!(html.contains("claude-opus-4-6"));
        assert!(html.contains("Spend by model"));
        assert!(html.contains("Tokens by model"));
    }

    #[test]
    fn empty_summary_does_not_panic() {
        let s = Summary {
            since_label: "2026-08-10".into(),
            until_label: "2026-08-16".into(),
            org_name: None,
            total_spend_dollars: 0.0,
            spend_by_model: vec![],
            daily_spend: vec![],
            token_totals: TokenTotals::default(),
            tokens_by_model: vec![],
            spend_by_project: vec![],
            daily_spend_by_project: vec![],
            is_estimate: false,
            has_unpriced_data: false,
        };
        let html = render(&s);
        assert!(html.contains("Total spend"));
    }

    /// Not part of the test suite proper — run with
    /// `cargo test --ignored write_preview_to_disk` to dump a sample report
    /// for manual visual review.
    #[test]
    #[ignore]
    fn write_preview_to_disk() {
        let html = render(&sample_summary());
        let path = std::env::var("PREVIEW_OUT").unwrap_or_else(|_| "/tmp/claude-usage-preview.html".into());
        std::fs::write(&path, html).unwrap();
        eprintln!("wrote {path}");
    }
}
