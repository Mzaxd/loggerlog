use crate::core::entry::{SearchQuery, SearchResult};
use anyhow::Result;
use rusqlite::{params, Connection};
use std::time::Instant;

/// Search engine that queries the SQLite FTS5 index
pub struct SearchEngine<'a> {
    conn: &'a Connection,
}

impl<'a> SearchEngine<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Execute a search query and return results
    pub fn search(&self, query: &SearchQuery) -> Result<SearchResultSet> {
        let start = Instant::now();
        let mut conditions = vec!["1=1".to_string()];
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        // FTS full-text search
        if let Some(ref fts) = query.fts_query {
            conditions.push(
                "e.id IN (SELECT rowid FROM log_entries_fts WHERE log_entries_fts MATCH ?)".to_string()
            );
            params.push(Box::new(fts.clone()));
        }

        // Level filter
        if !query.levels.is_empty() {
            let placeholders: Vec<String> = query.levels.iter().map(|_| "?".to_string()).collect();
            conditions.push(format!("level IN ({})", placeholders.join(",")));
            for level in &query.levels {
                params.push(Box::new(level.clone()));
            }
        }

        // Source filter
        if let Some(ref source) = query.source {
            conditions.push("file_id IN (SELECT id FROM files WHERE path LIKE ?)".to_string());
            params.push(Box::new(format!("%{}%", source)));
        }

        // Time range filters
        if let Some(ref after) = query.after {
            conditions.push("timestamp >= ?".to_string());
            params.push(Box::new(after.to_rfc3339()));
        }
        if let Some(ref before) = query.before {
            conditions.push("timestamp <= ?".to_string());
            params.push(Box::new(before.to_rfc3339()));
        }

        // Thread filter
        if let Some(ref thread) = query.thread {
            conditions.push("thread LIKE ?".to_string());
            params.push(Box::new(format!("%{}%", thread)));
        }

        // Logger filter
        if let Some(ref logger) = query.logger {
            conditions.push("logger LIKE ?".to_string());
            params.push(Box::new(format!("%{}%", logger)));
        }

        let where_clause = conditions.join(" AND ");

        // Count total matching results
        let count_sql = format!(
            "SELECT COUNT(*) FROM log_entries e WHERE {}", where_clause
        );
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|b| b.as_ref()).collect();
        let total_count: u64 = self.conn.query_row(&count_sql, param_refs.as_slice(), |row| row.get(0))?;

        // Fetch results with pagination — rebuild params with limit/offset appended
        let select_sql = format!(
            "SELECT e.id, f.path, e.line_number, e.byte_offset, e.timestamp, e.level, e.thread, e.logger, e.message, e.fields_json, e.raw
             FROM log_entries e
             JOIN files f ON e.file_id = f.id
             WHERE {}
             ORDER BY e.timestamp DESC
             LIMIT ? OFFSET ?",
            where_clause
        );

        let mut select_params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(ref fts) = query.fts_query { select_params.push(Box::new(fts.clone())); }
        if !query.levels.is_empty() {
            for level in &query.levels { select_params.push(Box::new(level.clone())); }
        }
        if let Some(ref source) = query.source { select_params.push(Box::new(format!("%{}%", source))); }
        if let Some(ref after) = query.after { select_params.push(Box::new(after.to_rfc3339())); }
        if let Some(ref before) = query.before { select_params.push(Box::new(before.to_rfc3339())); }
        if let Some(ref thread) = query.thread { select_params.push(Box::new(format!("%{}%", thread))); }
        if let Some(ref logger) = query.logger { select_params.push(Box::new(format!("%{}%", logger))); }
        select_params.push(Box::new(query.limit as i64));
        select_params.push(Box::new(query.offset as i64));
        let select_refs: Vec<&dyn rusqlite::types::ToSql> = select_params.iter().map(|b| b.as_ref()).collect();

        let mut stmt = self.conn.prepare(&select_sql)?;
        let rows = stmt.query_map(select_refs.as_slice(), |row| {
            Ok(SearchResult {
                id: row.get(0)?,
                source: row.get(1)?,
                line_number: row.get::<_, i64>(2)? as u64,
                byte_offset: row.get::<_, i64>(3)? as u64,
                timestamp: row.get(4)?,
                level: row.get(5)?,
                thread: row.get(6)?,
                logger: row.get(7)?,
                message: row.get(8)?,
                fields_json: row.get(9)?,
                raw: row.get(10)?,
            })
        })?;

        let results: Vec<SearchResult> = rows.filter_map(|r| r.ok()).collect();
        let elapsed = start.elapsed();

        Ok(SearchResultSet {
            total_count,
            returned_count: results.len() as u64,
            offset: query.offset,
            elapsed_ms: elapsed.as_millis() as u64,
            results,
        })
    }

    /// Regex search: scan raw column with regex (slower)
    pub fn search_regex(&self, pattern: &str, query: &SearchQuery) -> Result<SearchResultSet> {
        let re = regex::Regex::new(pattern)
            .map_err(|e| anyhow::anyhow!("Invalid regex pattern: {}", e))?;

        let start = Instant::now();

        // First get candidate rows using non-FTS filters
        let mut conditions = vec!["1=1".to_string()];
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if !query.levels.is_empty() {
            let placeholders: Vec<String> = query.levels.iter().map(|_| "?".to_string()).collect();
            conditions.push(format!("level IN ({})", placeholders.join(",")));
            for level in &query.levels {
                param_values.push(Box::new(level.clone()));
            }
        }

        if let Some(ref source) = query.source {
            conditions.push("file_id IN (SELECT id FROM files WHERE path LIKE ?)".to_string());
            param_values.push(Box::new(format!("%{}%", source)));
        }

        if let Some(ref after) = query.after {
            conditions.push("timestamp >= ?".to_string());
            param_values.push(Box::new(after.to_rfc3339()));
        }
        if let Some(ref before) = query.before {
            conditions.push("timestamp <= ?".to_string());
            param_values.push(Box::new(before.to_rfc3339()));
        }

        let where_clause = conditions.join(" AND ");

        // Fetch more candidates for regex filtering (up to 10x limit)
        let scan_limit = query.limit * 10;
        let select_sql = format!(
            "SELECT e.id, f.path, e.line_number, e.byte_offset, e.timestamp, e.level, e.thread, e.logger, e.message, e.fields_json, e.raw
             FROM log_entries e
             JOIN files f ON e.file_id = f.id
             WHERE {}
             ORDER BY e.timestamp DESC
             LIMIT ?",
            where_clause
        );

        let mut select_params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if !query.levels.is_empty() {
            for level in &query.levels { select_params.push(Box::new(level.clone())); }
        }
        if let Some(ref source) = query.source { select_params.push(Box::new(format!("%{}%", source))); }
        if let Some(ref after) = query.after { select_params.push(Box::new(after.to_rfc3339())); }
        if let Some(ref before) = query.before { select_params.push(Box::new(before.to_rfc3339())); }
        select_params.push(Box::new(scan_limit as i64));
        let select_refs: Vec<&dyn rusqlite::types::ToSql> = select_params.iter().map(|b| b.as_ref()).collect();

        let mut stmt = self.conn.prepare(&select_sql)?;
        let rows = stmt.query_map(select_refs.as_slice(), |row| {
            Ok(SearchResult {
                id: row.get(0)?,
                source: row.get(1)?,
                line_number: row.get::<_, i64>(2)? as u64,
                byte_offset: row.get::<_, i64>(3)? as u64,
                timestamp: row.get(4)?,
                level: row.get(5)?,
                thread: row.get(6)?,
                logger: row.get(7)?,
                message: row.get(8)?,
                fields_json: row.get(9)?,
                raw: row.get(10)?,
            })
        })?;

        let mut results: Vec<SearchResult> = rows
            .filter_map(|r| r.ok())
            .filter(|r| re.is_match(&r.raw))
            .take(query.limit as usize)
            .collect();

        let total_count = results.len() as u64;
        results.truncate(query.limit as usize);
        let elapsed = start.elapsed();

        Ok(SearchResultSet {
            total_count,
            returned_count: results.len() as u64,
            offset: query.offset,
            elapsed_ms: elapsed.as_millis() as u64,
            results,
        })
    }

    /// Get context lines around a specific entry
    pub fn get_context(&self, file_id: i64, line_number: u64, context_size: u32) -> Result<Vec<SearchResult>> {
        let min_line = if line_number as i64 - context_size as i64 > 0 {
            line_number - context_size as u64
        } else {
            1
        };
        let max_line = line_number + context_size as u64 + 1;

        let mut stmt = self.conn.prepare(
            "SELECT e.id, f.path, e.line_number, e.byte_offset, e.timestamp, e.level, e.thread, e.logger, e.message, e.fields_json, e.raw
             FROM log_entries e
             JOIN files f ON e.file_id = f.id
             WHERE e.file_id = ? AND e.line_number >= ? AND e.line_number < ?
             ORDER BY e.line_number"
        )?;

        let rows = stmt.query_map(params![file_id, min_line as i64, max_line as i64], |row| {
            Ok(SearchResult {
                id: row.get(0)?,
                source: row.get(1)?,
                line_number: row.get::<_, i64>(2)? as u64,
                byte_offset: row.get::<_, i64>(3)? as u64,
                timestamp: row.get(4)?,
                level: row.get(5)?,
                thread: row.get(6)?,
                logger: row.get(7)?,
                message: row.get(8)?,
                fields_json: row.get(9)?,
                raw: row.get(10)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

/// Search result set with metadata
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResultSet {
    pub total_count: u64,
    pub returned_count: u64,
    pub offset: u32,
    pub elapsed_ms: u64,
    pub results: Vec<SearchResult>,
}

/// Parse a search query string into a SearchQuery
pub fn parse_query_string(query_str: &str, limit: u32) -> SearchQuery {
    let mut sq = SearchQuery::default();
    sq.limit = limit;

    let mut rest = query_str.trim();

    // Parse filter prefixes
    while !rest.is_empty() {
        if rest.starts_with("level=") {
            rest = &rest[6..];
            let (val, remaining) = take_value(rest);
            if !val.is_empty() {
                sq.levels = val.split(',').map(|s| s.trim().to_uppercase()).collect();
            }
            rest = remaining.trim_start();
        } else if rest.starts_with("after=") {
            rest = &rest[6..];
            let (val, remaining) = take_value(rest);
            if let Some(dt) = parse_relative_time(&val) {
                sq.after = Some(dt);
            }
            rest = remaining.trim_start();
        } else if rest.starts_with("before=") {
            rest = &rest[7..];
            let (val, remaining) = take_value(rest);
            if let Some(dt) = parse_relative_time(&val) {
                sq.before = Some(dt);
            }
            rest = remaining.trim_start();
        } else if rest.starts_with("source=") {
            rest = &rest[7..];
            let (val, remaining) = take_value(rest);
            sq.source = if val.is_empty() { None } else { Some(val) };
            rest = remaining.trim_start();
        } else if rest.starts_with("thread=") {
            rest = &rest[7..];
            let (val, remaining) = take_value(rest);
            sq.thread = if val.is_empty() { None } else { Some(val) };
            rest = remaining.trim_start();
        } else if rest.starts_with("logger=") {
            rest = &rest[7..];
            let (val, remaining) = take_value(rest);
            sq.logger = if val.is_empty() { None } else { Some(val) };
            rest = remaining.trim_start();
        } else if rest.starts_with("regex:") {
            rest = &rest[6..];
            sq.regex_query = Some(rest.trim().to_string());
            rest = "";
        } else {
            // Remaining text is the FTS query
            sq.fts_query = Some(rest.trim().to_string());
            rest = "";
        }
    }

    sq
}

/// Take a value until whitespace
fn take_value(s: &str) -> (String, &str) {
    if let Some(pos) = s.find(char::is_whitespace) {
        (s[..pos].to_string(), &s[pos..])
    } else {
        (s.to_string(), "")
    }
}

/// Parse relative time expressions like "1h-ago", "30m", "2024-01-15"
pub fn parse_relative_time(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let trimmed = s.trim();

    // Absolute timestamp
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc());
    }
    if let Ok(dt) = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return dt.and_hms_opt(0, 0, 0).map(|ndt| ndt.and_utc());
    }

    // Relative time: "1h-ago", "30m-ago", "7d-ago"
    let s_lower = trimmed.to_lowercase();
    if let Some(numeric) = s_lower.strip_suffix("-ago") {
        return parse_duration(numeric).map(|dur| chrono::Utc::now() - dur);
    }

    // Also try "1h", "30m", "7d" format (shorthand)
    parse_duration(trimmed).map(|dur| chrono::Utc::now() - dur)
}

fn parse_duration(s: &str) -> Option<chrono::Duration> {
    let s = s.trim();
    let (num_str, unit) = if s.ends_with('h') {
        (&s[..s.len()-1], "h")
    } else if s.ends_with('m') {
        (&s[..s.len()-1], "m")
    } else if s.ends_with('d') {
        (&s[..s.len()-1], "d")
    } else if s.ends_with('s') {
        (&s[..s.len()-1], "s")
    } else {
        return None;
    };

    let num: i64 = num_str.trim().parse().ok()?;
    let secs = match unit {
        "h" => num * 3600,
        "m" => num * 60,
        "d" => num * 86400,
        "s" => num,
        _ => return None,
    };

    chrono::Duration::try_seconds(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_query() {
        let sq = parse_query_string("error timeout", 100);
        assert_eq!(sq.fts_query.as_deref(), Some("error timeout"));
    }

    #[test]
    fn test_parse_filter_query() {
        let sq = parse_query_string("level=ERROR after=2024-01-15", 100);
        assert!(sq.fts_query.is_none());
        assert_eq!(sq.levels, vec!["ERROR"]);
        assert!(sq.after.is_some());
    }

    #[test]
    fn test_parse_mixed_query() {
        let sq = parse_query_string("level=ERROR timeout connection", 50);
        assert_eq!(sq.fts_query.as_deref(), Some("timeout connection"));
        assert_eq!(sq.levels, vec!["ERROR"]);
        assert_eq!(sq.limit, 50);
    }

    #[test]
    fn test_parse_regex_query() {
        let sq = parse_query_string("regex:Exception\\s+in\\s+thread", 100);
        assert!(sq.regex_query.is_some());
        assert!(sq.fts_query.is_none());
    }
}
