pub mod state;

use crate::core::config;
use crate::core::discovery::FileDiscovery;
use crate::core::encoding;
use crate::core::engine;
use crate::core::index::IndexManager;
use crate::core::parser::LogLineParser;

#[derive(serde::Serialize)]
struct SearchResultSet {
    total_count: u64,
    returned_count: u64,
    offset: u32,
    elapsed_ms: u64,
    results: Vec<SearchResultRow>,
}

#[derive(serde::Serialize)]
struct SearchResultRow {
    id: i64,
    source: String,
    line_number: u64,
    byte_offset: u64,
    timestamp: Option<String>,
    level: Option<String>,
    thread: Option<String>,
    logger: Option<String>,
    message: String,
    fields_json: Option<String>,
    raw: String,
}

#[derive(serde::Serialize)]
struct IndexStatsResponse {
    database_size_mb: String,
    total_files: usize,
    total_entries: u64,
    files: Vec<FileStatResponse>,
}

#[derive(serde::Serialize)]
struct FileStatResponse {
    path: String,
    format: String,
    entries: i64,
    size_mb: String,
    byte_offset: i64,
}

#[derive(serde::Deserialize)]
struct SearchParams {
    query: String,
    #[serde(default)]
    levels: Vec<String>,
    source: Option<String>,
    after: Option<String>,
    before: Option<String>,
    thread: Option<String>,
    #[serde(default)]
    use_regex: bool,
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 { 100 }

#[cfg(feature = "gui")]
#[tauri::command]
async fn search_logs(params: SearchParams) -> Result<SearchResultSet, String> {
    let cfg = config::load(None).map_err(|e| e.to_string())?;
    let idx = IndexManager::open(&cfg.general.database_path).map_err(|e| e.to_string())?;

    let mut sq = engine::parse_query_string(&params.query, params.limit);
    if !params.levels.is_empty() {
        sq.levels = params.levels;
    }
    if let Some(s) = params.source {
        sq.source = Some(s);
    }
    if let Some(a) = params.after {
        if let Some(dt) = engine::parse_relative_time(&a) {
            sq.after = Some(dt);
        }
    }
    if let Some(b) = params.before {
        if let Some(dt) = engine::parse_relative_time(&b) {
            sq.before = Some(dt);
        }
    }
    if let Some(t) = params.thread {
        sq.thread = Some(t);
    }

    let search_engine = engine::SearchEngine::new(idx.conn());

    let result_set = if params.use_regex || sq.regex_query.is_some() {
        let pattern = sq.regex_query.as_deref().unwrap_or(&params.query);
        search_engine.search_regex(pattern, &sq).map_err(|e| e.to_string())?
    } else {
        search_engine.search(&sq).map_err(|e| e.to_string())?
    };

    let results = result_set.results.into_iter().map(|r| SearchResultRow {
        id: r.id,
        source: r.source,
        line_number: r.line_number,
        byte_offset: r.byte_offset,
        timestamp: r.timestamp,
        level: r.level,
        thread: r.thread,
        logger: r.logger,
        message: r.message,
        fields_json: r.fields_json,
        raw: r.raw,
    }).collect();

    Ok(SearchResultSet {
        total_count: result_set.total_count,
        returned_count: result_set.returned_count,
        offset: result_set.offset,
        elapsed_ms: result_set.elapsed_ms,
        results,
    })
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn get_index_stats() -> Result<IndexStatsResponse, String> {
    let cfg = config::load(None).map_err(|e| e.to_string())?;
    let idx = IndexManager::open(&cfg.general.database_path).map_err(|e| e.to_string())?;

    let total_files = idx.total_files().map_err(|e| e.to_string())?;
    let total_entries = idx.total_entries().map_err(|e| e.to_string())?;
    let db_size = idx.db_size_bytes().map_err(|e| e.to_string())?;
    let files = idx.get_files().map_err(|e| e.to_string())?;

    let file_stats = files.into_iter().map(|f| FileStatResponse {
        path: f.path,
        format: f.format,
        entries: f.line_count,
        size_mb: format_size(f.size as u64),
        byte_offset: f.byte_offset,
    }).collect();

    Ok(IndexStatsResponse {
        database_size_mb: format_size(db_size),
        total_files,
        total_entries,
        files: file_stats,
    })
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn update_index() -> Result<String, String> {
    let cfg = config::load(None).map_err(|e| e.to_string())?;
    let idx = IndexManager::open(&cfg.general.database_path).map_err(|e| e.to_string())?;

    let discovered = FileDiscovery::scan_directories(&cfg.sources.directories);
    let max_file_size = config::parse_size(&cfg.general.max_file_size);
    let mut total = 0u64;

    for file in &discovered {
        if file.size > max_file_size || file.is_compressed {
            continue;
        }

        let file_path = file.path.to_string_lossy().to_string();
        let file_id = idx.get_or_create_file(&file_path).map_err(|e| e.to_string())?;

        let existing_files = idx.get_files().map_err(|e| e.to_string())?;
        let existing = existing_files.iter().find(|f| f.path == file_path);
        let current_byte_offset = existing.map(|f| f.byte_offset).unwrap_or(0);

        if file.size as i64 <= current_byte_offset && existing.is_some() {
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
            idx.insert_entries(&entries).map_err(|e| e.to_string())?;
            total += entries.len() as u64;
        }

        let mut line_count = existing.map(|f| f.line_count).unwrap_or(0);
        line_count += entries.len() as i64;
        idx.update_file(file_id, file.size as i64, file.size as i64, line_count, &parser.format.to_string())
            .map_err(|e| e.to_string())?;
    }

    Ok(format!("Indexed {} new entries", total))
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn add_directory(path: String, recursive: bool, encoding: String) -> Result<String, String> {
    let mut cfg = config::load(None).map_err(|e| e.to_string())?;
    let added = config::add_directory(&mut cfg, &path, recursive, &encoding);
    if added {
        config::save(&cfg, None).map_err(|e| e.to_string())?;
        Ok(format!("Added: {}", path))
    } else {
        Ok(format!("Already exists: {}", path))
    }
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn remove_directory(path: String) -> Result<String, String> {
    let mut cfg = config::load(None).map_err(|e| e.to_string())?;
    let removed = config::remove_directory(&mut cfg, &path);
    if removed {
        config::save(&cfg, None).map_err(|e| e.to_string())?;
        Ok(format!("Removed: {}", path))
    } else {
        Ok(format!("Not found: {}", path))
    }
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

#[cfg(feature = "gui")]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            search_logs,
            get_index_stats,
            update_index,
            add_directory,
            remove_directory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(not(feature = "gui"))]
pub fn run() {
    eprintln!("GUI feature not enabled. Rebuild with --features gui");
}
