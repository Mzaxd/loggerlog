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

    /// Build WHERE clause conditions and params from a SearchQuery.
    /// Returns (where_clause_string, params_vec) for use in SQL queries.
    fn build_where_clause(&self, query: &SearchQuery) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
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

        // Project filter
        if let Some(ref project) = query.project {
            conditions.push(
                "f.project_id IN (SELECT id FROM projects WHERE name = ?)".to_string()
            );
            params.push(Box::new(project.clone()));
        }

        // Module filter — match subdirectory name within project path
        // Normalize path separators to '/' for cross-platform matching
        if let Some(ref module) = query.module {
            conditions.push(
                "EXISTS (SELECT 1 FROM projects p WHERE f.project_id = p.id \
                 AND REPLACE(substr(f.path, length(p.path) + 2), '\\', '/') \
                 LIKE ? || '/%')".to_string()
            );
            params.push(Box::new(module.clone()));
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

        // Exclude filter
        for keyword in &query.exclude {
            conditions.push("(e.message NOT LIKE ? AND e.raw NOT LIKE ?)".to_string());
            params.push(Box::new(format!("%{}%", keyword)));
            params.push(Box::new(format!("%{}%", keyword)));
        }

        (conditions.join(" AND "), params)
    }

    /// Build fresh params from query (for a second SQL statement using the same WHERE)
    fn build_params_from_query(&self, query: &SearchQuery) -> Vec<Box<dyn rusqlite::types::ToSql>> {
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(ref fts) = query.fts_query { params.push(Box::new(fts.clone())); }
        if !query.levels.is_empty() {
            for level in &query.levels { params.push(Box::new(level.clone())); }
        }
        if let Some(ref source) = query.source { params.push(Box::new(format!("%{}%", source))); }
        if let Some(ref project) = query.project { params.push(Box::new(project.clone())); }
        if let Some(ref module) = query.module { params.push(Box::new(module.clone())); }
        if let Some(ref after) = query.after { params.push(Box::new(after.to_rfc3339())); }
        if let Some(ref before) = query.before { params.push(Box::new(before.to_rfc3339())); }
        if let Some(ref thread) = query.thread { params.push(Box::new(format!("%{}%", thread))); }
        if let Some(ref logger) = query.logger { params.push(Box::new(format!("%{}%", logger))); }
        for keyword in &query.exclude {
            params.push(Box::new(format!("%{}%", keyword)));
            params.push(Box::new(format!("%{}%", keyword)));
        }
        params
    }

    /// Execute a search query and return results
    pub fn search(&self, query: &SearchQuery) -> Result<SearchResultSet> {
        let start = Instant::now();
        let (where_clause, count_params) = self.build_where_clause(query);

        // Count total matching results
        let count_sql = format!(
            "SELECT COUNT(*) FROM log_entries e JOIN files f ON e.file_id = f.id WHERE {}", where_clause
        );
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = count_params.iter().map(|b| b.as_ref()).collect();
        let total_count: u64 = self.conn.query_row(&count_sql, param_refs.as_slice(), |row| row.get(0))?;

        // Fetch results with pagination — build fresh params with limit/offset appended
        let select_sql = format!(
            "SELECT e.id, e.file_id, f.path, e.line_number, e.byte_offset, e.timestamp, e.level, e.thread, e.logger, e.message, e.fields_json, e.raw
             FROM log_entries e
             JOIN files f ON e.file_id = f.id
             WHERE {}
             ORDER BY e.timestamp DESC
             LIMIT ? OFFSET ?",
            where_clause
        );

        let mut select_params = self.build_params_from_query(query);
        select_params.push(Box::new(query.limit as i64));
        select_params.push(Box::new(query.offset as i64));
        let select_refs: Vec<&dyn rusqlite::types::ToSql> = select_params.iter().map(|b| b.as_ref()).collect();

        let mut stmt = self.conn.prepare(&select_sql)?;
        let rows = stmt.query_map(select_refs.as_slice(), |row| {
            Ok(SearchResult {
                id: row.get(0)?,
                file_id: row.get(1)?,
                source: row.get(2)?,
                line_number: row.get::<_, i64>(3)? as u64,
                byte_offset: row.get::<_, i64>(4)? as u64,
                timestamp: row.get(5)?,
                level: row.get(6)?,
                thread: row.get(7)?,
                logger: row.get(8)?,
                message: row.get(9)?,
                fields_json: row.get(10)?,
                raw: row.get(11)?,
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
        let (where_clause, _params) = self.build_where_clause(query);

        // Fetch more candidates for regex filtering (up to 10x limit)
        let scan_limit = query.limit * 10;
        let select_sql = format!(
            "SELECT e.id, e.file_id, f.path, e.line_number, e.byte_offset, e.timestamp, e.level, e.thread, e.logger, e.message, e.fields_json, e.raw
             FROM log_entries e
             JOIN files f ON e.file_id = f.id
             WHERE {}
             ORDER BY e.timestamp DESC
             LIMIT ?",
            where_clause
        );

        let mut select_params = self.build_params_from_query(query);
        select_params.push(Box::new(scan_limit as i64));
        let select_refs: Vec<&dyn rusqlite::types::ToSql> = select_params.iter().map(|b| b.as_ref()).collect();

        let mut stmt = self.conn.prepare(&select_sql)?;
        let rows = stmt.query_map(select_refs.as_slice(), |row| {
            Ok(SearchResult {
                id: row.get(0)?,
                file_id: row.get(1)?,
                source: row.get(2)?,
                line_number: row.get::<_, i64>(3)? as u64,
                byte_offset: row.get::<_, i64>(4)? as u64,
                timestamp: row.get(5)?,
                level: row.get(6)?,
                thread: row.get(7)?,
                logger: row.get(8)?,
                message: row.get(9)?,
                fields_json: row.get(10)?,
                raw: row.get(11)?,
            })
        })?;

        let mut results: Vec<SearchResult> = rows
            .filter_map(|r| r.ok())
            .filter(|r| re.is_match(&r.raw)
                && !query.exclude.iter().any(|ex| r.raw.contains(ex)))
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
            "SELECT e.id, e.file_id, f.path, e.line_number, e.byte_offset, e.timestamp, e.level, e.thread, e.logger, e.message, e.fields_json, e.raw
             FROM log_entries e
             JOIN files f ON e.file_id = f.id
             WHERE e.file_id = ? AND e.line_number >= ? AND e.line_number < ?
             ORDER BY e.line_number"
        )?;

        let rows = stmt.query_map(params![file_id, min_line as i64, max_line as i64], |row| {
            Ok(SearchResult {
                id: row.get(0)?,
                file_id: row.get(1)?,
                source: row.get(2)?,
                line_number: row.get::<_, i64>(3)? as u64,
                byte_offset: row.get::<_, i64>(4)? as u64,
                timestamp: row.get(5)?,
                level: row.get(6)?,
                thread: row.get(7)?,
                logger: row.get(8)?,
                message: row.get(9)?,
                fields_json: row.get(10)?,
                raw: row.get(11)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get level distribution for the given query
    pub fn level_stats(&self, query: &SearchQuery) -> Result<Vec<LevelCount>> {
        let (where_clause, params) = self.build_where_clause(query);
        let sql = format!(
            "SELECT COALESCE(level,'UNKNOWN'), COUNT(*) FROM log_entries e JOIN files f ON e.file_id = f.id WHERE {} GROUP BY level ORDER BY COUNT(*) DESC",
            where_clause
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|b| b.as_ref()).collect();
        let rows: Vec<LevelCount> = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(LevelCount {
                level: row.get::<_, String>(0)?,
                count: row.get(1)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }

    /// Generate an aggregated summary from the same WHERE clause as a normal search.
    pub fn search_summary(&self, query: &SearchQuery, top_n: usize) -> Result<SearchSummary> {
        let start = Instant::now();
        let (where_clause, params) = self.build_where_clause(query);

        let count_sql = format!(
            "SELECT COUNT(*) FROM log_entries e JOIN files f ON e.file_id = f.id WHERE {}",
            where_clause
        );
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|b| b.as_ref()).collect();
        let total_count: u64 = self.conn.query_row(
            &count_sql, param_refs.as_slice(), |row| row.get(0)
        )?;

        // Level distribution
        let level_sql = format!(
            "SELECT COALESCE(level,'UNKNOWN'), COUNT(*) FROM log_entries e JOIN files f ON e.file_id = f.id WHERE {} GROUP BY level ORDER BY COUNT(*) DESC",
            where_clause
        );
        let level_param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|b| b.as_ref()).collect();
        let level_distribution: Vec<LevelCount> = {
            let mut stmt = self.conn.prepare(&level_sql)?;
            let rows: Vec<LevelCount> = stmt.query_map(level_param_refs.as_slice(), |row| {
                Ok(LevelCount {
                    level: row.get::<_, String>(0)?,
                    count: row.get(1)?,
                })
            })?.filter_map(|r| r.ok()).collect();
            rows
        };

        // Source breakdown (top N)
        let source_sql = format!(
            "SELECT f.path, COUNT(*) FROM log_entries e JOIN files f ON e.file_id = f.id WHERE {} GROUP BY f.path ORDER BY COUNT(*) DESC LIMIT ?",
            where_clause
        );
        let source_params = {
            let mut p = self.build_params_from_query(query);
            p.push(Box::new(top_n as i64));
            p
        };
        let source_refs: Vec<&dyn rusqlite::types::ToSql> = source_params.iter().map(|b| b.as_ref()).collect();
        let source_breakdown: Vec<SourceCount> = {
            let mut stmt = self.conn.prepare(&source_sql)?;
            let rows: Vec<SourceCount> = stmt.query_map(source_refs.as_slice(), |row| {
                Ok(SourceCount {
                    source: row.get(0)?,
                    count: row.get(1)?,
                })
            })?.filter_map(|r| r.ok()).collect();
            rows
        };

        // Time range
        let time_sql = format!(
            "SELECT MIN(timestamp), MAX(timestamp) FROM log_entries e JOIN files f ON e.file_id = f.id WHERE {}",
            where_clause
        );
        let time_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|b| b.as_ref()).collect();
        let time_range: Option<(String, String)> = self.conn
            .query_row(&time_sql, time_refs.as_slice(), |row| {
                Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .ok()
            .and_then(|(min, max)| min.zip(max));

        // Top frequent messages (deduplicated by first 100 chars)
        let msg_sql = format!(
            "SELECT SUBSTR(message,1,100) as msg_prefix, COUNT(*) FROM log_entries e JOIN files f ON e.file_id = f.id WHERE {} AND LENGTH(message)>0 GROUP BY msg_prefix ORDER BY COUNT(*) DESC LIMIT ?",
            where_clause
        );
        let mut msg_params = self.build_params_from_query(query);
        msg_params.push(Box::new(top_n as i64));
        let msg_refs: Vec<&dyn rusqlite::types::ToSql> = msg_params.iter().map(|b| b.as_ref()).collect();
        let top_messages: Vec<MessageCount> = {
            let mut stmt = self.conn.prepare(&msg_sql)?;
            let rows: Vec<MessageCount> = stmt.query_map(msg_refs.as_slice(), |row| {
                Ok(MessageCount {
                    message_prefix: row.get(0)?,
                    count: row.get(1)?,
                })
            })?.filter_map(|r| r.ok()).collect();
            rows
        };

        let elapsed = start.elapsed();

        Ok(SearchSummary {
            total_count,
            level_distribution,
            source_breakdown,
            time_range,
            top_messages,
            elapsed_ms: elapsed.as_millis() as u64,
        })
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

/// Aggregated search summary (used by --summary mode)
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchSummary {
    pub total_count: u64,
    pub level_distribution: Vec<LevelCount>,
    pub source_breakdown: Vec<SourceCount>,
    pub time_range: Option<(String, String)>,
    pub top_messages: Vec<MessageCount>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LevelCount {
    pub level: String,
    pub count: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceCount {
    pub source: String,
    pub count: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MessageCount {
    pub message_prefix: String,
    pub count: u64,
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
        } else if rest.starts_with("project=") {
            rest = &rest[8..];
            let (val, remaining) = take_value(rest);
            sq.project = if val.is_empty() { None } else { Some(val) };
            rest = remaining.trim_start();
        } else if rest.starts_with("module=") {
            rest = &rest[7..];
            let (val, remaining) = take_value(rest);
            sq.module = if val.is_empty() { None } else { Some(val) };
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
        } else if rest.starts_with("exclude=") {
            rest = &rest[8..];
            let (val, remaining) = take_value(rest);
            if !val.is_empty() {
                sq.exclude.push(val);
            }
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
    use crate::core::entry::LogEntry;
    use crate::core::index::IndexManager;
    use chrono::Utc;

    // ── helper ──
    fn setup_db() -> IndexManager {
        let idx = IndexManager::open_in_memory().unwrap();
        // Create a test file
        let file_id = idx.get_or_create_file("/logs/test/console.log").unwrap();
        let now = Utc::now();
        // Insert test log entries
        let entries = vec![
            LogEntry{id:None, file_id, line_number:1, byte_offset:0,
                timestamp:Some(now - chrono::Duration::try_seconds(60).unwrap()),
                level:Some("ERROR".into()), thread:Some("main".into()),
                logger:Some("com.example.App".into()),
                message:"NullPointerException: something broke".into(),
                fields_json:None, raw:"2024-01-15 10:00:00 ERROR [main] com.example.App - NullPointerException: something broke".into()},
            LogEntry{id:None, file_id, line_number:2, byte_offset:100,
                timestamp:Some(now - chrono::Duration::try_seconds(30).unwrap()),
                level:Some("WARN".into()), thread:Some("worker-1".into()),
                logger:Some("com.example.Service".into()),
                message:"connection timeout after 5s".into(),
                fields_json:None, raw:"2024-01-15 10:00:30 WARN [worker-1] com.example.Service - connection timeout after 5s".into()},
            LogEntry{id:None, file_id, line_number:3, byte_offset:200,
                timestamp:Some(now),
                level:Some("INFO".into()), thread:Some("main".into()),
                logger:Some("com.example.Health".into()),
                message:"health check ok".into(),
                fields_json:None, raw:"2024-01-15 10:01:00 INFO [main] com.example.Health - health check ok".into()},
        ];
        idx.insert_entries(&entries).unwrap();
        idx
    }

    fn make_query() -> SearchQuery {
        SearchQuery { limit: 100, ..Default::default() }
    }

    // ──────── parse_duration ────────
    #[test]
    fn test_parse_duration_hours() {
        let d = parse_duration("2h").unwrap();
        assert_eq!(d.num_seconds(), 7200);
    }
    #[test]
    fn test_parse_duration_minutes() {
        let d = parse_duration("30m").unwrap();
        assert_eq!(d.num_seconds(), 1800);
    }
    #[test]
    fn test_parse_duration_days() {
        let d = parse_duration("7d").unwrap();
        assert_eq!(d.num_seconds(), 604800);
    }
    #[test]
    fn test_parse_duration_seconds() {
        let d = parse_duration("90s").unwrap();
        assert_eq!(d.num_seconds(), 90);
    }
    #[test]
    fn test_parse_duration_whitespace() {
        let d = parse_duration("  5m  ").unwrap();
        assert_eq!(d.num_seconds(), 300);
    }
    #[test]
    fn test_parse_duration_invalid() {
        assert!(parse_duration("abc").is_none());
        assert!(parse_duration("").is_none());
        assert!(parse_duration("5w").is_none());
    }

    // ──────── parse_relative_time ────────
    #[test]
    fn test_parse_relative_time_ago() {
        assert!(parse_relative_time("1h-ago").is_some());
        assert!(parse_relative_time("30m-ago").is_some());
        assert!(parse_relative_time("7d-ago").is_some());
    }
    #[test]
    fn test_parse_relative_time_shorthand() {
        assert!(parse_relative_time("2h").is_some());
    }
    #[test]
    fn test_parse_relative_time_absolute() {
        let dt = parse_relative_time("2024-01-15").unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-01-15");
    }
    #[test]
    fn test_parse_relative_time_datetime() {
        assert!(parse_relative_time("2024-01-15 10:30:00").is_some());
    }
    #[test]
    fn test_parse_relative_time_rfc3339() {
        assert!(parse_relative_time("2024-01-15T10:30:00Z").is_some());
    }
    #[test]
    fn test_parse_relative_time_invalid() {
        assert!(parse_relative_time("not-a-time").is_none());
    }

    // ──────── take_value ────────
    #[test]
    fn test_take_value_simple() {
        let (val, rest) = take_value("hello world");
        assert_eq!(val, "hello");
        assert_eq!(rest, " world");
    }
    #[test]
    fn test_take_value_eol() {
        let (val, rest) = take_value("hello");
        assert_eq!(val, "hello");
        assert_eq!(rest, "");
    }
    #[test]
    fn test_take_value_empty() {
        let (val, rest) = take_value("");
        assert_eq!(val, "");
        assert_eq!(rest, "");
    }

    // ──────── parse_query_string ────────
    #[test]
    fn test_parse_query_exclude() {
        let sq = parse_query_string("exclude=health error", 50);
        assert_eq!(sq.exclude, vec!["health"]);
        assert_eq!(sq.fts_query.as_deref(), Some("error"));
    }
    #[test]
    fn test_parse_query_exclude_no_value() {
        let sq = parse_query_string("error exclude=", 50);
        assert!(sq.exclude.is_empty());
    }
    #[test]
    fn test_parse_query_all_filters() {
        let sq = parse_query_string(
            "level=ERROR,INFO thread=http source=auth after=2024-01-01 before=2024-06-01 project=prod module=api logger=com.example excl", 200);
        assert_eq!(sq.levels, vec!["ERROR","INFO"]);
        assert_eq!(sq.thread.as_deref(), Some("http"));
        assert_eq!(sq.source.as_deref(), Some("auth"));
        assert!(sq.after.is_some());
        assert!(sq.before.is_some());
        assert_eq!(sq.project.as_deref(), Some("prod"));
        assert_eq!(sq.module.as_deref(), Some("api"));
        assert_eq!(sq.logger.as_deref(), Some("com.example"));
    }
    #[test]
    fn test_parse_query_only_filters_no_fts() {
        let sq = parse_query_string("level=ERROR source=app.log thread=nio", 100);
        assert!(sq.fts_query.is_none());
        assert_eq!(sq.levels, vec!["ERROR"]);
        assert_eq!(sq.source.as_deref(), Some("app.log"));
        assert_eq!(sq.thread.as_deref(), Some("nio"));
    }

    // ──────── build_where_clause shape ────────
    #[test]
    fn test_build_where_clause_baseline() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let q = make_query();
        let (sql, p) = engine.build_where_clause(&q);
        assert_eq!(sql.trim(), "1=1");
        assert!(p.is_empty());
    }
    #[test]
    fn test_build_where_clause_with_filters() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let mut q = make_query();
        q.fts_query = Some("timeout".into());
        q.levels = vec!["ERROR".into()];
        q.exclude = vec!["health".into()];
        let (sql, p) = engine.build_where_clause(&q);
        assert!(sql.contains("MATCH"));
        assert!(sql.contains("level IN"));
        assert!(sql.contains("NOT LIKE"));
        assert!(!p.is_empty());
    }

    // ──────── search (integration) ────────
    #[test]
    fn test_search_fts() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let mut q = make_query();
        q.fts_query = Some("timeout".into());
        let rs = engine.search(&q).unwrap();
        assert_eq!(rs.total_count, 1);
        assert_eq!(rs.results.len(), 1);
        assert_eq!(rs.results[0].level.as_deref(), Some("WARN"));
    }
    #[test]
    fn test_search_no_results() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let mut q = make_query();
        q.fts_query = Some("nonexistent999".into());
        let rs = engine.search(&q).unwrap();
        assert_eq!(rs.total_count, 0);
        assert!(rs.results.is_empty());
    }
    #[test]
    fn test_search_level_filter() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let mut q = make_query();
        q.levels = vec!["ERROR".into()];
        let rs = engine.search(&q).unwrap();
        assert_eq!(rs.total_count, 1);
        assert_eq!(rs.results[0].level.as_deref(), Some("ERROR"));
    }
    #[test]
    fn test_search_exclude_filter() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let mut q = make_query();
        q.fts_query = Some("connection".into());
        q.exclude = vec!["health".into()];
        let rs = engine.search(&q).unwrap();
        assert_eq!(rs.total_count, 1);
    }
    #[test]
    fn test_search_limit_and_offset() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let mut q = make_query();
        q.limit = 1;
        q.offset = 1;
        let rs = engine.search(&q).unwrap();
        assert_eq!(rs.returned_count, 1);
        assert_eq!(rs.offset, 1);
    }
    #[test]
    fn test_search_selects_file_id() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let mut q = make_query();
        q.fts_query = Some("something".into());
        let rs = engine.search(&q).unwrap();
        assert_eq!(rs.results.len(), 1);
        assert!(rs.results[0].file_id > 0);
        assert!(!rs.results[0].source.is_empty());
    }

    // ──────── search_regex (integration) ────────
    #[test]
    fn test_search_regex_basic() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let mut q = make_query();
        q.levels = vec!["ERROR".into()];
        let rs = engine.search_regex(r"NullPointer", &q).unwrap();
        assert_eq!(rs.total_count, 1);
    }
    #[test]
    fn test_search_regex_no_match() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let q = make_query();
        let rs = engine.search_regex(r"NonExistentPattern123", &q).unwrap();
        assert_eq!(rs.total_count, 0);
    }
    #[test]
    fn test_search_regex_exclude() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let mut q = make_query();
        q.exclude = vec!["NullPointer".into()];
        let rs = engine.search_regex(r".", &q).unwrap();
        // All 3 entries exist, one excluded
        assert_eq!(rs.total_count, 2);
    }

    // ──────── level_stats ────────
    #[test]
    fn test_level_stats_all() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let q = make_query();
        let stats = engine.level_stats(&q).unwrap();
        assert_eq!(stats.len(), 3);
        let levels: Vec<&str> = stats.iter().map(|s| s.level.as_str()).collect();
        assert!(levels.contains(&"ERROR"));
        assert!(levels.contains(&"WARN"));
        assert!(levels.contains(&"INFO"));
    }
    #[test]
    fn test_level_stats_filtered() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let mut q = make_query();
        q.levels = vec!["ERROR".into()];
        let stats = engine.level_stats(&q).unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].level, "ERROR");
        assert_eq!(stats[0].count, 1);
    }

    // ──────── search_summary ────────
    #[test]
    fn test_search_summary_counts() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let q = make_query();
        let summary = engine.search_summary(&q, 10).unwrap();
        assert_eq!(summary.total_count, 3);
        assert!(!summary.level_distribution.is_empty());
        assert!(!summary.source_breakdown.is_empty());
        assert!(summary.time_range.is_some());
    }
    #[test]
    fn test_search_summary_top_messages() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let q = make_query();
        let summary = engine.search_summary(&q, 1).unwrap();
        assert_eq!(summary.top_messages.len(), 1);
    }

    // ──────── get_context ────────
    #[test]
    fn test_get_context_around_line() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        // First, find a result to get its file_id
        let rs = engine.search(&make_query()).unwrap();
        let file_id = rs.results[0].file_id;
        let line = rs.results[1].line_number; // line 2 (WARN)
        let ctx = engine.get_context(file_id, line, 1).unwrap();
        assert!(ctx.len() >= 2);
        // Should include lines 1 and 3 around line 2
        let line_nums: Vec<u64> = ctx.iter().map(|r| r.line_number).collect();
        assert!(line_nums.contains(&1));
        assert!(line_nums.contains(&3));
    }
    #[test]
    fn test_get_context_line_one() {
        let db = setup_db();
        let engine = SearchEngine::new(db.conn());
        let rs = engine.search(&make_query()).unwrap();
        let file_id = rs.results[0].file_id;
        let ctx = engine.get_context(file_id, 1, 5).unwrap();
        // Line 1 should have no "before" context — min_line clamped to 1
        assert!(ctx.len() >= 1);
    }

    // ──────── parse_query_string (keep existing) ────────
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
    #[test]
    fn test_parse_project_module_query() {
        let sq = parse_query_string("project=aico module=aico-cloud-auth level=ERROR", 100);
        assert_eq!(sq.project.as_deref(), Some("aico"));
        assert_eq!(sq.module.as_deref(), Some("aico-cloud-auth"));
        assert_eq!(sq.levels, vec!["ERROR"]);
    }
    #[test]
    fn test_parse_project_only_query() {
        let sq = parse_query_string("project=aico level=ERROR", 100);
        assert_eq!(sq.project.as_deref(), Some("aico"));
        assert!(sq.module.is_none());
        assert_eq!(sq.levels, vec!["ERROR"]);
    }
}
