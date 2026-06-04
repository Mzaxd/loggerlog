# LoggerLog 测试集 (Test Fixtures)

这是 LoggerLog 日志搜索引擎的标准化测试数据集，覆盖了该工具支持的全部日志格式和边界情况。

## 目录结构

```
tests/fixtures/
├── README.md                    # 本文件
├── log4j/                       # Log4j/Logback 格式 (Java 生态)
│   ├── 01_basic.log             # 标准 PatternLayout 格式
│   ├── 02_levels.log            # 全部日志级别 (TRACE→FATAL, WARNING→WARN, SEVERE→ERROR)
│   ├── 03_threads_loggers.log   # 各种线程名和 Logger 名
│   ├── 04_date_formats.log      # 时间格式变体 (逗号/点号毫秒, 斜杠日期, ISO8601, epoch)
│   ├── 05_multiline_stacktrace.log # 多行消息和 Java 异常堆栈跟踪
│   ├── 06_special_characters.log   # 特殊字符, JSON payload, XML, SQL, Unicode, Emoji
│   ├── 07_edge_cases.log        # 边界情况 (空消息, 超长 logger, 注入攻击等)
│   ├── 08_large_sample.log      # 大样本 (>50行) 用于 auto-detect 阈值测试
│   └── expected.json            # 每条日志的预期 parse 结果
│
├── logback/                     # Logback 格式 (Spring Boot 默认)
│   ├── 01_basic.log             # Spring Boot 默认日志格式
│   ├── 02_variants.log          # 变体 (自定义线程名, 不同 logger)
│   └── expected.json
│
├── json/                        # JSON 结构化日志
│   ├── 01_standard.jsonl        # 标准字段 (timestamp, level, message, thread, logger)
│   ├── 02_alternative_fields.jsonl # 替代字段名 (time, severity, lvl, msg, @timestamp, class)
│   ├── 03_extra_fields.jsonl    # 自定义扩展字段 (requestId, userId, duration 等)
│   ├── 04_timestamp_variants.jsonl # 时间戳格式变体 (ISO8601, epoch ms, epoch s, 时区)
│   ├── 05_edge_cases.jsonl      # 边界情况 (null, 空字符串, 嵌套JSON, 数组, 缺失字段)
│   ├── 06_large_sample.jsonl    # 大样本 (>50行)
│   └── expected.json
│
├── plain/                       # 纯文本/非标准格式
│   ├── 01_structured.log        # 有时间戳+级别的半结构化日志
│   ├── 02_unstructured.log      # 完全无结构的自由文本
│   ├── 03_nginx.log             # Nginx access log
│   ├── 04_apache.log            # Apache combined + error log
│   ├── 05_syslog.log            # Syslog (RFC 3164 + RFC 5424)
│   ├── 06_docker.log            # Docker/containerd/K8s 容器日志
│   ├── 07_custom_app.log        # Python/Go/Node.js 自定义格式
│   └── expected.json
│
├── encoding/                    # 编码测试
│   ├── utf8_chinese.log         # UTF-8 编码 + 中文内容
│   ├── gb2312_chinese.log       # GB2312 编码 + 中文内容
│   ├── shift_jis.log            # Shift-JIS 编码 + 日文内容
│   └── expected.json
│
├── mixed/                       # 混合格式 (真实场景常见)
│   ├── 01_log4j_and_plain.log   # Log4j + 纯文本横幅/堆栈混合
│   ├── 02_json_and_text.log     # JSON + 纯文本混合
│   └── expected.json
│
├── real_world/                  # 真实生产环境模拟
│   ├── java_spring_app.log      # Java Spring Boot 微服务 (支付系统)
│   ├── python_service.log       # Python FastAPI 微服务 (用户服务)
│   ├── nodejs_app.jsonl         # Node.js API Gateway (JSON 格式)
│   ├── kubernetes_multi.log     # K8s 多容器 Pod 日志
│   └── expected.json
│
└── search/                      # 搜索功能测试专用
    ├── fts_test.log             # FTS5 全文搜索测试
    ├── regex_test.log           # 正则表达式回退搜索测试
    ├── filter_test.log          # 过滤器测试 (level, source, after, before, thread)
    └── expected.json
```

## 统计数据

| 类别 | 文件数 | 用途 |
|------|--------|------|
| Log4j | 8 + expected.json | 核心格式解析 |
| Logback | 2 + expected.json | Spring Boot 格式 |
| JSON | 6 + expected.json | 结构化日志 |
| Plain | 7 + expected.json | 纯文本回退 |
| Encoding | 3 + expected.json | 编码检测/转换 |
| Mixed | 2 + expected.json | 格式自动识别 |
| Real World | 4 + expected.json | 集成回归 |
| Search | 3 + expected.json | FTS5/Regex/Filter 验证 |
| **总计** | **35 日志文件 + 7 expected.json** | |

## expected.json 格式说明

每个子目录下的 `expected.json` 定义了对应日志文件的预期解析结果。测试框架应该：

1. 遍历每个 fixture 文件
2. 用对应格式的 parser 解析
3. 比对解析结果与 expected.json 中的预期值

### expected.json 结构

```json
{
  "format": "log4j",
  "description": "...",
  "fixtures": {
    "01_basic.log": {
      "format": "log4j",
      "total_lines": 7,
      "entries": [
        {
          "line_number": 3,
          "level": "INFO",
          "thread": "main",
          "logger": "com.example.App",
          "message": "Application started successfully"
        }
      ]
    }
  }
}
```

### 验证字段

| 字段 | 说明 |
|------|------|
| `line_number` | 在文件中的行号（1-based，包含注释行） |
| `timestamp` | (可选) 如果为空字符串表示应该有 `None` 或有效时间戳 |
| `level` | 日志级别，`null` 表示 `None` |
| `thread` | 线程名，`null` 或缺失表示 `None` |
| `logger` | Logger 名，`null` 或缺失表示 `None` |
| `message` | 日志消息内容 |
| `has_timestamp` | 如果为 `true`，断言 `timestamp.is_some()` |
| `has_extra_fields` | 如果为 `true`，断言 `fields_json.is_some()` |
| `note` | (仅文档) 测试注意事项 |

## 测试场景覆盖矩阵

| 场景 | Log4j | Logback | JSON | Plain |
|------|-------|---------|------|-------|
| 标准格式解析 | ✅ | ✅ | ✅ | ✅ |
| ALL 日志级别 (TRACE→FATAL) | ✅ | ✅ | ✅ | ✅ |
| WARNING→WARN 规范化 | ✅ | ✅ | — | ✅ |
| SEVERE→ERROR 规范化 | ✅ | — | — | — |
| 逗号毫秒时间戳 | ✅ | — | — | ✅ |
| 点号毫秒时间戳 | ✅ | ✅ | — | ✅ |
| ISO 8601 时间戳 | ✅ | ✅ | ✅ | ✅ |
| Epoch 时间戳 | ✅ | — | ✅ | — |
| 多时区时间戳 | ✅ | ✅ | ✅ | — |
| 多行消息/异常堆栈 | ✅ | — | — | — |
| Caused by 链式异常 | ✅ | — | — | — |
| 特殊字符 (!@#$%^&*) | ✅ | — | — | — |
| 消息中含 JSON | ✅ | — | ✅ | — |
| 消息中含 XML | ✅ | — | — | — |
| 消息中含 SQL | ✅ | — | — | — |
| Unicode/CJK 字符 | ✅ | ✅ | ✅ | ✅ |
| Emoji 字符 | ✅ | — | ✅ | — |
| 日志注入攻击检测 | ✅ | — | — | — |
| 空消息/空字段 | ✅ | — | ✅ | — |
| 超长 Logger 名 | ✅ | — | — | — |
| 替代字段名映射 | — | — | ✅ | — |
| 自定义扩展字段 | — | — | ✅ | — |
| 嵌套 JSON 对象 | — | — | ✅ | — |
| 数组字段值 | — | — | ✅ | — |
| null 字段处理 | — | — | ✅ | — |
| Nginx access log | — | — | — | ✅ |
| Apache combined log | — | — | — | ✅ |
| Syslog RFC 3164/5424 | — | — | — | ✅ |
| Docker/容器日志 | — | — | — | ✅ |
| GB2312 编码检测 | ✅ | — | — | — |
| Shift-JIS 编码检测 | ✅ | — | — | — |
| 格式自动检测 (60%阈值) | ✅ | — | ✅ | — |
| FTS 全文搜索 | ✅ | — | — | — |
| 正则表达式搜索 | ✅ | — | — | — |
| 复合过滤器 | ✅ | — | — | — |

## 如何添加新的 fixture

1. 在对应格式子目录下创建新的 `.log` / `.jsonl` 文件
2. 按编号命名（如 `09_new_scenario.log`）
3. 在 `expected.json` 中添加对应的解析预期结果
4. 确保文件末尾有一个空行（POSIX 标准）

## 已知的设计取舍

- **注释行**：以 `#` 开头的行视为注释，不参与解析测试
- **空行**：空行在解析时产生空消息的 LogEntry，但格式检测时会跳过
- **堆栈跟踪行**：二次开发需自行处理多行归属 — parser 只解析单行
- **行号**：expected.json 中的行号是 1-based，包含注释行和空行
