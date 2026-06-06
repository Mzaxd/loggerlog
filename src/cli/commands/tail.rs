use crate::cli::OutputFormat;
use crate::core::config;
use crate::core::watcher::FileWatcher;
use anyhow::Result;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

pub fn run(
    source: Option<&str>,
    levels: &[String],
    filter: Option<&str>,
    _output: &OutputFormat,
    config_path: Option<&str>,
) -> Result<()> {
    let cfg = config::load(config_path)?;

    // Resolve the file to tail and directories to watch
    let (initial_file, watch_dirs) = resolve_target(source, &cfg)?;

    println!("Tailing: {} (press Ctrl+C to stop)", initial_file.display());

    // Start file watcher
    let (_watcher, event_rx) = FileWatcher::watch(&watch_dirs, 500)?;

    // Open the file and seek to end
    let mut tailed = TailedFile::open(&initial_file)?;

    loop {
        // Drain all available lines from the file
        loop {
            let mut line = String::new();
            match tailed.reader.read_line(&mut line) {
                Ok(0) => break, // EOF — no more data right now
                Ok(_) => {
                    if matches_filters(&line, levels, filter) {
                        println!("{}", line.trim());
                    }
                }
                Err(_) => break,
            }
        }

        // Block until event or timeout
        match event_rx.recv_timeout(Duration::from_secs(1)) {
            Ok(event) => {
                if event.path == tailed.path {
                    // File we're tailing changed — wake up and read new lines
                    continue;
                }
                // A different file changed — could be a rotation creating a new log file
                if is_log_file(&event.path) {
                    if let Ok(new_meta) = std::fs::metadata(&event.path) {
                        let should_switch = if tailed.path.exists() {
                            if let Ok(cur_meta) = std::fs::metadata(&tailed.path) {
                                let new_modified = new_meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                                let cur_modified = cur_meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                                // Switch if the new file is more recently modified or smaller (fresh rotation)
                                new_modified > cur_modified || new_meta.len() < cur_meta.len()
                            } else {
                                true // current file gone
                            }
                        } else {
                            true // current file no longer exists
                        };
                        if should_switch {
                            eprintln!("--- Log rotated, switching to {} ---", event.path.display());
                            tailed = TailedFile::open(&event.path)?;
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Periodic fallback: check for rotation via file size
                tailed.check_rotation();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                eprintln!("File watcher disconnected, exiting");
                break;
            }
        }
    }

    Ok(())
}

/// A file being tailed, tracking its path and read position.
struct TailedFile {
    path: PathBuf,
    reader: BufReader<File>,
    last_size: u64,
}

impl TailedFile {
    fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let last_size = file.metadata()?.len();
        let mut reader = BufReader::new(file);
        reader.get_mut().seek(SeekFrom::End(0))?;
        Ok(Self { path: path.to_path_buf(), reader, last_size })
    }

    /// Check if the file size decreased (log rotation), and re-seek to end if so.
    fn check_rotation(&mut self) {
        if let Ok(meta) = std::fs::metadata(&self.path) {
            if meta.len() < self.last_size {
                eprintln!("--- Log rotated, re-reading file ---");
                let _ = self.reader.get_mut().seek(SeekFrom::End(0));
            }
            self.last_size = meta.len();
        }
    }
}

/// Resolve source argument into the initial file to tail and directories to watch.
fn resolve_target(
    source: Option<&str>,
    cfg: &config::Config,
) -> Result<(PathBuf, Vec<PathBuf>)> {
    match source {
        Some(s) => {
            let p = PathBuf::from(s);
            if p.is_file() {
                // Watch parent directory to detect sibling file changes (rotation)
                let watch_dir = p.parent().unwrap_or(&p).to_path_buf();
                Ok((p, vec![watch_dir]))
            } else if p.is_dir() {
                let file = find_most_recent_log(&p)?;
                Ok((file, vec![p]))
            } else {
                anyhow::bail!("{}: not a file or directory", s);
            }
        }
        None => {
            if cfg.sources.directories.is_empty() {
                anyhow::bail!("No log directories configured. Use 'loggerlog config add-dir <path>' first.");
            }
            let dirs: Vec<PathBuf> = cfg.sources.directories.iter().map(|d| PathBuf::from(&d.path)).collect();
            let first_dir = &dirs[0];
            let file = find_most_recent_log(first_dir)?;
            Ok((file, dirs))
        }
    }
}

/// Find the most recently modified log file in a directory.
fn find_most_recent_log(dir: &Path) -> Result<PathBuf> {
    let mut best: Option<PathBuf> = None;
    let mut best_time = std::time::SystemTime::UNIX_EPOCH;

    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && is_log_file(&path) {
                if let Ok(modified) = entry.metadata()?.modified() {
                    if modified > best_time {
                        best_time = modified;
                        best = Some(path);
                    }
                }
            }
        }
    }

    best.ok_or_else(|| anyhow::anyhow!("No log files found in {}", dir.display()))
}

/// Check if a file path looks like a log file by extension.
fn is_log_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map_or(false, |e| matches!(e, "log" | "txt" | "out" | "app"))
}

/// Check if a log line matches the level and keyword filters.
fn matches_filters(line: &str, levels: &[String], filter: Option<&str>) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    if !levels.is_empty() {
        let matches_level = levels.iter().any(|l| {
            let upper = trimmed.to_uppercase();
            upper.contains(&format!(" {} ", l)) || upper.starts_with(&format!("{} ", l))
        });
        if !matches_level {
            return false;
        }
    }
    if let Some(f) = filter {
        if !trimmed.contains(f) {
            return false;
        }
    }
    true
}
