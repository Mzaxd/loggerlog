use crate::core::entry::LogEntry;
use crate::core::formats::LogParser;
use chrono::{NaiveDateTime, Utc};
use regex::Regex;

/// Parser for log4j style logs:
/// 2024-01-15 10:23:45,123 INFO  [main] c.m.App - Message
/// 2024-01-15 10:23:45 INFO  [main] c.m.App - Message
pub struct Log4jParser;

/// Parser for logback style logs:
/// 2024-01-15 10:23:45.123  INFO --- [main] c.m.App : Message
pub struct LogbackParser;

// Pre-compiled regex patterns (lazy_static equivalent)
fn log4j_pattern() -> &'static Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"^(\d{4}[-/]\d{2}[-/]\d{2}\s+\d{2}:\d{2}:\d{2}(?:[,.]\d+)?)\s+(TRACE|DEBUG|INFO|WARN(?:ING)?|ERROR|FATAL|SEVERE)\s+\[?([^\]]*)\]?\s+(\S+)\s*[-:]\s*(.*)$"#,
        )
        .unwrap()
    })
}

fn logback_pattern() -> &'static Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"^(\d{4}[-/]\d{2}[-/]\d{2}\s+\d{2}:\d{2}:\d{2}[,.]\d+)\s+(TRACE|DEBUG|INFO|WARN(?:ING)?|ERROR|FATAL|SEVERE)\s+---\s+\[?([^\]]*)\]?\s+(\S+)\s*[-:]\s*(.*)$"#,
        )
        .unwrap()
    })
}

fn parse_timestamp(ts: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    // Normalize: replace comma→dot for ms, slash→hyphen for date (log4j variants)
    let ts_normalized = ts.replace(',', ".").replace('/', "-");

    // Try comma-separated milliseconds (log4j) — now handled above via normalization
    if let Ok(ndt) = NaiveDateTime::parse_from_str(&ts_normalized, "%Y-%m-%d %H:%M:%S%.3f") {
        return Some(ndt.and_utc());
    }
    // Try dot-separated milliseconds (logback)
    if let Ok(ndt) = NaiveDateTime::parse_from_str(&ts_normalized, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(ndt.and_utc());
    }
    // Try ISO8601
    if let Ok(ndt) = NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S") {
        return Some(ndt.and_utc());
    }
    // Try no milliseconds
    if let Ok(ndt) = NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S") {
        return Some(ndt.and_utc());
    }
    // Try ISO8601 with timezone
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
        return Some(dt.with_timezone(&Utc));
    }
    // Try epoch millis
    if let Ok(ms) = ts.parse::<i64>() {
        if ms > 1_000_000_000_000 && ms < 2_500_000_000_000 {
            return chrono::DateTime::from_timestamp_millis(ms);
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

impl LogParser for Log4jParser {
    fn try_parse(&self, line: &str, line_number: u64, byte_offset: u64, file_id: i64) -> Option<LogEntry> {
        let re = log4j_pattern();
        let caps = re.captures(line)?;

        let timestamp_str = caps.get(1)?.as_str();
        let level = normalize_level(caps.get(2)?.as_str());
        let thread = caps.get(3).map(|m| m.as_str().to_string());
        let logger = caps.get(4).map(|m| m.as_str().to_string());
        let message = caps.get(5).map(|m| m.as_str().to_string()).unwrap_or_default();

        Some(LogEntry {
            id: None,
            file_id,
            line_number,
            byte_offset,
            timestamp: parse_timestamp(timestamp_str),
            level: Some(level),
            thread,
            logger,
            message,
            fields_json: None,
            raw: line.to_string(),
        })
    }

    fn format_name(&self) -> &str {
        "log4j"
    }
}

impl LogParser for LogbackParser {
    fn try_parse(&self, line: &str, line_number: u64, byte_offset: u64, file_id: i64) -> Option<LogEntry> {
        let re = logback_pattern();
        let caps = re.captures(line)?;

        let timestamp_str = caps.get(1)?.as_str();
        let level = normalize_level(caps.get(2)?.as_str());
        let thread = caps.get(3).map(|m| m.as_str().to_string());
        let logger = caps.get(4).map(|m| m.as_str().to_string());
        let message = caps.get(5).map(|m| m.as_str().to_string()).unwrap_or_default();

        Some(LogEntry {
            id: None,
            file_id,
            line_number,
            byte_offset,
            timestamp: parse_timestamp(timestamp_str),
            level: Some(level),
            thread,
            logger,
            message,
            fields_json: None,
            raw: line.to_string(),
        })
    }

    fn format_name(&self) -> &str {
        "logback"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log4j() {
        let line = "2024-01-15 10:23:45,123 INFO  [main] c.m.App - Application started";
        let parser = Log4jParser;
        let entry = parser.try_parse(line, 1, 0, 1).unwrap();
        assert_eq!(entry.level.as_deref(), Some("INFO"));
        assert_eq!(entry.thread.as_deref(), Some("main"));
        assert_eq!(entry.logger.as_deref(), Some("c.m.App"));
        assert_eq!(entry.message, "Application started");
    }

    #[test]
    fn test_parse_logback() {
        let line = "2024-01-15 10:23:45.123  INFO --- [http-nio-8080-exec-1] c.e.s.UserService : User login successful";
        let parser = LogbackParser;
        let entry = parser.try_parse(line, 1, 0, 1).unwrap();
        assert_eq!(entry.level.as_deref(), Some("INFO"));
        assert_eq!(entry.thread.as_deref(), Some("http-nio-8080-exec-1"));
        assert_eq!(entry.message, "User login successful");
    }
}
