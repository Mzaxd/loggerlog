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

# LoggerLog — 本地日志搜索与分析

loggerlog 是基于 SQLite FTS5 的轻量级 CLI 日志搜索工具，单个 Rust 二进制文件 (`loggerlog` 或 `loggerlog.exe`)。内置索引、全文搜索、实时 tail、多项目管理等能力。

## 架构心智模型

```
loggerlog config add-dir <dir>   →  注册日志目录
loggerlog index update           →  扫描文件 → 解析 → 写入 SQLite FTS5 索引
loggerlog search "query"         →  对索引执行 FTS5 全文搜索 + 过滤器
loggerlog tail <source>          →  实时跟随文件 (旁路索引，直接读文件)
```

日志格式会**自动检测**（log4j/logback pattern、JSON 结构化、纯文本），无需手动指定。编码也会自动检测（UTF-8/GBK 等），支持 `.gz` 压缩文件。

## 子命令速查

### `search` — 搜索已索引日志

```bash
loggerlog search [OPTIONS] [QUERY]
```

| 选项 | 说明 |
|---|---|
| `-l, --level` | 按级别过滤，可多次指定 (如 `-l ERROR -l WARN`) |
| `-s, --source` | 按源文件名 glob 过滤 |
| `--project` | 按项目名过滤 |
| `--module` | 按模块名 (子目录) 过滤 |
| `--after` | 只查此时间之后 |
| `--before` | 只查此时间之前 |
| `--thread` | 按线程名过滤 |
| `--regex` | 使用正则而非 FTS5 |
| `-n, --limit` | 最多返回结果数 (默认 100) |
| `-o, --output` | 输出格式: `table` / `json` / `compact` / `raw` |
| `-C, --context` | 展示匹配行前后 N 行上下文 |
| `--max-chars` | 硬截断输出到指定字符数 (UTF-8 安全) |
| `--exclude` | 排除含此关键字的行 (可多次指定) |
| `--unique` | 按级别+消息前缀去重，标注重复次数 |
| `--output-file` | 完整结果写入文件，终端只显示统计摘要 |
| `-S, --summary` | 摘要模式：仅显示级别分布、Top 消息、来源统计 |
| `--no-sync` | 跳过搜索前自动增量索引 (大数据集时加速) |

#### 查询内联语法 (重要!)

搜索字符串中可以直接嵌入过滤器，用空格分隔：

| 语法 | 说明 | 示例 |
|---|---|---|
| `level=ERROR,WARN` | 多级别逗号分隔 (大写) | `level=ERROR` |
| `after=1h-ago` | 相对时间 | `after=30m-ago` |
| `after=2024-01-15` | 绝对日期 | `after=2024-01-15` |
| `after=2024-01-15T10:30:00Z` | RFC3339 时间戳 | — |
| `after="2024-01-15 10:30:00"` | 日期时间 | — |
| `before=...` | 同上三种格式 | `before=1h-ago` |
| `source=app.log` | 按文件名 | `source=server.log` |
| `project=myapp` | 按项目名 | `project=backend` |
| `module=auth` | 按模块名 | `module=api` |
| `thread=http-nio` | 按线程名 | `thread=main` |
| `logger=com.example.X` | 按日志器名 | `logger=com.example.Controller` |
| `exclude=heartbeat` | 排除含此关键字的行 | `exclude=health` |
| `regex:Exception\s+in` | 正则搜索前缀 | `regex:NullPointer` |
| 其余文字 | 作为 FTS5 全文搜索词 | `timeout connection` |

时间表达式支持: `15m-ago`, `2h-ago`, `3d-ago`, `1w-ago`。

内联语法和 CLI 选项可以同时使用，效果叠加 (AND 逻辑)。

### `tail` — 实时跟随日志

```bash
loggerlog tail [SOURCE] [OPTIONS]
```

| 选项 | 说明 |
|---|---|
| `-l, --level` | 按级别过滤 |
| `-f, --filter` | FTS 关键字过滤 |
| `-o, --output` | 输出格式 (默认 raw) |

SOURCE 可以是文件或目录 (监控目录下所有日志文件)。不指定 SOURCE 时，监控所有已配置的日志源。

### `config` — 管理配置

```bash
loggerlog config show                          # 显示当前配置
loggerlog config edit                          # 用 $EDITOR 打开配置
loggerlog config add-dir <PATH>                # 添加日志目录
loggerlog config add-dir <PATH> --encoding gbk # 指定编码
loggerlog config remove-dir <PATH>             # 移除日志目录
```

配置默认保存到 `~/.config/LoggerLog/config.toml`。

### `index` — 管理索引

```bash
loggerlog index update    # 增量索引 (只处理新/修改的文件)
loggerlog index rebuild   # 全量重建索引
loggerlog index compact   # 优化 FTS 索引
loggerlog index stats     # 查看索引统计 (文件数/条目数/大小)
```

搜索前会自动执行增量同步 (除非加 `--no-sync`)。

### `project` — 管理多项目

```bash
loggerlog project add <NAME> <PATH>             # 注册项目
loggerlog project add <NAME> <PATH> --recursive # 递归扫描
loggerlog project list                           # 列出所有项目及其模块
loggerlog project remove <NAME>                  # 移除项目
```

项目根目录下的子目录自动识别为**模块** (module)，搜索时可用 `--module` 过滤。

## AI Agent 最佳实践

你是 AI agent，目标是高效地帮用户排障。loggerlog 为此做了很多优化：

### 场景 1: 探索性查询 → 先看摘要，再深入

用户说 "帮我看看最近有什么错误" 时，**不要**直接 `search "error"` 灌 100 条日志。先看全局：

```bash
loggerlog search --summary -t 30m
```

这会输出级别分布、Top 高频错误消息、来源统计。如果发现主要集中在 `auth` 模块，再精确查询：

```bash
loggerlog search "level=ERROR module=auth after=30m-ago" -o compact
```

### 场景 2: 精确排障 → compact + 截断 + 排除噪音

当用户描述了具体错误，你需要精确查找时：

```bash
loggerlog search "NullPointerException" -C 3 --max-chars 5000 --exclude heartbeat --exclude healthcheck -o compact
```

- `-o compact`: 一行一条，零装饰，token 效率最高
- `--max-chars 5000`: 硬截断，防止上下文爆炸
- `--exclude`: 排除已知噪音 (heartbeat、health check 等)
- `-C 3`: 展示前后 3 行上下文，理解错误根因

### 场景 3: 大规模导出 → JSON + 文件

需要导出大量日志做离线分析时：

```bash
loggerlog search "error" -o json --output-file errors.json
```

终端只显示简短统计，完整 JSON 写入文件。后续可以 `Read` 文件内容进行分析。

### 场景 4: 定期巡检

用户让你 "检查一下最近有没有异常"：

```bash
loggerlog search "level=ERROR,WARN after=1h-ago" --summary
```

先看摘要。如果 ERROR 数量超过预期，再加 `--unique` 去重找高频错误：

```bash
loggerlog search "level=ERROR after=1h-ago" --unique -o compact
```

### 场景 5: 上下文分析

发现一条异常日志后，需要理解上下文：

```bash
loggerlog search "connection timeout" -C 5 -o compact
```

loggerlog 会自动折叠 Java (`\tat`/`Caused by:`)、Python (`File "..."`)、Go (`goroutine`) 的异常堆栈，避免堆栈行占用过多上下文。

### 场景 6: 实时监控

用户说 "帮我看一下 app 现在在输出什么"：

```bash
loggerlog tail --filter "ERROR" -l WARN
```

如果知道具体文件：

```bash
loggerlog tail /var/log/app.log --filter "ERROR"
```

## 初次使用三步曲

如果用户还没有配置过 loggerlog，按以下顺序引导：

```
1. loggerlog config add-dir /path/to/logs     # 注册日志目录
2. loggerlog index update                      # 构建索引
3. loggerlog search "error"                    # 开始搜索
```

如果日志分布在多个项目中：

```
1. loggerlog project add backend /path/to/backend/logs
2. loggerlog project add frontend /path/to/frontend/logs
3. loggerlog index update
4. loggerlog search --project backend --module auth "error"
```

## 输出格式选择指南

| 场景 | 推荐格式 | 原因 |
|---|---|---|
| AI agent 消费 | `-o compact` | 一行一条，token 最省 |
| 程序化处理 | `-o json` | 结构化，可解析 |
| 人类阅读 | `-o table` (默认) | 表格对齐，易于扫读 |
| 保留原始格式 | `-o raw` | 原样输出，不做处理 |

## 重要注意事项

- **搜索前自动同步**: 每次 `search` 会自动执行 `index update` (增量)，确保索引不落后于文件。可以用 `--no-skip` 跳过以加速。
- **异常堆栈折叠**: 连续堆栈行会被自动折叠成 `[... N stack lines]`，节省上下文。
- **编码自动检测**: 无需手动指定 UTF-8/GBK，chardetng 自动识别。
- **日志轮转**: 自动识别 `.1`、`.gz` 等轮转文件，不会重复索引。
- **数据库位置**: SQLite 索引文件存放在用户配置目录，而非日志目录。
