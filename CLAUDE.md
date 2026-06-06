# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概览

LoggerLog 是轻量级 CLI 日志搜索工具，单个 Rust 二进制（crate 名 `loggerlog`）。核心搜索引擎基于 SQLite FTS5，支持 log4j/logback、JSON 结构化日志和纯文本的自动检测。支持多项目管理，每个项目自动识别子目录为模块。整个代码库是同步的（无 async），并行性仅通过 `rayon` 实现。

## 构建与测试

```bash
rtk cargo build                     # 构建
rtk cargo test                      # 全部测试（~267 个）
rtk cargo test --lib core::scanner  # 单模块测试
rtk cargo test --test integration_test              # 仅集成测试
rtk cargo test -- --ignored test_e2e_high_volume   # 含 #[ignore] 标记的测试
```

`dev-dependencies` 仅有 `rand 0.8`（测试数据生成）。Release profile: `opt-level=3, lto=true`。

## 架构

### 严格分层

```
src/main.rs → src/lib.rs → pub fn run() → cli::run()
                                    ├── cli/      clap derive 子命令 + 输出格式化
                                    └── core/     纯库，零 clap 依赖
```

- **core/** — 所有业务逻辑，`anyhow::Result<T>` 错误传播。可独立作为库使用。
- **cli/** — 薄适配层：解析参数 → 调用 core → 格式化输出。出错 `eprintln! + exit(1)`。

### core 模块职责

- `entry.rs` — 数据模型：`LogEntry`, `SearchResult`, `SearchQuery`, `SearchResultSet`
- `mod.rs` — core 模块根
- `engine.rs` — 查询构建 + FTS5 搜索 + 正则回退 + 摘要统计
- `index.rs` — SQLite CRUD + FTS5 触发器同步 + 项目归属 + `sync_projects()`
- `scanner.rs` — 日志字段提取（level/timestamp/message/thread/logger）+ 格式自动检测
- `discovery.rs` — walkdir 文件遍历 + 日志轮转识别（`.1`, `.gz` 等）
- `config.rs` — TOML 配置管理（`~/.config/LoggerLog/config.toml`）
- `encoding.rs` — chardetng + encoding_rs 编码检测
- `watcher.rs` — notify-debouncer-mini 实时文件监控

### 关键架构决策：内存过滤

这是最容易踩坑的设计。`level`/`timestamp`/`thread`/`logger` 的过滤**不在 SQL 中完成**，而是在 Rust 内存中通过 `scanner::extract_*()` 逐行过滤：

- `build_where_clause()` — 仅处理 FTS、source、project、module、exclude（SQL 层）
- `matches_memory_filters()` — 处理 level/after/before/thread/logger（Rust 层）
- FTS5 查询被包裹在 `"..."` 中，`AND`/`OR`/`NOT`/`*` 等 FTS5 运算符被禁用（整个字符串视为字面短语）
- 扫描上限：`DEFAULT_SCAN_LIMIT = 100_000`（搜索）、`AGGREGATION_SCAN_LIMIT = 500_000`（摘要），超出时 stderr 警告

### SQLite 模式（4 表）

```
files          (id, path, size, modified_at, format, byte_offset, line_count, project_id, ...)
log_entries    (id, file_id, line_number, byte_offset, raw)  ← 仅存原始文本
log_entries_fts(raw)                                          ← FTS5 虚拟表，触发器自动同步
projects       (id, name, path, ...)
```

`log_entries` 表只存 `raw` 列，无结构化列。增量索引通过 `files.byte_offset` 实现。轮转检测：若 `file_size < stored_offset` 则从头重建。`.gz` 文件每次全量重读。

### 项目映射

`sync_projects()` 按最长路径前缀匹配（`ORDER BY length(p.path) DESC`）。模块从文件路径相对项目路径的首个子目录自动推导。

## 测试

测试内联在各自模块的 `#[cfg(test)] mod tests` 中，另有 `tests/integration_test.rs`（~1350 行，36 个端到端测试，使用 `tests/fixtures/` 下的日志文件和 `expected.json` 做数据驱动断言）。

## 依赖

| 关键 crate | 用途 |
|-----------|------|
| rusqlite 0.32 (bundled) | SQLite FTS5，静态链接 |
| clap 4 (derive) | CLI 解析 |
| regex 1 | 字段提取 + 正则搜索 |
| chrono 0.4 | 时间戳解析 |
| rayon 1.10 | 并行索引 |
| serde/serde_json 1 | JSON 输出 + 配置序列化 |
| comfy-table 7 | 表格输出 |
| anyhow 1 | 错误处理 |
| notify 7 + debouncer-mini | 文件监控 |
| walkdir 2 | 文件系统遍历 |
| flate2 1 | gzip 解压缩 |
| dirs 6 | 系统目录获取 |

注意：项目自身不使用任何日志框架（no `tracing`/`log`/`env_logger`）。无 CI 配置。

## CLI 子命令

- **search** — FTS5 全文搜索 + 过滤器（`--level`, `--after`, `--before`, `--project`, `--module`, `--regex`, `--summary`, `--unique`, `--exclude`, `--context`, `--max-chars`, `--output-file`, `--no-sync`）
- **tail** — 实时跟随文件/目录（旁路索引，直接读文件）
- **config** — show / edit / add-dir / remove-dir
- **index** — update / rebuild / compact / stats
- **project** — add / remove / list
