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
        let timestamp =
            crate::core::scanner::extract_timestamp(raw).map(|dt| dt.to_rfc3339());
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
