use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single parsed log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: Option<i64>,
    pub file_id: i64,
    pub line_number: u64,
    pub byte_offset: u64,
    pub timestamp: Option<DateTime<Utc>>,
    pub level: Option<String>,
    pub thread: Option<String>,
    pub logger: Option<String>,
    pub message: String,
    pub fields_json: Option<String>,
    pub raw: String,
}

/// A search result with additional context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: i64,
    pub source: String,
    pub line_number: u64,
    pub byte_offset: u64,
    pub timestamp: Option<String>,
    pub level: Option<String>,
    pub thread: Option<String>,
    pub logger: Option<String>,
    pub message: String,
    pub fields_json: Option<String>,
    pub raw: String,
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
    pub limit: u32,
    pub offset: u32,
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
