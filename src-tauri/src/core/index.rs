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
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA cache_size=-64000;")?;
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
                timestamp   TEXT,
                level       TEXT,
                thread      TEXT,
                logger      TEXT,
                message     TEXT NOT NULL DEFAULT '',
                fields_json TEXT,
                raw         TEXT NOT NULL DEFAULT ''
            );

            CREATE INDEX IF NOT EXISTS idx_entries_file ON log_entries(file_id);
            CREATE INDEX IF NOT EXISTS idx_entries_timestamp ON log_entries(timestamp);
            CREATE INDEX IF NOT EXISTS idx_entries_level ON log_entries(level);
            CREATE INDEX IF NOT EXISTS idx_entries_thread ON log_entries(thread);

            CREATE VIRTUAL TABLE IF NOT EXISTS log_entries_fts USING fts5(
                message,
                raw,
                content='log_entries',
                content_rowid='id',
                tokenize='unicode61 remove_diacritics 1'
            );

            CREATE TRIGGER IF NOT EXISTS log_entries_ai AFTER INSERT ON log_entries BEGIN
                INSERT INTO log_entries_fts(rowid, message, raw) VALUES (new.id, new.message, new.raw);
            END;

            CREATE TRIGGER IF NOT EXISTS log_entries_ad AFTER DELETE ON log_entries BEGIN
                INSERT INTO log_entries_fts(log_entries_fts, rowid, message, raw)
                    VALUES('delete', old.id, old.message, old.raw);
            END;

            CREATE TRIGGER IF NOT EXISTS log_entries_au AFTER UPDATE ON log_entries BEGIN
                INSERT INTO log_entries_fts(log_entries_fts, rowid, message, raw)
                    VALUES('delete', old.id, old.message, old.raw);
                INSERT INTO log_entries_fts(rowid, message, raw) VALUES (new.id, new.message, new.raw);
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
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)"
        )?;

        let version: i32 = self.conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        if version < 2 {
            self.migrate_v2()?;
            self.conn.execute(
                "INSERT INTO schema_version (version) VALUES (2)",
                [],
            )?;
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
            );"
        )?;

        // Add project_id column to files if it doesn't exist (must come before index)
        let has_project_id: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('files') WHERE name = 'project_id'",
            [],
            |row| row.get::<_, i32>(0).map(|c| c > 0),
        ).unwrap_or(false);

        if !has_project_id {
            self.conn.execute_batch(
                "ALTER TABLE files ADD COLUMN project_id INTEGER REFERENCES projects(id) ON DELETE SET NULL;"
            )?;
        }

        // Create index after column exists
        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_files_project ON files(project_id);"
        )?;

        Ok(())
    }

    /// Get or create a file record
    pub fn get_or_create_file(&self, path: &str) -> Result<i64> {
        let existing: Option<i64> = self.conn.query_row(
            "SELECT id FROM files WHERE path = ?",
            params![path],
            |row| row.get(0),
        ).ok();

        if let Some(id) = existing {
            Ok(id)
        } else {
            self.conn.execute(
                "INSERT INTO files (path) VALUES (?)",
                params![path],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    /// Update file metadata
    pub fn update_file(&self, file_id: i64, size: i64, byte_offset: i64, line_count: i64, format: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE files SET size = ?, byte_offset = ?, line_count = ?, format = ?, updated_at = datetime('now') WHERE id = ?",
            params![size, byte_offset, line_count, format, file_id],
        )?;
        Ok(())
    }

    /// Insert log entries in batch
    pub fn insert_entries(&self, entries: &[crate::core::entry::LogEntry]) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        let tx = self.conn.unchecked_transaction()?;
        let mut count = 0;

        {
            let mut stmt = tx.prepare(
                "INSERT INTO log_entries (file_id, line_number, byte_offset, timestamp, level, thread, logger, message, fields_json, raw)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )?;

            for entry in entries {
                let ts = entry.timestamp.map(|t| t.to_rfc3339());
                stmt.execute(params![
                    entry.file_id,
                    entry.line_number as i64,
                    entry.byte_offset as i64,
                    ts,
                    &entry.level,
                    &entry.thread,
                    &entry.logger,
                    &entry.message,
                    &entry.fields_json,
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
        self.conn.execute("DELETE FROM log_entries WHERE file_id = ?", params![file_id])?;
        Ok(())
    }

    /// Compact/optimize the FTS index
    pub fn compact(&self) -> Result<()> {
        self.conn.execute_batch(
            "INSERT INTO log_entries_fts(log_entries_fts) VALUES('rebuild');
             PRAGMA optimize;"
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
        let count: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM log_entries",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Get total file count
    pub fn total_files(&self) -> Result<usize> {
        let count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM files",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Get file byte offset (for incremental indexing)
    pub fn get_file_byte_offset(&self, file_id: i64) -> Result<i64> {
        let offset: i64 = self.conn.query_row(
            "SELECT byte_offset FROM files WHERE id = ?",
            params![file_id],
            |row| row.get(0),
        ).unwrap_or(0);
        Ok(offset)
    }

    /// Get a single file record by path (point query, efficient)
    pub fn get_file_by_path(&self, path: &str) -> Result<Option<FileRecord>> {
        let result = self.conn.query_row(
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
        ).ok();
        Ok(result)
    }

    /// Get all indexed files
    pub fn get_files(&self) -> Result<Vec<FileRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, size, format, byte_offset, line_count FROM files ORDER BY path"
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
             PRAGMA optimize;"
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
        let affected = self.conn.execute(
            "DELETE FROM projects WHERE name = ?",
            params![name],
        )?;
        Ok(affected > 0)
    }

    /// Get all projects
    pub fn get_all_projects(&self) -> Result<Vec<ProjectRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, path FROM projects ORDER BY name"
        )?;
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
        let result = self.conn.query_row(
            "SELECT id, name, path FROM projects WHERE name = ?",
            params![name],
            |row| {
                Ok(ProjectRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                })
            },
        ).ok();
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
            self.conn.execute("UPDATE files SET project_id = NULL WHERE project_id IS NOT NULL", [])?;
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
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|b| b.as_ref()).collect();
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

        let mut matched = 0u64;
        let mut unmatched = 0u64;

        let tx = self.conn.unchecked_transaction()?;

        {
            let mut update_stmt = tx.prepare(
                "UPDATE files SET project_id = ? WHERE id = ?"
            )?;

            for file in &files {
                // Find the longest matching project path
                let project_id = sorted_projects.iter().find(|p| {
                    is_subpath(&p.path, &file.path)
                }).map(|p| p.id);

                if project_id.is_some() {
                    matched += 1;
                } else {
                    unmatched += 1;
                }

                update_stmt.execute(params![
                    project_id,
                    file.id,
                ])?;
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

        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT path FROM files WHERE project_id = ? ORDER BY path"
        )?;

        let paths: Vec<String> = stmt.query_map(params![project.id], |row| {
            row.get::<_, String>(0)
        })?.filter_map(|r| r.ok()).collect();

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
    file.starts_with(&dir) && file.len() > dir.len()
        && file[dir.len()..].starts_with('/')
}
