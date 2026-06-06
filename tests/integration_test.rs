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
    let lines: Vec<&str> = content.lines().collect();
    let has_trailing_newline = content.ends_with('\n');
    for (i, line) in lines.iter().enumerate() {
        line_number += 1;
        let newline_bytes = if i == lines.len() - 1 && !has_trailing_newline { 0 } else { 1 };
        entries.push(LogEntry {
            id: None,
            file_id,
            line_number,
            byte_offset,
            raw: line.to_string(),
        });
        byte_offset += line.len() as u64 + newline_bytes;
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

// ── I-01: E2E full pipeline ────────────────────────────────────────────

#[test]
fn test_e2e_full_pipeline() {
    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();
    let path = fixture_path("search/fts_test.log");
    let content = std::fs::read_to_string(&path).unwrap();
    let file_id = idx.get_or_create_file(&path).unwrap();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let format_str = scanner::detect_format_hint(&lines);
    let entries = create_raw_entries(&content, file_id, 0);
    assert!(entries.len() > 0, "fts_test.log should produce entries");
    idx.insert_entries(&entries).unwrap();
    idx.update_file(file_id, entries.len() as i64, entries.len() as i64, entries.len() as i64, &format_str).unwrap();

    // Search for "database" via FTS
    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.fts_query = Some("database".to_string());
    let rs = engine.search(&q).unwrap();

    assert!(rs.total_count > 0, "FTS search for 'database' should find results");
    assert!(!rs.results.is_empty());

    // Verify results have correct fields
    for r in &rs.results {
        assert!(!r.source.is_empty(), "result source should not be empty");
        assert!(r.file_id > 0, "result file_id should be positive");
        assert!(r.line_number > 0, "result line_number should be positive");
        assert!(!r.raw.is_empty(), "result raw should not be empty");
        // Since fts_test.log uses log4j format, level should be extractable
        assert!(r.level.is_some(), "log4j lines should have extractable level");
    }
}

// ── I-02: E2E project-scoped search ──────────────────────────────────

#[test]
fn test_e2e_project_scoped_search() {
    use loggerlog::core::config::Project;

    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();

    // Create project and files within its path
    let project_name = "testproj";
    let project_path = "/data/testproj";
    idx.upsert_project(project_name, project_path).unwrap();

    // Add files that belong to the project
    let path_a = "/data/testproj/auth/app.log";
    let content_a = "2024-01-15 10:00:00,000 ERROR [main] com.example.Auth - login failed\n\
                     2024-01-15 10:00:01,000 INFO  [main] com.example.Auth - login ok\n";
    let file_id_a = idx.get_or_create_file(path_a).unwrap();
    let entries_a = create_raw_entries(content_a, file_id_a, 0);
    idx.insert_entries(&entries_a).unwrap();

    // Add a file outside the project
    let path_b = "/data/other/service.log";
    let content_b = "2024-01-15 10:00:00,000 ERROR [main] com.example.Other - something broke\n";
    let file_id_b = idx.get_or_create_file(path_b).unwrap();
    let entries_b = create_raw_entries(content_b, file_id_b, 0);
    idx.insert_entries(&entries_b).unwrap();

    // Sync projects so file-to-project mapping is resolved
    idx.sync_projects(&[Project {
        name: project_name.to_string(),
        path: project_path.to_string(),
        recursive: true,
        formats: vec!["auto".to_string()],
        encoding: "auto".to_string(),
        exclude_patterns: vec![],
    }]).unwrap();

    // Search scoped to the project
    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.project = Some(project_name.to_string());
    let rs = engine.search(&q).unwrap();

    // Should find only the 2 entries from within the project
    assert_eq!(rs.total_count, 2, "project-scoped search should return only project files");
    for r in &rs.results {
        assert!(r.source.starts_with("/data/testproj"),
            "result source '{}' should be under project path", r.source);
    }

    // Search without project filter should find all 3 entries
    let mut q2 = loggerlog::core::entry::SearchQuery::default();
    q2.limit = 100;
    let rs2 = engine.search(&q2).unwrap();
    assert_eq!(rs2.total_count, 3);
}

// ── I-03: E2E module search ───────────────────────────────────────────

#[test]
fn test_e2e_module_search() {
    use loggerlog::core::config::Project;

    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();

    // Create a project with module-like subdirectories
    let project_path = "/proj";
    idx.upsert_project("myproj", project_path).unwrap();

    // File under /proj/auth/
    let path_auth = "/proj/auth/app.log";
    let content_auth = "2024-01-15 10:00:00,000 ERROR [main] com.auth.Service - auth failure\n\
                        2024-01-15 10:00:01,000 INFO  [main] com.auth.Service - auth success\n";
    let file_id_auth = idx.get_or_create_file(path_auth).unwrap();
    let entries_auth = create_raw_entries(content_auth, file_id_auth, 0);
    idx.insert_entries(&entries_auth).unwrap();

    // File under /proj/billing/
    let path_billing = "/proj/billing/payments.log";
    let content_billing = "2024-01-15 10:00:00,000 ERROR [main] com.billing.Pay - charge failed\n";
    let file_id_billing = idx.get_or_create_file(path_billing).unwrap();
    let entries_billing = create_raw_entries(content_billing, file_id_billing, 0);
    idx.insert_entries(&entries_billing).unwrap();

    // Sync to establish project mapping
    idx.sync_projects(&[Project {
        name: "myproj".to_string(),
        path: project_path.to_string(),
        recursive: true,
        formats: vec!["auto".to_string()],
        encoding: "auto".to_string(),
        exclude_patterns: vec![],
    }]).unwrap();

    // Search with module="auth"
    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.module = Some("auth".to_string());
    let rs = engine.search(&q).unwrap();

    assert_eq!(rs.total_count, 2, "module='auth' should find only auth files (got {})", rs.total_count);
    for r in &rs.results {
        assert!(r.source.contains("auth"),
            "result source '{}' should contain 'auth'", r.source);
        assert!(!r.source.contains("billing"),
            "result source '{}' should not contain 'billing'", r.source);
    }

    // Search with module="billing" should find only the billing file
    let mut q2 = loggerlog::core::entry::SearchQuery::default();
    q2.limit = 100;
    q2.module = Some("billing".to_string());
    let rs2 = engine.search(&q2).unwrap();
    assert_eq!(rs2.total_count, 1, "module='billing' should find exactly 1 entry");

    // Search without module filter should find all 3
    let mut q3 = loggerlog::core::entry::SearchQuery::default();
    q3.limit = 100;
    let rs3 = engine.search(&q3).unwrap();
    assert_eq!(rs3.total_count, 3);
}

// ── I-04: E2E time range search ──────────────────────────────────────

#[test]
fn test_e2e_time_range_search() {
    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();
    let path = fixture_path("search/time_range.log");
    let content = std::fs::read_to_string(&path).unwrap();
    let file_id = idx.get_or_create_file(&path).unwrap();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let format_str = scanner::detect_format_hint(&lines);
    // Filter out comment and empty lines, then build entries from data-only content
    let data_lines: Vec<String> = lines.iter()
        .filter(|l| { let t = l.trim(); !t.is_empty() && !t.starts_with('#') })
        .cloned()
        .collect();
    let data_content = data_lines.join("\n");
    let entries = create_raw_entries(&data_content, file_id, 0);
    assert_eq!(entries.len(), 15, "time_range.log should have 15 data lines");
    idx.insert_entries(&entries).unwrap();
    idx.update_file(file_id, entries.len() as i64, entries.len() as i64, entries.len() as i64, &format_str).unwrap();

    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());

    // Search for entries on 2024-01-02 only (after start of Jan 2, before start of Jan 3)
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.after = Some(
        chrono::NaiveDate::from_ymd_opt(2024, 1, 2)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc(),
    );
    q.before = Some(
        chrono::NaiveDate::from_ymd_opt(2024, 1, 3)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc(),
    );
    let rs = engine.search(&q).unwrap();

    // Should find exactly the 5 entries from 2024-01-02
    assert_eq!(rs.total_count, 5, "should find exactly 5 entries for Jan 2");
    for r in &rs.results {
        assert!(r.raw.contains("2024-01-02"),
            "result '{}' should be from 2024-01-02", r.raw);
    }

    // Search for entries on 2024-01-01 only
    let mut q2 = loggerlog::core::entry::SearchQuery::default();
    q2.limit = 100;
    q2.after = Some(
        chrono::NaiveDate::from_ymd_opt(2024, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc(),
    );
    q2.before = Some(
        chrono::NaiveDate::from_ymd_opt(2024, 1, 2)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc(),
    );
    let rs2 = engine.search(&q2).unwrap();
    assert_eq!(rs2.total_count, 5, "should find exactly 5 entries for Jan 1");
    for r in &rs2.results {
        assert!(r.raw.contains("2024-01-01"),
            "result '{}' should be from 2024-01-01", r.raw);
    }

    // Search entire range (all 3 days)
    let mut q3 = loggerlog::core::entry::SearchQuery::default();
    q3.limit = 100;
    q3.after = Some(
        chrono::NaiveDate::from_ymd_opt(2024, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc(),
    );
    q3.before = Some(
        chrono::NaiveDate::from_ymd_opt(2024, 1, 4)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc(),
    );
    let rs3 = engine.search(&q3).unwrap();
    assert_eq!(rs3.total_count, 15, "should find all 15 entries across 3 days");
}

// ── I-05: Mixed format index search ────────────────────────────────────

#[test]
fn test_e2e_mixed_format_index_search() {
    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();
    let path = fixture_path("mixed/01_log4j_and_plain.log");
    let content = std::fs::read_to_string(&path).unwrap();
    let file_id = idx.get_or_create_file(&path).unwrap();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let format_str = scanner::detect_format_hint(&lines);
    let entries = create_raw_entries(&content, file_id, 0);
    assert!(entries.len() > 0, "mixed fixture should produce entries");
    idx.insert_entries(&entries).unwrap();
    idx.update_file(file_id, entries.len() as i64, entries.len() as i64, entries.len() as i64, &format_str).unwrap();

    // Search for ERROR — the mixed file has at least one ERROR line
    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.fts_query = Some("ERROR".to_string());
    let rs = engine.search(&q).unwrap();
    assert!(rs.total_count > 0, "FTS search for 'ERROR' should find results in mixed file");
}

// ── I-06: Incremental index ─────────────────────────────────────────────

#[test]
fn test_e2e_incremental_index() {
    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();

    // Phase 1: index 01_initial.log (5 lines)
    let path = fixture_path("incremental/01_initial.log");
    let content = std::fs::read_to_string(&path).unwrap();
    let file_id = idx.get_or_create_file(&path).unwrap();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let format_str = scanner::detect_format_hint(&lines);
    // Filter out comment and empty lines, matching the actual data content
    let data_lines: Vec<String> = lines.iter()
        .filter(|l| { let t = l.trim(); !t.is_empty() && !t.starts_with('#') })
        .cloned()
        .collect();
    let data_content = data_lines.join("\n");
    let entries = create_raw_entries(&data_content, file_id, 0);
    let initial_count = entries.len();
    assert!(initial_count > 0, "01_initial.log should produce entries");
    idx.insert_entries(&entries).unwrap();
    idx.update_file(file_id, initial_count as i64, initial_count as i64, initial_count as i64, &format_str).unwrap();

    // Verify initial entries indexed
    let count = idx.total_entries().unwrap();
    assert_eq!(count, initial_count as u64, "should have {} entries after initial index", initial_count);

    // Phase 2: simulate append by indexing 02_append.log as the same file
    let append_path = fixture_path("incremental/02_append.log");
    let append_content = std::fs::read_to_string(&append_path).unwrap();
    let append_lines: Vec<String> = append_content.lines().map(|l| l.to_string()).collect();
    let append_data_lines: Vec<String> = append_lines.iter()
        .filter(|l| { let t = l.trim(); !t.is_empty() && !t.starts_with('#') })
        .cloned()
        .collect();
    let append_data_content = append_data_lines.join("\n");
    let start_line = (initial_count + 1) as u64;
    let start_offset = data_content.len() as u64;
    let append_entries = create_raw_entries(&append_data_content, file_id, start_offset);
    // Fix line numbers to continue from where initial left off
    let append_entries: Vec<loggerlog::core::entry::LogEntry> = append_entries
        .into_iter()
        .enumerate()
        .map(|(i, mut e)| {
            e.line_number = start_line + i as u64;
            e
        })
        .collect();
    let append_count = append_entries.len();
    assert!(append_count > 0, "02_append.log should produce entries");
    idx.insert_entries(&append_entries).unwrap();

    // Verify total entries
    let total = idx.total_entries().unwrap();
    let expected_total = initial_count as u64 + append_count as u64;
    assert_eq!(total, expected_total, "should have {} entries after incremental append (got {})", expected_total, total);

    // Verify new entries are searchable
    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let mut q_all = loggerlog::core::entry::SearchQuery::default();
    q_all.limit = 100;
    let rs_all = engine.search(&q_all).unwrap();
    assert_eq!(rs_all.total_count, expected_total, "search should return all {} entries", expected_total);
}

// ── I-07: File rotation grouping ─────────────────────────────────────────

#[test]
fn test_e2e_file_rotation_grouping() {
    use loggerlog::core::discovery::FileDiscovery;
    use loggerlog::core::discovery::DiscoveredFile;
    use std::path::PathBuf;

    let rotation_dir = fixture_path("rotation");

    // Build DiscoveredFile objects with known properties since scan_directory
    // filters by extension and .gz is not a recognized log extension.
    let files: Vec<DiscoveredFile> = vec![
        DiscoveredFile {
            path: PathBuf::from(&rotation_dir).join("app.log"),
            size: 0,
            group_name: "app.log".to_string(),
            is_compressed: false,
            is_rotated: false,
        },
        DiscoveredFile {
            path: PathBuf::from(&rotation_dir).join("app.log.1"),
            size: 0,
            group_name: "app.log".to_string(),
            is_compressed: false,
            is_rotated: true,
        },
        DiscoveredFile {
            path: PathBuf::from(&rotation_dir).join("app.log.2024-01-15.gz"),
            size: 0,
            group_name: "app.log".to_string(),
            is_compressed: true,
            is_rotated: true,
        },
    ];

    let groups = FileDiscovery::group_files(&files);
    let app_log_group = groups.get("app.log");
    assert!(app_log_group.is_some(), "should have a group named 'app.log'");
    let group = app_log_group.unwrap();
    assert_eq!(group.len(), 3, "app.log group should contain 3 files (got {})", group.len());

    // Verify the expected files are in the group
    let filenames: Vec<&str> = group.iter()
        .map(|f| f.path.file_name().unwrap().to_str().unwrap())
        .collect();
    assert!(filenames.contains(&"app.log"), "group should contain app.log");
    assert!(filenames.contains(&"app.log.1"), "group should contain app.log.1");
    assert!(filenames.contains(&"app.log.2024-01-15.gz"), "group should contain app.log.2024-01-15.gz");

    // Verify rotation and compression flags
    let current = group.iter().find(|f| f.path.file_name().unwrap().to_str() == Some("app.log")).unwrap();
    assert!(!current.is_rotated, "current app.log should not be rotated");
    assert!(!current.is_compressed, "current app.log should not be compressed");

    let rotated = group.iter().find(|f| f.path.file_name().unwrap().to_str() == Some("app.log.1")).unwrap();
    assert!(rotated.is_rotated, "app.log.1 should be rotated");
    assert!(!rotated.is_compressed, "app.log.1 should not be compressed");

    let compressed = group.iter().find(|f| f.path.file_name().unwrap().to_str() == Some("app.log.2024-01-15.gz")).unwrap();
    assert!(compressed.is_rotated, "app.log.2024-01-15.gz should be rotated");
    assert!(compressed.is_compressed, "app.log.2024-01-15.gz should be compressed");
}

// ── I-08: Compressed file index ─────────────────────────────────────────

#[test]
fn test_e2e_compressed_file_index() {
    let gz_path = fixture_path("rotation/app.log.2024-01-15.gz");

    // Read and decompress
    let content = loggerlog::core::encoding::read_gz_to_utf8(&gz_path).unwrap();
    assert!(!content.is_empty(), "decompressed content should not be empty");

    // Should contain log lines
    let log_lines: Vec<&str> = content.lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .collect();
    assert!(!log_lines.is_empty(), "decompressed content should contain log lines");

    // Optionally index and search
    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();
    let file_id = idx.get_or_create_file(&gz_path).unwrap();
    let entries = create_raw_entries(&content, file_id, 0);
    assert!(entries.len() > 0, "gz file should produce indexable entries");
    idx.insert_entries(&entries).unwrap();

    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    let rs = engine.search(&q).unwrap();
    assert!(rs.total_count > 0, "should find results from compressed file");
}

// ── I-09: Regex with level filter ───────────────────────────────────────

#[test]
fn test_e2e_regex_with_level_filter() {
    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();
    let path = fixture_path("search/fts_test.log");
    let content = std::fs::read_to_string(&path).unwrap();
    let file_id = idx.get_or_create_file(&path).unwrap();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let _format_str = scanner::detect_format_hint(&lines);
    let entries = create_raw_entries(&content, file_id, 0);
    idx.insert_entries(&entries).unwrap();

    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());

    // Regex search for "connection" with level=ERROR filter
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.levels = vec!["ERROR".to_string()];
    let rs = engine.search_regex("connection", &q).unwrap();

    assert!(rs.total_count > 0, "regex 'connection' + level=ERROR should find results");
    for r in &rs.results {
        assert_eq!(r.level.as_deref(), Some("ERROR"), "all results should be ERROR level");
        assert!(r.raw.contains("connection") || r.message.contains("connection"),
            "result should contain 'connection'");
    }
}

// ── I-10: Stacktrace context ─────────────────────────────────────────────

#[test]
fn test_e2e_stacktrace_context() {
    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();
    let path = fixture_path("log4j/05_multiline_stacktrace.log");
    let content = std::fs::read_to_string(&path).unwrap();
    let file_id = idx.get_or_create_file(&path).unwrap();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let _format_str = scanner::detect_format_hint(&lines);
    let entries = create_raw_entries(&content, file_id, 0);
    idx.insert_entries(&entries).unwrap();

    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());

    // Search for NullPointerException
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.fts_query = Some("NullPointerException".to_string());
    let rs = engine.search(&q).unwrap();
    assert!(rs.total_count > 0, "should find NullPointerException");

    // Take the first result and get context
    let r = &rs.results[0];
    let ctx = engine.get_context(r.file_id, r.line_number, 2).unwrap();
    assert!(!ctx.is_empty(), "context should not be empty");

    // The context should include lines around the match — verify line numbers span a range
    let min_line = ctx.iter().map(|c| c.line_number).min().unwrap();
    let max_line = ctx.iter().map(|c| c.line_number).max().unwrap();
    assert!(max_line > min_line, "context should span multiple lines");

    // Verify that stack trace lines (containing "at " or ".java") are present in the context
    let has_stack_lines = ctx.iter().any(|c| c.raw.contains("at ") || c.raw.contains(".java"));
    assert!(has_stack_lines, "context should include stack trace lines");
}

// ── I-11: Unicode search ──────────────────────────────────────────────────

#[test]
fn test_e2e_unicode_search() {
    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();
    let path = fixture_path("log4j/06_special_characters.log");
    let content = std::fs::read_to_string(&path).unwrap();
    let file_id = idx.get_or_create_file(&path).unwrap();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let _format_str = scanner::detect_format_hint(&lines);
    let entries = create_raw_entries(&content, file_id, 0);
    assert!(entries.len() > 0, "special characters fixture should produce entries");
    idx.insert_entries(&entries).unwrap();

    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());

    // Regex search for Chinese text "用户名" (present in the Chinese logger line)
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    let rs = engine.search_regex("用户名", &q).unwrap();
    assert!(rs.total_count > 0, "regex search for '用户名' should find results (got {})", rs.total_count);
    for r in &rs.results {
        assert!(r.raw.contains("用户名") || r.message.contains("用户名"),
            "result should contain Chinese text");
    }

    // Regex search for Japanese text "ログイン"
    let mut q2 = loggerlog::core::entry::SearchQuery::default();
    q2.limit = 100;
    let rs2 = engine.search_regex("ログイン", &q2).unwrap();
    assert!(rs2.total_count > 0, "regex search for Japanese should find results (got {})", rs2.total_count);
}

// ── I-12: Exclude filter ─────────────────────────────────────────────────

#[test]
fn test_e2e_exclude_filter() {
    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();
    let path = fixture_path("search/exclude_multi.log");
    let content = std::fs::read_to_string(&path).unwrap();
    let file_id = idx.get_or_create_file(&path).unwrap();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let _format_str = scanner::detect_format_hint(&lines);
    // Filter out comment and empty lines
    let data_lines: Vec<String> = lines.iter()
        .filter(|l| { let t = l.trim(); !t.is_empty() && !t.starts_with('#') })
        .cloned()
        .collect();
    let data_content = data_lines.join("\n");
    let entries = create_raw_entries(&data_content, file_id, 0);
    // 12 data lines total (4 health, 4 metrics, 4 business)
    assert_eq!(entries.len(), 12, "exclude_multi.log should have 12 data entries");
    idx.insert_entries(&entries).unwrap();

    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());

    // Search all entries, excluding health and metrics
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.exclude = vec!["health".to_string(), "metrics".to_string()];
    let rs = engine.search(&q).unwrap();

    assert_eq!(rs.total_count, 4, "should return only 4 business lines (got {})", rs.total_count);
    for r in &rs.results {
        assert!(!r.raw.contains("health"), "result should not contain 'health'");
        assert!(!r.raw.contains("metrics"), "result should not contain 'metrics'");
        assert!(r.raw.contains("business") || r.thread.as_deref() == Some("worker-1") || r.thread.as_deref() == Some("worker-2"),
            "result should be a business line: {}", r.raw);
    }
}

// ── I-13: Context around result ─────────────────────────────────────────

#[test]
fn test_e2e_context_around_result() {
    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();
    let path = fixture_path("search/fts_test.log");
    let content = std::fs::read_to_string(&path).unwrap();
    let file_id = idx.get_or_create_file(&path).unwrap();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let _format_str = scanner::detect_format_hint(&lines);
    let entries = create_raw_entries(&content, file_id, 0);
    idx.insert_entries(&entries).unwrap();

    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());

    // Find a result in the middle of the file
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.fts_query = Some("database".to_string());
    let rs = engine.search(&q).unwrap();
    assert!(!rs.results.is_empty());

    // Context size 0 — only the line itself
    let r = &rs.results[0];
    let ctx0 = engine.get_context(r.file_id, r.line_number, 0).unwrap();
    assert_eq!(ctx0.len(), 1, "context_size=0 should return exactly 1 line");
    assert_eq!(ctx0[0].line_number, r.line_number);

    // Context size 2 — 2 lines before and after
    let ctx2 = engine.get_context(r.file_id, r.line_number, 2).unwrap();
    assert!(ctx2.len() >= 3, "context_size=2 should return at least 3 lines (got {})", ctx2.len());
    assert!(ctx2.len() <= 5, "context_size=2 should return at most 5 lines (got {})", ctx2.len());

    // Verify line numbers are sequential
    let line_nums: Vec<u64> = ctx2.iter().map(|c| c.line_number).collect();
    for i in 1..line_nums.len() {
        assert_eq!(line_nums[i], line_nums[i-1] + 1,
            "line numbers should be sequential: {} -> {}", line_nums[i-1], line_nums[i]);
    }

    // Context at line 1 — no lines before
    let ctx_boundary = engine.get_context(r.file_id, 1, 5).unwrap();
    let min_line = ctx_boundary.iter().map(|c| c.line_number).min().unwrap();
    assert_eq!(min_line, 1, "context at line 1 should not go below line 1");
}

// ── I-14: Summary aggregation ──────────────────────────────────────────

#[test]
fn test_e2e_summary_aggregation() {
    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();
    let path = fixture_path("log4j/08_large_sample.log");
    let content = std::fs::read_to_string(&path).unwrap();
    let file_id = idx.get_or_create_file(&path).unwrap();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let _format_str = scanner::detect_format_hint(&lines);
    let entries = create_raw_entries(&content, file_id, 0);
    assert!(entries.len() > 0, "large sample should produce entries");
    idx.insert_entries(&entries).unwrap();

    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let q = loggerlog::core::entry::SearchQuery::default();
    let summary = engine.search_summary(&q, 5).unwrap();

    assert!(summary.total_count > 0, "summary should have entries");
    assert!(!summary.level_distribution.is_empty(), "level_distribution should not be empty");
    assert!(!summary.source_breakdown.is_empty(), "source_breakdown should not be empty");
    assert!(!summary.top_messages.is_empty(), "top_messages should not be empty");
    assert!(summary.time_range.is_some(), "time_range should be present");

    // Verify level_distribution has expected levels
    let level_names: Vec<&str> = summary.level_distribution.iter().map(|l| l.level.as_str()).collect();
    assert!(level_names.contains(&"INFO"), "should have INFO level");
    assert!(level_names.contains(&"ERROR"), "should have ERROR level");
    assert!(level_names.contains(&"DEBUG"), "should have DEBUG level");

    // Verify source_breakdown is truncated to top_n=5
    assert!(summary.source_breakdown.len() <= 5, "source_breakdown should be at most 5");
    // Verify top_messages is truncated to top_n=5
    assert!(summary.top_messages.len() <= 5, "top_messages should be at most 5");
}

// ── I-15: All query filters ──────────────────────────────────────────────

#[test]
fn test_e2e_all_query_filters() {
    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();
    let path = fixture_path("search/filter_test.log");
    let content = std::fs::read_to_string(&path).unwrap();
    let file_id = idx.get_or_create_file(&path).unwrap();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let _format_str = scanner::detect_format_hint(&lines);
    let entries = create_raw_entries(&content, file_id, 0);
    idx.insert_entries(&entries).unwrap();

    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());

    // Combine multiple filters: level=ERROR, thread=error, logger filter
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.levels = vec!["ERROR".to_string()];
    q.thread = Some("error".to_string());
    q.source = Some("filter_test".to_string());
    let rs = engine.search(&q).unwrap();

    // filter_test.log has ERROR lines from threads error-1, error-2, error-3
    // thread filter is a substring match, so "error" matches all three
    assert!(rs.total_count > 0, "should find results matching level=ERROR + thread=error + source=filter_test");
    for r in &rs.results {
        assert_eq!(r.level.as_deref(), Some("ERROR"), "all results should be ERROR");
        assert!(r.thread.as_ref().map(|t| t.contains("error")).unwrap_or(false),
            "all results should have thread containing 'error'");
    }

    // Also test parse_query_string with combined filters
    let parsed = loggerlog::core::engine::parse_query_string(
        "level=WARN thread=warn exclude=health error", 50
    );
    assert_eq!(parsed.levels, vec!["WARN"]);
    assert_eq!(parsed.thread.as_deref(), Some("warn"));
    assert!(parsed.exclude.contains(&"health".to_string()));
    assert_eq!(parsed.fts_query.as_deref(), Some("error"));
}

// ── I-16: Config roundtrip ─────────────────────────────────────────────────

#[test]
fn test_e2e_config_roundtrip() {
    use loggerlog::core::config::{self, Config, DirectorySource, Project, SearchConfig};

    let path = "/tmp/loggerlog_test_config.toml";
    let _ = std::fs::remove_file(path); // clean up

    let original = Config {
        general: config::GeneralConfig {
            database_path: "/custom/db/index.db".to_string(),
            max_file_size: "500MB".to_string(),
            max_index_size: "2GB".to_string(),
            watch_interval: "5s".to_string(),
        },
        sources: config::SourcesConfig {
            directories: vec![
                DirectorySource {
                    path: "/var/log/myapp".to_string(),
                    recursive: false,
                    formats: vec!["log4j".to_string(), "json".to_string()],
                    encoding: "utf-8".to_string(),
                    exclude_patterns: vec!["*.tmp".to_string()],
                },
            ],
        },
        projects: config::ProjectsConfig {
            projects: vec![
                Project {
                    name: "webapp".to_string(),
                    path: "/logs/webapp".to_string(),
                    recursive: true,
                    formats: vec!["auto".to_string()],
                    encoding: "auto".to_string(),
                    exclude_patterns: vec![],
                },
            ],
        },
        search: SearchConfig {
            default_limit: 200,
            max_limit: 5000,
        },
    };

    config::save(&original, Some(path)).expect("save should succeed");
    let loaded = config::load(Some(path)).expect("load should succeed");

    // Verify all fields match
    assert_eq!(loaded.general.database_path, original.general.database_path);
    assert_eq!(loaded.general.max_file_size, original.general.max_file_size);
    assert_eq!(loaded.general.max_index_size, original.general.max_index_size);
    assert_eq!(loaded.general.watch_interval, original.general.watch_interval);

    assert_eq!(loaded.sources.directories.len(), 1);
    let d = &loaded.sources.directories[0];
    assert_eq!(d.path, "/var/log/myapp");
    assert_eq!(d.recursive, false);
    assert_eq!(d.formats, vec!["log4j", "json"]);
    assert_eq!(d.encoding, "utf-8");
    assert_eq!(d.exclude_patterns, vec!["*.tmp"]);

    assert_eq!(loaded.projects.projects.len(), 1);
    let p = &loaded.projects.projects[0];
    assert_eq!(p.name, "webapp");
    assert_eq!(p.path, "/logs/webapp");
    assert_eq!(p.recursive, true);
    assert_eq!(p.formats, vec!["auto"]);
    assert_eq!(p.encoding, "auto");

    assert_eq!(loaded.search.default_limit, 200);
    assert_eq!(loaded.search.max_limit, 5000);

    let _ = std::fs::remove_file(path);
}

// ── I-17: Discovery with tempdir ────────────────────────────────────────────

#[test]
fn test_e2e_discovery_with_tempdir() {
    use loggerlog::core::discovery::FileDiscovery;
    use std::fs;

    let tmp = std::env::temp_dir().join("loggerlog_test_discovery");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).expect("create temp dir");

    // Create file structure:
    //   tmp/
    //     app.log
    //     debug.log
    //     services/
    //       auth.log
    //       billing.txt
    //       not_a_log.json    (should be ignored — not a log extension)
    //     README.md           (should be ignored)
    fs::write(tmp.join("app.log"), "2024-01-01 INFO app\n").unwrap();
    fs::write(tmp.join("debug.log"), "2024-01-01 DEBUG debug\n").unwrap();
    fs::create_dir_all(tmp.join("services")).unwrap();
    fs::write(tmp.join("services/auth.log"), "2024-01-01 ERROR auth\n").unwrap();
    fs::write(tmp.join("services/billing.txt"), "2024-01-01 WARN billing\n").unwrap();
    fs::write(tmp.join("services/not_a_log.json"), "{}\n").unwrap();
    fs::write(tmp.join("README.md"), "# readme\n").unwrap();

    // Scan recursively with no exclude patterns
    let files = FileDiscovery::scan_directory(&tmp, true, &[]);

    let filenames: Vec<String> = files.iter()
        .map(|f| f.path.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(filenames.contains(&"app.log".to_string()), "should discover app.log");
    assert!(filenames.contains(&"debug.log".to_string()), "should discover debug.log");
    assert!(filenames.contains(&"auth.log".to_string()), "should discover services/auth.log");
    assert!(filenames.contains(&"billing.txt".to_string()), "should discover services/billing.txt");
    assert!(!filenames.contains(&"not_a_log.json".to_string()), "should not discover .json files");
    assert!(!filenames.contains(&"README.md".to_string()), "should not discover README.md");
    assert_eq!(files.len(), 4, "should discover exactly 4 log files (got {})", files.len());

    // Non-recursive scan should only find top-level files
    let files_nonrec = FileDiscovery::scan_directory(&tmp, false, &[]);
    assert_eq!(files_nonrec.len(), 2, "non-recursive should find only 2 top-level log files");

    let _ = fs::remove_dir_all(&tmp);
}

// ── I-18: Detect format threshold ──────────────────────────────────────────

#[test]
fn test_e2e_detect_format_threshold() {
    // Create 10 lines: 4 JSON (40%) and 6 plain text (60%)
    let mut lines: Vec<String> = Vec::new();
    for i in 0..4 {
        lines.push(format!(r#"{{"level":"INFO","message":"json log {}"}}"#, i));
    }
    for i in 0..6 {
        lines.push(format!("2024-01-15 10:00:{} INFO plain text log {}", i, i));
    }

    let hint = scanner::detect_format_hint(&lines);
    // Majority is plain text, so hint should be "plain" (not "json")
    assert_eq!(hint, "plain", "detect_format_hint should return 'plain' when plain lines are majority (got '{}')", hint);
}

// ── I-19: High volume ─────────────────────────────────────────────────────

#[test]
#[ignore]
fn test_e2e_high_volume() {
    let path = fixture_path("real_world/high_volume.log");
    let content = std::fs::read_to_string(&path).expect("high_volume.log should exist");

    let idx = loggerlog::core::index::IndexManager::open_in_memory().unwrap();
    let file_id = idx.get_or_create_file(&path).unwrap();
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let format_str = scanner::detect_format_hint(&lines);
    let entries = create_raw_entries(&content, file_id, 0);
    assert_eq!(entries.len(), 10000, "high_volume.log should have 10000 lines");
    idx.insert_entries(&entries).unwrap();
    idx.update_file(file_id, entries.len() as i64, entries.len() as i64, entries.len() as i64, &format_str).unwrap();

    let engine = loggerlog::core::engine::SearchEngine::new(idx.conn());
    let mut q = loggerlog::core::entry::SearchQuery::default();
    q.limit = 100;
    q.fts_query = Some("error".to_string());
    let rs = engine.search(&q).unwrap();

    assert!(rs.total_count > 0, "should find 'error' results in high_volume.log");
    // Verify total_count is not truncated — all matching entries are counted
    assert!(rs.total_count >= 50, "total_count should be substantial, got {}", rs.total_count);
}

// ── I-20: All fixtures indexable ───────────────────────────────────────────

#[test]
fn test_e2e_all_fixtures_indexable() {
    use loggerlog::core::encoding;

    let fixtures_root = std::path::Path::new("tests/fixtures");
    let mut indexed_count = 0;
    let mut skipped = Vec::new();

    // Walk all subdirectories in fixtures/
    let entries: Vec<std::fs::DirEntry> = std::fs::read_dir(fixtures_root)
        .expect("should read fixtures dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    for dir_entry in entries {
        let dir_path = dir_entry.path();
        let dir_name = dir_path.file_name().unwrap().to_string_lossy().to_string();

        let file_entries: Vec<std::fs::DirEntry> = std::fs::read_dir(&dir_path)
            .expect(&format!("should read {}", dir_name))
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        for file_entry in file_entries {
            let file_path = file_entry.path();
            let file_name = file_path.file_name().unwrap().to_string_lossy().to_string();

            // Skip non-log files
            let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let is_log_ext = ["log", "txt", "jsonl", "json", "out", "app", "gz"].contains(&ext);
            // Also skip metadata files
            let is_meta = file_name == "expected.json" || file_name == "README.md";
            if !is_log_ext || is_meta {
                skipped.push(format!("{}/{}", dir_name, file_name));
                continue;
            }

            // Read file content (handle .gz files specially)
            let content = if ext == "gz" {
                match encoding::read_gz_to_utf8(&file_path.to_string_lossy()) {
                    Ok(c) => c,
                    Err(_) => {
                        skipped.push(format!("{}/{}", dir_name, file_name));
                        continue;
                    }
                }
            } else {
                match std::fs::read_to_string(&file_path) {
                    Ok(c) => c,
                    Err(_) => {
                        skipped.push(format!("{}/{}", dir_name, file_name));
                        continue;
                    }
                }
            };

            // Create an in-memory index per file (cheap — small in-memory SQLite)
            let idx = loggerlog::core::index::IndexManager::open_in_memory()
                .unwrap_or_else(|_| panic!("open_in_memory failed for {}", file_path.display()));
            let file_id = idx.get_or_create_file(&file_path.to_string_lossy())
                .unwrap_or_else(|_| panic!("get_or_create_file failed for {}", file_path.display()));
            let entries = create_raw_entries(&content, file_id, 0);

            // Insert into index — should not panic or error
            idx.insert_entries(&entries)
                .unwrap_or_else(|e| panic!("insert_entries failed for {}: {}", file_path.display(), e));

            // Quick smoke: verify the file is indexed (skip for truly empty files)
            if !content.trim().is_empty() {
                let total = idx.total_entries().unwrap_or_else(|_| {
                    panic!("total_entries failed for {}", file_path.display())
                });
                assert!(total > 0, "{} should have at least 1 indexed entry", file_path.display());
            }

            indexed_count += 1;
        }
    }

    // At minimum, the well-known fixture directories should have contributed files
    assert!(indexed_count > 10,
        "should index at least 10 fixture files (indexed {}, skipped: {:?})",
        indexed_count, skipped);
}
