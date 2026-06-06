use chrono::{DateTime, NaiveDateTime, Utc};
use regex::Regex;
use std::sync::OnceLock;

/// Known log level keywords (from Syslog RFC 5424, common across all logging systems).
/// Order: longer keywords first to avoid partial matching (WARNING before WARN, SEVERE before ERROR).
const LEVEL_KEYWORDS: &[&str] = &[
    "WARNING",
    "SEVERE",
    "FATAL",
    "ERROR",
    "DEBUG",
    "TRACE",
    "INFO",
    "WARN",
];

/// Regex capturing all level keywords as whole words.
/// Returns the first (longest) match found in the line.
fn level_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Build alternation: longer keywords first so regex engine matches WARNING before WARN
        let pattern = LEVEL_KEYWORDS
            .iter()
            .map(|kw| regex::escape(kw))
            .collect::<Vec<_>>()
            .join("|");
        // Use \b word boundary — \b treats [a-zA-Z0-9_] as word chars,
        // so INFO_GRAPHICS and 3ERROR are correctly rejected.
        Regex::new(&format!(r"\b({})\b", pattern)).unwrap()
    })
}

/// Extract log level from a raw log line.
/// Searches for known level keywords as whole words (boundary-checked).
/// Works regardless of position — no assumptions about log format.
/// For JSON lines, also checks JSON fields "level", "severity", "lvl".
pub fn extract_level(raw: &str) -> Option<String> {
    // 1. Try JSON field extraction
    let trimmed = raw.trim();
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(obj) = value.as_object() {
                if let Some(level) = obj
                    .get("level")
                    .or_else(|| obj.get("severity"))
                    .or_else(|| obj.get("lvl"))
                    .and_then(|v| v.as_str())
                {
                    return Some(normalize_level(level));
                }
            }
        }
    }

    // 2. Try regex-based whole-word matching
    let re = level_regex();
    if let Some(caps) = re.captures(raw) {
        if let Some(m) = caps.get(1) {
            return Some(normalize_level(m.as_str()));
        }
    }

    None
}

fn normalize_level(level: &str) -> String {
    match level.to_uppercase().as_str() {
        "WARNING" => "WARN".to_string(),
        "SEVERE" => "ERROR".to_string(),
        other => other.to_string(),
    }
}

/// Extract timestamp from a raw log line.
/// Strategy: try position-based parsing (line start), then fallback to regex search.
pub fn extract_timestamp(raw: &str) -> Option<DateTime<Utc>> {
    let trimmed = raw.trim_start();

    // 1. Try parsing first token (timestamps usually at line start)
    let first_token = trimmed.split_whitespace().next()?;
    if let Some(ts) = try_parse_timestamp(first_token) {
        return Some(ts);
    }

    // 2. Try first two tokens combined ("2024-01-15 10:23:45")
    let tokens: Vec<&str> = trimmed.split_whitespace().take(2).collect();
    if tokens.len() == 2 {
        let combined = format!("{} {}", tokens[0], tokens[1]);
        if let Some(ts) = try_parse_timestamp(&combined) {
            return Some(ts);
        }
    }

    // 3. Try first three tokens for date+time+ms ("2024-01-15T10:23:45.123")
    let tokens3: Vec<&str> = trimmed.split_whitespace().take(3).collect();
    if tokens3.len() >= 2 {
        let combined3 = tokens3.join(" ");
        // Already tried 2 tokens above, try 3 for cases like "2024-01-15 10:23:45,123"
        if let Some(ts) = try_parse_timestamp(&combined3) {
            return Some(ts);
        }
    }

    // 4. Regex search across entire line for timestamp patterns
    if let Some(caps) = timestamp_regex().captures(raw) {
        if let Some(ts) = try_parse_timestamp(caps.get(0)?.as_str()) {
            return Some(ts);
        }
    }

    // 5. Try JSON timestamp extraction
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(obj) = value.as_object() {
                if let Some(ts) = extract_json_timestamp(obj) {
                    return Some(ts);
                }
            }
        }
    }

    None
}

/// Regex to find timestamp-like patterns in a line.
fn timestamp_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"\d{4}[-/]\d{2}[-/]\d{2}[\sT]\d{2}:\d{2}:\d{2}(?:[,.]\d+)?(?:Z|[+-]\d{2}:?\d{2})?",
        )
        .unwrap()
    })
}

/// Try to parse a timestamp string into DateTime<Utc>.
/// Handles: ISO8601, RFC3339, epoch millis/seconds, common log formats.
fn try_parse_timestamp(ts: &str) -> Option<DateTime<Utc>> {
    let ts_normalized = ts.replace(',', ".").replace('/', "-");

    // RFC3339 (handles timezone offsets)
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
        return Some(dt.with_timezone(&Utc));
    }

    // ISO8601 with 'T'
    for fmt in &[
        "%Y-%m-%dT%H:%M:%S%.9fZ",
        "%Y-%m-%dT%H:%M:%S%.6fZ",
        "%Y-%m-%dT%H:%M:%S%.3fZ",
        "%Y-%m-%dT%H:%M:%SZ",
        "%Y-%m-%dT%H:%M:%S%.9f",
        "%Y-%m-%dT%H:%M:%S%.6f",
        "%Y-%m-%dT%H:%M:%S%.3f",
        "%Y-%m-%dT%H:%M:%S",
    ] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(&ts_normalized, fmt) {
            return Some(ndt.and_utc());
        }
    }

    // Space-separated formats (common in log4j/logback)
    for fmt in &[
        "%Y-%m-%d %H:%M:%S%.3f",
        "%Y-%m-%d %H:%M:%S%.6f",
        "%Y-%m-%d %H:%M:%S%.9f",
        "%Y-%m-%d %H:%M:%S",
    ] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(&ts_normalized, fmt) {
            return Some(ndt.and_utc());
        }
    }

    // Epoch millis
    if let Ok(ms) = ts.parse::<i64>() {
        if ms > 1_000_000_000_000 && ms < 2_500_000_000_000 {
            return DateTime::from_timestamp_millis(ms);
        }
        // Epoch seconds
        if ms > 1_000_000_000 && ms < 2_500_000_000 {
            return DateTime::from_timestamp(ms, 0);
        }
    }

    None
}

/// Extract timestamp from a JSON object.
fn extract_json_timestamp(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Option<DateTime<Utc>> {
    let ts_val = obj
        .get("timestamp")
        .or_else(|| obj.get("@timestamp"))
        .or_else(|| obj.get("time"))
        .or_else(|| obj.get("ts"))?;

    if let Some(s) = ts_val.as_str() {
        return try_parse_timestamp(s);
    }
    if let Some(n) = ts_val.as_i64() {
        if n > 1_000_000_000_000 && n < 2_500_000_000_000 {
            return DateTime::from_timestamp_millis(n);
        }
        if n > 1_000_000_000 && n < 2_500_000_000 {
            return DateTime::from_timestamp(n, 0);
        }
    }

    None
}

/// Extract the message portion from a raw log line.
/// For JSON lines: extracts the "message" or "msg" field.
/// For structured lines: finds the content after the last common separator.
/// Falls back to the raw line itself if no extraction succeeds.
pub fn extract_message(raw: &str) -> String {
    // For JSON lines, extract the "message" or "msg" field
    let trimmed = raw.trim();
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(obj) = value.as_object() {
                if let Some(msg) = obj
                    .get("message")
                    .or_else(|| obj.get("msg"))
                    .and_then(|v| v.as_str())
                {
                    return msg.to_string();
                }
            }
        }
    }

    // Try common separators. Search from the position after the last ']'
    // (thread bracket) to avoid matching dashes inside thread names,
    // while handling both short and very long log lines.
    let search_area = if let Some(pos) = raw.find(']') {
        &raw[pos + 1..]
    } else {
        // No thread brackets — skip first 20% as a safe heuristic
        let start = raw.floor_char_boundary(raw.len() / 5);
        &raw[start..]
    };

    for sep in &[" : ", " - ", " | "] {
        if let Some(pos) = search_area.find(sep) {
            let msg = search_area[pos + sep.len()..].trim();
            // Don't return if the "message" part looks like JSON
            if !msg.starts_with('{') && !msg.starts_with('"') {
                return msg.to_string();
            }
        }
        // Also check for separator at end of line (no trailing space)
        let trailing = sep.trim_end(); // " -" " :" " |"
        if search_area.ends_with(trailing) {
            return String::new();
        }
    }

    raw.trim().to_string()
}

/// Extract thread name from a raw log line.
/// Looks for [...] bracket patterns, excluding level keywords.
pub fn extract_thread(raw: &str) -> Option<String> {
    // For JSON lines
    let trimmed = raw.trim();
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(obj) = value.as_object() {
                if let Some(thread) = obj
                    .get("thread")
                    .or_else(|| obj.get("thread_name"))
                    .and_then(|v| v.as_str())
                {
                    if !thread.is_empty() {
                        return Some(thread.to_string());
                    }
                }
            }
        }
    }

    // Find [...] bracket patterns
    let re = bracket_regex();
    for caps in re.captures_iter(raw) {
        let content = caps.get(1)?.as_str().trim();
        // Skip if it looks like a level keyword
        let upper = content.to_uppercase();
        if LEVEL_KEYWORDS.contains(&upper.as_str()) {
            continue;
        }
        // Skip empty brackets
        if content.is_empty() {
            continue;
        }
        return Some(content.to_string());
    }

    None
}

/// Regex to find [content] bracket patterns.
fn bracket_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[([^\]]+)\]").unwrap())
}

/// Find the area between the last thread bracket `]` and the message separator.
/// This is where the logger name typically lives in log4j/logback formats.
/// Returns (area slice, true if a bracket was found).
fn find_logger_area(raw: &str) -> (&str, bool) {
    // Find the first ']' (thread bracket close)
    if let Some(bracket_pos) = raw.find(']') {
        let after_bracket = &raw[bracket_pos + 1..];
        // Now find the separator in this remaining portion
        let sep_len = if let Some(p) = after_bracket.find(" - ") {
            p
        } else if let Some(p) = after_bracket.find(" : ") {
            p
        } else {
            return (after_bracket, true); // No separator found, search entire area after ]
        };
        (&after_bracket[..sep_len], true)
    } else {
        (raw, false)
    }
}

/// Extract logger/class name from a raw log line.
/// Looks for dotted.path tokens (tokens containing '.' with alphabetic parts).
pub fn extract_logger(raw: &str) -> Option<String> {
    // For JSON lines
    let trimmed = raw.trim();
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(obj) = value.as_object() {
                if let Some(logger) = obj
                    .get("logger")
                    .or_else(|| obj.get("logger_name"))
                    .or_else(|| obj.get("class"))
                    .and_then(|v| v.as_str())
                {
                    if !logger.is_empty() {
                        return Some(logger.to_string());
                    }
                }
            }
        }
    }

    // Find dotted.path tokens (e.g., com.example.service.UserService)
    // Search between the last ']' (thread bracket close) and the message separator.
    // This avoids matching dotted tokens inside thread names or message content.
    let (search_area, has_bracket) = find_logger_area(raw);
    let re = dotted_path_regex();
    if let Some(caps) = re.captures(search_area) {
        let matched = caps.get(0)?.as_str();
        // Skip if it's a timestamp (contains : or /)
        if matched.contains(':') || matched.contains('/') {
            return None;
        }
        return Some(matched.to_string());
    }

    // Fallback: try single-word identifier between ] and separator
    // Only when we found a bracket (structured log line), not for plain text
    if has_bracket {
        let single_re = single_word_logger_regex();
        if let Some(caps) = single_re.captures(search_area) {
            let matched = caps.get(0)?.as_str().trim();
            if !matched.is_empty() && !matched.contains(':') && !matched.contains('/') {
                return Some(matched.to_string());
            }
        }
    }

    None
}

/// Regex to find dotted.path class names (at least two dot-separated segments of letters).
fn dotted_path_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:[a-zA-Z_][\w$]*(?:\.[a-zA-Z_][\w$]*){1,})").unwrap())
}

/// Regex to find single-word logger names (e.g., "a", "Main") between ] and separator.
fn single_word_logger_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:[a-zA-Z_][\w$]*)").unwrap())
}

/// Quick format detection for metadata purposes (not for parsing).
/// Returns a format hint string: "json", "logback", "log4j", or "plain".
pub fn detect_format_hint(lines: &[String]) -> &'static str {
    let sample: Vec<&str> = lines
        .iter()
        .take(50)
        .filter(|l| !l.trim().is_empty())
        .map(|s| s.as_str())
        .collect();

    if sample.is_empty() {
        return "plain";
    }

    let total = sample.len();

    // JSON detection
    let json_count = sample.iter().filter(|l| l.trim().starts_with('{')).count();
    if json_count * 100 / total >= 60 {
        return "json";
    }

    // Logback detection (has " --- " separator)
    let logback_count = sample.iter().filter(|l| l.contains(" --- ")).count();
    if logback_count * 100 / total >= 60 {
        return "logback";
    }

    // Log4j detection (has level + thread bracket + logger token)
    let log4j_count = sample
        .iter()
        .filter(|l| extract_level(l).is_some() && extract_thread(l).is_some())
        .count();
    if log4j_count * 100 / total >= 60 {
        return "log4j";
    }

    "plain"
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── extract_level tests ───

    #[test]
    fn test_extract_level_log4j() {
        assert_eq!(
            extract_level("2024-01-15 10:23:45,123 INFO  [main] c.m.App - Application started"),
            Some("INFO".to_string())
        );
    }

    #[test]
    fn test_extract_level_logback() {
        assert_eq!(
            extract_level("2024-01-15 10:23:45.123  INFO --- [http-nio-8080-exec-1] c.e.s.UserService : User login"),
            Some("INFO".to_string())
        );
    }

    #[test]
    fn test_extract_level_error() {
        assert_eq!(
            extract_level("2024-01-15 10:23:45 ERROR [Thread-34] c.m.App - NullPointerException"),
            Some("ERROR".to_string())
        );
    }

    #[test]
    fn test_extract_level_gas_style() {
        // GAS format: ... [Thread-34] ERROR [] ...
        assert_eq!(
            extract_level("2026-06-04 08:39:55.004 [Thread-34] ERROR [] c.gas.service - something failed"),
            Some("ERROR".to_string())
        );
    }

    #[test]
    fn test_extract_level_json() {
        assert_eq!(
            extract_level(r#"{"level":"ERROR","message":"Connection failed"}"#),
            Some("ERROR".to_string())
        );
    }

    #[test]
    fn test_extract_level_json_lowercase() {
        assert_eq!(
            extract_level(r#"{"level":"warn","message":"Slow query"}"#),
            Some("WARN".to_string())
        );
    }

    #[test]
    fn test_extract_level_json_severity() {
        assert_eq!(
            extract_level(r#"{"severity":"SEVERE","msg":"Out of memory"}"#),
            Some("ERROR".to_string())
        );
    }

    #[test]
    fn test_extract_level_plain() {
        assert_eq!(
            extract_level("ERROR: something broke"),
            Some("ERROR".to_string())
        );
    }

    #[test]
    fn test_extract_level_warn_normalized() {
        assert_eq!(
            extract_level("WARNING: disk space low"),
            Some("WARN".to_string())
        );
    }

    #[test]
    fn test_extract_level_severe_normalized() {
        assert_eq!(
            extract_level("SEVERE: critical failure"),
            Some("ERROR".to_string())
        );
    }

    #[test]
    fn test_extract_level_none() {
        // Unstructured text without known level keywords
        assert_eq!(
            extract_level("this is just some random text"),
            None
        );
    }

    #[test]
    fn test_extract_level_empty() {
        assert_eq!(extract_level(""), None);
    }

    #[test]
    fn test_extract_level_no_false_positive() {
        // "ERRORED" should NOT match ERROR (whole word check)
        assert_eq!(
            extract_level("The operation ERRORED out"),
            None
        );
    }

    #[test]
    fn test_extract_level_no_false_positive_underscore() {
        // "INFO_GRAPHICS" should NOT match INFO (underscore is a word char)
        assert_eq!(extract_level("INFO_GRAPHICS rendering pipeline started"), None);
    }

    #[test]
    fn test_extract_level_no_false_positive_digit() {
        // "ERROR3" should NOT match ERROR (digit is a word char)
        assert_eq!(extract_level("ERROR3 process exited"), None);
    }

    #[test]
    fn test_extract_level_bracket() {
        // Level inside brackets: [ERROR]
        assert_eq!(
            extract_level("[ERROR] Connection refused"),
            Some("ERROR".to_string())
        );
    }

    // ─── extract_timestamp tests ───

    #[test]
    fn test_extract_timestamp_log4j() {
        let result = extract_timestamp("2024-01-15 10:23:45,123 INFO [main] c.m.App - msg");
        assert!(result.is_some());
        assert_eq!(result.unwrap().format("%Y-%m-%d %H:%M:%S").to_string(), "2024-01-15 10:23:45");
    }

    #[test]
    fn test_extract_timestamp_logback() {
        let result = extract_timestamp("2024-01-15 10:23:45.123  INFO --- [main] c.m.App : msg");
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_timestamp_iso8601() {
        let result = extract_timestamp("2024-01-15T10:23:45Z INFO msg");
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_timestamp_rfc3339() {
        let result = extract_timestamp("2024-01-15T10:23:45.123+08:00 INFO msg");
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_timestamp_json() {
        let result = extract_timestamp(r#"{"timestamp":"2024-01-15T10:23:45.123Z","level":"INFO"}"#);
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_timestamp_json_epoch() {
        let result = extract_timestamp(r#"{"time":1705313025123,"level":"INFO"}"#);
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_timestamp_none() {
        assert_eq!(extract_timestamp("just a plain message"), None);
    }

    #[test]
    fn test_extract_timestamp_empty() {
        assert_eq!(extract_timestamp(""), None);
    }

    // ─── extract_message tests ───

    #[test]
    fn test_extract_message_log4j() {
        assert_eq!(
            extract_message("2024-01-15 10:23:45,123 INFO  [main] c.m.App - Application started"),
            "Application started"
        );
    }

    #[test]
    fn test_extract_message_logback() {
        assert_eq!(
            extract_message("2024-01-15 10:23:45.123  INFO --- [main] c.m.App : User login successful"),
            "User login successful"
        );
    }

    #[test]
    fn test_extract_message_json() {
        assert_eq!(
            extract_message(r#"{"timestamp":"2024-01-15T10:23:45Z","level":"INFO","message":"Application started"}"#),
            "Application started"
        );
    }

    #[test]
    fn test_extract_message_json_msg_field() {
        assert_eq!(
            extract_message(r#"{"level":"ERROR","msg":"Connection failed"}"#),
            "Connection failed"
        );
    }

    #[test]
    fn test_extract_message_plain() {
        assert_eq!(
            extract_message("just a plain message"),
            "just a plain message"
        );
    }

    #[test]
    fn test_extract_message_empty() {
        assert_eq!(extract_message(""), "");
    }

    #[test]
    fn test_extract_message_takes_first_separator() {
        // Multiple " - " separators — should take the first structural separator
        // (the one after the logger name), keeping the rest as message content
        assert_eq!(
            extract_message("2024-01-15 INFO [main] c.m.App - first part - actual message"),
            "first part - actual message"
        );
    }

    // ─── extract_thread tests ───

    #[test]
    fn test_extract_thread_log4j() {
        assert_eq!(
            extract_thread("2024-01-15 10:23:45,123 INFO  [main] c.m.App - msg"),
            Some("main".to_string())
        );
    }

    #[test]
    fn test_extract_thread_logback() {
        assert_eq!(
            extract_thread("2024-01-15 10:23:45.123  INFO --- [http-nio-8080-exec-1] c.m.App : msg"),
            Some("http-nio-8080-exec-1".to_string())
        );
    }

    #[test]
    fn test_extract_thread_json() {
        assert_eq!(
            extract_thread(r#"{"thread":"main","level":"INFO","message":"started"}"#),
            Some("main".to_string())
        );
    }

    #[test]
    fn test_extract_thread_empty_brackets() {
        // GAS style: [] empty brackets should be skipped
        assert_eq!(
            extract_thread("2026-06-04 08:39:55 [Thread-34] ERROR [] c.m.App - msg"),
            Some("Thread-34".to_string())
        );
    }

    #[test]
    fn test_extract_thread_none() {
        assert_eq!(extract_thread("just a plain message"), None);
    }

    // ─── extract_logger tests ───

    #[test]
    fn test_extract_logger_log4j() {
        assert_eq!(
            extract_logger("2024-01-15 10:23:45,123 INFO  [main] c.m.App - msg"),
            Some("c.m.App".to_string())
        );
    }

    #[test]
    fn test_extract_logger_logback() {
        assert_eq!(
            extract_logger("2024-01-15 10:23:45.123  INFO --- [main] c.e.s.UserService : msg"),
            Some("c.e.s.UserService".to_string())
        );
    }

    #[test]
    fn test_extract_logger_json() {
        assert_eq!(
            extract_logger(r#"{"logger":"com.example.App","level":"INFO","message":"started"}"#),
            Some("com.example.App".to_string())
        );
    }

    #[test]
    fn test_extract_logger_none() {
        assert_eq!(extract_logger("just a plain message"), None);
    }

    #[test]
    fn test_extract_logger_skips_timestamps() {
        // Timestamps with dots (e.g., 2024.01.15) should not be mistaken for loggers
        assert_eq!(
            extract_logger("2024.01.15 10:23:45 INFO msg"),
            None
        );
    }

    // ─── detect_format_hint tests ───

    #[test]
    fn test_detect_format_json() {
        let lines = vec![
            r#"{"level":"INFO","message":"msg1"}"#.to_string(),
            r#"{"level":"ERROR","message":"msg2"}"#.to_string(),
            r#"{"level":"WARN","message":"msg3"}"#.to_string(),
        ];
        assert_eq!(detect_format_hint(&lines), "json");
    }

    #[test]
    fn test_detect_format_logback() {
        let lines = vec![
            "2024-01-15 10:23:45.123  INFO --- [main] c.m.App : msg1".to_string(),
            "2024-01-15 10:23:45.124  INFO --- [main] c.m.App : msg2".to_string(),
        ];
        assert_eq!(detect_format_hint(&lines), "logback");
    }

    #[test]
    fn test_detect_format_log4j() {
        let lines = vec![
            "2024-01-15 10:23:45 INFO [main] c.m.App - msg1".to_string(),
            "2024-01-15 10:23:46 ERROR [main] c.m.App - msg2".to_string(),
        ];
        assert_eq!(detect_format_hint(&lines), "log4j");
    }

    #[test]
    fn test_detect_format_plain() {
        let lines = vec![
            "just some text".to_string(),
            "more random text".to_string(),
        ];
        assert_eq!(detect_format_hint(&lines), "plain");
    }

    #[test]
    fn test_detect_format_empty() {
        let lines: Vec<String> = vec![];
        assert_eq!(detect_format_hint(&lines), "plain");
    }

    // ─── Performance test ───

    #[test]
    fn test_extract_level_performance() {
        let lines: Vec<String> = (0..228_481)
            .map(|i| {
                format!(
                    "2026-06-04 08:39:55.{:03} [Thread-{:02}] ERROR [] c.gas.Service - error number {}",
                    i % 1000,
                    i % 50,
                    i
                )
            })
            .collect();

        let start = std::time::Instant::now();
        let mut found = 0usize;
        for line in &lines {
            if extract_level(line).is_some() {
                found += 1;
            }
        }
        let elapsed = start.elapsed();

        assert_eq!(found, 228_481, "all lines should have a level");
        assert!(
            elapsed.as_millis() < 2000,
            "228k lines should be scanned in <2s, took {}ms",
            elapsed.as_millis()
        );
    }
}
