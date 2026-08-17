# claude-usage

A small CLI for checking your Anthropic credit usage and spend — in the
terminal, as JSON, or as a self-contained HTML report.

Two data sources, two subcommands:

- **`summary` / `usage` / `cost`** — the official [Usage and Cost Admin API](https://platform.claude.com/docs/en/manage-claude/usage-cost-api).
  Authoritative billing data, but needs an **Admin API key** (org-only — see
  Requirements below).
- **`code`** — reads Claude Code's own local session logs
  (`~/.claude/projects/**/*.jsonl`) and estimates spend from published
  per-model pricing. **No credentials needed at all** — use this if you
  don't have (or can't get) an Admin API key, or if what you actually want
  is your local `claude` CLI usage rather than org-wide API billing.

Neither shows Claude.ai Pro/Max subscription usage — Anthropic doesn't
publish an API for that.

## Requirements

- For `summary` / `usage` / `cost`: an **Admin API key** for your
  organization (starts with `sk-ant-admin...`). This is different from a
  normal API key — only an org admin can create one, from the
  [Claude Console](https://platform.claude.com/) under
  **Settings → Organization → Admin keys**. See
  [Create an Admin API key](https://platform.claude.com/docs/en/manage-claude/admin-api-keys)
  if you don't have one yet. (The Admin API is unavailable for individual /
  non-organization accounts — you can self-serve one by setting up an
  organization under Console → Settings → Organization, or just use `code`
  below instead.)
- For `code`: nothing — it only reads files already on disk.

## Build

```sh
cargo build --release
# binary at target/release/claude-usage
```

## Usage

Set your key once:

```sh
export ANTHROPIC_ADMIN_KEY=sk-ant-admin-...
```

Show the last 7 days (default) as a terminal summary:

```sh
claude-usage
```

Pick a different range:

```sh
claude-usage --days 30
claude-usage --since 2026-08-01 --until 2026-08-15
```

Write an HTML report (dark-mode aware, no external dependencies — safe to
email or open offline). It opens automatically in your default browser.
Omit the filename (or pass `-`) to get one generated from a random UUID:

```sh
claude-usage --days 30 --html report.html
claude-usage --days 30 --html
```

Machine-readable output, for piping into other tools:

```sh
claude-usage --json | jq '.total_spend_dollars'
```

### Raw reports

For finer-grained access to the underlying API, `usage` and `cost` expose the
raw, groupable reports directly:

```sh
# Token usage grouped by workspace, hourly
claude-usage usage --group-by workspace_id --bucket-width 1h --days 1

# Cost grouped by workspace
claude-usage cost --group-by workspace_id,description --days 30 --json
```

## What "spend" means here

Cost figures for `summary`/`usage`/`cost` come straight from the Cost Report
API (`amount` is USD, already converted from the API's cents-denominated
decimal string) — this is real billing data. Priority Tier usage is billed
differently and isn't included in the cost endpoint — track it via
`usage --group-by service_tier` instead, per Anthropic's docs.

## `code` — no API key needed

If you don't have an Admin API key (individual account, no org), or you just
want to know how much your local Claude Code usage is costing you:

```sh
claude-usage code                              # last 7 days, all projects
claude-usage code --days 30 --html report.html
```

This reads the JSONL session transcripts Claude Code already writes to
`~/.claude/projects/` (override with `--claude-dir` or `$CLAUDE_CONFIG_DIR`)
and estimates cost from Anthropic's published per-model pricing — the same
approach community tools like `ccusage` use. It needs no credentials because
it never talks to the network.

This is an **estimate**, not a bill: it uses list pricing (no negotiated
discounts), and a model released after this tool was built will show up with
its tokens counted but no dollar figure (flagged in the output) until the
pricing table is updated. For authoritative numbers, use `summary` instead.

The report additionally breaks spend down **by project** (the working
directory Claude Code was run from), which the Admin API has no concept of.

## Notes

- Data typically lands within ~5 minutes of a request completing.
- Ranges longer than the API's per-request bucket limit (31 daily buckets)
  are paginated automatically — ask for as wide a range as you like.
- Only `summary` supports `--html`; `usage`/`cost` are table/JSON only.
