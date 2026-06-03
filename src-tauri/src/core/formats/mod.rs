pub mod json_log;
pub mod log4j;
pub mod plain;

use crate::core::entry::LogEntry;

/// Supported log format types
#[derive(Debug, Clone, PartialEq)]
pub enum LogFormat {
    Log4j,
    Logback,
    JsonStructured,
    PlainText,
}

impl std::fmt::Display for LogFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogFormat::Log4j => write!(f, "log4j"),
            LogFormat::Logback => write!(f, "logback"),
            LogFormat::JsonStructured => write!(f, "json"),
            LogFormat::PlainText => write!(f, "plain"),
        }
    }
}

/// Trait for log format parsers
pub trait LogParser {
    /// Try to parse a single line. Returns Some(entry) if successful, None if this format doesn't match.
    fn try_parse(&self, line: &str, line_number: u64, byte_offset: u64, file_id: i64) -> Option<LogEntry>;

    /// Returns the format name
    fn format_name(&self) -> &str;
}

/// Auto-detect the log format by sampling lines
pub fn detect_format(lines: &[String]) -> LogFormat {
    if lines.is_empty() {
        return LogFormat::PlainText;
    }

    let sample_count = lines.len().min(50);
    let sample = &lines[..sample_count];

    // Count how many lines each parser can handle
    let json_parser = json_log::JsonParser;
    let log4j_parser = log4j::Log4jParser;
    let logback_parser = log4j::LogbackParser;

    let mut json_count = 0;
    let mut log4j_count = 0;
    let mut logback_count = 0;

    for line in sample {
        if !line.trim().is_empty() {
            if json_parser.try_parse(line, 0, 0, 0).is_some() {
                json_count += 1;
            }
            if logback_parser.try_parse(line, 0, 0, 0).is_some() {
                logback_count += 1;
            }
            if log4j_parser.try_parse(line, 0, 0, 0).is_some() {
                log4j_count += 1;
            }
        }
    }

    let threshold = (sample_count as f64 * 0.6) as usize;

    if json_count >= threshold {
        return LogFormat::JsonStructured;
    }
    if logback_count >= threshold {
        return LogFormat::Logback;
    }
    if log4j_count >= threshold {
        return LogFormat::Log4j;
    }

    // If no format reaches threshold, pick the best match
    let best = json_count.max(logback_count).max(log4j_count);
    if best == 0 {
        return LogFormat::PlainText;
    }

    if json_count == best {
        LogFormat::JsonStructured
    } else if logback_count == best {
        LogFormat::Logback
    } else {
        LogFormat::Log4j
    }
}
