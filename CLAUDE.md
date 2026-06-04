# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概览

LoggerLog 是轻量级 CLI 日志搜索工具，单个 Rust 二进制。核心搜索引擎基于 SQLite FTS5，支持 log4j/logback、JSON 结构化日志和纯文本的自动检测。支持多项目管理，每个项目自动识别子目录为模块。

## 构建与运行

```bash
# 构建
cargo build

# 运行测试
cargo test

# 示例命令
cargo run -- search "error timeout"
cargo run -- search "level=ERROR" -n 50
cargo run -- search "regex:NullPointerException" --regex
cargo run -- tail /var/log/app.log --filter "ERROR"
cargo run -- config add-dir /path/to/logs
cargo run -- index update
cargo run -- index stats
cargo run -- project add myproject "/path/to/logs"
cargo run -- project list
cargo run -- search --project myproject --module auth "error"
```

## 测试

```bash
# 运行所有测试（Rust 单元测试，内联在各自模块中）
cargo test

# 运行单个模块的测试
cargo test --lib core::parser     # parser 模块测试
cargo test --lib core::engine     # engine 模块测试
cargo test --lib core::discovery  # discovery 模块测试
cargo test --lib core::formats    # formats 子模块测试
```

无集成测试。

## 架构

### Rust 后端（`src-tauri/src/`）

**core/** — 核心引擎，零 clap 依赖的纯库：
- `engine.rs` — SearchEngine：构建 FTS5 SQL 查询 + 正则回退，`parse_query_string()` 解析搜索语法（`level=ERROR`、`after=1h-ago`、`regex:...`、`project=`、`module=`）
- `index.rs` — IndexManager：SQLite WAL 模式，`files` / `log_entries` / `log_entries_fts` / `projects` 四表 + 自动同步触发器，通过 `byte_offset` 实现增量索引，`sync_projects()` 实现项目归属
- `parser.rs` — LogLineParser：采样 50 行自动检测格式（60% 阈值），分派到各格式解析器
- `formats/` — LogParser trait 实现：`log4j.rs`（Log4j + Logback）、`json_log.rs`（JSON 结构化）、`plain.rs`（纯文本回退）
- `discovery.rs` — FileDiscovery：walkdir 遍历 + 日志轮转识别（`.1`、`.2024-01-15.gz` 等）
- `encoding.rs` — chardetng + encoding_rs 自动编码检测
- `watcher.rs` — notify + debouncer-mini 实时文件监控
- `config.rs` — TOML 配置（`~/.config/LoggerLog/config.toml`），包含 `sources.directories` 和 `projects` 两部分

**cli/** — clap derive 子命令：search、tail、config（show/edit/add-dir/remove-dir）、index（update/rebuild/compact/stats）、project（add/remove/list）

## 关键模式

- **错误处理**：core 用 `anyhow::Result<T>`，CLI 出错直接 `eprintln!` + `exit(1)`
- **数据库**：rusqlite bundled，WAL 模式，batch insert 用事务，FTS5 通过 INSERT/UPDATE/DELETE 触发器自动同步
- **搜索语法**：FTS 查询 + `level=X`、`after=X`、`before=X`、`source=X`、`project=X`、`module=X` 过滤器，`regex:` 前缀触发正则搜索
- **项目管理**：`projects` 表 + `files.project_id` 外键，`sync_projects()` 按最长路径前缀匹配文件到项目，模块从文件路径的子目录名自动推导

## 依赖版本

| 技术 | 版本 |
|------|------|
| rusqlite | 0.32 (bundled) |
| clap | 4 (derive) |
