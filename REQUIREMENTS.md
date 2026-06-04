# 日志查询 CLI 工具 — 需求规格书

## 背景

面向 Claude Code Agent 使用的日志查询工具。通过 Loki HTTP API 查询本地开发环境日志。项目有 20+ 个微服务，日志通过 Promtail 采集到 Loki，服务间通过 traceId 串联请求链路。

**Loki 地址:** `http://localhost:3100`

**日志目录:** `D:\AppTest\logs\aico\{服务名}\`（console.log / info.log / error.log）

**Promtail 配置:** `scripts/docker/logging/promtail-config.yml`

**技术约束:** 零外部依赖，仅使用 Python 标准库（urllib、json、argparse、re 等）。

---

## 功能需求

### P0 — 必须实现

#### 1. 按服务过滤 `-s / --service`

- 精确匹配：`-s gas-scene`
- 通配符：`-s gas-*` 匹配所有 gas 开头的服务
- 多服务：`-s gas-scene,gas-customer`
- 与其他条件（关键字、级别、时间）自由组合
- 未指定时查询所有服务

#### 2. 按 traceId 追踪 `--trace-id`

- 输入一个 traceId，查询该 traceId 在**所有服务**中出现的所有日志
- 这是排障的核心入口：前端返回 traceId → 用 traceId 查全链路
- 输出按时间排序，标注每条日志所属服务

#### 3. 上下文展开 `--context N`

- 匹配到日志行后，展示该行前后各 N 行同 stream 的日志（默认 3）
- 使用 Loki 的 LogQL 上下文查询能力获取上下文
- 场景：error 日志只一行，真正的 cause 和参数在前几行

#### 4. 异常堆栈折叠

- 检测连续的 `\tat ` 开头的堆栈行，折叠为一条摘要记录
- 输出格式：`异常类名: 消息 (共 M 条堆栈, 出现 N 次)`
- 避免同一个异常的堆栈占满所有 limit 配额

#### 5. `--since` 支持绝对时间

- 相对时间（保持兼容）：`-t 30m`, `-t 2h`, `-t 3d`
- 绝对时间：`-t "2026-06-04 14:30:00"`
- 今天的简写：`-t "14:30"` 等同于当天 `2026-06-04 14:30:00`

---

### P1 — 强烈建议实现

#### 6. 按请求路径过滤 `--path`

- `--path "/scene/inspection/task"` 只看包含该路径的日志
- 适用于：已知某个接口有问题，只看该接口的请求日志

#### 7. 链路追踪 `--trace-path <URL_PATH>`

- 一键完成 Gateway → Service 全链路追踪
- 执行流程：先在 Gateway 日志中搜索 `URL[<path>]`，提取 traceId，再用该 traceId 查所有服务
- 输出：按时间线展示完整请求链（Gateway 开始请求 → 下游处理 → Gateway 结束请求）

#### 8. 日志级别统计摘要

- 搜索结果头部输出分布统计：`error: 12 | warn: 45 | info: 320 | 共 377 条`
- 让使用者一眼判断问题严重程度

#### 9. 排除关键字 `--exclude`

- 支持多个：`--exclude "health check" --exclude "heartbeat"`
- 排除已知的噪音日志

#### 10. 实时跟踪 `--tail`

- 持续输出新日志，类似 `tail -f`
- 支持与 `-s`、`-k`、`-l` 组合

#### 11. 正则匹配 `--regex`

- `--regex "耗时:\s*\d{4,}ms"` 匹配超过 1 秒的慢请求
- `-k` 是精确子串匹配，`--regex` 用于复杂模式

---

### P2 — 建议实现

#### 12. 输出去重 `--unique`

- 相同 service + level + 日志内容前 100 字符视为同一条
- 只显示一次 + 出现次数：`(重复 23 次)`

#### 13. token 预算控制 `--max-chars N`

- 硬性截断输出总字符数，超出部分输出 `[已截断，共 M 条匹配，显示前 N 条]`
- 防止日志输出撑爆 Claude Code 上下文窗口

#### 14. 摘要模式 `--summary`

- 只输出：匹配总数、涉及哪些服务、各级别分布、top 5 高频错误消息
- 先看摘要再决定要不要展开详细日志

#### 15. 输出到文件 `--output <FILE>`

- 将完整日志写入文件，终端只输出摘要统计
- 用法：`--output result.log`

#### 16. 终端着色

- error 红色、warn 黄色、info 默认色、traceId 高亮
- 有 `--no-color` 选项关闭着色

#### 17. JSON 格式输出优化

- `-f json` 时输出结构化 JSON，包含所有字段（service, time, level, traceId, message）
- 方便程序化处理

---

### P3 — Claude Code Agent 集成需求

#### 18. 精简输出模式 `--compact`

- 专为 Claude Code 设计：无装饰边框、无颜色、无重复表头
- 格式：`[时间] [服务] [级别] 日志内容`，每条一行
- 多行日志（堆栈等）缩进拼接为一行
- 目标：最小化 token 消耗

#### 19. 自动摘要（无参数时的默认行为）

- 当不指定任何过滤条件执行 `search` 时，默认先输出最近 30 分钟的摘要（服务数、各级别数量），而非直接灌日志
- 避免 Agent 盲目查询导致上下文爆炸

---

## Promtail 配置改进建议

当前 Promtail 只从路径提取了 `module` 标签，建议增加 pipeline 从日志行中提取更多信息：

| 新增标签 | 提取方式 | 用途 |
|---|---|---|
| `level` | 正则提取日志级别 `INFO/WARN/ERROR` | 按级别精确过滤 |
| `traceId` | 从日志内容中提取 traceId | `{traceId="xxx"}` 精确查询 |

注意：如果 Promtail 配置有改动，需同步更新 CLI 的标签查询逻辑。建议 CLI 能自适应——先查询 Loki 有哪些标签，再决定用哪个标签名查询（兼容 `module` / `service_name`、`detected_level` / `level` 等不同命名）。

---

## 项目日志体系参考

### 服务列表

| 服务 | artifactId | 端口 |
|---|---|---|
| Gateway | aico-cloud-gateway | 8080 |
| Auth | aico-cloud-auth | 9200 |
| System | aico-cloud-system | 9201 |
| Resource | aico-cloud-resource | 9204 |
| Gas Customer | gas-customer | 9701 |
| Gas Charging | gas-charging | 9702 |
| Gas Inspection | gas-inspection | 9703 |
| Gas Flow | gas-flow | 9704 |
| Gas Hall | gas-hall | 9705 |
| Gas Hall Management | gas-hall-management | 9706 |
| Gas Scene | gas-scene | 9707 |
| Gas Front Service | gas-front-service | 9708 |
| Gas RocketMQ Consumer | gas-rocketmq-consumer | 9709 |
| Gas Messaging Service | gas-messaging-service | 9712 |
| Gas Clearing Center | gas-clearing-center-service | 9713 |
| Gas Itfc Log Service | gas-itfc-log-service | 9714 |
| Gas Smart Meter Data | gas-smart-meter-data-service | 9715 |
| Gas External Payment | gas-external-payment | 9720 |

### 日志格式

console.log 标准格式（logback pattern）：
```
2026-06-04 14:30:00.123 [http-nio-9707-exec-1] INFO  com.cnpc.gas.scene.controller.XxxController - 日志内容
```

Gateway 请求日志格式：
```
[PLUS]开始请求 => URL[POST /scene/inspection/task],参数类型[json],参数:[{...}]
[PLUS]结束请求 => URL[POST /scene/inspection/task],耗时:[123]毫秒
```

### traceId 传播机制

1. Gateway 生成 traceId（或从请求头获取），通过 `traceId` 请求头传递给下游服务
2. 下游服务的 `LogTraceAspect` 从请求头读取 traceId 放入 MDC
3. 所有日志通过 MDC 携带 traceId（如果 logback pattern 配置了 `%X{traceId}`）
4. 业务代码在 Dubbo RPC 和定时任务中也可能自行生成 traceId 放入 MDC

### 当前 Promtail 配置

```yaml
# 采集 console.log — 标签: job=aico, module=服务名
- job_name: aico-console
  static_configs:
    - targets: [localhost]
      labels:
        job: aico
        __path__: /logs/*/console.log
  pipeline_stages:
    - replace:
        expression: /logs/([^/]+)/console\.log$
        source: __path__
        replace: "$1"
    - labels:
        module:
    - labeldrop:
        - filename

# 采集 error.log — 标签: job=aico, module=服务名, level=error
- job_name: aico-errors
  static_configs:
    - targets: [localhost]
      labels:
        job: aico
        level: error
        __path__: /logs/*/error.log
  pipeline_stages:
    - replace:
        expression: /logs/([^/]+)/error\.log$
        source: __path__
        replace: "$1"
    - labels:
        module:
    - labeldrop:
        - filename
```
