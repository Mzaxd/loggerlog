---
name: loggerlog
description: >
  Use loggerlog to search, tail, and analyze local log files. Trigger this skill
  whenever the user asks to search logs, find errors, check what happened in
  their app, tail live logs, debug from log output, investigate exceptions,
  analyze log patterns, or query indexed log data. loggerlog is a local CLI
  tool backed by SQLite FTS5 — it is NOT a remote log service, so use it for
  any log investigation task where the user has log files on disk.
---

# LoggerLog — Local Log Search & Analysis

loggerlog is a lightweight CLI log search tool built on SQLite FTS5, distributed as a single Rust binary (`loggerlog` or `loggerlog.exe`). It provides built-in indexing, full-text search, real-time tail, and multi-project management.

## Mental Model

```
loggerlog config add-dir <dir>   →  Register log directories
loggerlog index update           →  Scan files → Parse → Write to SQLite FTS5 index
loggerlog search "query"         →  Execute FTS5 full-text search + filters on the index
loggerlog tail <source>          →  Follow file in real time (bypasses index, reads file directly)
```

Log formats are **auto-detected** (log4j/logback patterns, JSON structured, plain text) — no manual configuration needed. Encoding is also auto-detected (UTF-8/GBK/etc.), and `.gz` compressed files are supported.

## Command Reference

### `search` — Search Indexed Logs

```bash
loggerlog search [OPTIONS] [QUERY]
```

| Option | Description |
|---|---|
| `-l, --level` | Filter by log level; supports comma-separated or multiple flags (`-l ERROR,WARN` or `-l ERROR -l WARN`) |
| `-s, --source` | Filter by source filename glob |
| `--project` | Filter by project name |
| `--module` | Filter by module name (subdirectory) |
| `--after` | Only logs after this time |
| `--before` | Only logs before this time |
| `--thread` | Filter by thread name |
| `--regex` | Use regex instead of FTS5 |
| `-n, --limit` | Max results to return (default 100) |
| `-o, --output` | Output format: `table` / `json` / `compact` / `raw` |
| `-C, --context` | Show N lines of context before and after each match |
| `--max-chars` | Hard-truncate output at N characters (UTF-8 safe) |
| `--exclude` | Exclude lines containing this keyword (can specify multiple) |
| `--unique` | Deduplicate by level + message prefix, annotate repeat count |
| `--output-file` | Write full results to file, show only stats in terminal |
| `-S, --summary` | Summary mode: show only level distribution, top messages, source stats |
| `--no-sync` | Skip auto incremental indexing before search (faster for large datasets) |

#### Inline Query Syntax (Important!)

Filters can be embedded directly in the search string, space-separated:

| Syntax | Description | Example |
|---|---|---|
| `level=ERROR,WARN` | Multiple levels, comma-separated (uppercase) | `level=ERROR` |
| `after=1h-ago` | Relative time | `after=30m-ago` |
| `after=2024-01-15` | Absolute date | `after=2024-01-15` |
| `after=2024-01-15T10:30:00Z` | RFC3339 timestamp | — |
| `after="2024-01-15 10:30:00"` | Datetime | — |
| `before=...` | Same three formats as above | `before=1h-ago` |
| `source=app.log` | By filename | `source=server.log` |
| `project=myapp` | By project name | `project=backend` |
| `module=auth` | By module name | `module=api` |
| `thread=http-nio` | By thread name | `thread=main` |
| `logger=com.example.X` | By logger name | `logger=com.example.Controller` |
| `exclude=heartbeat` | Exclude lines with this keyword | `exclude=health` |
| `regex:Exception\s+in` | Regex search prefix | `regex:NullPointer` |
| Remaining text | Used as FTS5 full-text search terms | `timeout connection` |

Time expressions supported: `15m-ago`, `2h-ago`, `3d-ago`, `1w-ago`.

Inline syntax and CLI flags can be combined — they stack with AND logic.

### `tail` — Real-time Log Following

```bash
loggerlog tail [SOURCE] [OPTIONS]
```

| Option | Description |
|---|---|
| `-l, --level` | Filter by log level |
| `-f, --filter` | FTS keyword filter |
| `-o, --output` | Output format (default raw) |

SOURCE can be a file or directory (monitors all log files under the directory). When SOURCE is omitted, monitors all configured log sources.

### `config` — Manage Configuration

```bash
loggerlog config show                          # Display current config
loggerlog config edit                          # Open config in $EDITOR
loggerlog config add-dir <PATH>                # Add log directory
loggerlog config add-dir <PATH> --encoding gbk # Specify encoding
loggerlog config remove-dir <PATH>             # Remove log directory
```

Config is saved to `~/.config/LoggerLog/config.toml` by default.

### `index` — Manage Index

```bash
loggerlog index update    # Incremental index (only new/modified files)
loggerlog index rebuild   # Full index rebuild
loggerlog index compact   # Optimize FTS index
loggerlog index stats     # View index statistics (file count / entry count / size)
```

Incremental sync runs automatically before each search (unless `--no-sync` is specified).

### `project` — Multi-Project Management

```bash
loggerlog project add <NAME> <PATH>             # Register project
loggerlog project add <NAME> <PATH> --recursive # Recursive scan
loggerlog project list                           # List all projects and their modules
loggerlog project remove <NAME>                  # Remove project
```

Subdirectories under a project root are auto-identified as **modules** — filterable with `--module` at search time.

## Best Practices for AI Agents

You are an AI agent; your goal is to help users troubleshoot efficiently. loggerlog is optimized for this:

### Scenario 1: Exploratory Query → Summary First, Then Drill Down

When the user says "check for recent errors," **do not** just `search "error"` and dump 100 log lines. Get the big picture first:

```bash
loggerlog search "after=30m-ago" --summary
```

This outputs level distribution, top frequent error messages, and source statistics. If you see errors concentrated in the `auth` module, drill down:

```bash
loggerlog search "level=ERROR module=auth after=30m-ago" -o compact
```

### Scenario 2: Precise Troubleshooting → Compact + Truncation + Noise Exclusion

When the user describes a specific error and you need to find it:

```bash
loggerlog search "NullPointerException" -C 3 --max-chars 5000 --exclude heartbeat --exclude healthcheck -o compact
```

- `-o compact`: One line per entry, zero decoration, maximum token efficiency
- `--max-chars 5000`: Hard truncation to prevent context explosion
- `--exclude`: Remove known noise (heartbeat, health check, etc.)
- `-C 3`: Show 3 lines of context before/after to understand root cause

### Scenario 3: Large-scale Export → JSON + File

When you need to export a large volume of logs for offline analysis:

```bash
loggerlog search "error" -o json --output-file errors.json
```

Terminal shows only a brief summary; full JSON is written to file. Read the file afterwards for analysis.

### Scenario 4: Periodic Health Check

When the user asks "check if anything unusual happened recently":

```bash
loggerlog search "level=ERROR,WARN after=1h-ago" --summary
```

Start with the summary. If the ERROR count is higher than expected, add `--unique` to find frequent errors:

```bash
loggerlog search "level=ERROR after=1h-ago" --unique -o compact
```

### Scenario 5: Context Analysis

After finding an anomaly, you need to understand the surrounding context:

```bash
loggerlog search "connection timeout" -C 5 -o compact
```

loggerlog auto-folds Java (`\tat`/`Caused by:`), Python (`File "..."`), and Go (`goroutine`) stack traces to prevent stack lines from consuming too much context.

### Scenario 6: Real-time Monitoring

When the user says "watch what the app is outputting right now":

```bash
loggerlog tail --filter "ERROR" -l WARN
```

If you know the specific file:

```bash
loggerlog tail /var/log/app.log --filter "ERROR"
```

## First-Time Setup

If the user hasn't configured loggerlog yet, guide them in this order:

```
1. cargo install loggerlog                      # Install (requires Rust toolchain)
2. loggerlog config add-dir /path/to/logs     # Register log directory
3. loggerlog index update                      # Build index
4. loggerlog search "error"                    # Start searching
```

For logs distributed across multiple projects:

```
1. loggerlog project add backend /path/to/backend/logs
2. loggerlog project add frontend /path/to/frontend/logs
3. loggerlog index update
4. loggerlog search --project backend --module auth "error"
```

## Output Format Guide

| Scenario | Recommended Format | Reason |
|---|---|---|
| AI agent consumption | `-o compact` | One line per entry, fewest tokens |
| Programmatic processing | `-o json` | Structured and parseable |
| Human reading | `-o table` (default) | Aligned table, easy to scan |
| Preserve raw format | `-o raw` | Original output, no processing |

## Important Notes

- **Auto-sync before search**: Every `search` runs an automatic `index update` (incremental) to keep the index in sync with files. Use `--no-sync` to skip for speed.
- **Stack trace folding**: Consecutive stack lines are auto-folded into `[... N stack lines]` to save context.
- **Encoding auto-detection**: No need to manually specify UTF-8/GBK — chardetng handles it automatically.
- **Log rotation**: Auto-detects `.1`, `.gz`, and other rotated files to avoid duplicate indexing.
- **Database location**: The SQLite index file is stored in the user config directory, not in the log directory.
