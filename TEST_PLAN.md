# LoggerLog 测试完善计划

> 生成日期: 2026-06-06  
> 当前状态: 158 tests passing, 35 fixture files, 8 expected.json  
> 目标: 全面覆盖所有核心模块、集成流程和边缘情况

---

## 一、现有问题修复（前置任务）

这些是已有测试/fixture 中的 bug 和不足，必须先修复，否则后续新增测试可能掩盖已有问题。

### 1.1 Fixture 数据 Bug

| # | 任务 | 位置 | 优先级 | 工作量 |
|---|------|------|--------|--------|
| F-01 | 修正 `total_lines: 28` → `40` | `log4j/expected.json` → `05_multiline_stacktrace.log` | 🔴 P0 | 5min |
| F-02 | 修正 `expected_lines: [7, 7]` → `[7]` 并添加 `expected_match_count: 2` | `search/expected.json` → `regex_test.log` email 测试 | 🔴 P0 | 5min |

### 1.2 `expected.json` 字段验证补充

大量 fixture 文件**存在但无字段级验证**，等于白写了。

| # | 任务 | 位置 | 优先级 | 工作量 |
|---|------|------|--------|--------|
| F-03 | 补充 `03_nginx.log` 的 level/message 验证 | `plain/expected.json` | 🟠 P1 | 30min |
| F-04 | 补充 `04_apache.log` 的 level/message 验证 (access+error 部分) | `plain/expected.json` | 🟠 P1 | 30min |
| F-05 | 补充 `05_syslog.log` 的 level/message 验证 | `plain/expected.json` | 🟠 P1 | 20min |
| F-06 | 补充 `06_docker.log` 的 level/message 验证 | `plain/expected.json` | 🟠 P1 | 30min |
| F-07 | 补充 `07_custom_app.log` 的 level/message 验证 | `plain/expected.json` | 🟠 P1 | 30min |
| F-08 | 补充 `01_log4j_and_plain.log` 10 条 log4j 行的 level/thread/logger/message | `mixed/expected.json` | 🟠 P1 | 30min |
| F-09 | 补充 `02_json_and_text.log` 7 条 JSON 行的 level/message | `mixed/expected.json` | 🟠 P1 | 20min |
| F-10 | 补充 `java_spring_app.log` 至少 10 条 logback 行的字段验证 | `real_world/expected.json` | 🟡 P2 | 45min |
| F-11 | 补充 `nodejs_app.jsonl` 全部 15 条 JSON 行的字段验证 | `real_world/expected.json` | 🟡 P2 | 30min |
| F-12 | 补充 `python_service.log` 至少 5 条字段验证 + traceback 处理 | `real_world/expected.json` | 🟡 P2 | 30min |
| F-13 | 补充 `kubernetes_multi.log` 每种格式 2-3 条字段验证 | `real_world/expected.json` | 🟡 P2 | 45min |
| F-14 | 补充 `05_edge_cases.jsonl` 第 6、7 行的验证 | `json/expected.json` | 🟡 P2 | 10min |
| F-15 | 补充 `04_date_formats.log` 的 logger/thread 验证 | `log4j/expected.json` | 🟡 P2 | 20min |
| F-16 | 补充 `logback/01_basic.log` 和 `02_variants.log` 的 `has_timestamp` 验证 | `logback/expected.json` | 🟡 P2 | 10min |
| F-17 | 补充 `encoding/` 的 `has_timestamp` 和缺失的 thread/logger 验证 | `encoding/expected.json` | 🟡 P2 | 15min |
| F-18 | 为 `search/expected.json` 添加负向匹配（`not_expected_lines`） | `search/expected.json` | 🟡 P2 | 20min |

---

## 二、新增 Fixture 文件

### 2.1 缺失日志格式（P0-P1）

| # | 文件路径 | 内容描述 | 优先级 | 工作量 |
|---|---------|---------|--------|--------|
| N-01 | `plain/08_go_zap.log` + expected | Go zap 结构化日志 (`{"level":"info","msg":"..."`) + panic 堆栈 | 🟠 P1 | 30min |
| N-02 | `plain/09_ruby_rails.log` + expected | Rails `[I] [W] [E]` 单字母级别 | 🟡 P2 | 20min |
| N-03 | `plain/10_dotnet.log` + expected | .NET `info: Microsoft.Hosting.Lifetime` 格式 | 🟡 P2 | 20min |
| N-04 | `plain/11_iis_w3c.log` + expected | IIS W3C 日期格式 `2024-01-15 00:00:00` | 🟡 P2 | 20min |
| N-05 | `json/07_mixed_case_fields.jsonl` + expected | `{"Level":"ERROR","MESSAGE":"fail"}` 大小写混合 | 🟠 P1 | 15min |
| N-06 | `logback/03_levels.log` + expected | logback TRACE→DEBUG→INFO→WARN→ERROR 全级别 | 🟡 P2 | 15min |
| N-07 | `encoding/euc_kr.log` + expected | EUC-KR 编码韩文日志 | 🟡 P2 | 15min |
| N-08 | `encoding/latin1.log` + expected | Latin-1 (ISO-8859-1) 西欧字符 | 🟡 P2 | 15min |

### 2.2 搜索/过滤功能测试数据（P0）

| # | 文件路径 | 内容描述 | 优先级 | 工作量 |
|---|---------|---------|--------|--------|
| N-09 | `search/source_filter.log` + expected | 两个不同路径模拟数据，用于 `source=` 过滤 | 🔴 P0 | 15min |
| N-10 | `search/project_module.log` + expected | `/proj/auth/` 和 `/proj/api/` 路径的日志，用于 `project=`/`module=` | 🔴 P0 | 15min |
| N-11 | `search/time_range.log` + expected | 跨 3 天（2024-01-01 ~ 2024-01-03）的日志 | 🔴 P0 | 15min |
| N-12 | `search/thread_filter.log` + expected | 多线程名 (`http-nio-8080`, `scheduler-1`, `main`) | 🔴 P0 | 15min |
| N-13 | `search/logger_filter.log` + expected | 多 logger (`c.e.s.UserService`, `c.e.s.PaymentService`) | 🔴 P0 | 15min |
| N-14 | `search/exclude_multi.log` + expected | 需要排除 `health` 和 `metrics` 的日志 | 🟠 P1 | 15min |
| N-15 | `search/regex_special_chars.log` + expected | 含 `.*+?^${}()\|[]` 正则元字符的日志 | 🟠 P1 | 15min |
| N-16 | `search/empty_queries.log` + expected | 空 query、仅过滤器、通配符 `*` | 🟡 P2 | 15min |

### 2.3 流程/场景测试数据（P1）

| # | 文件路径 | 内容描述 | 优先级 | 工作量 |
|---|---------|---------|--------|--------|
| N-17 | `incremental/01_initial.log` | 初始 10 行日志（用于增量索引首次全量） | 🟠 P1 | 10min |
| N-18 | `incremental/02_append.log` | 追加 5 行新日志（用于增量索引） | 🟠 P1 | 10min |
| N-19 | `rotation/app.log` | 当前活跃日志 | 🟠 P1 | 10min |
| N-20 | `rotation/app.log.1` | 数字轮转副本 | 🟠 P1 | 5min |
| N-21 | `rotation/app.log.2024-01-15.gz` | 日期+压缩轮转副本 | 🟠 P1 | 10min |

### 2.4 边缘情况测试数据（P2）

| # | 文件路径 | 内容描述 | 优先级 | 工作量 |
|---|---------|---------|--------|--------|
| N-22 | `edge/ultra_long_line.log` | 单行 >100KB | 🟡 P2 | 5min |
| N-23 | `edge/binary_garbage.log` | 二进制乱码字节 | 🟡 P2 | 5min |
| N-24 | `edge/empty_file.log` | 空文件 | 🟡 P2 | 2min |
| N-25 | `edge/single_line.log` | 仅一行 | 🟡 P2 | 2min |
| N-26 | `edge/mixed_newlines.log` | `\r\n` 和 `\n` 混合 | 🟡 P2 | 5min |
| N-27 | `edge/null_bytes.log` | 含 `\x00` | 🟡 P2 | 5min |
| N-28 | `edge/only_whitespace.log` | 只有空格/制表符 | 🟡 P2 | 2min |

### 2.5 真实世界场景补充（P2）

| # | 文件路径 | 内容描述 | 优先级 | 工作量 |
|---|---------|---------|--------|--------|
| N-29 | `real_world/golang_microservice.log` + expected | Go zap + panic 堆栈 | 🟡 P2 | 30min |
| N-30 | `real_world/dotnet_app.log` + expected | .NET Hosting Lifetime | 🟡 P2 | 20min |
| N-31 | `real_world/multilang_service.log` + expected | JSON + 纯文本 + 日志注入混合 | 🟡 P2 | 30min |
| N-32 | `real_world/high_volume.log` | 10,000+ 行（性能回归 + scan_limit） | 🟡 P2 | 15min |

---

## 三、新增单元测试

### 3.1 `config.rs` — 全新测试模块（P0，零依赖纯函数）

```rust
#[cfg(test)]
mod tests {
    // ── parse_size ──
    test_parse_size_bytes          // "500B" → 500
    test_parse_size_kb             // "1024KB" → 1048576
    test_parse_size_mb             // "2MB" → 2097152
    test_parse_size_gb             // "1GB" → 1073741824
    test_parse_size_overflow       // "99999999999999999GB" → None
    test_parse_size_negative       // "-5MB" → None
    test_parse_size_invalid        // "abc" → None, "5TB" → None
    test_parse_size_whitespace     // "  2 MB  " → 2097152
    test_parse_size_lowercase      // "2gb" → 2147483648
    test_parse_size_bare_number    // "4096" → 4096
    test_parse_size_zero           // "0B" → 0
    test_parse_size_decimal        // "1.5GB" → None

    // ── add_directory / remove_directory ──
    test_add_directory_basic
    test_add_directory_duplicate  // 重复路径 → false
    test_remove_directory_exists
    test_remove_directory_not_exists

    // ── add_project / remove_project / get_project_by_name ──
    test_add_project_basic
    test_add_project_duplicate_name
    test_add_project_duplicate_path
    test_remove_project_exists
    test_remove_project_not_exists
    test_get_project_by_name_found
    test_get_project_by_name_not_found

    // ── load / save ── (需要 tempfile::NamedTempFile)
    test_save_and_load_roundtrip
    test_load_nonexistent_creates_default
    test_load_invalid_toml_errors
}
```

**预估**: ~20 个测试, 2-3h

### 3.2 `encoding.rs` — 全新测试模块（P0，需 tempfile）

```rust
#[cfg(test)]
mod tests {
    // ── read_file_to_utf8 ──
    test_read_utf8_file
    test_read_utf8_with_bom
    test_read_with_encoding_override_gbk
    test_read_with_encoding_override_shiftjis
    test_read_nonexistent_file     // → ""
    test_read_empty_file           // → ""
    test_read_replacement_chars    // 大量 \u{FFFD} 触发重编码

    // ── read_lines ──
    test_read_lines_basic
    test_read_lines_empty_file

    // ── read_gz_to_utf8 ── (创建临时 .gz)
    test_read_gz_valid
    test_read_gz_not_gz           // 非 gzip → Err
    test_read_gz_nonexistent

    // ── read_file_from_offset ── (创建临时文件)
    test_read_from_offset_zero
    test_read_from_offset_middle
    test_read_from_offset_past_end
    test_read_from_offset_nonexistent
}
```

**预估**: ~12 个测试, 2h

### 3.3 `entry.rs` — 全新测试模块（P0，纯函数）

```rust
#[cfg(test)]
mod tests {
    test_search_result_from_raw_log4j      // 全字段验证
    test_search_result_from_raw_json       // JSON 行
    test_search_result_from_raw_plain      // 纯文本
    test_search_result_timestamp_rfc3339   // timestamp 格式化
}
```

**预估**: ~4 个测试, 30min

### 3.4 `discovery.rs` — 大量补充（P1）

```rust
// 已有: test_analyze_filename (3 case)

// ── analyze_filename 补充 ──
test_analyze_filename_no_ext           // "syslog" → not rotated
test_analyze_filename_date_rotation    // "app.log.2024-01-15"
test_analyze_filename_date_hour        // "app.log.2024-01-15-08"
test_analyze_filename_zip             // "app.log.1.zip"
test_analyze_filename_bz2             // "app.log.2024-01-01.bz2"
test_analyze_filename_single_dot      // "app.log" → not rotated
test_analyze_filename_not_number      // "app.log.backup" → not rotated
test_analyze_filename_no_ext_number   // "server.out.10" → rotated

// ── matches_glob ──
test_matches_glob_star_ext
test_matches_glob_star_only
test_matches_glob_exact
test_matches_glob_no_match

// ── group_files ──
test_group_files_basic
test_group_files_empty
test_group_files_sorted

// ── scan_directory ── (需 tempfile::TempDir)
test_scan_directory_recursive
test_scan_directory_exclude
test_scan_directory_nonexistent
```

**预估**: ~16 个测试, 2h

### 3.5 `index.rs` — 补充测试（P1）

```rust
// ── 增量索引相关 ──
test_get_file_byte_offset
test_update_file_metadata

// ── 项目/模块相关 ──
test_sync_projects_file_assignment   // 验证 file.project_id 实际更新
test_sync_projects_longest_prefix    // "/data/proj" vs "/data/proj2"
test_get_modules_for_project_found
test_get_modules_for_project_flat

// ── 边缘情况 ──
test_insert_large_batch              // 1000+ 条
test_fts_search_after_delete         // clear_file_entries 后 FTS 同步
test_schema_migration_v3_to_v4        // v3→v4 数据保留
```

**预估**: ~10 个测试, 2h

### 3.6 `engine.rs` — 补充测试（P1）

```rust
// ── SQL WHERE 过滤器 ──
test_search_source_filter
test_search_project_filter           // (需要项目关联)
test_search_module_filter

// ── 内存过滤器 ──
test_search_thread_filter
test_search_logger_filter
test_search_after_filter
test_search_before_filter
test_search_combined_filters

// ── regex 边缘 ──
test_search_regex_invalid_pattern    // "[invalid" → Err

// ── parse_query_string 补充 ──
test_parse_query_empty
test_parse_query_only_regex
test_parse_query_thread_logger
test_parse_query_multiple_exclude

// ── 聚合 ──
test_level_stats_with_source_filter
test_search_summary_empty_db

// ── 上下文 ──
test_get_context_boundary

// ── 行为 ──
test_search_approximate_flag
test_scan_limit_truncation
test_search_no_fts_with_filters
```

**预估**: ~20 个测试, 3h

### 3.7 `watcher.rs` — 基础测试（P2）

```rust
test_watch_creates_channel
test_watch_nonexistent_directory
```

**预估**: ~2 个测试, 30min

---

## 四、新增集成测试

在 `tests/integration_test.rs` 中新增（或在 `tests/` 下新建文件）。

| # | 测试名 | 验证什么 | 优先级 | 依赖 |
|---|--------|---------|--------|------|
| I-01 | `test_e2e_full_pipeline` | 发现文件 → 索引 → FTS 搜索 → 验证结果 | 🔴 P0 | N-09 |
| I-02 | `test_e2e_project_scoped_search` | 项目 → 关联 → project= 过滤 | 🔴 P0 | N-10 |
| I-03 | `test_e2e_module_search` | 多子目录 → module= 过滤 | 🔴 P0 | N-10 |
| I-04 | `test_e2e_time_range_search` | 跨时间段 → after/before 过滤 | 🔴 P0 | N-11 |
| I-05 | `test_e2e_mixed_format_index_search` | 混合格式 → 自动检测 → 索引 → 搜索 | 🟠 P1 | 已有 fixture |
| I-06 | `test_e2e_incremental_index` | 首次全量 → 追加 → 增量索引 → 只索引新增 | 🟠 P1 | N-17, N-18 |
| I-07 | `test_e2e_file_rotation_grouping` | `app.log` + `.1` + `.gz` → group_files | 🟠 P1 | N-19~N-21 |
| I-08 | `test_e2e_compressed_file_index` | gzip → 解压 → 索引 → 搜索 | 🟠 P1 | N-21 |
| I-09 | `test_e2e_regex_with_level_filter` | 正则 + level 过滤组合 | 🟠 P1 | 已有 fixture |
| I-10 | `test_e2e_stacktrace_context` | 异常搜索 → get_context → 堆栈行 | 🟠 P1 | 已有 fixture |
| I-11 | `test_e2e_unicode_search` | 中日韩/Emoji → FTS + regex | 🟠 P1 | 已有 fixture |
| I-12 | `test_e2e_exclude_filter` | exclude= 排除 + 验证 | 🟠 P1 | N-14 |
| I-13 | `test_e2e_context_around_result` | 搜索 → get_context → 行数和内容 | 🟠 P1 | 已有 fixture |
| I-14 | `test_e2e_summary_aggregation` | 大量日志 → search_summary → 统计 | 🟠 P1 | 已有 fixture |
| I-15 | `test_e2e_all_query_filters` | 全过滤器组合 → search | 🟠 P1 | 已有 fixture |
| I-16 | `test_e2e_config_roundtrip` | Config → save → load → 值不变 | 🟠 P1 | — |
| I-17 | `test_e2e_discovery_with_tempdir` | 临时目录 → scan_directory → 验证 | 🟠 P1 | — |
| I-18 | `test_e2e_detect_format_threshold` | 40% JSON + 60% plain → plain | 🟡 P2 | 已有 fixture |
| I-19 | `test_e2e_high_volume` | 10k+ 行 → 索引 → 搜索 → 不截断 | 🟡 P2 | N-32 |
| I-20 | `test_e2e_all_fixtures_indexable` | 遍历全部 fixture → 索引 → 不 panic | 🟡 P2 | — |

---

## 五、开发顺序与依赖关系

### Phase 0: 前置修复（1-2h）🔴

```
┌─────────────┐  ┌─────────────┐
│ F-01: 修正   │  │ F-02: 修正   │   ← 无依赖，可并行
│ total_lines │  │ expected_   │
│ 错误        │  │ lines 重复  │
└─────────────┘  └─────────────┘
```

**完成后**: `cargo test` 仍然全绿，fixture 数据准确。

---

### Phase 1: 核心纯函数单元测试（4-5h）🔴

这批测试**零依赖**，只需要写代码，不需要新 fixture。

```
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│ 3.1 config.rs    │ │ 3.2 encoding.rs  │ │ 3.3 entry.rs     │
│ ~20 tests        │ │ ~12 tests        │ │ ~4 tests         │
│ (纯函数,无IO)    │ │ (需tempfile)     │ │ (纯函数)          │
│ 2-3h             │ │ 2h               │ │ 30min            │
└──────────────────┘ └──────────────────┘ └──────────────────┘
        ↓ 并行                    ↓ 并行                ↓ 并行
```

**并行策略**: config.rs 和 entry.rs 可由不同开发者并行。encoding.rs 需要创建临时文件，不影响其他。

---

### Phase 2: 搜索过滤 Fixture + 单元测试（4-5h）🔴

搜索过滤器（source/project/module/time/thread/logger）是**当前零覆盖**的核心功能。

```
Step 1: 创建 Fixture 文件（并行）
┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│ N-09 source_  │ │ N-10 project_ │ │ N-11 time_    │ │ N-12 thread_  │ │ N-13 logger_  │
│ filter.log    │ │ module.log   │ │ range.log     │ │ filter.log    │ │ filter.log    │
└──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘
        ↓ 并行              ↓ 并行          ↓ 并行          ↓ 并行          ↓ 并行

Step 2: 补充 engine.rs 过滤器测试（依赖 Step 1）
┌──────────────────────────────────────────────────────────────┐
│ 3.6 engine.rs 补充 (~20 tests)                                │
│ • source/project/module 过滤 → 依赖 N-09, N-10                │
│ • after/before 时间过滤 → 依赖 N-11                          │
│ • thread/logger 过滤 → 依赖 N-12, N-13                       │
│ • parse_query_string 补充 → 依赖上述全部                      │
│ 3h                                                              │
└──────────────────────────────────────────────────────────────┘

Step 3: 补充 discovery.rs 测试（可并行）
┌──────────────────────────────────────────────────────────────┐
│ 3.4 discovery.rs 补充 (~16 tests)                            │
│ • analyze_filename 边缘情况 — 纯函数，无依赖                  │
│ • matches_glob — 纯函数                                      │
│ • group_files — 纯函数                                       │
│ • scan_directory — 需 tempfile                                │
│ 2h                                                              │
└──────────────────────────────────────────────────────────────┘
```

**并行策略**: 
- Step 1 的 5 个 fixture 文件可完全并行
- Step 2 的 engine.rs 测试和 Step 3 的 discovery.rs 测试可并行（无依赖）
- Step 2 内部有顺序依赖：先写 fixture → 再写测试

---

### Phase 3: expected.json 补充 + 集成测试（6-8h）🟠

```
Step 1: 补充 expected.json（并行）
┌────────────────────────┐ ┌────────────────────────┐ ┌────────────────────────┐
│ F-03~F-07: plain/      │ │ F-08~F-09: mixed/       │ │ F-10~F-13: real_world/ │
│ 5 个文件 ~2h           │ │ 2 个文件 ~50min         │ │ 4 个文件 ~2.5h         │
└────────────────────────┘ └────────────────────────┘ └────────────────────────┘
        ↓ 并行                      ↓ 并行                     ↓ 并行

Step 2: 集成测试（依赖 Step 1 的 fixture）
┌──────────────────────────────────────────────────────────────────────┐
│ I-01~I-04: 核心过滤器集成测试                                       │
│ 依赖: N-09~N-13 (Phase 2 已创建)                                    │
│ 2h                                                                     │
├──────────────────────────────────────────────────────────────────────┤
│ I-05~I-15: 格式/索引/搜索集成测试                                   │
│ 依赖: 已有 fixture + Phase 2                                         │
│ 3h                                                                     │
├──────────────────────────────────────────────────────────────────────┤
│ I-16~I-17: config/discovery 集成测试                                │
│ 依赖: Phase 1 的单元测试通过                                         │
│ 1h                                                                     │
└──────────────────────────────────────────────────────────────────────┘

Step 3: 补充 index.rs 测试（可并行）
┌──────────────────────────────────────────────────────────────────────┐
│ 3.5 index.rs 补充 (~10 tests)                                       │
│ • sync_projects / modules → 依赖项目 fixture                        │
│ • incremental → 依赖增量 fixture                                     │
│ 2h                                                                     │
└──────────────────────────────────────────────────────────────────────┘
```

---

### Phase 4: 格式覆盖 + 边缘情况（4-5h）🟡

```
Step 1: 新增格式 Fixture（并行）
┌────────────────┐ ┌────────────────┐ ┌────────────────┐
│ N-01 Go zap    │ │ N-05 JSON大小写 │ │ N-06 logback全级│
│ N-02 Ruby      │ │ N-07 EUC-KR    │ │ N-08 Latin-1   │
│ N-03 .NET      │ │ N-04 IIS        │ │                │
│ ~2h            │ │ ~1h            │ │ ~45min          │
└────────────────┘ └────────────────┘ └────────────────┘

Step 2: 边缘情况 Fixture（并行）
┌──────────────────────────────────────────────────┐
│ N-22~N-28: edge/ 7 个文件                         │
│ ~30min                                            │
├──────────────────────────────────────────────────┤
│ N-17~N-21: incremental/ + rotation/               │
│ ~45min                                            │
├──────────────────────────────────────────────────┤
│ N-29~N-32: real_world 补充                        │
│ ~1.5h                                            │
└──────────────────────────────────────────────────┘

Step 3: 补充 expected.json + 集成测试
┌──────────────────────────────────────────────────┐
│ F-14~F-18: 剩余 expected.json 补充               │
│ I-18~I-20: 格式检测/高容量集成测试                 │
│ ~2h                                               │
└──────────────────────────────────────────────────┘

Step 4: watcher.rs 测试（独立）
┌──────────────────────────────────────────────────┐
│ 3.7 watcher.rs (~2 tests)                        │
│ 30min                                             │
└──────────────────────────────────────────────────┘
```

---

### Phase 5: expected.json 剩余补充 + 全量回归（2-3h）🟡

```
┌──────────────────────────────────────────────────┐
│ F-14~F-18: log4j/logback/encoding/search 补充   │
│ ~1h                                               │
├──────────────────────────────────────────────────┤
│ 全量回归: cargo test --all                        │
│ 修复发现的任何回归问题                             │
│ ~1-2h                                             │
└──────────────────────────────────────────────────┘
```

---

## 六、并行开发矩阵

### 可完全并行的任务组（无任何依赖）

| 组 | 任务 | 预估时间 | 说明 |
|----|------|---------|------|
| **A** | `3.1 config.rs` 全套单元测试 | 2-3h | 纯函数，独立文件 |
| **B** | `3.3 entry.rs` 全套单元测试 | 30min | 纯函数，独立文件 |
| **C** | `3.4 discovery.rs` analyze_filename + matches_glob + group_files | 1.5h | 纯函数部分（不含 scan_directory tempfile） |
| **D** | `F-01` + `F-02` 修正 fixture bug | 10min | 独立 |
| **E** | `N-22~N-28` edge/ fixture 文件 | 30min | 独立 |
| **F** | `N-01~N-08` 格式 fixture 文件 | 2h | 独立 |
| **G** | `F-14~F-18` 剩余 expected.json 补充 | 1h | 独立 |

### 有顺序依赖的任务链

```
Chain 1 (encoding 路线):
  N-07, N-08 → 3.2 encoding.rs → I-16 config roundtrip

Chain 2 (搜索过滤路线):
  N-09~N-13 → 3.6 engine.rs 补充 → I-01~I-04 集成测试

Chain 3 (项目/模块路线):
  N-10 → 3.5 index.rs 补充 → I-02~I-03 集成测试

Chain 4 (时间过滤路线):
  N-11 → 3.6 engine.rs after/before 测试 → I-04 集成测试

Chain 5 (增量索引路线):
  N-17, N-18 → I-06 增量索引集成测试

Chain 6 (文件轮转路线):
  N-19~N-21 → I-07, I-08 轮转/压缩集成测试

Chain 7 (高容量路线):
  N-32 → I-19 高容量集成测试

Chain 8 (discovery 路线):
  3.4 discovery.rs scan_directory → I-17 集成测试
```

### 推荐的 2 人并行分配

```
┌─────────────────────────────────────────────────────────┐
│ 开发者 A (偏核心逻辑)                                     │
│                                                          │
│ Phase 0:  F-01, F-02 (10min)                             │
│ Phase 1:  3.1 config.rs (2-3h) ─┐                       │
│           3.3 entry.rs (30min) ──┼→ 并行                  │
│           3.2 encoding.rs (2h)  ─┘                       │
│ Phase 2:  N-09~N-13 fixture (1h) → 3.6 engine.rs (3h)    │
│ Phase 3:  I-01~I-04 集成测试 (2h)                         │
│ Phase 4:  3.5 index.rs 补充 (2h)                          │
│ Phase 5:  回归修复                                         │
│                                                          │
│ 总计: ~15h                                               │
├─────────────────────────────────────────────────────────┤
│ 开发者 B (偏 fixture + 集成)                              │
│                                                          │
│ Phase 0:  (无)                                            │
│ Phase 1:  3.4 discovery.rs (2h)                           │
│ Phase 2:  N-01~N-08 格式 fixture (2h)                     │
│           F-03~F-09 expected.json 补充 (3h)               │
│ Phase 3:  F-10~F-13 real_world (2.5h)                    │
│           I-05~I-15 集成测试 (4h)                        │
│ Phase 4:  N-17~N-21 增量/轮转 (45min)                    │
│           N-22~N-28 边缘情况 (30min)                     │
│           N-29~N-32 真实世界补充 (1.5h)                   │
│           I-16~I-20 集成测试 (2h)                         │
│           3.7 watcher.rs (30min)                          │
│ Phase 5:  回归修复                                         │
│                                                          │
│ 总计: ~18h                                               │
├─────────────────────────────────────────────────────────┤
│ 合计: ~33h (约 4-5 个工作日，2 人并行)                     │
└─────────────────────────────────────────────────────────┘
```

### 推荐的 1 人顺序执行

```
Day 1 (8h):
  Phase 0: F-01, F-02                    (10min)
  Phase 1: 3.1 config.rs                  (2.5h)
           3.3 entry.rs                   (30min)
           3.2 encoding.rs                (2h)
           3.4 discovery.rs 纯函数部分      (1.5h)

Day 2 (8h):
  Phase 2: N-09~N-16 搜索 fixture          (2h)
           3.6 engine.rs 补充              (3h)
           N-01~N-08 格式 fixture           (2h)

Day 3 (8h):
  Phase 3: F-03~F-13 expected.json         (4h)
           I-01~I-04 核心集成测试            (2h)
           N-17~N-21 增量/轮转 fixture      (1h)

Day 4 (8h):
  Phase 3: I-05~I-15 剩余集成测试          (3h)
           3.5 index.rs 补充               (2h)
           N-22~N-32 边缘+真实世界 fixture  (2h)

Day 5 (4-5h):
  Phase 4-5: F-14~F-18 补充 + 3.7 watcher  (1.5h)
             I-16~I-20 最终集成测试          (1.5h)
             全量回归 + 修复                  (1.5h)
```

---

## 七、验收标准

### 最低可行目标（MVP）

- [ ] F-01, F-02 修复完成
- [ ] `config.rs` 完整单元测试 (~20 tests)
- [ ] `encoding.rs` 完整单元测试 (~12 tests)
- [ ] `entry.rs` 完整单元测试 (~4 tests)
- [ ] `engine.rs` source/project/module/time/thread/logger 过滤器测试 (~12 tests)
- [ ] `discovery.rs` 补充测试 (~10 tests)
- [ ] 搜索过滤 fixture (N-09~N-13) + 对应集成测试 (I-01~I-04)
- [ ] `cargo test` 全绿，**总测试数 ≥ 230**

### 完整目标

- [ ] 所有 Phase 0~5 完成
- [ ] 所有 `expected.json` 字段验证补充
- [ ] 所有新 fixture 文件 + 对应集成测试
- [ ] `cargo test` 全绿，**总测试数 ≥ 280**
- [ ] 无 `#[ignore]` 测试（除 watcher 的 OS 依赖测试外）
- [ ] 性能测试 (high_volume) 在 CI 中 <5s

---

## 八、工作量汇总

| 类别 | 新增 |
|------|------|
| Fixture 文件修复 | 2 个 bug |
| `expected.json` 补充 | 18 项 |
| 新增 Fixture 文件 | 32 个 |
| `config.rs` 测试 | ~20 个 |
| `encoding.rs` 测试 | ~12 个 |
| `entry.rs` 测试 | ~4 个 |
| `discovery.rs` 测试 | ~16 个 |
| `index.rs` 测试 | ~10 个 |
| `engine.rs` 测试 | ~20 个 |
| `watcher.rs` 测试 | ~2 个 |
| 集成测试 | ~20 个 |
| **总计** | **~104 个新测试 + 32 个 fixture + 18 项 expected.json 补充** |
| **预估总工时** | **33h (2 人并行 4-5 天，1 人顺序 5 天)** |
