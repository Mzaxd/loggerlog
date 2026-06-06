# LoggerLog

轻量级 CLI 日志搜索工具。基于 SQLite FTS5 本地索引，零外部依赖，毫秒级全文搜索。

专为 AI Agent（Claude Code 等）和日常开发排障场景设计。

[English](README.md)

## 特性

### 核心引擎
- 🔍 **全文搜索** — SQLite FTS5 索引，支持中文分词，毫秒级响应
- 📊 **多格式自动检测** — log4j / logback / JSON 结构化 / 纯文本，采样 50 行自动识别
- 📁 **实时 tail** — 跟随日志文件实时输出，支持级别和关键字过滤
- 🌍 **编码自动检测** — chardetng + encoding_rs，支持 UTF-8 / GBK / 常见编码
- 🔧 **增量索引** — 按 byte_offset 增量索引新内容，不重复处理已有数据
- 📦 **多项目管理** — 注册多个项目根目录，自动按最长前缀匹配文件到项目，子目录识别为模块

### AI Agent 友好
- `--compact` — 一行一条，零装饰，最小化 token 消耗
- `--summary -S` — 不灌日志，先看摘要（级别分布、来源统计、Top 高频消息、时间范围）
- `--max-chars N` — 硬截断，UTF-8 安全，防止上下文爆炸
- `--unique` — 按级别+消息前缀去重，标注重复次数
- `--exclude` — 排除已知噪音日志（如 heartbeat、health check）
- `-o json` — 结构化 JSON 输出，供 AI Agent 程序化消费

### 日志分析增强
- 🔗 **上下文展开 `-C N`** — 展示匹配行前后各 N 行，理解错误根因
- 📐 **异常堆栈折叠** — 自动检测并折叠 Java (`\tat`/`Caused by:`)、Python (`File "..."`)、Go (`goroutine`) 的连续堆栈行
- 📈 **级别统计头部** — Table 输出自动显示各级别分布
- `--regex` — 正则搜索，支持复杂模式匹配
- `--output-file <FILE>` — 完整日志写入文件，终端只显示统计

### 搜索过滤
- **FTS5 全文搜索** — 空格分隔多关键字
- **级别过滤** — `level=ERROR` / `--level ERROR,WARN`
- **时间范围** — `after=1h-ago` / `after=2024-01-15` / `before=30m-ago`
- **来源过滤** — `source=app.log` / `--source app.log`
- **项目/模块** — `project=myapp` / `module=auth-service`
- **线程/日志器** — `thread=http-nio` / `logger=com.example.Controller`
- **正则** — `regex:Exception\s+in\s+thread` / `--regex`
- **排除** — `exclude=health` / `--exclude heartbeat`

## 快速开始

```bash
cargo build --release

# 添加日志目录
loggerlog config add-dir /path/to/logs

# 构建索引
loggerlog index update

# 基本搜索
loggerlog search "error timeout"
loggerlog search "level=ERROR" -n 50
loggerlog search "level=ERROR,WARN after=1h-ago"

# Compact 格式（AI agent 推荐）
loggerlog search "error" -o compact

# 摘要模式（先看全局）
loggerlog search "after=30m-ago" --summary

# JSON 输出（AI agent 消费）
loggerlog search "error" -o json
```

## 命令参考

### `search` — 搜索日志

```
loggerlog search [OPTIONS] [QUERY]
```

| 选项 | 说明 |
|------|------|
| `-l, --level` | 按级别过滤（可多次指定） |
| `-s, --source` | 按源文件过滤 |
| `--project` | 按项目名过滤 |
| `--module` | 按模块名（子目录）过滤 |
| `--after` | 只查此时间之后的日志 |
| `--before` | 只查此时间之前的日志 |
| `--thread` | 按线程名过滤 |
| `--regex` | 使用正则搜索 |
| `-n, --limit` | 最大结果数（默认 100） |
| `-o, --output` | 输出格式：`table` / `json` / `raw` / `compact`（默认 table） |
| `-C, --context` | 上下文行数 |
| `--max-chars` | 输出截断字符数 |
| `--exclude` | 排除含此关键字的行（可多次指定） |
| `--unique` | 按级别+消息前缀去重 |
| `--output-file` | 写入文件而非 stdout |
| `-S, --summary` | 摘要模式（不输出单条日志） |
| `--no-sync` | 跳过搜索前自动增量索引 |

### 查询内联语法

搜索字符串中可直接嵌入过滤器（空格分隔）：

```
level=ERROR,WARN       按级别（逗号分隔，大写）
after=1h-ago           相对时间
after=2024-01-15T10:30:00Z  绝对时间（RFC3339）
after=2024-01-15       绝对日期
after="2024-01-15 10:30:00"  绝对日期时间
before=...             同上
source=app.log         按文件名
project=myapp          按项目名
module=auth            按模块名
thread=http-nio        按线程名
logger=com.example.X   按日志器名
exclude=heartbeat      排除含此关键字的行
regex:Exception\s+in  正则搜索前缀
error timeout          剩余部分作为 FTS 全文查询
```

示例：
```bash
loggerlog search "level=ERROR project=myapp module=auth after=1h-ago NullPointerException"
```

### `tail` — 实时跟踪

```bash
loggerlog tail /var/log/app.log --filter "ERROR"
loggerlog tail --level WARN --filter "timeout"
```

| 选项 | 说明 |
|------|------|
| `-l, --level` | 按级别过滤 |
| `-f, --filter` | FTS 关键字过滤 |
| `-o, --output` | 输出格式（默认 raw） |

### `config` — 管理配置

```bash
loggerlog config show
loggerlog config edit
loggerlog config add-dir /path/to/logs
loggerlog config remove-dir /path/to/logs
```

### `index` — 管理索引

```bash
loggerlog index update     # 增量索引
loggerlog index rebuild    # 全量重建
loggerlog index compact    # 优化 FTS 索引
loggerlog index stats      # 查看统计
```

### `project` — 管理项目

```bash
loggerlog project add myapp "/path/to/project/logs"
loggerlog project add myapp "/path/to/project/logs" --recursive
loggerlog project list
loggerlog project remove myapp
```

## 输出格式

### table（默认）
```
TIMESTAMP            LEVEL  SOURCE      LINE  MESSAGE
2024-01-15 10:00:30  WARN   /app.log    142   connection timeout after...
2024-01-15 10:00:00  ERROR  /app.log    89    NullPointerException: ...

Showing 2 of 2 results (1.2ms)
```

### compact（AI Agent 推荐）
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

## AI Agent 最佳实践

### 场景 1：探索性查询 → 先看摘要

```bash
loggerlog search "after=30m-ago" --summary
```

输出级别分布、Top 高频错误消息、来源统计，防止低质量查询灌满上下文。

### 场景 2：精确排障 → 紧凑 + 截断

```bash
loggerlog search "NullPointerException" -C 3 --max-chars 5000 --exclude health --exclude heartbeat -o compact
```

### 场景 3：大规模导出 → 写文件

```bash
loggerlog search "error" -o json --output-file errors.json
```

### 场景 4：定期巡检

```bash
loggerlog search "level=ERROR,WARN after=1h-ago" --summary
```

## 开发

```bash
cargo build            # 构建（debug）
cargo build --release  # 构建（release）
cargo test             # 运行 267 个测试
cargo run -- --help    # 查看 CLI 帮助
```

## 测试覆盖

| 模块 | 测试数 | 覆盖内容 |
|------|--------|----------|
| `engine.rs` | 60 | parse_duration/relative_time/take_value, parse_query_string 全部过滤器组合, build_where_clause, search/search_regex 含 FTS/level/exclude/limit/offset/file_id, search_summary, level_stats, get_context |
| `index.rs` | 31 | 建表/迁移, files CRUD, entries CRUD, FTS 触发器同步, projects CRUD, sync_projects, modules, normalize_path/is_subpath, compact/clear_all/db_size |
| `scanner.rs` | 47 | extract_level/timestamp/message/thread/logger 全格式 + JSON, detect_format_hint, 性能测试 |
| `config.rs` | 26 | 配置加载, TOML 解析, 路径规范化, 默认设置 |
| `discovery.rs` | 19 | FileDiscovery, 轮转检测, 文件名分析 |
| `encoding.rs` | 13 | chardetng 编码检测, gzip 解压, UTF-8/GBK/Shift-JIS |
| `cli/commands/search.rs` | 24 | collapse_multiline, apply_max_chars, deduplicate_results, is_stack_line 多语言, extract_exception_class, fold_stack_traces, shorten_path, truncate |
| `entry.rs` | 5 | 数据模型构建, SearchQuery 构建 |
| `cli/commands/index.rs` | 4 | index 子命令参数解析 |
| `watcher.rs` | 2 | 文件监控初始化 |

## 架构

```
src/
├── cli/                    # CLI 层（clap derive）
│   ├── mod.rs              # Cli 结构 + 子命令定义
│   └── commands/
│       ├── search.rs       # search 子命令 + 所有输出格式
│       ├── tail.rs         # tail 子命令
│       ├── config.rs       # config 子命令
│       ├── index.rs        # index 子命令
│       └── project.rs      # project 子命令
├── core/                   # 核心库（零 clap 依赖）
│   ├── engine.rs           # SearchEngine + 查询构建 + 摘要 + 统计
│   ├── index.rs            # IndexManager + SQLite schema + 增量索引
│   ├── entry.rs            # 数据模型（LogEntry, SearchResult, SearchQuery）
│   ├── scanner.rs          # 日志字段提取 + 格式自动检测（log4j/logback/JSON/plain）
│   ├── discovery.rs        # FileDiscovery + 轮转识别
│   ├── encoding.rs         # chardetng 编码检测
│   ├── watcher.rs          # notify 文件监控
│   └── config.rs           # TOML 配置管理
└── main.rs                 # 入口
```

## 技术栈

| 层 | 技术 | 版本 |
|----|------|------|
| 语言 | Rust | stable |
| 数据库 | SQLite FTS5 | rusqlite 0.32 (bundled) |
| CLI | clap | 4 (derive) |
| 时间 | chrono | 0.4 |
| 序列化 | serde / serde_json | 1 |
| 表格 | comfy-table | 7 |
| 正则 | regex | 1 |
| 编码 | chardetng + encoding_rs | — |
| 文件监控 | notify + debouncer-mini | — |
| 文件遍历 | walkdir | — |
| 压缩 | flate2 | — |
| 系统目录 | dirs | — |

## License

MIT
