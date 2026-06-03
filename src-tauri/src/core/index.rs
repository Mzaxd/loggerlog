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
