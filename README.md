# LoggerLog

轻量级 CLI 日志搜索工具

## 特性

- 🔍 **全文搜索** — SQLite FTS5 索引，毫秒级搜索
- 📊 **多格式支持** — 自动检测 log4j/logback、JSON 结构化日志、纯文本
- ⌨️ **CLI 工具** — 支持 table/json/raw 输出，方便 AI agent 集成
- 📁 **实时 tail** — 跟随日志文件实时输出
- 🌍 **编码检测** — 自动检测 UTF-8/GBK 编码
- 🔧 **增量索引** — 只索引新增内容，高效处理大文件
- 📦 **多项目管理** — 注册多个项目，自动识别子目录为模块

## 快速开始

```bash
# 构建索引
cargo build --release

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

# 多项目支持
loggerlog project add myproject "/path/to/project/logs"
loggerlog project list
loggerlog search --project myproject --module auth "error"
loggerlog search "project=myproject module=auth level=ERROR"

# 索引管理
loggerlog index stats
loggerlog index rebuild
loggerlog index compact
```

## 搜索语法

```
level=ERROR level=WARN          按日志级别过滤（逗号分隔）
after=1h-ago before=30m-ago     时间范围
source=app.log                  按文件名过滤
project=myproject               按项目名过滤
module=auth-service             按模块名（子目录）过滤
regex:Exception\s+in\s+thread  正则搜索（前缀 regex:）
error timeout                   FTS 全文搜索
```

## 开发

```bash
cargo build          # 构建
cargo test           # 运行测试
cargo run -- --help  # 查看帮助
```

## 架构

- **Rust** — 核心引擎 + CLI（clap derive）
- **SQLite FTS5** — 全文搜索索引
- **两层模型** — core（纯库）→ cli（上层调用者）

## 技术栈

| 层 | 技术 |
|----|------|
| 语言 | Rust |
| 数据库 | SQLite (FTS5) |
| CLI 解析 | clap 4 |

## License

MIT
