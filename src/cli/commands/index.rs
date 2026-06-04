use crate::cli::IndexAction;
use crate::core::config;
use crate::core::discovery::FileDiscovery;
use crate::core::encoding;
use crate::core::index::IndexManager;
use crate::core::parser::LogLineParser;
use anyhow::Result;

pub fn run(action: IndexAction, config_path: Option<&str>) -> Result<()> {
    match action {
        IndexAction::Update => update_index(config_path),
        IndexAction::Rebuild => rebuild_index(config_path),
        IndexAction::Compact => compact_index(config_path),
        IndexAction::Stats => show_stats(config_path),
    }
}

fn update_index(config_path: Option<&str>) -> Result<()> {
    let cfg = config::load(config_path)?;
    if cfg.sources.directories.is_empty() {
        println!("No log directories configured. Use 'loggerlog config add-dir <path>' first.");
        return Ok(());
    }

    let idx = IndexManager::open(&cfg.general.database_path)?;

    let discovered = FileDiscovery::scan_directories(&cfg.sources.directories);
    println!("Discovered {} log files", discovered.len());

    let max_file_size = config::parse_size(&cfg.general.max_file_size);

    // ── Phase 1 (serial): filter + get file_ids ──
    struct Job {
        file_path: String,
        display_path: std::path::PathBuf,
        file_id: i64,
        is_compressed: bool,
    }

    let existing_files = idx.get_files()?;
    let mut jobs: Vec<Job> = Vec::new();

    for file in &discovered {
        if file.size > max_file_size && !file.is_rotated {
            println!("  Skipping (too large, {}): {}", format_size(file.size), file.path.display());
            continue;
        }
        let file_path = file.path.to_string_lossy().to_string();
        let file_id = idx.get_or_create_file(&file_path)?;
        let existing = existing_files.iter().find(|f| f.path == file_path);
        let is_new = existing.is_none();
        let current_byte_offset = existing.map(|f| f.byte_offset).unwrap_or(0);
        let file_size = file.size as i64;

        if !file.is_compressed && file_size <= current_byte_offset && !is_new {
            continue; // already up-to-date
        }

        jobs.push(Job {
            file_path,
            display_path: file.path.clone(),
            file_id,
            is_compressed: file.is_compressed,
        });
    }

    // ── Phase 2 (parallel): read + parse ──
    struct JobResult {
        file_id: i64,
        display_path: std::path::PathBuf,
        entries: Vec<crate::core::entry::LogEntry>,
        format_str: String,
        is_compressed: bool,
    }

    use rayon::prelude::*;
    let results: Vec<JobResult> = jobs
        .par_iter()
        .map(|job| -> Result<JobResult> {
            let (entries, format_str) = if job.is_compressed {
                let content = encoding::read_gz_to_utf8(&job.file_path)?;
                let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
                let parser = LogLineParser::auto_detect(&lines);
                (parser.parse_lines(&content, job.file_id, 0), parser.format.to_string())
            } else {
                let existing = existing_files.iter().find(|f| f.path == job.file_path);
                let current_byte_offset = existing.map(|f| f.byte_offset).unwrap_or(0);
                let content = encoding::read_file_to_utf8(&job.file_path, None);
                let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
                let parser = LogLineParser::auto_detect(&lines);
                let start_byte = current_byte_offset as u64;
                let mut byte_count = 0u64;
                let new_lines: Vec<String> = lines
                    .into_iter()
                    .filter(|line| {
                        byte_count += line.len() as u64 + 1;
                        byte_count > start_byte
                    })
                    .collect();
                (parser.parse_lines(&new_lines.join("\n"), job.file_id, start_byte), parser.format.to_string())
            };

            Ok(JobResult {
                file_id: job.file_id,
                display_path: job.display_path.clone(),
                entries,
                format_str,
                is_compressed: job.is_compressed,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    // ── Phase 3 (serial): write to SQLite ──
    let mut total_new_entries = 0u64;
    let mut files_indexed = 0usize;

    for r in &results {
        let entry_count = r.entries.len();
        if !r.entries.is_empty() {
            idx.insert_entries(&r.entries)?;
            total_new_entries += entry_count as u64;
        }

        if r.is_compressed {
            idx.update_file(r.file_id, entry_count as i64, entry_count as i64, entry_count as i64, &r.format_str)?;
        } else {
            let existing = existing_files.iter().find(|f| f.path == r.display_path.to_string_lossy());
            let prior_lines = existing.map(|f| f.line_count).unwrap_or(0);
            let path_str = r.display_path.to_string_lossy().to_string();
            let file_size = discovered.iter()
                .find(|d| d.path.to_string_lossy() == path_str)
                .map(|d| d.size as i64)
                .unwrap_or(0);
            idx.update_file(r.file_id, file_size, file_size, prior_lines + entry_count as i64, &r.format_str)?;
        }

        files_indexed += 1;
        println!("  Indexed: {} ({} entries, format={}{})",
            r.display_path.display(), entry_count, r.format_str,
            if r.is_compressed { ", compressed" } else { "" });
    }

    println!("\nDone. {} files indexed, {} new entries.", files_indexed, total_new_entries);

    if !cfg.projects.projects.is_empty() {
        println!("Syncing project mappings...");
        if let Err(e) = idx.sync_projects(&cfg.projects.projects) {
            eprintln!("Warning: project sync failed: {}", e);
        } else {
            println!("Project mappings synced.");
        }
    }

    Ok(())
}

fn rebuild_index(config_path: Option<&str>) -> Result<()> {
    let cfg = config::load(config_path)?;
    let idx = IndexManager::open(&cfg.general.database_path)?;

    println!("Clearing existing index...");
    idx.clear_all()?;
    println!("Index cleared. Run 'loggerlog index update' to re-index.");
    Ok(())
}

fn compact_index(config_path: Option<&str>) -> Result<()> {
    let cfg = config::load(config_path)?;
    let idx = IndexManager::open(&cfg.general.database_path)?;

    println!("Compacting FTS index...");
    idx.compact()?;

    let db_size = idx.db_size_bytes()?;
    println!("Done. Database size: {}", format_size(db_size));
    Ok(())
}

fn show_stats(config_path: Option<&str>) -> Result<()> {
    let cfg = config::load(config_path)?;
    let idx = IndexManager::open(&cfg.general.database_path)?;

    let total_files = idx.total_files()?;
    let total_entries = idx.total_entries()?;
    let db_size = idx.db_size_bytes()?;
    let files = idx.get_files()?;

    println!("Index Statistics");
    println!("================");
    println!("Database size: {}", format_size(db_size));
    println!("Total files:   {}", total_files);
    println!("Total entries: {}", total_entries);
    println!();

    if !files.is_empty() {
        println!("{:<50} {:>10} {:>10} {:>8}", "File", "Entries", "Size", "Format");
        println!("{}", "-".repeat(82));
        for f in &files {
            let short_path = if f.path.len() > 47 {
                format!("...{}", &f.path[f.path.len()-47..])
            } else {
                f.path.clone()
            };
            println!("{:<50} {:>10} {:>8} {:>8}",
                short_path,
                f.line_count,
                format_size(f.size as u64),
                f.format
            );
        }
    }

    Ok(())
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

/// Result of a quick incremental sync
#[derive(Debug, Default)]
pub struct SyncResult {
    pub files_scanned: usize,
    pub files_updated: usize,
    pub new_entries: u64,
}

/// Quick incremental sync: index only new content in configured directories.
/// Silent by default — the caller decides whether to print results.
/// Handles log rotation (file truncation) by detecting size shrink and re-indexing.
pub fn incremental_sync(config_path: Option<&str>) -> Result<SyncResult> {
    let cfg = config::load(config_path)?;
    if cfg.sources.directories.is_empty() {
        return Ok(SyncResult::default());
    }

    let idx = IndexManager::open(&cfg.general.database_path)?;
    let discovered = FileDiscovery::scan_directories(&cfg.sources.directories);
    let max_file_size = config::parse_size(&cfg.general.max_file_size);

    // ── Phase 1 (serial): collect work items ──
    struct IncrJob {
        file_path: String,
        file_id: i64,
        file_size: i64,
        is_compressed: bool,
        existing_line_count: i64,
        stored_offset: i64,
        needs_clear: bool,       // true if rotation detected (size shrunk)
        needs_full_read: bool,   // true for compressed, rotation fallback, or seek failure
    }

    let mut result = SyncResult::default();
    let mut jobs: Vec<IncrJob> = Vec::new();

    for file in &discovered {
        if file.size > max_file_size && !file.is_rotated {
            continue;
        }
        let file_path = file.path.to_string_lossy().to_string();
        let file_id = idx.get_or_create_file(&file_path)?;
        let file_size = file.size as i64;

        result.files_scanned += 1;

        if file.is_compressed {
            let existing = idx.get_file_by_path(&file_path)?;
            let stored_line_count = existing.as_ref().map(|f| f.line_count).unwrap_or(0);
            jobs.push(IncrJob {
                file_path, file_id, file_size,
                is_compressed: true,
                existing_line_count: stored_line_count,
                stored_offset: 0,
                needs_clear: existing.is_some(),
                needs_full_read: true,
            });
            continue;
        }

        let existing = idx.get_file_by_path(&file_path)?;
        let stored_offset = existing.as_ref().map(|f| f.byte_offset).unwrap_or(0);
        let stored_line_count = existing.as_ref().map(|f| f.line_count).unwrap_or(0);

        if file_size == stored_offset {
            continue; // no change
        }

        let needs_clear = file_size < stored_offset;
        if needs_clear {
            idx.clear_file_entries(file_id)?;
        }

        jobs.push(IncrJob {
            file_path, file_id, file_size,
            is_compressed: false,
            existing_line_count: stored_line_count,
            stored_offset,
            needs_clear,
            needs_full_read: needs_clear,
        });
    }

    // ── Phase 2 (parallel): read + parse ──
    struct IncrResult {
        file_id: i64,
        file_size: i64,
        entries: Vec<crate::core::entry::LogEntry>,
        format_str: String,
        is_compressed: bool,
        needs_clear: bool,
        existing_line_count: i64,
    }

    use rayon::prelude::*;
    let db_results: Vec<IncrResult> = jobs
        .par_iter()
        .map(|job| -> Result<IncrResult> {
            let (entries, format_str) = if job.is_compressed {
                let content = encoding::read_gz_to_utf8(&job.file_path)?;
                let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
                let parser = LogLineParser::auto_detect(&lines);
                (parser.parse_lines(&content, job.file_id, 0), parser.format.to_string())
            } else if job.needs_full_read {
                let content = encoding::read_file_to_utf8(&job.file_path, None);
                let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
                let parser = LogLineParser::auto_detect(&lines);
                (parser.parse_lines(&content, job.file_id, 0), parser.format.to_string())
            } else {
                let content = match encoding::read_file_from_offset(&job.file_path, job.stored_offset as u64) {
                    Ok(c) => c,
                    Err(_) => encoding::read_file_to_utf8(&job.file_path, None),
                };
                let content = content;
                if content.trim().is_empty() {
                    return Ok(IncrResult {
                        file_id: job.file_id, file_size: job.file_size,
                        entries: Vec::new(), format_str: "unknown".into(),
                        is_compressed: false, needs_clear: false,
                        existing_line_count: 0,
                    });
                }
                let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
                let parser = LogLineParser::auto_detect(&lines);
                (parser.parse_lines(&content, job.file_id, job.stored_offset as u64), parser.format.to_string())
            };

            Ok(IncrResult {
                file_id: job.file_id, file_size: job.file_size,
                entries,
                format_str,
                is_compressed: job.is_compressed,
                needs_clear: job.needs_clear,
                existing_line_count: job.existing_line_count,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    // ── Phase 3 (serial): write to SQLite ──
    for r in &db_results {
        let entry_count = r.entries.len() as i64;

        if r.is_compressed {
            if entry_count <= r.existing_line_count && !r.needs_clear {
                continue; // unchanged gz file
            }
        } else if entry_count == 0 {
            continue;
        }

        if r.is_compressed || r.needs_clear {
            if r.is_compressed {
                idx.clear_file_entries(r.file_id)?;
            }
            if !r.entries.is_empty() {
                idx.insert_entries(&r.entries)?;
                result.new_entries += r.entries.len() as u64;
            }
            idx.update_file(r.file_id, entry_count, entry_count, entry_count, &r.format_str)?;
        } else {
            if !r.entries.is_empty() {
                idx.insert_entries(&r.entries)?;
                result.new_entries += r.entries.len() as u64;
            }
            let line_count = r.existing_line_count + entry_count;
            idx.update_file(r.file_id, r.file_size, r.file_size, line_count, &r.format_str)?;
        }
        result.files_updated += 1;
    }

    // Sync project mappings
    if !cfg.projects.projects.is_empty() {
        let _ = idx.sync_projects(&cfg.projects.projects);
    }

    Ok(result)
}
