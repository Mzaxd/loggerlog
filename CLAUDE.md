# LoggerLog

轻量级日志搜索工具，桌面 GUI + CLI 双模式。

## 项目结构

- `src-tauri/` — Rust 后端（Tauri + CLI + 核心引擎）
  - `src/core/` — 核心模块（配置、解析、索引、搜索）
  - `src/cli/` — CLI 命令实现
  - `src/gui/` — Tauri IPC 命令和 GUI 状态
- `ui/` — React + TypeScript + Tailwind 前端

## 构建

```bash
cargo build --features "cli,gui"   # GUI + CLI
cargo build --features cli           # CLI only
cd ui && npm run build             # 前端构建
```

## 运行

```bash
cargo run -- search "error"        # CLI 模式
cargo run -- gui                    # GUI 模式
```

## 关键文件

- `src-tauri/src/core/engine.rs` — 搜索引擎（FTS5 + 正则）
- `src-tauri/src/core/index.rs` — SQLite FTS5 索引管理
- `src-tauri/src/core/parser.rs` — 日志格式自动检测和解析
- `src-tauri/src/core/discovery.rs` — 文件发现和日志轮转识别
- `src-tauri/src/cli/mod.rs` — CLI 命令定义（clap）
- `src-tauri/src/gui/mod.rs` — Tauri IPC 命令（GUI 搜索/索引）
