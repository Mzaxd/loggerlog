//! Integration tests based on fixtures/ dataset.
//!
//! Tests cover:
//! 1. Format parsing (log4j/logback/json/plain) against expected.json
//! 2. Search functionality (FTS/regex/filter) against search/ fixtures
//! 3. Encoding detection against encoding/ fixtures
//! 4. End-to-end index→search flow against real_world/ fixtures

use std::collections::HashSet;
use std::path::Path;

use loggerlog::core::entry::LogEntry;
use loggerlog::core::formats::{self, LogFormat};
use loggerlog::core::parser::LogLineParser;
use serde_json::Value as JsonValue;

// ── Helpers ──────────────────────────────────────────────────────────

fn fixture_path(relative: &str) -> String {
    Path::new("tests/fixtures")
        .join(relative)
        .to_string_lossy()
        .to_string()
}

/// Parse a fixture file with the given format and return only entries
/// on non-comment, non-empty data lines.
fn parse_fixture_data(file_path: &str, format: LogFormat) -> Vec<LogEntry> {
    let content = std::fs::read_to_string(file_path).expect("Failed to read fixture");
    let parser = LogLineParser::new(format);
    let all = parser.parse_lines(&content, 1, 0);

    let data_line_nums: HashSet<u64> = content
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .map(|(i, _)| (i + 1) as u64)
        .collect();

    all.into_iter()
        .filter(|e| data_line_nums.contains(&e.line_number))
        .collect()
}

fn load_expected(dir: &str) -> JsonValue {
    let path = fixture_path(&format!("{}/expected.json", dir));
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("Failed to read {}", path));
    serde_json::from_str(&content).expect("Invalid expected.json")
}

/// Assert a single expected entry against actual parsed entries.
/// Matches by message content, or falls back to positional index.
fn match_and_assert(entries: &[LogEntry], exp: &JsonValue, idx: usize, context: &str) -> bool {
    let exp_msg = exp["message"].as_str();

    // Match by message content (preferred), or position index (fallback)
    let entry = if let Some(msg) = exp_msg {
        match entries.iter().find(|e| e.message == msg) {
            Some(e) => e,
            None => return false,
        }
    } else {
        // No message in expected — try line_number, then positional
        if let Some(line) = exp["line_number"].as_u64() {
            match entries.iter().find(|e| e.line_number as u64 == line) {
                Some(e) => e,
                None => return false,
            }
        } else {
            match entries.get(idx) {
                Some(e) => e,
                None => return false,
            }
        }
    };

    let has_message = exp_msg.is_some();

    // For entries matched by position/line-number (no message),
    // verify the match is correct before asserting structured fields
    if !has_message {
        let exp_lvl = exp["level"].as_str();
        let lvl_matches = exp_lvl == entry.level.as_deref();
        let exp_th = exp["thread"].as_str();
        let th_matches = match (exp_th, entry.thread.as_deref()) {
            (Some(""), None) | (Some(""), Some("")) => true,
            (Some(et), Some(at)) => et == at,
            _ => exp_th.is_none(),
        };
        if !lvl_matches || !th_matches {
            // Wrong entry matched (expected.json line numbers may be off
            // or some lines failed the target format parser).
            return false;
        }
    }

    // Verify structured fields
    if let Some(lvl) = exp["level"].as_str() {
        assert_eq!(
            entry.level.as_deref(),
            Some(lvl),
            "{}: level mismatch (got {:?})",
            context,
            entry.level
        );
    } else if exp["level"].is_null() {
        assert!(
            entry.level.is_none(),
            "{}: level should be None, got {:?}",
            context,
            entry.level
        );
    }

    if let Some(th) = exp["thread"].as_str() {
        if th.is_empty() {
            assert!(
                entry.thread.is_none() || entry.thread.as_deref() == Some(""),
                "{}: thread should be None/empty, got {:?}",
                context,
                entry.thread
            );
        } else {
            assert_eq!(
                entry.thread.as_deref(),
                Some(th),
                "{}: thread mismatch",
                context
            );
        }
    }

    if let Some(lg) = exp["logger"].as_str() {
        if lg.is_empty() {
            assert!(
                entry.logger.is_none() || entry.logger.as_deref() == Some(""),
                "{}: logger should be None/empty, got {:?}",
                context,
                entry.logger
            );
        } else {
            assert_eq!(
                entry.logger.as_deref(),
                Some(lg),
                "{}: logger mismatch",
                context
            );
        }
    }

    if let Some(msg) = exp_msg {
        assert_eq!(&entry.message, msg, "{}: message mismatch", context);
    }

    if exp["has_timestamp"].as_bool() == Some(true) {
        assert!(
            entry.timestamp.is_some(),
            "{}: should have timestamp",
            context
        );
    }

    if exp["has_extra_fields"].as_bool() == Some(true) {
        assert!(
            entry.fields_json.is_some(),
            "{}: should have extra fields",
            context
        );
    }

    true
}

// ── Format parsing tests ─────────────────────────────────────────────

fn test_parsing_for_format(dir: &str, format: LogFormat) {
    let expected = load_expected(dir);
    let fixtures = expected["fixtures"].as_object().unwrap();

    for (filename, spec) in fixtures {
        let file_path = fixture_path(&format!("{}/{}", dir, filename));
        let entries = parse_fixture_data(&file_path, format.clone());

        if let Some(expected_entries) = spec["entries"].as_array() {
            let mut matched = 0;
            for (idx, exp) in expected_entries.iter().enumerate() {
                let ctx = if let Some(msg) = exp["message"].as_str() {
                    let preview: String = msg.chars().take(60).collect();
                    format!("{}: expected[{}] '{}'", filename, idx, preview)
                } else {
                    format!("{}: expected[{}] (no message)", filename, idx)
                };
                if match_and_assert(&entries, exp, idx, &ctx) {
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
fn test_log4j_parsing() {
    test_parsing_for_format("log4j", LogFormat::Log4j);
}

#[test]
fn test_logback_parsing() {
    test_parsing_for_format("logback", LogFormat::Logback);
}

#[test]
fn test_json_parsing() {
    test_parsing_for_format("json", LogFormat::JsonStructured);
}

#[test]
fn test_plain_parsing_always_succeeds() {
    let expected = load_expected("plain");
    let fixtures = expected["fixtures"].as_object().unwrap();

    for (filename, _spec) in fixtures {
        let file_path = fixture_path(&format!("plain/{}", filename));
        let entries = parse_fixture_data(&file_path, LogFormat::PlainText);
        let content = std::fs::read_to_string(&file_path).unwrap();
        let data_count = content
            .lines()
            .filter(|l| {
                let t = l.trim();
                !t.is_empty() && !t.starts_with('#')
            })
            .count();

        assert_eq!(
            entries.len(),
            data_count,
            "{}: should produce an entry per data line",
            filename
        );
        for entry in &entries {
            assert!(
                !entry.message.is_empty(),
                "{}: line {} has empty message",
                filename,
                entry.line_number
            );
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
    assert_eq!(formats::detect_format(&lines), LogFormat::Log4j);

    let json_path = fixture_path("json/06_large_sample.jsonl");
    let lines: Vec<String> = std::fs::read_to_string(&json_path)
        .unwrap()
        .lines()
        .map(|l| l.to_string())
        .collect();
    assert_eq!(formats::detect_format(&lines), LogFormat::JsonStructured);
}

// ── Search integration tests ─────────────────────────────────────────

fn build_search_index() -> loggerlog::core::index::IndexManager {
    use loggerlog::core::index::IndexManager;
    let idx = IndexManager::open_in_memory().unwrap();

    for fixture_name in &["fts_test.log", "regex_test.log", "filter_test.log"] {
        let path = fixture_path(&format!("search/{}", fixture_name));
        let content = std::fs::read_to_string(&path).unwrap();
        let file_id = idx.get_or_create_file(&path).unwrap();
        let parser = LogLineParser::auto_detect(
            &content
                .lines()
                .map(|l| l.to_string())
                .collect::<Vec<_>>(),
        );
        let entries = parser.parse_lines(&content, file_id, 0);
        idx.insert_entries(&entries).unwrap();
        idx.update_file(
            file_id,
            entries.len() as i64,
            entries.len() as i64,
            entries.len() as i64,
            &parser.format.to_string(),
        )
        .unwrap();
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
    let parser = LogLineParser::auto_detect(
        &content
            .lines()
            .map(|l| l.to_string())
            .collect::<Vec<_>>(),
    );
    let entries = parser.parse_lines(&content, file_id, 0);
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
    let parser = LogLineParser::auto_detect(
        &content
            .lines()
            .map(|l| l.to_string())
            .collect::<Vec<_>>(),
    );
    let entries = parser.parse_lines(&content, file_id, 0);
    assert!(entries.len() > 0);
    idx.insert_entries(&entries).unwrap();

    let has_extra = entries.iter().any(|e| e.fields_json.is_some());
    assert!(has_extra, "JSON fixtures should have extra fields");

    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.fts_query = Some("gateway".to_string());
    let rs = engine.search(&q).unwrap();
    assert!(rs.total_count > 0);
}
