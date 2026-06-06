use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single log entry — simplified to raw-only storage.
/// Structured fields (timestamp, level, thread, logger, message) are extracted at query time
/// by scanner functions, not stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: Option<i64>,
    pub file_id: i64,
    pub line_number: u64,
    pub byte_offset: u64,
    pub raw: String,
}

/// A search result with display fields populated at query time from raw text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: i64,
    pub file_id: i64,
    pub source: String,
    pub line_number: u64,
    pub byte_offset: u64,
    pub timestamp: Option<String>,
    pub level: Option<String>,
    pub thread: Option<String>,
    pub logger: Option<String>,
    pub message: String,
    pub raw: String,
}

impl SearchResult {
    /// Create a SearchResult from a raw log line, extracting all fields via scanner functions.
    pub fn from_raw(
        id: i64,
        file_id: i64,
        source: String,
        line_number: u64,
        byte_offset: u64,
        raw: &str,
    ) -> Self {
        let level = crate::core::scanner::extract_level(raw);
        let timestamp = crate::core::scanner::extract_timestamp(raw).map(|dt| dt.to_rfc3339());
        let thread = crate::core::scanner::extract_thread(raw);
        let logger = crate::core::scanner::extract_logger(raw);
        let message = crate::core::scanner::extract_message(raw);
        Self {
            id,
            file_id,
            source,
            line_number,
            byte_offset,
            timestamp,
            level,
            thread,
            logger,
            message,
            raw: raw.to_string(),
        }
    }
}

/// Search query parameters
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub fts_query: Option<String>,
    pub regex_query: Option<String>,
    pub levels: Vec<String>,
    pub source: Option<String>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub thread: Option<String>,
    pub logger: Option<String>,
    pub project: Option<String>,
    pub module: Option<String>,
    pub limit: u32,
    pub offset: u32,
    pub exclude: Vec<String>,
}

/// Index statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub database_size_mb: f64,
    pub total_files: usize,
    pub total_entries: u64,
    pub fts_index_size_mb: f64,
    pub files: Vec<FileStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStats {
    pub path: String,
    pub format: String,
    pub entries: u64,
    pub size_mb: f64,
    pub indexed_to: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::scanner;

    #[test]
    fn test_search_result_from_raw_log4j() {
        let raw = "2024-01-15 10:00:00,000 ERROR [main] com.example.App - NullPointerException: something broke";
        let result = SearchResult::from_raw(1, 10, "app.log".to_string(), 42, 1024, raw);

        assert_eq!(result.id, 1);
        assert_eq!(result.file_id, 10);
        assert_eq!(result.source, "app.log");
        assert_eq!(result.line_number, 42);
        assert_eq!(result.byte_offset, 1024);
        assert_eq!(result.level, Some("ERROR".to_string()));
        assert_eq!(result.thread, Some("main".to_string()));
        assert_eq!(result.logger, Some("com.example.App".to_string()));
        assert!(result.message.contains("NullPointerException"));
        assert!(result.timestamp.is_some());
        assert_eq!(result.raw, raw);
    }

    #[test]
    fn test_search_result_from_raw_json() {
        let raw =
            r#"{"level":"error","message":"connection failed","timestamp":"2024-01-15T10:00:00Z"}"#;
        let result = SearchResult::from_raw(2, 20, "app.json.log".to_string(), 10, 512, raw);

        // JSON "error" is normalized to uppercase by scanner
        assert_eq!(result.level, Some("ERROR".to_string()));
        assert!(result.message.contains("connection failed"));
        assert!(result.timestamp.is_some());
        assert_eq!(result.raw, raw);
    }

    #[test]
    fn test_search_result_from_raw_plain() {
        let raw = "something happened at midnight";
        let result = SearchResult::from_raw(3, 30, "debug.log".to_string(), 1, 0, raw);

        assert_eq!(result.level, None);
        assert_eq!(result.thread, None);
        assert_eq!(result.logger, None);
        assert_eq!(result.timestamp, None);
        assert_eq!(result.message, raw.trim().to_string());
        assert_eq!(result.raw, raw);
    }

    #[test]
    fn test_search_result_timestamp_rfc3339() {
        let raw = "2024-01-15 10:00:00,000 INFO [main] com.example.App - all good";
        let result = SearchResult::from_raw(4, 40, "app.log".to_string(), 5, 256, raw);

        let ts = result.timestamp.expect("should have a timestamp");
        assert!(
            ts.starts_with("2024"),
            "timestamp should be in RFC3339 format and start with '2024', got: {}",
            ts
        );
    }

    #[test]
    fn test_search_result_from_raw_scanner_consistency() {
        let raw = "2024-01-15 10:00:00,000 WARN [pool-1-thread-2] com.example.DB - slow query";
        let result = SearchResult::from_raw(5, 50, "slow.log".to_string(), 99, 8192, raw);

        // Verify that from_raw fields match direct scanner calls
        assert_eq!(result.level, scanner::extract_level(raw));
        assert_eq!(
            result.timestamp,
            scanner::extract_timestamp(raw).map(|dt| dt.to_rfc3339())
        );
        assert_eq!(result.thread, scanner::extract_thread(raw));
        assert_eq!(result.logger, scanner::extract_logger(raw));
        assert_eq!(result.message, scanner::extract_message(raw));
    }
}
