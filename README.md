# LoggerLog

轻量级日志搜索工具 — 桌面 GUI + CLI 双模式

## 特性

- 🔍 **全文搜索** — SQLite FTS5 索引，毫秒级搜索
- 📊 **多格式支持** — 自动检测 log4j/logback、JSON 结构化日志、纯文本
- 🖥️ **桌面 GUI** — 三栏布局，深色主题，日志源树 + 搜索结果 + 详情面板
- ⌨️ **CLI 工具** — 支持 table/json/raw 输出，方便 AI agent 集成
- 📁 **实时 tail** — 跟随日志文件实时输出
- 🌏 **跨平台** — macOS + Windows（Tauri v2）
- 🔧 **增量索引** — 只索引新增内容，高效处理大文件
- 🌍 **编码检测** — 自动检测 UTF-8/GBK 编码

## 快速开始

### CLI 使用

```bash
# 添加日志目录
loggerlog config add-dir /path/to/logs

# 构建索引
loggerlog index update

# 搜索
loggerlog search "error timeout"
loggerlog search "level=ERROR" -n 50
loggerlog search "regex:NullPointerException" --regex

# JSON 输出（供 AI agent 使用）
loggerlog search "error" -o json

# 实时 tail
loggerlog tail /var/log/app.log --filter "ERROR"

# 索引管理
loggerlog index stats
loggerlog index rebuild
loggerlog index compact
```

### GUI 使用

```bash
loggerlog gui
# 或直接运行（无参数默认 GUI）
loggerlog
```

## 开发

```bash
# CLI 模式编译
cargo build --features cli

# GUI + CLI 编译
cargo build --features "cli,gui"

# 前端开发
cd ui && npm install && npm run dev

# Tauri 开发模式
cd ui && npm run dev
cargo run --features "cli,gui" -- gui
```

## 架构

- **Tauri v2** — Rust 后端 + React 前端
- **SQLite FTS5** — 全文搜索索引
- **同一二进制** — CLI/GUI 双模式自动切换

## 技术栈

| 层 | 技术 |
|----|------|
| 桌面框架 | Tauri v2 |
| 后端语言 | Rust |
| 数据库 | SQLite (FTS5) |
| 前端 | React + TypeScript |
| 样式 | Tailwind CSS 4 |
| 状态管理 | Zustand |
| CLI 解析 | clap 4 |

## License

MIT
