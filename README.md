# LoggerLog

A lightweight CLI log search tool. Powered by SQLite FTS5 local indexing with zero external dependencies and millisecond-level full-text search.

Designed for AI agents (Claude Code, etc.) and daily development troubleshooting.

[中文文档](README.zh-CN.md)

## Features

### Core Engine
- 🔍 **Full-text search** — SQLite FTS5 indexing with Chinese tokenization support and millisecond response times
- 📊 **Multi-format auto-detection** — log4j / logback / JSON structured / plain text, auto-detected with a 50-line sample
- 📁 **Real-time tail** — Follow log files in real time with level and keyword filtering
- 🌍 **Encoding auto-detection** — chardetng + encoding_rs for UTF-8 / GBK / common encodings
- 🔧 **Incremental indexing** — Indexes new content by `byte_offset`, never reprocesses existing data
- 📦 **Multi-project management** — Register multiple project root directories, auto-match files to projects via longest prefix, subdirectories become modules

### AI Agent Friendly
- `--compact` — One line per entry, zero decoration, minimized token consumption
- `--summary -S` — See the summary first (level distribution, source stats, top frequent messages, time range) instead of flooding context
- `--max-chars N` — Hard truncation, UTF-8 safe, prevents context explosion
- `--unique` — Deduplicate by level + message prefix, annotate repeat counts
- `--exclude` — Exclude known noise (e.g. heartbeat, health check)
- `-o json` — Structured JSON output for programmatic consumption by AI agents

### Log Analysis
- 🔗 **Context expansion `-C N`** — Show N lines before and after each match to understand error root causes
- 📐 **Stack trace folding** — Auto-detect and fold consecutive stack lines for Java (`\tat`/`Caused by:`), Python (`File "..."`), and Go (`goroutine`)
- 📈 **Level statistics header** — Table output auto-displays per-level distribution
- `--regex` — Regular expression search for complex pattern matching
- `--output-file <FILE>` — Write full logs to file, show only statistics in terminal

### Search Filters
- **FTS5 full-text search** — Space-separated keywords
- **Level filter** — `level=ERROR` / `--level ERROR,WARN`
- **Time range** — `after=1h-ago` / `after=2024-01-15` / `before=30m-ago`
- **Source filter** — `source=app.log` / `--source app.log`
- **Project/module** — `project=myapp` / `module=auth-service`
- **Thread/logger** — `thread=http-nio` / `logger=com.example.Controller`
- **Regex** — `regex:Exception\s+in\s+thread` / `--regex`
- **Exclude** — `exclude=health` / `--exclude heartbeat`

## Quick Start

```bash
cargo build --release

# Add log directories
loggerlog config add-dir /path/to/logs

# Build index
loggerlog index update

# Basic search
loggerlog search "error timeout"
loggerlog search "level=ERROR" -n 50
loggerlog search "level=ERROR,WARN after=1h-ago"

# Compact format (recommended for AI agents)
loggerlog search "error" -o compact

# Summary mode (overview first)
loggerlog search "after=30m-ago" --summary

# JSON output (for AI agent consumption)
loggerlog search "error" -o json
```

## Command Reference

### `search` — Search Logs

```
loggerlog search [OPTIONS] [QUERY]
```

| Option | Description |
|--------|-------------|
| `-l, --level` | Filter by log level (can specify multiple) |
| `-s, --source` | Filter by source file |
| `--project` | Filter by project name |
| `--module` | Filter by module name (subdirectory) |
| `--after` | Only logs after this time |
| `--before` | Only logs before this time |
| `--thread` | Filter by thread name |
| `--regex` | Use regex search |
| `-n, --limit` | Max results (default 100) |
| `-o, --output` | Output format: `table` / `json` / `raw` / `compact` (default table) |
| `-C, --context` | Context lines around matches |
| `--max-chars` | Output truncation character limit |
| `--exclude` | Exclude lines containing this keyword (can specify multiple) |
| `--unique` | Deduplicate by level + message prefix |
| `--output-file` | Write to file instead of stdout |
| `-S, --summary` | Summary mode (no individual log lines) |
| `--no-sync` | Skip auto incremental indexing before search |

### Inline Query Syntax

Filters can be embedded directly in the search string (space-separated):

```
level=ERROR,WARN       By level (comma-separated, uppercase)
after=1h-ago           Relative time
after=2024-01-15T10:30:00Z  Absolute time (RFC3339)
after=2024-01-15       Absolute date
after="2024-01-15 10:30:00"  Absolute datetime
before=...             Same as above
source=app.log         By filename
project=myapp          By project name
module=auth            By module name
thread=http-nio        By thread name
logger=com.example.X   By logger name
exclude=heartbeat      Exclude lines with this keyword
regex:Exception\s+in  Regex search prefix
error timeout          Remaining text as FTS full-text query
```

Example:
```bash
loggerlog search "level=ERROR project=myapp module=auth after=1h-ago NullPointerException"
```

### `tail` — Real-time Follow

```bash
loggerlog tail /var/log/app.log --filter "ERROR"
loggerlog tail --level WARN --filter "timeout"
```

### `config` — Manage Configuration

```bash
loggerlog config show
loggerlog config add-dir /path/to/logs
loggerlog config remove-dir /path/to/logs
```

### `index` — Manage Index

```bash
loggerlog index update     # Incremental index
loggerlog index rebuild    # Full rebuild
loggerlog index compact    # Optimize FTS index
loggerlog index stats      # View statistics
```

### `project` — Manage Projects

```bash
loggerlog project add myapp "/path/to/project/logs"
loggerlog project list
loggerlog project remove myapp
```

## Output Formats

### table (default)
```
TIMESTAMP            LEVEL  SOURCE      LINE  MESSAGE
2024-01-15 10:00:30  WARN   /app.log    142   connection timeout after...
2024-01-15 10:00:00  ERROR  /app.log    89    NullPointerException: ...

Showing 2 of 2 results (1.2ms)
```

### compact (recommended for AI agents)
```
[2024-01-15T10:00:30Z] [WARN] [/app.log:142] connection timeout after 5s
[2024-01-15T10:00:00Z] [ERROR] [/app.log:89] NullPointerException: something broke

Showing 2 of 2 results (1.2ms)
```

### json
```json
{
  "total_count": 2,
  "returned_count": 2,
  "offset": 0,
  "elapsed_ms": 1200,
  "results": [...]
}
```

### raw
```
2024-01-15 10:00:30 WARN [worker-1] connection timeout after 5s
2024-01-15 10:00:00 ERROR [main] NullPointerException: something broke
```

## Best Practices for AI Agents

### Scenario 1: Exploratory Query → Summary First

```bash
loggerlog search --summary -t 30m
```

Outputs level distribution, top frequent error messages, and source statistics — prevents flooding context with low-quality queries.

### Scenario 2: Precise Troubleshooting → Compact + Truncation

```bash
loggerlog search "NullPointerException" -C 3 --max-chars 5000 --exclude health --exclude heartbeat -o compact
```

### Scenario 3: Large-scale Export → Write to File

```bash
loggerlog search "error" -o json --output-file errors.json
```

### Scenario 4: Periodic Health Check

```bash
loggerlog search "level=ERROR,WARN after=1h-ago" --summary
```

## Development

```bash
cargo build            # Debug build
cargo build --release  # Release build
cargo test             # Run 158 tests
cargo run -- --help    # View CLI help
```

## Test Coverage

| Module | Tests | Coverage |
|--------|-------|----------|
| `engine.rs` | 37 | parse_duration/relative_time/take_value, parse_query_string all filter combos, build_where_clause, search/search_regex with FTS/level/exclude/limit/offset/file_id, search_summary, level_stats, get_context |
| `index.rs` | 21 | Table creation/migration, files CRUD, entries CRUD, FTS trigger sync, projects CRUD, sync_projects, modules, normalize_path/is_subpath, compact/clear_all/db_size |
| `scanner.rs` | 47 | extract_level/timestamp/message/thread/logger all formats + JSON, detect_format_hint, benchmarks |
| `search.rs` | 30 | collapse_multiline, apply_max_chars, deduplicate_results, is_stack_line multi-language, extract_exception_class, fold_stack_traces, shorten_path, truncate |
| `discovery.rs` | 1 | Filename analysis |

## Architecture

```
src/
├── cli/                    # CLI layer (clap derive)
│   ├── mod.rs              # Cli struct + subcommand definitions
│   └── commands/
│       ├── search.rs       # search subcommand + all output formats
│       ├── tail.rs         # tail subcommand
│       ├── config.rs       # config subcommand
│       ├── index.rs        # index subcommand
│       └── project.rs      # project subcommand
├── core/                   # Core library (zero clap dependency)
│   ├── engine.rs           # SearchEngine + query building + summary + stats
│   ├── index.rs            # IndexManager + SQLite schema + incremental indexing
│   ├── entry.rs            # Data models (LogEntry, SearchResult, SearchQuery)
│   ├── scanner.rs          # Log field extraction + format auto-detection (log4j/logback/JSON/plain)
│   ├── discovery.rs        # FileDiscovery + rotation detection
│   ├── encoding.rs         # chardetng encoding detection
│   ├── watcher.rs          # notify file watcher
│   └── config.rs           # TOML configuration management
└── main.rs                 # Entry point
```

## Tech Stack

| Layer | Technology | Version |
|------|------------|---------|
| Language | Rust | stable |
| Database | SQLite FTS5 | rusqlite 0.32 (bundled) |
| CLI | clap | 4 (derive) |
| Time | chrono | 0.4 |
| Serialization | serde / serde_json | 1 |
| Tables | comfy-table | 7 |
| Regex | regex | 1 |
| Encoding | chardetng + encoding_rs | — |
| File watching | notify + debouncer-mini | — |

## License

MIT
