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
    let mut total_new_entries = 0u64;
    let mut files_indexed = 0usize;

    for file in &discovered {
        if file.size > max_file_size && !file.is_rotated {
            println!("  Skipping (too large, {}): {}", format_size(file.size), file.path.display());
            continue;
        }
        if file.is_compressed {
            println!("  Skipping (compressed): {}", file.path.display());
            continue;
        }

        let file_path = file.path.to_string_lossy().to_string();
        let file_id = idx.get_or_create_file(&file_path)?;

        let existing_files = idx.get_files()?;
        let existing = existing_files.iter().find(|f| f.path == file_path);
        let current_byte_offset = existing.map(|f| f.byte_offset).unwrap_or(0);

        let file_size = file.size as i64;
        if file_size <= current_byte_offset && existing.is_some() {
            continue;
        }

        let content = encoding::read_file_to_utf8(&file_path, None);
        let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        let parser = LogLineParser::auto_detect(&lines);

        let start_byte = current_byte_offset as u64;
        let mut byte_count = 0u64;
        let new_lines: Vec<String> = lines.into_iter()
            .filter(|line| {
                byte_count += line.len() as u64 + 1;
                byte_count > start_byte
            })
            .collect();

        let entries = parser.parse_lines(&new_lines.join("\n"), file_id, start_byte);

        if !entries.is_empty() {
            idx.insert_entries(&entries)?;
            total_new_entries += entries.len() as u64;
        }

        let mut line_count = existing.map(|f| f.line_count).unwrap_or(0);
        line_count += entries.len() as i64;
        idx.update_file(file_id, file_size, file_size, line_count, &parser.format.to_string())?;

        files_indexed += 1;
        println!("  Indexed: {} ({} entries, format={})",
            file.path.display(), entries.len(), parser.format);
    }

    println!("\nDone. {} files indexed, {} new entries.", files_indexed, total_new_entries);
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
