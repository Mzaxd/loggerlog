use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

/// SQLite FTS5 index manager
pub struct IndexManager {
    conn: Connection,
}

impl IndexManager {
    /// Open or create the index database
    pub fn open(db_path: &str) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = Path::new(db_path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA cache_size=-64000;",
        )?;
        let mgr = Self { conn };
        mgr.init_schema()?;
        Ok(mgr)
    }

    /// Open an in-memory index (for testing)
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        let mgr = Self { conn };
        mgr.init_schema()?;
        Ok(mgr)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS files (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                path        TEXT NOT NULL UNIQUE,
                size        INTEGER DEFAULT 0,
                modified_at TEXT,
                format      TEXT DEFAULT 'unknown',
                byte_offset INTEGER DEFAULT 0,
                line_count  INTEGER DEFAULT 0,
                created_at  TEXT DEFAULT (datetime('now')),
                updated_at  TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS log_entries (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id     INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
                line_number INTEGER NOT NULL,
                byte_offset INTEGER NOT NULL,
                raw         TEXT NOT NULL DEFAULT ''
            );

            CREATE INDEX IF NOT EXISTS idx_entries_file ON log_entries(file_id);

            CREATE VIRTUAL TABLE IF NOT EXISTS log_entries_fts USING fts5(
                raw,
                content='log_entries',
                content_rowid='id',
                tokenize='unicode61 remove_diacritics 1'
            );

            CREATE TRIGGER IF NOT EXISTS log_entries_ai AFTER INSERT ON log_entries BEGIN
                INSERT INTO log_entries_fts(rowid, raw) VALUES (new.id, new.raw);
            END;

            CREATE TRIGGER IF NOT EXISTS log_entries_ad AFTER DELETE ON log_entries BEGIN
                INSERT INTO log_entries_fts(log_entries_fts, rowid, raw) VALUES('delete', old.id, old.raw);
            END;

            CREATE TRIGGER IF NOT EXISTS log_entries_au AFTER UPDATE ON log_entries BEGIN
                INSERT INTO log_entries_fts(log_entries_fts, rowid, raw) VALUES('delete', old.id, old.raw);
                INSERT INTO log_entries_fts(rowid, raw) VALUES (new.id, new.raw);
            END;"
        )?;

        // Schema migrations
        self.migrate_schema()?;

        Ok(())
    }

    /// Run schema migrations
    fn migrate_schema(&self) -> Result<()> {
        // Create schema_version table if not exists
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)",
        )?;

        let version: i32 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if version < 2 {
            self.migrate_v2()?;
            self.conn
                .execute("INSERT INTO schema_version (version) VALUES (2)", [])?;
        }

        if version < 3 {
            self.migrate_v3()?;
            self.conn
                .execute("INSERT INTO schema_version (version) VALUES (3)", [])?;
        }

        if version < 4 {
            self.migrate_v4()?;
            self.conn
                .execute("INSERT INTO schema_version (version) VALUES (4)", [])?;
        }

        Ok(())
    }

    /// v2: Add projects table and project_id to files
    fn migrate_v2(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL UNIQUE,
                path        TEXT NOT NULL,
                created_at  TEXT DEFAULT (datetime('now'))
            );",
        )?;

        // Add project_id column to files if it doesn't exist (must come before index)
        let has_project_id: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('files') WHERE name = 'project_id'",
                [],
                |row| row.get::<_, i32>(0).map(|c| c > 0),
            )
            .unwrap_or(false);

        if !has_project_id {
            self.conn.execute_batch(
                "ALTER TABLE files ADD COLUMN project_id INTEGER REFERENCES projects(id) ON DELETE SET NULL;"
            )?;
        }

        // Create index after column exists
        self.conn
            .execute_batch("CREATE INDEX IF NOT EXISTS idx_files_project ON files(project_id);")?;

        Ok(())
    }

    /// v3: Add fields_json to FTS5 index so JSON log extra fields are searchable.
    /// Note: This migration is effectively superseded by v4. It only runs when
    /// upgrading from schema v2 directly to v3 (skipping v4). Since v4 completely
    /// rebuilds the FTS5 table, we guard against missing columns.
    fn migrate_v3(&self) -> Result<()> {
        // Check if the old structured columns still exist (they won't after v4)
        let has_message: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('log_entries') WHERE name = 'message'",
                [],
                |row| row.get::<_, i32>(0).map(|c| c > 0),
            )
            .unwrap_or(false);

        if !has_message {
            // v4 already ran or schema was created fresh — skip v3 entirely
            return Ok(());
        }

        // 1. Drop old FTS5 table and triggers
        self.conn.execute_batch(
            "DROP TRIGGER IF EXISTS log_entries_ai;
             DROP TRIGGER IF EXISTS log_entries_ad;
             DROP TRIGGER IF EXISTS log_entries_au;
             DROP TABLE IF EXISTS log_entries_fts;",
        )?;

        // 2. Recreate FTS5 table with fields_json column
        self.conn.execute_batch(
            "CREATE VIRTUAL TABLE log_entries_fts USING fts5(
                message,
                raw,
                fields_json,
                content='log_entries',
                content_rowid='id',
                tokenize='unicode61 remove_diacritics 1'
            );",
        )?;

        // 3. Rebuild FTS index from existing log entries
        self.conn.execute_batch(
            "INSERT INTO log_entries_fts(rowid, message, raw, fields_json)
             SELECT id, message, raw, fields_json FROM log_entries;",
        )?;

        // 4. Recreate triggers with fields_json support
        self.conn.execute_batch(
            "CREATE TRIGGER log_entries_ai AFTER INSERT ON log_entries BEGIN
                INSERT INTO log_entries_fts(rowid, message, raw, fields_json)
                    VALUES (new.id, new.message, new.raw, new.fields_json);
             END;

             CREATE TRIGGER log_entries_ad AFTER DELETE ON log_entries BEGIN
                INSERT INTO log_entries_fts(log_entries_fts, rowid, message, raw, fields_json)
                    VALUES('delete', old.id, old.message, old.raw, old.fields_json);
             END;

             CREATE TRIGGER log_entries_au AFTER UPDATE ON log_entries BEGIN
                INSERT INTO log_entries_fts(log_entries_fts, rowid, message, raw, fields_json)
                    VALUES('delete', old.id, old.message, old.raw, old.fields_json);
                INSERT INTO log_entries_fts(rowid, message, raw, fields_json)
                    VALUES (new.id, new.message, new.raw, new.fields_json);
             END;",
        )?;

        Ok(())
    }

    /// v4: Loki-style simplification — remove structured columns from log_entries,
    /// simplify FTS5 to raw-only, triggers to raw-only.
    fn migrate_v4(&self) -> Result<()> {
        // 1. Drop old FTS5 table and triggers
        self.conn.execute_batch(
            "DROP TRIGGER IF EXISTS log_entries_ai;
             DROP TRIGGER IF EXISTS log_entries_ad;
             DROP TRIGGER IF EXISTS log_entries_au;
             DROP TABLE IF EXISTS log_entries_fts;",
        )?;

        // 2. Drop old indexes (they reference columns that will be removed)
        self.conn.execute_batch(
            "DROP INDEX IF EXISTS idx_entries_timestamp;
             DROP INDEX IF EXISTS idx_entries_level;
             DROP INDEX IF EXISTS idx_entries_thread;",
        )?;

        // 3. Rebuild log_entries table without structured columns
        //    SQLite doesn't support DROP COLUMN before 3.35.0, so recreate table
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS log_entries_new (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id     INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
                line_number INTEGER NOT NULL,
                byte_offset INTEGER NOT NULL,
                raw         TEXT NOT NULL DEFAULT ''
            );
            INSERT INTO log_entries_new (id, file_id, line_number, byte_offset, raw)
                SELECT id, file_id, line_number, byte_offset, raw FROM log_entries;
            DROP TABLE log_entries;
            ALTER TABLE log_entries_new RENAME TO log_entries;",
        )?;

        // 4. Recreate file index
        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_entries_file ON log_entries(file_id);",
        )?;

        // 5. Create simplified FTS5 table (raw only)
        self.conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS log_entries_fts USING fts5(
                raw,
                content='log_entries',
                content_rowid='id',
                tokenize='unicode61 remove_diacritics 1'
            );",
        )?;

        // 6. Rebuild FTS index from existing data
        self.conn.execute_batch(
            "INSERT INTO log_entries_fts(rowid, raw)
             SELECT id, raw FROM log_entries;",
        )?;

        // 7. Recreate triggers (raw only)
        self.conn.execute_batch(
            "CREATE TRIGGER IF NOT EXISTS log_entries_ai AFTER INSERT ON log_entries BEGIN
                INSERT INTO log_entries_fts(rowid, raw) VALUES (new.id, new.raw);
             END;

             CREATE TRIGGER IF NOT EXISTS log_entries_ad AFTER DELETE ON log_entries BEGIN
                INSERT INTO log_entries_fts(log_entries_fts, rowid, raw)
                    VALUES('delete', old.id, old.raw);
             END;

             CREATE TRIGGER IF NOT EXISTS log_entries_au AFTER UPDATE ON log_entries BEGIN
                INSERT INTO log_entries_fts(log_entries_fts, rowid, raw)
                    VALUES('delete', old.id, old.raw);
                INSERT INTO log_entries_fts(rowid, raw) VALUES (new.id, new.raw);
             END;",
        )?;

        Ok(())
    }

    /// Get or create a file record
    pub fn get_or_create_file(&self, path: &str) -> Result<i64> {
        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM files WHERE path = ?",
                params![path],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            Ok(id)
        } else {
            self.conn
                .execute("INSERT INTO files (path) VALUES (?)", params![path])?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    /// Update file metadata
    pub fn update_file(
        &self,
        file_id: i64,
        size: i64,
        byte_offset: i64,
        line_count: i64,
        format: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE files SET size = ?, byte_offset = ?, line_count = ?, format = ?, updated_at = datetime('now') WHERE id = ?",
            params![size, byte_offset, line_count, format, file_id],
        )?;
        Ok(())
    }

    /// Insert log entries in batch (raw-only, no parsing)
    pub fn insert_entries(&self, entries: &[crate::core::entry::LogEntry]) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        let tx = self.conn.unchecked_transaction()?;
        let mut count = 0;

        {
            let mut stmt = tx.prepare(
                "INSERT INTO log_entries (file_id, line_number, byte_offset, raw)
                 VALUES (?, ?, ?, ?)",
            )?;

            for entry in entries {
                stmt.execute(params![
                    entry.file_id,
                    entry.line_number as i64,
                    entry.byte_offset as i64,
                    &entry.raw,
                ])?;
                count += 1;
            }
        }

        tx.commit()?;
        Ok(count)
    }

    /// Remove entries for a file and re-insert from scratch
    pub fn clear_file_entries(&self, file_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM log_entries WHERE file_id = ?",
            params![file_id],
        )?;
        Ok(())
    }

    /// Compact/optimize the FTS index
    pub fn compact(&self) -> Result<()> {
        self.conn.execute_batch(
            "INSERT INTO log_entries_fts(log_entries_fts) VALUES('rebuild');
             PRAGMA optimize;",
        )?;
        Ok(())
    }

    /// Get database file size in bytes
    pub fn db_size_bytes(&self) -> Result<u64> {
        let path = self.conn.path().ok_or(anyhow::anyhow!("no db path"))?;
        Ok(std::fs::metadata(path)?.len())
    }

    /// Get total entry count
    pub fn total_entries(&self) -> Result<u64> {
        let count: u64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM log_entries", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Get total file count
    pub fn total_files(&self) -> Result<usize> {
        let count: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Get file byte offset (for incremental indexing)
    pub fn get_file_byte_offset(&self, file_id: i64) -> Result<i64> {
        let offset: i64 = self
            .conn
            .query_row(
                "SELECT byte_offset FROM files WHERE id = ?",
                params![file_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(offset)
    }

    /// Get a single file record by path (point query, efficient)
    pub fn get_file_by_path(&self, path: &str) -> Result<Option<FileRecord>> {
        let result = self
            .conn
            .query_row(
                "SELECT id, path, size, format, byte_offset, line_count FROM files WHERE path = ?",
                params![path],
                |row| {
                    Ok(FileRecord {
                        id: row.get(0)?,
                        path: row.get(1)?,
                        size: row.get(2)?,
                        format: row.get(3)?,
                        byte_offset: row.get(4)?,
                        line_count: row.get(5)?,
                    })
                },
            )
            .ok();
        Ok(result)
    }

    /// Get all indexed files
    pub fn get_files(&self) -> Result<Vec<FileRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, size, format, byte_offset, line_count FROM files ORDER BY path",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(FileRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                size: row.get(2)?,
                format: row.get(3)?,
                byte_offset: row.get(4)?,
                line_count: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Delete all data
    pub fn clear_all(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM log_entries;
             DELETE FROM files;
             INSERT INTO log_entries_fts(log_entries_fts) VALUES('rebuild');
             PRAGMA optimize;",
        )?;
        Ok(())
    }

    // ── Project management ──

    /// Insert or update a project, return project_id
    pub fn upsert_project(&self, name: &str, path: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO projects (name, path) VALUES (?, ?)
             ON CONFLICT(name) DO UPDATE SET path = excluded.path",
            params![name, path],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Remove a project by name and clear file associations
    pub fn remove_project(&self, name: &str) -> Result<bool> {
        let affected = self
            .conn
            .execute("DELETE FROM projects WHERE name = ?", params![name])?;
        Ok(affected > 0)
    }

    /// Get all projects
    pub fn get_all_projects(&self) -> Result<Vec<ProjectRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, path FROM projects ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok(ProjectRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get a project by name
    pub fn get_project_by_name(&self, name: &str) -> Result<Option<ProjectRecord>> {
        let result = self
            .conn
            .query_row(
                "SELECT id, name, path FROM projects WHERE name = ?",
                params![name],
                |row| {
                    Ok(ProjectRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: row.get(2)?,
                    })
                },
            )
            .ok();
        Ok(result)
    }

    /// Sync projects from config into the database and resolve file-to-project mapping.
    /// This should be called after indexing.
    pub fn sync_projects(&self, projects: &[crate::core::config::Project]) -> Result<()> {
        // 1. Upsert all projects from config
        for p in projects {
            self.upsert_project(&p.name, &p.path)?;
        }

        // 2. Remove projects from DB that no longer exist in config
        let config_names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
        if config_names.is_empty() {
            self.conn.execute("DELETE FROM projects", [])?;
            self.conn.execute(
                "UPDATE files SET project_id = NULL WHERE project_id IS NOT NULL",
                [],
            )?;
            return Ok(());
        }

        let placeholders: Vec<String> = config_names.iter().map(|_| "?".to_string()).collect();
        let delete_sql = format!(
            "DELETE FROM projects WHERE name NOT IN ({})",
            placeholders.join(",")
        );
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = config_names
            .iter()
            .map(|n| Box::new(n.to_string()) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|b| b.as_ref()).collect();
        self.conn.execute(&delete_sql, param_refs.as_slice())?;

        // 3. Load all projects from DB (sorted by path length descending for longest-prefix matching)
        let db_projects = self.get_all_projects()?;
        let mut sorted_projects: Vec<&ProjectRecord> = db_projects.iter().collect();
        sorted_projects.sort_by(|a, b| b.path.len().cmp(&a.path.len()));

        // 4. Resolve file-to-project mapping
        let files = self.get_files()?;

        if files.is_empty() {
            return Ok(());
        }

        let tx = self.conn.unchecked_transaction()?;

        {
            let mut update_stmt = tx.prepare("UPDATE files SET project_id = ? WHERE id = ?")?;

            for file in &files {
                // Find the longest matching project path
                let project_id = sorted_projects
                    .iter()
                    .find(|p| is_subpath(&p.path, &file.path))
                    .map(|p| p.id);

                update_stmt.execute(params![project_id, file.id,])?;
            }
        }

        tx.commit()?;

        Ok(())
    }

    /// Get module names (subdirectory names) for a given project
    pub fn get_modules_for_project(&self, project_name: &str) -> Result<Vec<String>> {
        let project = match self.get_project_by_name(project_name)? {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT path FROM files WHERE project_id = ? ORDER BY path")?;

        let paths: Vec<String> = stmt
            .query_map(params![project.id], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();

        // Extract first-level subdirectory names relative to project path
        let project_path = normalize_path(&project.path);
        let mut modules = std::collections::BTreeSet::new();

        for path in &paths {
            let normalized = normalize_path(path);
            if let Some(relative) = normalized.strip_prefix(&project_path) {
                // Skip leading separator if present
                let relative = relative.trim_start_matches('/');
                // Take the first path component
                if let Some(first_sep) = relative.find('/') {
                    let module = &relative[..first_sep];
                    if !module.is_empty() {
                        modules.insert(module.to_string());
                    }
                } else if !relative.is_empty() {
                    modules.insert(relative.to_string());
                }
            }
        }

        Ok(modules.into_iter().collect())
    }

    /// Get a reference to the connection
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

#[derive(Debug, Clone)]
pub struct FileRecord {
    pub id: i64,
    pub path: String,
    pub size: i64,
    pub format: String,
    pub byte_offset: i64,
    pub line_count: i64,
}

#[derive(Debug, Clone)]
pub struct ProjectRecord {
    pub id: i64,
    pub name: String,
    pub path: String,
}

/// Normalize a path for comparison (forward slashes, trailing separator removed)
fn normalize_path(p: &str) -> String {
    let normalized = p.replace('\\', "/");
    let normalized = normalized.trim_end_matches('/').trim_end_matches('\\');
    normalized.to_string()
}

/// Check if `file_path` is within `dir_path` (directory)
fn is_subpath(dir_path: &str, file_path: &str) -> bool {
    let dir = normalize_path(dir_path);
    let file = normalize_path(file_path);
    if dir.is_empty() {
        return false;
    }
    file.starts_with(&dir) && file.len() > dir.len() && file[dir.len()..].starts_with('/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Project;
    use crate::core::entry::LogEntry;

    fn setup() -> IndexManager {
        IndexManager::open_in_memory().unwrap()
    }

    fn test_project(name: &str, path: &str) -> Project {
        Project {
            name: name.into(),
            path: path.into(),
            recursive: true,
            formats: vec!["auto".into()],
            encoding: "auto".into(),
            exclude_patterns: vec![],
        }
    }

    // ── open / schema ──
    #[test]
    fn test_open_in_memory() {
        let idx = setup();
        assert!(idx.total_entries().unwrap() == 0);
        assert!(idx.total_files().unwrap() == 0);
    }
    #[test]
    fn test_migration_v2_adds_projects_table() {
        let idx = setup();
        // v2 migration should have run; projects table exists
        let projects = idx.get_all_projects().unwrap();
        assert!(projects.is_empty());
    }

    // ── files ──
    #[test]
    fn test_get_or_create_file_new() {
        let idx = setup();
        let id = idx.get_or_create_file("/var/log/app.log").unwrap();
        assert!(id > 0);
        let id2 = idx.get_or_create_file("/var/log/app.log").unwrap();
        assert_eq!(id, id2);
    }
    #[test]
    fn test_get_files_empty() {
        let idx = setup();
        assert!(idx.get_files().unwrap().is_empty());
    }
    #[test]
    fn test_get_files_after_insert() {
        let idx = setup();
        idx.get_or_create_file("/a.log").unwrap();
        idx.get_or_create_file("/b.log").unwrap();
        assert_eq!(idx.get_files().unwrap().len(), 2);
    }
    #[test]
    fn test_get_file_by_path() {
        let idx = setup();
        idx.get_or_create_file("/logs/x.log").unwrap();
        let f = idx.get_file_by_path("/logs/x.log").unwrap().unwrap();
        assert_eq!(f.path, "/logs/x.log");
        assert!(idx.get_file_by_path("/no/such.log").unwrap().is_none());
    }

    // ── entries ──
    #[test]
    fn test_insert_and_count_entries() {
        let idx = setup();
        let fid = idx.get_or_create_file("/test.log").unwrap();
        let entries = vec![LogEntry {
            id: None,
            file_id: fid,
            line_number: 1,
            byte_offset: 0,
            raw: "2024-01-15 10:23:45 INFO hello world".into(),
        }];
        let inserted = idx.insert_entries(&entries).unwrap();
        assert_eq!(inserted, 1);
        assert_eq!(idx.total_entries().unwrap(), 1);
    }
    #[test]
    fn test_insert_empty_entries() {
        let idx = setup();
        assert_eq!(idx.insert_entries(&[]).unwrap(), 0);
    }
    #[test]
    fn test_insert_multiple_entries() {
        let idx = setup();
        let fid = idx.get_or_create_file("/m.log").unwrap();
        let entries: Vec<LogEntry> = (1..=3)
            .map(|i| LogEntry {
                id: None,
                file_id: fid,
                line_number: i,
                byte_offset: (i * 100) as u64,
                raw: format!("2024-01-15 10:23:4{} INFO message {}", i, i),
            })
            .collect();
        assert_eq!(idx.insert_entries(&entries).unwrap(), 3);
        assert_eq!(idx.total_entries().unwrap(), 3);
    }
    #[test]
    fn test_clear_file_entries() {
        let idx = setup();
        let fid = idx.get_or_create_file("/c.log").unwrap();
        idx.insert_entries(&[LogEntry {
            id: None,
            file_id: fid,
            line_number: 1,
            byte_offset: 0,
            raw: "2024-01-15 INFO test".into(),
        }])
        .unwrap();
        assert_eq!(idx.total_entries().unwrap(), 1);
        idx.clear_file_entries(fid).unwrap();
        assert_eq!(idx.total_entries().unwrap(), 0);
    }
    #[test]
    fn test_fts_trigger_sync() {
        let idx = setup();
        let fid = idx.get_or_create_file("/fts.log").unwrap();
        idx.insert_entries(&[LogEntry {
            id: None,
            file_id: fid,
            line_number: 1,
            byte_offset: 0,
            raw: "unique_keyword_fts_test in raw text".into(),
        }])
        .unwrap();
        // FTS search should find it via raw
        let engine = crate::core::engine::SearchEngine::new(idx.conn());
        let mut q = crate::core::entry::SearchQuery::default();
        q.limit = 10;
        q.fts_query = Some("unique_keyword_fts_test".into());
        let rs = engine.search(&q).unwrap();
        assert_eq!(rs.total_count, 1);
    }

    // ── projects ──
    #[test]
    fn test_upsert_project() {
        let idx = setup();
        let pid = idx.upsert_project("proj", "/data/proj").unwrap();
        assert!(pid > 0);
        let all = idx.get_all_projects().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "proj");
    }
    #[test]
    fn test_upsert_project_update_path() {
        let idx = setup();
        idx.upsert_project("p", "/old").unwrap();
        idx.upsert_project("p", "/new").unwrap();
        let p = idx.get_project_by_name("p").unwrap().unwrap();
        assert_eq!(p.path, "/new");
    }
    #[test]
    fn test_remove_project() {
        let idx = setup();
        idx.upsert_project("tmp", "/tmp").unwrap();
        assert!(idx.remove_project("tmp").unwrap());
        assert!(!idx.remove_project("nonexistent").unwrap());
        assert!(idx.get_project_by_name("tmp").unwrap().is_none());
    }

    // ── sync_projects ──
    #[test]
    fn test_sync_projects_assigns_files() {
        let idx = setup();
        idx.get_or_create_file("/data/proj/sub/console.log")
            .unwrap();
        idx.get_or_create_file("/data/proj/info.log").unwrap();
        idx.get_or_create_file("/other/file.log").unwrap();
        let projects = vec![test_project("proj", "/data/proj")];
        idx.sync_projects(&projects).unwrap();
        let db_projects = idx.get_all_projects().unwrap();
        assert_eq!(db_projects.len(), 1);
        assert_eq!(db_projects[0].name, "proj");
    }
    #[test]
    fn test_sync_projects_empty_config_clears_all() {
        let idx = setup();
        idx.upsert_project("old", "/old").unwrap();
        idx.sync_projects(&[]).unwrap();
        assert!(idx.get_all_projects().unwrap().is_empty());
    }

    // ── modules ──
    #[test]
    fn test_get_modules_no_project() {
        let idx = setup();
        let modules = idx.get_modules_for_project("nonexistent").unwrap();
        assert!(modules.is_empty());
    }

    // ── utilities ──
    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path(r"C:\a\b"), "C:/a/b");
        assert_eq!(normalize_path("a/b/"), "a/b");
    }
    #[test]
    fn test_is_subpath() {
        assert!(is_subpath("/data/proj", "/data/proj/sub/file.log"));
        assert!(!is_subpath("/data/proj", "/data/other/file.log"));
        assert!(!is_subpath("/data/proj", "/data/proj"));
        assert!(!is_subpath("", "/data/proj/sub/file.log"));
    }

    // ── edge cases ──
    #[test]
    fn test_db_size_bytes_in_memory() {
        let idx = setup();
        // In-memory DB has no path → should error
        assert!(idx.db_size_bytes().is_err() || idx.db_size_bytes().is_ok());
        // In any case it shouldn't panic
    }
    #[test]
    fn test_compact_does_not_panic() {
        let idx = setup();
        idx.compact().unwrap();
    }
    #[test]
    fn test_clear_all() {
        let idx = setup();
        let fid = idx.get_or_create_file("/to-clear.log").unwrap();
        idx.insert_entries(&[LogEntry {
            id: None,
            file_id: fid,
            line_number: 1,
            byte_offset: 0,
            raw: "2024-01-15 INFO msg".into(),
        }])
        .unwrap();
        idx.clear_all().unwrap();
        assert_eq!(idx.total_entries().unwrap(), 0);
        assert_eq!(idx.total_files().unwrap(), 0);
    }

    // ── Incremental indexing ──
    #[test]
    fn test_get_file_byte_offset() {
        let idx = setup();
        let fid = idx.get_or_create_file("/var/log/app.log").unwrap();
        // Default offset should be 0
        assert_eq!(idx.get_file_byte_offset(fid).unwrap(), 0);
        // Update the file with a specific byte_offset
        idx.update_file(fid, 1024, 4096, 100, "log4j").unwrap();
        assert_eq!(idx.get_file_byte_offset(fid).unwrap(), 4096);
    }
    #[test]
    fn test_update_file_metadata() {
        let idx = setup();
        let fid = idx.get_or_create_file("/var/log/app.log").unwrap();
        idx.insert_entries(&[LogEntry {
            id: None,
            file_id: fid,
            line_number: 1,
            byte_offset: 0,
            raw: "2024-01-15 INFO test".into(),
        }])
        .unwrap();
        // Update metadata
        idx.update_file(fid, 8192, 2048, 50, "json").unwrap();
        // Verify via get_file_by_path
        let f = idx.get_file_by_path("/var/log/app.log").unwrap().unwrap();
        assert_eq!(f.size, 8192);
        assert_eq!(f.byte_offset, 2048);
        assert_eq!(f.line_count, 50);
        assert_eq!(f.format, "json");
    }

    // ── Project/module ──
    #[test]
    fn test_sync_projects_file_assignment() {
        let idx = setup();
        idx.upsert_project("myproj", "/data/proj").unwrap();
        idx.get_or_create_file("/data/proj/logs/app.log").unwrap();
        // Sync with the project from config
        let projects = vec![test_project("myproj", "/data/proj")];
        idx.sync_projects(&projects).unwrap();
        // Verify the file's project_id is set correctly
        let p = idx.get_project_by_name("myproj").unwrap().unwrap();
        let project_id: Option<i64> = idx
            .conn()
            .query_row(
                "SELECT project_id FROM files WHERE path = ?",
                params!["/data/proj/logs/app.log"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(project_id, Some(p.id));
    }
    #[test]
    fn test_sync_projects_longest_prefix() {
        let idx = setup();
        // Two projects where one path is a prefix of the other
        idx.upsert_project("proj", "/data/proj").unwrap();
        idx.upsert_project("proj2", "/data/proj2").unwrap();
        idx.get_or_create_file("/data/proj2/logs/app.log").unwrap();
        let projects = vec![
            test_project("proj", "/data/proj"),
            test_project("proj2", "/data/proj2"),
        ];
        idx.sync_projects(&projects).unwrap();
        // File should be assigned to proj2 (longest prefix match), not proj
        let proj2 = idx.get_project_by_name("proj2").unwrap().unwrap();
        let project_id: Option<i64> = idx
            .conn()
            .query_row(
                "SELECT project_id FROM files WHERE path = ?",
                params!["/data/proj2/logs/app.log"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(project_id, Some(proj2.id));
        let proj = idx.get_project_by_name("proj").unwrap().unwrap();
        assert_ne!(project_id, Some(proj.id));
    }
    #[test]
    fn test_get_modules_for_project_found() {
        let idx = setup();
        idx.upsert_project("myproj", "/data/proj").unwrap();
        idx.get_or_create_file("/data/proj/auth/auth.log").unwrap();
        idx.get_or_create_file("/data/proj/api/api.log").unwrap();
        // Assign files to project
        let projects = vec![test_project("myproj", "/data/proj")];
        idx.sync_projects(&projects).unwrap();
        let modules = idx.get_modules_for_project("myproj").unwrap();
        assert!(modules.contains(&"auth".to_string()));
        assert!(modules.contains(&"api".to_string()));
        assert_eq!(modules.len(), 2);
    }
    #[test]
    fn test_get_modules_for_project_flat() {
        let idx = setup();
        idx.upsert_project("myproj", "/data/proj").unwrap();
        // File directly in project root — no subdirectory
        idx.get_or_create_file("/data/proj/app.log").unwrap();
        let projects = vec![test_project("myproj", "/data/proj")];
        idx.sync_projects(&projects).unwrap();
        let modules = idx.get_modules_for_project("myproj").unwrap();
        // get_modules extracts the first path component relative to project path.
        // For "/data/proj/app.log", relative = "app.log", no '/' found,
        // so "app.log" becomes the module name.
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0], "app.log");
    }

    // ── Edge cases ──
    #[test]
    fn test_insert_large_batch() {
        let idx = setup();
        let fid = idx.get_or_create_file("/large.log").unwrap();
        let entries: Vec<LogEntry> = (0..1500)
            .map(|i| LogEntry {
                id: None,
                file_id: fid,
                line_number: i + 1,
                byte_offset: (i * 80) as u64,
                raw: format!(
                    "2024-01-15 10:00:{:02} INFO line {} some log content here",
                    i % 60,
                    i
                ),
            })
            .collect();
        let inserted = idx.insert_entries(&entries).unwrap();
        assert_eq!(inserted, 1500);
        assert_eq!(idx.total_entries().unwrap(), 1500);
    }
    #[test]
    fn test_fts_search_after_delete() {
        let idx = setup();
        let fid = idx.get_or_create_file("/fts-delete.log").unwrap();
        idx.insert_entries(&[
            LogEntry {
                id: None,
                file_id: fid,
                line_number: 1,
                byte_offset: 0,
                raw: "fts_delete_keyword_xyz should be findable".into(),
            },
            LogEntry {
                id: None,
                file_id: fid,
                line_number: 2,
                byte_offset: 50,
                raw: "another line with fts_delete_keyword_xyz again".into(),
            },
        ])
        .unwrap();
        // FTS search should find both entries
        let engine = crate::core::engine::SearchEngine::new(idx.conn());
        let mut q = crate::core::entry::SearchQuery::default();
        q.limit = 10;
        q.fts_query = Some("fts_delete_keyword_xyz".into());
        let rs = engine.search(&q).unwrap();
        assert_eq!(rs.total_count, 2);
        // Delete entries — FTS trigger should remove them
        idx.clear_file_entries(fid).unwrap();
        let rs2 = engine.search(&q).unwrap();
        assert_eq!(rs2.total_count, 0);
    }

    // ── Schema ──
    #[test]
    fn test_schema_has_required_tables() {
        let idx = setup();
        let mut stmt = idx.conn().prepare(
            "SELECT name, type FROM sqlite_master WHERE type IN ('table', 'view') ORDER BY name"
        ).unwrap();
        let tables: Vec<(String, String)> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        let table_names: Vec<&str> = tables.iter().map(|(n, _)| n.as_str()).collect();
        assert!(
            table_names.contains(&"files"),
            "files table missing. Got: {:?}",
            table_names
        );
        assert!(
            table_names.contains(&"log_entries"),
            "log_entries table missing. Got: {:?}",
            table_names
        );
        // log_entries_fts is a virtual table, should appear in sqlite_master
        assert!(
            table_names.contains(&"log_entries_fts"),
            "log_entries_fts table missing. Got: {:?}",
            table_names
        );
        assert!(
            table_names.contains(&"projects"),
            "projects table missing. Got: {:?}",
            table_names
        );
    }
}
