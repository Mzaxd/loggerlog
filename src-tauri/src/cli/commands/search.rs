use crate::cli::OutputFormat;
use crate::core::config;
use crate::core::engine::{self, SearchResultSet};
use crate::core::index::IndexManager;
use anyhow::Result;
use comfy_table::{Cell, Color, Table};

pub fn run(
    query_str: &str,
    levels: &[String],
    source: Option<&str>,
    after: Option<&str>,
    before: Option<&str>,
    thread: Option<&str>,
    use_regex: bool,
    limit: u32,
    _context_size: Option<u32>,
    output: &OutputFormat,
    config_path: Option<&str>,
) -> Result<()> {
    let cfg = config::load(config_path)?;
    let idx = IndexManager::open(&cfg.general.database_path)?;

    let mut sq = engine::parse_query_string(query_str, limit);
    if !levels.is_empty() {
        sq.levels = levels.to_vec();
    }
    if let Some(s) = source {
        sq.source = Some(s.to_string());
    }
    if let Some(a) = after {
        if let Some(dt) = engine::parse_relative_time(a) {
            sq.after = Some(dt);
        }
    }
    if let Some(b) = before {
        if let Some(dt) = engine::parse_relative_time(b) {
            sq.before = Some(dt);
        }
    }
    if let Some(t) = thread {
        sq.thread = Some(t.to_string());
    }

    let search_engine = engine::SearchEngine::new(idx.conn());

    let result_set = if use_regex || sq.regex_query.is_some() {
        let pattern = sq.regex_query.as_deref().unwrap_or(query_str);
        search_engine.search_regex(pattern, &sq)?
    } else {
        search_engine.search(&sq)?
    };

    match output {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&result_set)?);
        }
        OutputFormat::Raw => {
            for r in &result_set.results {
                println!("{}", r.raw);
            }
        }
        OutputFormat::Table => {
            print_table(&result_set);
        }
    }

    Ok(())
}

fn print_table(result_set: &SearchResultSet) {
    let mut table = Table::new();
    table.load_preset(comfy_table::presets::UTF8_FULL_CONDENSED)
        .set_header(vec![
            Cell::new("TIMESTAMP").fg(Color::DarkCyan),
            Cell::new("LEVEL").fg(Color::Yellow),
            Cell::new("SOURCE").fg(Color::Green),
            Cell::new("LINE").fg(Color::DarkGrey),
            Cell::new("MESSAGE"),
        ]);

    for r in &result_set.results {
        let ts = r.timestamp.as_deref().unwrap_or("-");
        let level = r.level.as_deref().unwrap_or("-");
        let source = shorten_path(&r.source);
        let msg = truncate(&r.message, 80);

        let level_cell = match level {
            "ERROR" | "FATAL" | "SEVERE" => Cell::new(level).fg(Color::Red),
            "WARN" | "WARNING" => Cell::new(level).fg(Color::Yellow),
            "INFO" => Cell::new(level).fg(Color::Green),
            "DEBUG" => Cell::new(level).fg(Color::Blue),
            _ => Cell::new(level),
        };

        table.add_row(vec![
            Cell::new(ts),
            level_cell,
            Cell::new(&source),
            Cell::new(r.line_number),
            Cell::new(&msg),
        ]);
    }

    println!("{table}");
    println!(
        "\nShowing {} of {} results ({:.1}ms)",
        result_set.returned_count,
        result_set.total_count,
        result_set.elapsed_ms as f64 / 1000.0
    );
}

fn shorten_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() > 3 {
        parts[parts.len()-3..].join("/")
    } else {
        path.to_string()
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}
