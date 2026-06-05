//! Integration tests based on fixtures/ dataset.
//!
//! Tests cover:
//! 1. Scanner extraction against fixture files
//! 2. Search functionality (FTS/regex/filter) against search/ fixtures
//! 3. Encoding detection against encoding/ fixtures
//! 4. End-to-end index→search flow against real_world/ fixtures

use loggerlog::core::entry::LogEntry;
use loggerlog::core::scanner;
use serde_json::Value as JsonValue;

// ── Helpers ──────────────────────────────────────────────────────────

fn fixture_path(relative: &str) -> String {
    std::path::Path::new("tests/fixtures")
        .join(relative)
        .to_string_lossy()
        .to_string()
}

fn load_expected(dir: &str) -> JsonValue {
    let path = fixture_path(&format!("{}/expected.json", dir));
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("Failed to read {}", path));
    serde_json::from_str(&content).expect("Invalid expected.json")
}

/// Create raw-only LogEntry objects from file content.
fn create_raw_entries(content: &str, file_id: i64, start_offset: u64) -> Vec<LogEntry> {
    let mut entries = Vec::new();
    let mut byte_offset = start_offset;
    let mut line_number = 0u64;
    for line in content.lines() {
        line_number += 1;
        entries.push(LogEntry {
            id: None,
            file_id,
            line_number,
            byte_offset,
            raw: line.to_string(),
        });
        byte_offset += line.len() as u64 + 1;
    }
    entries
}

/// Load non-comment, non-empty lines from a fixture file.
fn load_data_lines(file_path: &str) -> Vec<String> {
    let content = std::fs::read_to_string(file_path).expect("Failed to read fixture");
    content
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .map(|l| l.to_string())
        .collect()
}

/// Assert scanner can extract expected fields from fixture data lines.
fn assert_scanner_line(raw: &str, exp: &JsonValue, context: &str) {
    if let Some(lvl) = exp["level"].as_str() {
        let actual = scanner::extract_level(raw);
        assert_eq!(actual.as_deref(), Some(lvl), "{}: level mismatch (got {:?})", context, actual);
    } else if exp["level"].is_null() {
        let actual = scanner::extract_level(raw);
        assert!(actual.is_none(), "{}: level should be None, got {:?}", context, actual);
    }

    if let Some(th) = exp["thread"].as_str() {
        if th.is_empty() {
            let actual = scanner::extract_thread(raw);
            assert!(actual.is_none() || actual.as_deref() == Some(""), "{}: thread should be None/empty", context);
        } else {
            let actual = scanner::extract_thread(raw);
            assert_eq!(actual.as_deref(), Some(th), "{}: thread mismatch", context);
        }
    }

    if let Some(lg) = exp["logger"].as_str() {
        if lg.is_empty() {
            let actual = scanner::extract_logger(raw);
            assert!(actual.is_none() || actual.as_deref() == Some(""), "{}: logger should be None/empty", context);
        } else {
            let actual = scanner::extract_logger(raw);
            assert_eq!(actual.as_deref(), Some(lg), "{}: logger mismatch", context);
        }
    }

    if let Some(msg) = exp["message"].as_str() {
        let actual = scanner::extract_message(raw);
        assert_eq!(actual, msg, "{}: message mismatch", context);
    }

    if exp["has_timestamp"].as_bool() == Some(true) {
        let actual = scanner::extract_timestamp(raw);
        assert!(actual.is_some(), "{}: should have timestamp", context);
    }
}

// ── Scanner extraction tests ────────────────────────────────────────

fn test_scanner_for_fixtures(dir: &str) {
    let expected = load_expected(dir);
    let fixtures = expected["fixtures"].as_object().unwrap();

    for (filename, spec) in fixtures {
        let file_path = fixture_path(&format!("{}/{}", dir, filename));
        let lines = load_data_lines(&file_path);

        if let Some(expected_entries) = spec["entries"].as_array() {
            let mut matched = 0;
            for (idx, exp) in expected_entries.iter().enumerate() {
                // Find the matching line: prefer message content match,
                // then thread+level combination, then array index
                let exp_msg = exp["message"].as_str();
                let exp_thread = exp["thread"].as_str();
                let exp_level = exp["level"].as_str();
                let raw = if let Some(msg) = exp_msg {
                    if msg.is_empty() {
                        // Empty message — can't match by content, use index
                        lines.get(idx).map(|s| s.as_str())
                    } else {
                        match lines.iter().find(|l| l.contains(msg) || scanner::extract_message(l) == msg) {
                            Some(val) => Some(val.as_str()),
                            None => lines.get(idx).map(|s| s.as_str()),
                        }
                    }
                } else if let (Some(thread), Some(level)) = (exp_thread, exp_level) {
                    // Match by thread + level combination
                    match lines.iter().find(|l| {
                        scanner::extract_thread(l).as_deref() == Some(thread)
                            && scanner::extract_level(l).as_deref() == Some(level)
                    }) {
                        Some(val) => Some(val.as_str()),
                        None => lines.get(idx).map(|s| s.as_str()),
                    }
                } else if let Some(level) = exp_level {
                    // Match by level alone (for entries with null/missing message and no thread)
                    match lines.iter().find(|l| {
                        scanner::extract_level(l).as_deref() == Some(level)
                    }) {
                        Some(val) => Some(val.as_str()),
                        None => lines.get(idx).map(|s| s.as_str()),
                    }
                } else {
                    lines.get(idx).map(|s| s.as_str())
                };

                if let Some(raw) = raw {
                    let ctx = if let Some(msg) = exp_msg {
                        let preview: String = msg.chars().take(60).collect();
                        format!("{}: expected[{}] '{}'", filename, idx, preview)
                    } else {
                        format!("{}: expected[{}]", filename, idx)
                    };
                    assert_scanner_line(raw, exp, &ctx);
                    matched += 1;
                }
            }
            assert!(
                matched > 0,
                "{}: no expected entries matched ({} total expected)",
                filename,
                expected_entries.len()
            );
        }
    }
}

#[test]
fn test_scanner_log4j() {
    test_scanner_for_fixtures("log4j");
}

#[test]
fn test_scanner_logback() {
    test_scanner_for_fixtures("logback");
}

#[test]
fn test_scanner_json() {
    test_scanner_for_fixtures("json");
}

#[test]
fn test_scanner_plain() {
    let expected = load_expected("plain");
    let fixtures = expected["fixtures"].as_object().unwrap();

    for (filename, _spec) in fixtures {
        let file_path = fixture_path(&format!("plain/{}", filename));
        let lines = load_data_lines(&file_path);

        for line in &lines {
            // Plain lines should always produce a non-empty message
            let msg = scanner::extract_message(line);
            assert!(!msg.is_empty(), "{}: line should have non-empty message", filename);
        }
    }
}

#[test]
fn test_format_auto_detect() {
    let log4j_path = fixture_path("log4j/08_large_sample.log");
    let lines: Vec<String> = std::fs::read_to_string(&log4j_path)
        .unwrap()
        .lines()
        .map(|l| l.to_string())
        .collect();
    assert_eq!(scanner::detect_format_hint(&lines), "log4j");

    let json_path = fixture_path("json/06_large_sample.jsonl");
    let lines: Vec<String> = std::fs::read_to_string(&json_path)
        .unwrap()
        .lines()
        .map(|l| l.to_string())
        .collect();
    assert_eq!(scanner::detect_format_hint(&lines), "json");
}

// ── Search integration tests ─────────────────────────────────────────

fn build_search_index() -> loggerlog::core::index::IndexManager {
    use loggerlog::core::index::IndexManager;
    let idx = IndexManager::open_in_memory().unwrap();

    for fixture_name in &["fts_test.log", "regex_test.log", "filter_test.log"] {
        let path = fixture_path(&format!("search/{}", fixture_name));
        let content = std::fs::read_to_string(&path).unwrap();
        let file_id = idx.get_or_create_file(&path).unwrap();
        let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        let _format_str = scanner::detect_format_hint(&lines);
        let entries = create_raw_entries(&content, file_id, 0);
        idx.insert_entries(&entries).unwrap();
        idx.update_file(file_id, entries.len() as i64, entries.len() as i64, entries.len() as i64, _format_str).unwrap();
    }
    idx
}

#[test]
fn test_search_fts_basic() {
    let idx = build_search_index();
    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.fts_query = Some("database".to_string());
    let rs = engine.search(&q).unwrap();
    assert!(rs.total_count > 0, "FTS search should find results");
}

#[test]
fn test_search_fts_level_filter() {
    let idx = build_search_index();
    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.levels = vec!["ERROR".to_string()];
    let rs = engine.search(&q).unwrap();
    assert!(rs.total_count > 0);
    for r in &rs.results {
        assert_eq!(r.level.as_deref(), Some("ERROR"));
    }
}

#[test]
fn test_search_regex() {
    let idx = build_search_index();
    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    let rs = engine
        .search_regex(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}", &q)
        .unwrap();
    assert!(rs.total_count > 0, "Regex IP search should find results");
}

#[test]
fn test_search_level_stats() {
    let idx = build_search_index();
    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let stats = engine
        .level_stats(&loggerlog::core::entry::SearchQuery::default())
        .unwrap();
    assert!(!stats.is_empty());
}

#[test]
fn test_search_summary() {
    let idx = build_search_index();
    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let summary = engine
        .search_summary(&loggerlog::core::entry::SearchQuery::default(), 10)
        .unwrap();
    assert!(summary.total_count > 0);
    assert!(!summary.level_distribution.is_empty());
}

#[test]
fn test_search_context() {
    let idx = build_search_index();
    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 1;
    q.fts_query = Some("connection".to_string());
    let rs = engine.search(&q).unwrap();
    assert!(!rs.results.is_empty());
    let r = &rs.results[0];
    let ctx = engine.get_context(r.file_id, r.line_number, 2).unwrap();
    assert!(!ctx.is_empty());
}

// ── Encoding tests ───────────────────────────────────────────────────

#[test]
fn test_encoding_utf8_chinese() {
    let path = fixture_path("encoding/utf8_chinese.log");
    let content = loggerlog::core::encoding::read_file_to_utf8(&path, None);
    assert!(content.contains("系统启动成功"));
    assert!(content.contains("中文日志"));
}

#[test]
fn test_encoding_gb2312() {
    let path = fixture_path("encoding/gb2312_chinese.log");
    let content = loggerlog::core::encoding::read_file_to_utf8(&path, None);
    assert!(!content.is_empty());
    assert!(
        content.contains("系统") || content.contains("启动") || content.contains("成功"),
        "GB2312 should decode to readable Chinese"
    );
}

#[test]
fn test_encoding_shift_jis() {
    let path = fixture_path("encoding/shift_jis.log");
    let content = loggerlog::core::encoding::read_file_to_utf8(&path, None);
    assert!(!content.is_empty());
}

// ── End-to-end tests ─────────────────────────────────────────────────

#[test]
fn test_e2e_real_world_log4j() {
    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();
    let path = fixture_path("real_world/java_spring_app.log");
    let content = std::fs::read_to_string(&path).unwrap();
    let file_id = idx.get_or_create_file(&path).unwrap();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let _format_str = scanner::detect_format_hint(&lines);
    let entries = create_raw_entries(&content, file_id, 0);
    assert!(entries.len() > 0);
    idx.insert_entries(&entries).unwrap();

    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.fts_query = Some("payment".to_string());
    let rs = engine.search(&q).unwrap();
    assert!(rs.total_count > 0);
}

#[test]
fn test_e2e_real_world_json() {
    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();
    let path = fixture_path("real_world/nodejs_app.jsonl");
    let content = std::fs::read_to_string(&path).unwrap();
    let file_id = idx.get_or_create_file(&path).unwrap();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let _format_str = scanner::detect_format_hint(&lines);
    let entries = create_raw_entries(&content, file_id, 0);
    assert!(entries.len() > 0);
    idx.insert_entries(&entries).unwrap();

    // JSON lines should have extractable levels via scanner
    let has_levels = entries.iter().any(|e| scanner::extract_level(&e.raw).is_some());
    assert!(has_levels, "JSON fixtures should have extractable levels");

    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.fts_query = Some("gateway".to_string());
    let rs = engine.search(&q).unwrap();
    assert!(rs.total_count > 0);
}
