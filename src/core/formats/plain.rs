use crate::core::entry::LogEntry;
use crate::core::formats::LogParser;
use regex::Regex;

/// Plain text parser - extracts timestamp and level if possible, treats rest as message
pub struct PlainParser;

fn plain_pattern() -> &'static Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"^(\d{4}[-/]\d{2}[-/]\d{2}[\sT]\d{2}:\d{2}:\d{2}(?:[,.]\d+)?(?:Z|[+-]\d{2}:?\d{2})?)\s+(TRACE|DEBUG|INFO|WARN(?:ING)?|ERROR|FATAL|SEVERE)\s*:?\s*(.*)$"#,
        )
        .unwrap()
    })
}

impl LogParser for PlainParser {
    fn try_parse(&self, line: &str, line_number: u64, byte_offset: u64, file_id: i64) -> Option<LogEntry> {
        // Plain parser always succeeds (fallback)
        let trimmed = line.trim();

        // Try to extract structured info
        if let Some(entry) = plain_pattern().captures(trimmed).and_then(|caps| {
            let ts_str = caps.get(1)?.as_str();
            let level_upper = caps.get(2)?.as_str().to_uppercase();
            let level = match level_upper.as_str() {
                "WARNING" => "WARN",
                "SEVERE" => "ERROR",
                other => other,
            };
            let message = caps.get(3)?.as_str().to_string();

            let timestamp = parse_ts(ts_str);

            Some(LogEntry {
                id: None,
                file_id,
                line_number,
                byte_offset,
                timestamp,
                level: Some(level.to_string()),
                thread: None,
                logger: None,
                message,
                fields_json: None,
                raw: line.to_string(),
            })
        }) {
            Some(entry)
        } else {
            // No structured info found, treat entire line as message
            Some(LogEntry {
                id: None,
                file_id,
                line_number,
                byte_offset,
                timestamp: None,
                level: None,
                thread: None,
                logger: None,
                message: trimmed.to_string(),
                fields_json: None,
                raw: line.to_string(),
            })
        }
    }

    fn format_name(&self) -> &str {
        "plain"
    }
}

fn parse_ts(ts: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    for fmt in [
        "%Y-%m-%d %H:%M:%S%.3f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.3f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
    ] {
        if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(ts, fmt) {
            return Some(ndt.and_utc());
        }
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    None
}
