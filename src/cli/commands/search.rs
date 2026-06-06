use crate::cli::OutputFormat;
use crate::core::config;
use crate::core::engine::{self, SearchResultSet, SearchSummary};
use crate::core::index::IndexManager;
use anyhow::Result;
use comfy_table::{Cell, Color, Table};

pub fn run(
    query_str: &str,
    levels: &[String],
    source: Option<&str>,
    project: Option<&str>,
    module: Option<&str>,
    after: Option<&str>,
    before: Option<&str>,
    thread: Option<&str>,
    use_regex: bool,
    limit: u32,
    context_size: Option<u32>,
    max_chars: Option<usize>,
    exclude: Vec<String>,
    unique: bool,
    output_file: Option<String>,
    summary: bool,
    output: &OutputFormat,
    config_path: Option<&str>,
    no_sync: bool,
) -> Result<()> {
    // Auto incremental sync before search (unless --no-sync)
    if !no_sync {
        match super::index::incremental_sync(config_path) {
            Ok(result) if result.files_updated > 0 => {
                eprintln!(
                    "[sync] {} files updated, {} new entries",
                    result.files_updated, result.new_entries
                );
            }
            Err(e) => {
                eprintln!("Warning: incremental sync failed: {}", e);
            }
            _ => {}
        }
    }

    let cfg = config::load(config_path)?;
    let idx = IndexManager::open(&cfg.general.database_path)?;

    let mut sq = engine::parse_query_string(query_str, limit);
    if !levels.is_empty() {
        sq.levels = levels
            .iter()
            .flat_map(|l| l.split(',').map(|s| s.trim().to_uppercase()))
            .collect();
    }
    if let Some(s) = source {
        sq.source = Some(s.to_string());
    }
    if let Some(p) = project {
        sq.project = Some(p.to_string());
    }
    if let Some(m) = module {
        sq.module = Some(m.to_string());
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
    sq.exclude = exclude;

    let search_engine = engine::SearchEngine::new(idx.conn());

    // Summary mode — show aggregated stats instead of individual results
    if summary {
        let summary_result = search_engine.search_summary(&sq, 10)?;
        let output_str = format!("{}", format_summary(&summary_result, output));
        let final_output = match max_chars {
            Some(max) => apply_max_chars(&output_str, max, summary_result.total_count),
            None => output_str,
        };
        print!("{}", final_output);
        return Ok(());
    }

    let result_set = if use_regex || sq.regex_query.is_some() {
        let pattern = sq.regex_query.as_deref().unwrap_or(query_str);
        search_engine.search_regex(pattern, &sq)?
    } else {
        search_engine.search(&sq)?
    };

    // Post-processing: fold stack traces before formatting
    let mut result_set = result_set;
    fold_stack_traces(&mut result_set.results);

    // Deduplicate if requested
    if unique {
        result_set.results = deduplicate_results(std::mem::take(&mut result_set.results));
    }

    // Fetch context lines if requested
    let context_data: Option<
        Vec<(
            crate::core::entry::SearchResult,
            Vec<crate::core::entry::SearchResult>,
        )>,
    > = if let Some(n) = context_size {
        let mut ctx = Vec::new();
        for r in &result_set.results {
            let lines = search_engine.get_context(r.file_id, r.line_number, n)?;
            ctx.push((r.clone(), lines));
        }
        Some(ctx)
    } else {
        None
    };

    // Level stats for header display
    let level_stats = search_engine.level_stats(&sq).ok();

    // Format output to string
    let output_str = format_output(
        &result_set,
        level_stats.as_deref(),
        context_data.as_deref(),
        output,
    );

    // Apply max-chars truncation
    let final_output = match max_chars {
        Some(max) => apply_max_chars(&output_str, max, result_set.total_count),
        _ => output_str,
    };

    // Write to file if requested, otherwise print to stdout
    if let Some(ref path) = output_file {
        std::fs::write(path, &final_output)?;
        println!("Output written to: {}", path);
        println!(
            "Total: {} results ({:.1}ms)",
            result_set.total_count,
            result_set.elapsed_ms as f64 / 1000.0
        );
    } else {
        print!("{}", final_output);
    }

    Ok(())
}

/// Format search results into a string based on output format
fn format_output(
    result_set: &SearchResultSet,
    level_stats: Option<&[engine::LevelCount]>,
    context_data: Option<
        &[(
            crate::core::entry::SearchResult,
            Vec<crate::core::entry::SearchResult>,
        )],
    >,
    output: &OutputFormat,
) -> String {
    let level_header = level_stats
        .map(|ls| {
            let parts: Vec<String> = ls
                .iter()
                .map(|lc| format!("{}: {}", lc.level.to_lowercase(), lc.count))
                .collect();
            format!("{}\n\n", parts.join(" | "))
        })
        .unwrap_or_default();

    match output {
        OutputFormat::Json => serde_json::to_string_pretty(&result_set).unwrap_or_default(),
        OutputFormat::Raw => result_set
            .results
            .iter()
            .map(|r| r.raw.clone())
            .collect::<Vec<_>>()
            .join("\n"),
        OutputFormat::Table => {
            let body = if let Some(ctx) = context_data {
                format_table_with_context(ctx)
            } else {
                format_table(result_set)
            };
            format!("{}{}", level_header, body)
        }
        OutputFormat::Compact => {
            let body = if let Some(ctx) = context_data {
                format_compact_with_context(ctx)
            } else {
                format_compact(result_set)
            };
            format!("{}{}", level_header, body)
        }
    }
}

/// Format results as a bordered table string
fn format_table(result_set: &SearchResultSet) -> String {
    let mut table = Table::new();
    table
        .load_preset(comfy_table::presets::UTF8_FULL_CONDENSED)
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

    let mut output = format!("{table}");
    let count_str = if result_set.approximate {
        format!("~{}", result_set.total_count)
    } else {
        result_set.total_count.to_string()
    };
    output.push_str(&format!(
        "\nShowing {} of {} results ({:.1}ms)",
        result_set.returned_count,
        count_str,
        result_set.elapsed_ms as f64 / 1000.0
    ));
    output
}

/// Format results in compact one-line format: [TIME] [LEVEL] [SOURCE:LINE] MESSAGE
fn format_compact(result_set: &SearchResultSet) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(result_set.results.len() + 1);
    for r in &result_set.results {
        let ts = r.timestamp.as_deref().unwrap_or("-");
        let level = r.level.as_deref().unwrap_or("-");
        let source = shorten_path(&r.source);
        let msg = collapse_multiline(&r.message);
        lines.push(format!(
            "[{}] [{}] [{}:{}] {}",
            ts, level, source, r.line_number, msg
        ));
    }
    lines.push(format!(
        "\nShowing {} of {} results ({:.1}ms)",
        result_set.returned_count,
        if result_set.approximate {
            format!("~{}", result_set.total_count)
        } else {
            result_set.total_count.to_string()
        },
        result_set.elapsed_ms as f64 / 1000.0
    ));
    lines.join("\n")
}

/// Format table with context lines for each result
fn format_table_with_context(
    data: &[(
        crate::core::entry::SearchResult,
        Vec<crate::core::entry::SearchResult>,
    )],
) -> String {
    use std::collections::HashSet;
    let mut output = String::new();
    let mut seen_ids: HashSet<i64> = data.iter().map(|(r, _)| r.id).collect();

    for (result, context) in data {
        // Print context lines before the result
        for ctx_line in context {
            if seen_ids.contains(&ctx_line.id) {
                continue;
            }
            seen_ids.insert(ctx_line.id);
            let ts = ctx_line.timestamp.as_deref().unwrap_or("-");
            let level = ctx_line.level.as_deref().unwrap_or("-");
            let source = shorten_path(&ctx_line.source);
            let msg = truncate(&ctx_line.message, 60);
            output.push_str(&format!(
                "  | {} {} [{}:{}] {}\n",
                ts, level, source, ctx_line.line_number, msg
            ));
        }
        // Print the main result (highlighted)
        let ts = result.timestamp.as_deref().unwrap_or("-");
        let level = result.level.as_deref().unwrap_or("-");
        let source = shorten_path(&result.source);
        let msg = truncate(&result.message, 80);
        output.push_str(&format!(
            "> {} {} [{}:{}] {}\n",
            ts, level, source, result.line_number, msg
        ));
    }

    output.push_str(&format!("\n{} results with context", data.len()));
    output
}

/// Format compact with context lines for each result
fn format_compact_with_context(
    data: &[(
        crate::core::entry::SearchResult,
        Vec<crate::core::entry::SearchResult>,
    )],
) -> String {
    use std::collections::HashSet;
    let mut lines: Vec<String> = Vec::new();
    let mut seen_ids: HashSet<i64> = data.iter().map(|(r, _)| r.id).collect();

    for (result, context) in data {
        for ctx_line in context {
            if seen_ids.contains(&ctx_line.id) {
                continue;
            }
            seen_ids.insert(ctx_line.id);
            let ts = ctx_line.timestamp.as_deref().unwrap_or("-");
            let level = ctx_line.level.as_deref().unwrap_or("-");
            let source = shorten_path(&ctx_line.source);
            let msg = collapse_multiline(&ctx_line.message);
            lines.push(format!(
                "  | [{}] [{}] [{}:{}] {}",
                ts, level, source, ctx_line.line_number, msg
            ));
        }
        let ts = result.timestamp.as_deref().unwrap_or("-");
        let level = result.level.as_deref().unwrap_or("-");
        let source = shorten_path(&result.source);
        let msg = collapse_multiline(&result.message);
        lines.push(format!(
            "> [{}] [{}] [{}:{}] {}",
            ts, level, source, result.line_number, msg
        ));
    }
    lines.push(format!("\n{} results with context", data.len()));
    lines.join("\n")
}

fn shorten_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() > 3 {
        parts[parts.len() - 3..].join("/")
    } else {
        path.to_string()
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        let end = s.floor_char_boundary(max_len - 3);
        format!("{}...", &s[..end])
    } else {
        s.to_string()
    }
}

/// Collapse multi-line content (stack traces, etc.) into a single line
fn collapse_multiline(s: &str) -> String {
    s.lines().collect::<Vec<_>>().join("\\n")
}

/// Apply max-chars truncation to output string (UTF-8 safe)
fn apply_max_chars(output: &str, max: usize, total_count: u64) -> String {
    if output.chars().count() > max {
        let truncated: String = output.chars().take(max).collect();
        format!(
            "{}\n... [已截断，共 {} 条匹配，显示前 {} 字符]",
            truncated, total_count, max
        )
    } else {
        output.to_string()
    }
}

/// Deduplicate results by level + message prefix (first 100 chars).
/// First occurrence is kept, duplicates are annotated with count.
fn deduplicate_results(
    results: Vec<crate::core::entry::SearchResult>,
) -> Vec<crate::core::entry::SearchResult> {
    use std::collections::HashMap;

    let mut seen: HashMap<(String, String), u64> = HashMap::new();
    let mut deduped: Vec<crate::core::entry::SearchResult> = Vec::new();

    for r in results {
        let level = r.level.clone().unwrap_or_default();
        let prefix: String = r.message.chars().take(100).collect();
        let key = (level, prefix);
        let count = seen.entry(key.clone()).or_insert(0);
        *count += 1;
        if *count == 1u64 {
            deduped.push(r);
        }
    }

    // Annotate with repeat counts
    for r in &mut deduped {
        let level = r.level.clone().unwrap_or_default();
        let prefix: String = r.message.chars().take(100).collect();
        if let Some(&count) = seen.get(&(level, prefix)) {
            if count > 1 {
                r.message.push_str(&format!(" (重复 {} 次)", count));
            }
        }
    }

    deduped
}

/// Format search summary for display
fn format_summary(summary: &SearchSummary, output: &OutputFormat) -> String {
    if let OutputFormat::Json = output {
        return serde_json::to_string_pretty(summary).unwrap_or_default();
    }

    let mut s = String::new();
    s.push_str("=== Search Summary ===\n");
    s.push_str(&format!(
        "Total: {} results ({:.1}ms)\n",
        summary.total_count,
        summary.elapsed_ms as f64 / 1000.0
    ));

    // Level distribution
    if !summary.level_distribution.is_empty() {
        let parts: Vec<String> = summary
            .level_distribution
            .iter()
            .map(|lc| format!("{}: {}", lc.level.to_lowercase(), lc.count))
            .collect();
        s.push_str(&format!(
            "\n{} | {} total\n",
            parts.join(" | "),
            summary.total_count
        ));
    }

    // Time range
    if let Some((ref min, ref max)) = summary.time_range {
        s.push_str(&format!("\nRange: {} ~ {}\n", min, max));
    }

    // Top sources
    if !summary.source_breakdown.is_empty() {
        s.push_str("\nTop sources:\n");
        for sc in summary.source_breakdown.iter().take(10) {
            let path = if sc.source.len() > 60 {
                format!("...{}", &sc.source[sc.source.len().saturating_sub(57)..])
            } else {
                sc.source.clone()
            };
            s.push_str(&format!("  {} : {}\n", path, sc.count));
        }
    }

    // Top messages
    if !summary.top_messages.is_empty() {
        s.push_str("\nTop messages:\n");
        for mc in summary.top_messages.iter().take(10) {
            let prefix = if mc.message_prefix.len() > 80 {
                let end = mc.message_prefix.floor_char_boundary(77);
                format!("{}...", &mc.message_prefix[..end])
            } else {
                mc.message_prefix.clone()
            };
            s.push_str(&format!("  [{}x] {}\n", mc.count, prefix));
        }
    }

    s
}

// --- Stack trace folding ---

/// Stack trace pattern detection — supports Java, Python, Go
fn is_stack_line(raw: &str) -> bool {
    // Don't trim! Leading whitespace is part of the pattern.
    // But we do check the trimmed version for non-whitespace-leading patterns.
    let trimmed = raw.trim_start();

    // Java patterns — detect leading whitespace + "at ", "Caused by", etc.
    // The raw line may start with tabs or spaces, then "at "
    if raw.contains("\tat ") || raw.contains("    at ") {
        return true;
    }
    if raw.contains("Caused by:") {
        return true;
    }
    if raw.contains("Suppressed:") {
        return true;
    }
    if raw.trim().ends_with("more") && raw.contains("...") {
        return true;
    }

    // Python traceback: "  File \"...\", line N"
    if raw.contains("File \"") {
        let t = raw.trim_start();
        if t.starts_with("File \"") {
            return true;
        }
    }
    // Python exception headers
    if trimmed == "Traceback (most recent call last):" {
        return true;
    }

    // Go patterns
    if raw.contains("goroutine ") && raw.contains("[running]") {
        return true;
    }
    if raw.contains("created by ") {
        return true;
    }

    false
}

/// Extract exception class name from a stack trace line
fn extract_exception_class(raw: &str) -> Option<String> {
    let trimmed = raw.trim();

    // Java: "Caused by: java.lang.NullPointerException: message"
    if let Some(rest) = trimmed.strip_prefix("Caused by:") {
        let class = rest.trim().split(':').next()?.trim();
        if !class.is_empty() {
            // Take just the last segment: "java.lang.NullPointerException" -> "NullPointerException"
            let short = class.rsplit('.').next().unwrap_or(class);
            return Some(short.to_string());
        }
    }

    // Java: top-level exception in message (e.g., "Exception in thread" line)
    if trimmed.contains("Exception") || trimmed.contains("Error") {
        for word in trimmed.split_whitespace() {
            if word.contains("Exception") || word.contains("Error") {
                let short = word.rsplit('.').next().unwrap_or(word);
                // Remove trailing punctuation
                let cleaned = short.trim_end_matches(':').trim_end_matches(',');
                if !cleaned.is_empty() {
                    return Some(cleaned.to_string());
                }
            }
        }
    }

    // Python: dedented exception line like "ValueError: something"
    if !trimmed.starts_with(' ') && !trimmed.starts_with('\t') {
        if let Some(pos) = trimmed.find(": ") {
            let class = &trimmed[..pos];
            // Check it looks like an exception (uppercase, contains Error/Exception/etc.)
            if class.chars().next().map_or(false, |c| c.is_uppercase())
                && (class.contains("Error")
                    || class.contains("Exception")
                    || class.contains("ValueError")
                    || class.contains("TypeError")
                    || class.contains("KeyError")
                    || class.contains("RuntimeError")
                    || class.contains("AttributeError")
                    || class.contains("ImportError")
                    || class.contains("IndexError")
                    || class.contains("IOError")
                    || class.contains("OSError")
                    || class.contains("StopIteration"))
            {
                return Some(class.to_string());
            }
        }
    }

    None
}

/// Fold consecutive stack trace lines into their parent result.
/// Removes stack lines from results and appends a summary to the parent's message.
fn fold_stack_traces(results: &mut Vec<crate::core::entry::SearchResult>) {
    if results.is_empty() {
        return;
    }

    let mut i = 1; // start from second result
    while i < results.len() {
        if is_stack_line(&results[i].raw) {
            // Find the parent (previous non-stack result)
            let parent_idx = i.saturating_sub(1);
            let mut stack_count = 0u32;
            let mut last_exception: Option<String> = None;

            // Count consecutive stack lines
            while i < results.len() && is_stack_line(&results[i].raw) {
                stack_count += 1;
                if let Some(cls) = extract_exception_class(&results[i].raw) {
                    last_exception = Some(cls);
                }
                i += 1;
            }

            // Append fold summary to parent's message
            if stack_count > 0 {
                let summary = match &last_exception {
                    Some(cls) => format!("\n  [{}: {} 条堆栈已折叠]", cls, stack_count),
                    None => format!("\n  [{} 条堆栈已折叠]", stack_count),
                };
                results[parent_idx].message.push_str(&summary);
                results[parent_idx].raw.push_str(&summary);
            }
        } else {
            i += 1;
        }
    }

    // Remove all stack-only results (iterate backwards for safe removal)
    let mut write_idx = 0;
    for read_idx in 0..results.len() {
        if !is_stack_line(&results[read_idx].raw) {
            if write_idx != read_idx {
                results[write_idx] = results[read_idx].clone();
            }
            write_idx += 1;
        }
    }
    results.truncate(write_idx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collapse_multiline() {
        let input = "line1\nline2\nline3";
        assert_eq!(collapse_multiline(input), "line1\\nline2\\nline3");
    }

    #[test]
    fn test_collapse_multiline_single() {
        let input = "single line";
        assert_eq!(collapse_multiline(input), "single line");
    }

    #[test]
    fn test_collapse_multiline_empty() {
        assert_eq!(collapse_multiline(""), "");
    }

    #[test]
    fn test_collapse_multiline_carriage_return() {
        let input = "line1\r\nline2\r\nline3";
        assert_eq!(collapse_multiline(input), "line1\\nline2\\nline3");
    }

    #[test]
    fn test_apply_max_chars_no_truncation() {
        let output = "short";
        assert_eq!(apply_max_chars(output, 100, 5), "short");
    }

    #[test]
    fn test_apply_max_chars_truncation() {
        let output = "hello world";
        let result = apply_max_chars(output, 5, 10);
        assert!(result.starts_with("hello"));
        assert!(result.contains("已截断"));
        assert!(result.contains("10"));
    }

    #[test]
    fn test_apply_max_chars_utf8_safe() {
        // CJK characters are multi-byte
        let output = "你好世界";
        let result = apply_max_chars(output, 2, 1);
        assert_eq!(result.chars().take(2).collect::<String>(), "你好");
        assert!(result.contains("已截断"));
    }

    // --- Stack trace folding tests ---

    #[test]
    fn test_is_stack_line_java() {
        // Tab + "at " pattern (tab is preserved via format! macro)
        let tab = '\t';
        let line = format!("{}at com.example.Method(Class.java:42)", tab);
        assert!(is_stack_line(&line));
        assert!(is_stack_line(
            "Caused by: java.lang.NullPointerException: msg"
        ));
        assert!(is_stack_line("Suppressed: java.io.IOException: err"));
    }

    #[test]
    fn test_is_stack_line_python() {
        assert!(is_stack_line("Traceback (most recent call last):"));
        // Python: "  File "...\"
        assert!(is_stack_line("  File \"main.py\", line 10"));
    }

    #[test]
    fn test_is_stack_line_go() {
        assert!(is_stack_line("goroutine 1 [running]"));
        assert!(is_stack_line("created by main.main"));
    }

    #[test]
    fn test_is_stack_line_not_stack() {
        assert!(!is_stack_line(
            "2024-01-15 10:23:45 INFO  normal log message"
        ));
        assert!(!is_stack_line("request processed successfully"));
        assert!(!is_stack_line(""));
    }

    #[test]
    fn test_extract_exception_class_java() {
        assert_eq!(
            extract_exception_class("Caused by: java.lang.NullPointerException: something"),
            Some("NullPointerException".to_string())
        );
    }

    #[test]
    fn test_extract_exception_class_python() {
        assert_eq!(
            extract_exception_class("ValueError: invalid value"),
            Some("ValueError".to_string())
        );
    }

    #[test]
    fn test_fold_stack_traces_java() {
        use crate::core::entry::SearchResult;
        let tab = '\t';
        let mut results = vec![
            SearchResult {
                file_id: 0,
                id: 1,
                source: "test.log".to_string(),
                line_number: 1,
                byte_offset: 0,
                timestamp: Some("2024-01-01T00:00:00".to_string()),
                level: Some("ERROR".to_string()),
                thread: None,
                logger: None,
                message: "NullPointerException: something went wrong".to_string(),

                raw: "Exception NullPointerException: something went wrong".to_string(),
            },
            SearchResult {
                file_id: 0,
                id: 2,
                source: "test.log".to_string(),
                line_number: 2,
                byte_offset: 100,
                timestamp: Some("2024-01-01T00:00:00".to_string()),
                level: Some("ERROR".to_string()),
                thread: None,
                logger: None,
                message: format!("{}at com.example.App.run(App.java:42)", tab),

                raw: format!("{}at com.example.App.run(App.java:42)", tab),
            },
            SearchResult {
                file_id: 0,
                id: 3,
                source: "test.log".to_string(),
                line_number: 3,
                byte_offset: 200,
                timestamp: Some("2024-01-01T00:00:00".to_string()),
                level: Some("ERROR".to_string()),
                thread: None,
                logger: None,
                message: format!("{}at com.example.App.main(App.java:10)", tab),

                raw: format!("{}at com.example.App.main(App.java:10)", tab),
            },
        ];
        fold_stack_traces(&mut results);
        assert_eq!(results.len(), 1);
        assert!(results[0].message.contains("2"));
    }

    #[test]
    fn test_fold_stack_traces_no_stack() {
        use crate::core::entry::SearchResult;
        let mut results = vec![
            SearchResult {
                file_id: 0,
                id: 1,
                source: "test.log".to_string(),
                line_number: 1,
                byte_offset: 0,
                timestamp: Some("2024-01-01T00:00:00".to_string()),
                level: Some("INFO".to_string()),
                thread: None,
                logger: None,
                message: "normal log".to_string(),

                raw: "2024-01-01 INFO normal log".to_string(),
            },
            SearchResult {
                file_id: 0,
                id: 2,
                source: "test.log".to_string(),
                line_number: 2,
                byte_offset: 100,
                timestamp: Some("2024-01-01T00:00:00".to_string()),
                level: Some("INFO".to_string()),
                thread: None,
                logger: None,
                message: "another normal log".to_string(),

                raw: "2024-01-01 INFO another normal log".to_string(),
            },
        ];
        fold_stack_traces(&mut results);
        assert_eq!(results.len(), 2);
    }

    // ── utility function tests ──

    #[test]
    fn test_shorten_path_short() {
        assert_eq!(shorten_path("a/b.log"), "a/b.log");
    }
    #[test]
    fn test_shorten_path_long() {
        let result = shorten_path("a/b/c/d/e/f.log");
        assert!(result.len() <= "c/d/e/f.log".len() + 5);
    }
    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hi", 10), "hi");
    }
    #[test]
    fn test_truncate_long() {
        assert_eq!(truncate("hello world this is a test", 12), "hello wor...");
    }
    #[test]
    fn test_truncate_utf8_multibyte() {
        // Chinese characters are 3 bytes each — truncating at byte boundary would panic
        let input = "你好世界hello world this is a very long string";
        let result = truncate(input, 10);
        assert!(!result.is_empty());
        assert!(result.ends_with("..."));
    }

    // ── deduplicate_results tests ──

    #[test]
    fn test_deduplicate_no_dupes() {
        use crate::core::entry::SearchResult;
        let results = vec![
            SearchResult {
                file_id: 0,
                id: 1,
                source: "a".into(),
                line_number: 1,
                byte_offset: 0,
                timestamp: None,
                level: Some("INFO".into()),
                thread: None,
                logger: None,
                message: "msg one".into(),
                raw: "raw1".into(),
            },
            SearchResult {
                file_id: 0,
                id: 2,
                source: "a".into(),
                line_number: 2,
                byte_offset: 10,
                timestamp: None,
                level: Some("WARN".into()),
                thread: None,
                logger: None,
                message: "msg two".into(),
                raw: "raw2".into(),
            },
        ];
        let deduped = deduplicate_results(results);
        assert_eq!(deduped.len(), 2);
    }
    #[test]
    fn test_deduplicate_with_dupes() {
        use crate::core::entry::SearchResult;
        let results = vec![
            SearchResult {
                file_id: 0,
                id: 1,
                source: "a".into(),
                line_number: 1,
                byte_offset: 0,
                timestamp: None,
                level: Some("ERROR".into()),
                thread: None,
                logger: None,
                message: "same error".into(),
                raw: "raw1".into(),
            },
            SearchResult {
                file_id: 0,
                id: 2,
                source: "a".into(),
                line_number: 2,
                byte_offset: 10,
                timestamp: None,
                level: Some("ERROR".into()),
                thread: None,
                logger: None,
                message: "same error".into(),
                raw: "raw2".into(),
            },
            SearchResult {
                file_id: 0,
                id: 3,
                source: "a".into(),
                line_number: 3,
                byte_offset: 20,
                timestamp: None,
                level: Some("ERROR".into()),
                thread: None,
                logger: None,
                message: "same error".into(),
                raw: "raw3".into(),
            },
        ];
        let deduped = deduplicate_results(results);
        assert_eq!(deduped.len(), 1);
        assert!(deduped[0].message.contains("重复 3 次"));
    }
    #[test]
    fn test_deduplicate_different_levels() {
        use crate::core::entry::SearchResult;
        let results = vec![
            SearchResult {
                file_id: 0,
                id: 1,
                source: "a".into(),
                line_number: 1,
                byte_offset: 0,
                timestamp: None,
                level: Some("INFO".into()),
                thread: None,
                logger: None,
                message: "same msg".into(),
                raw: "raw1".into(),
            },
            SearchResult {
                file_id: 0,
                id: 2,
                source: "a".into(),
                line_number: 2,
                byte_offset: 10,
                timestamp: None,
                level: Some("ERROR".into()),
                thread: None,
                logger: None,
                message: "same msg".into(),
                raw: "raw2".into(),
            },
        ];
        let deduped = deduplicate_results(results);
        // Different levels → not duplicates
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_apply_max_chars_zero() {
        let result = apply_max_chars("hello", 0, 5);
        assert!(result.contains("已截断"));
    }
}
