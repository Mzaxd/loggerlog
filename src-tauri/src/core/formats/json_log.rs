use crate::core::entry::LogEntry;
use crate::core::formats::LogParser;

/// Parser for JSON structured logs
/// {"timestamp":"2024-01-15T10:23:45.123Z","level":"INFO","message":"Application started","thread":"main","logger":"c.m.App"}
pub struct JsonParser;

impl LogParser for JsonParser {
    fn try_parse(&self, line: &str, line_number: u64, byte_offset: u64, file_id: i64) -> Option<LogEntry> {
        let value: serde_json::Value = serde_json::from_str(line).ok()?;

        if !value.is_object() {
            return None;
        }

        let obj = value.as_object()?;

        // Extract timestamp from various field names
        let timestamp = extract_timestamp(obj);

        // Extract level from various field names
        let level = obj
            .get("level")
            .or_else(|| obj.get("severity"))
            .or_else(|| obj.get("lvl"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_uppercase());

        // Extract thread
        let thread = obj
            .get("thread")
            .or_else(|| obj.get("thread_name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Extract logger
        let logger = obj
            .get("logger")
            .or_else(|| obj.get("logger_name"))
            .or_else(|| obj.get("class"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Extract message
        let message = obj
            .get("message")
            .or_else(|| obj.get("msg"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Extract extra fields (everything except the standard fields)
        let mut fields = serde_json::Map::new();
        let standard_keys = [
            "timestamp", "time", "ts", "@timestamp",
            "level", "severity", "lvl",
            "thread", "thread_name",
            "logger", "logger_name", "class",
            "message", "msg",
        ];
        for (key, val) in obj.iter() {
            if !standard_keys.contains(&key.as_str()) {
                fields.insert(key.clone(), val.clone());
            }
        }
        let fields_json = if fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&fields).unwrap_or_default())
        };

        Some(LogEntry {
            id: None,
            file_id,
            line_number,
            byte_offset,
            timestamp,
            level,
            thread,
            logger,
            message,
            fields_json,
            raw: line.to_string(),
        })
    }

    fn format_name(&self) -> &str {
        "json"
    }
}

fn extract_timestamp(obj: &serde_json::Map<String, serde_json::Value>) -> Option<chrono::DateTime<chrono::Utc>> {
    let ts_str = obj
        .get("timestamp")
        .or_else(|| obj.get("@timestamp"))
        .or_else(|| obj.get("time"))
        .or_else(|| obj.get("ts"))
        .and_then(|v| {
            if v.is_string() {
                v.as_str().map(|s| s.to_string())
            } else if v.is_number() {
                v.as_i64().map(|n| n.to_string())
            } else {
                None
            }
        })?;

    // Try epoch millis
    if let Ok(ms) = ts_str.parse::<i64>() {
        if ms > 1_000_000_000_000 && ms < 2_500_000_000_000 {
            return chrono::DateTime::from_timestamp_millis(ms);
        }
        // Try epoch seconds
        if ms > 1_000_000_000 && ms < 2_500_000_000 {
            return chrono::DateTime::from_timestamp(ms, 0);
        }
    }

    // Try ISO8601
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&ts_str) {
        return Some(dt.with_timezone(&chrono::Utc));
    }

    // Try common formats
    for fmt in [
        "%Y-%m-%d %H:%M:%S%.3f",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.3f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.3fZ",
        "%Y-%m-%dT%H:%M:%SZ",
    ] {
        if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(&ts_str, fmt) {
            return Some(ndt.and_utc());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_log() {
        let line = r#"{"timestamp":"2024-01-15T10:23:45.123Z","level":"INFO","message":"Application started","thread":"main"}"#;
        let parser = JsonParser;
        let entry = parser.try_parse(line, 1, 0, 1).unwrap();
        assert_eq!(entry.level.as_deref(), Some("INFO"));
        assert_eq!(entry.thread.as_deref(), Some("main"));
        assert_eq!(entry.message, "Application started");
    }

    #[test]
    fn test_parse_json_log_with_fields() {
        let line = r#"{"time":"2024-01-15T10:23:45Z","severity":"ERROR","msg":"Connection failed","requestId":"abc-123"}"#;
        let parser = JsonParser;
        let entry = parser.try_parse(line, 1, 0, 1).unwrap();
        assert_eq!(entry.level.as_deref(), Some("ERROR"));
        assert!(entry.fields_json.is_some());
    }
}
