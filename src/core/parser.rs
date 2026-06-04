use crate::core::entry::LogEntry;
use crate::core::formats;
use crate::core::formats::{LogFormat, LogParser};

/// Parse log lines using auto-detected or specified format
pub struct LogLineParser {
    pub format: LogFormat,
}

impl LogLineParser {
    pub fn new(format: LogFormat) -> Self {
        Self { format }
    }

    /// Auto-detect format from a sample of lines and create parser
    pub fn auto_detect(lines: &[String]) -> Self {
        let format = formats::detect_format(lines);
        Self::new(format)
    }

    /// Parse a single line
    pub fn parse_line(&self, line: &str, line_number: u64, byte_offset: u64, file_id: i64) -> LogEntry {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return LogEntry {
                id: None,
                file_id,
                line_number,
                byte_offset,
                timestamp: None,
                level: None,
                thread: None,
                logger: None,
                message: String::new(),
                fields_json: None,
                raw: line.to_string(),
            };
        }

        // Try format-specific parser first
        let entry = match self.format {
            LogFormat::Log4j => formats::log4j::Log4jParser.try_parse(line, line_number, byte_offset, file_id),
            LogFormat::Logback => formats::log4j::LogbackParser.try_parse(line, line_number, byte_offset, file_id),
            LogFormat::JsonStructured => formats::json_log::JsonParser.try_parse(line, line_number, byte_offset, file_id),
            LogFormat::PlainText => None, // plain parser always succeeds, handled below
        };

        entry.unwrap_or_else(|| {
            // Fallback to plain parser
            formats::plain::PlainParser.try_parse(line, line_number, byte_offset, file_id).unwrap()
        })
    }

    /// Parse all lines from a string content
    pub fn parse_lines(&self, content: &str, file_id: i64, start_byte_offset: u64) -> Vec<LogEntry> {
        let mut entries = Vec::new();
        let mut byte_offset = start_byte_offset;
        let mut line_number = 0u64;

        for line in content.lines() {
            line_number += 1;
            let line_bytes = line.len() as u64 + 1; // +1 for newline
            let entry = self.parse_line(line, line_number, byte_offset, file_id);
            byte_offset += line_bytes;
            entries.push(entry);
        }

        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_detect_log4j() {
        let lines = vec![
            "2024-01-15 10:23:45,123 INFO  [main] c.m.App - Started".to_string(),
            "2024-01-15 10:23:46,456 ERROR [http-1] c.m.Svc - Failed".to_string(),
        ];
        let parser = LogLineParser::auto_detect(&lines);
        assert_eq!(parser.format, LogFormat::Log4j);
    }

    #[test]
    fn test_auto_detect_json() {
        let lines = vec![
            r#"{"level":"INFO","message":"Started"}"#.to_string(),
            r#"{"level":"ERROR","message":"Failed"}"#.to_string(),
        ];
        let parser = LogLineParser::auto_detect(&lines);
        assert_eq!(parser.format, LogFormat::JsonStructured);
    }

    #[test]
    fn test_auto_detect_plain() {
        let lines = vec![
            "Just some random text".to_string(),
            "Another random line".to_string(),
        ];
        let parser = LogLineParser::auto_detect(&lines);
        assert_eq!(parser.format, LogFormat::PlainText);
    }

    #[test]
    fn test_parse_line() {
        let parser = LogLineParser::new(LogFormat::Log4j);
        let entry = parser.parse_line(
            "2024-01-15 10:23:45,123 INFO  [main] c.m.App - Hello",
            1, 0, 1,
        );
        assert_eq!(entry.level.as_deref(), Some("INFO"));
        assert_eq!(entry.message, "Hello");
    }
}
