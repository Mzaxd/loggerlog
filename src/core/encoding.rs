use encoding_rs::Encoding;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};

/// Detect the encoding of a file by sampling its first bytes
pub fn detect_encoding(file_path: &str) -> &'static Encoding {
    let file = match File::open(file_path) {
        Ok(f) => f,
        Err(_) => return encoding_rs::UTF_8,
    };

    let mut reader = BufReader::new(file);
    let mut sample = vec![0u8; 4096];
    let bytes_read = match reader.read(&mut sample) {
        Ok(n) => n,
        Err(_) => return encoding_rs::UTF_8,
    };

    if bytes_read == 0 {
        return encoding_rs::UTF_8;
    }
    sample.truncate(bytes_read);

    // Use chardetng to detect encoding
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(&sample, true);
    let encoding = detector.guess(None, true);

    encoding
}

/// Read a file with detected or specified encoding, returning UTF-8 string
pub fn read_file_to_utf8(file_path: &str, encoding_override: Option<&str>) -> String {
    // Try UTF-8 first
    if let Ok(content) = std::fs::read_to_string(file_path) {
        // Validate that it's actually valid UTF-8 by checking for replacement chars
        if !content.contains('\u{FFFD}') || content.chars().count() > 100 && content.matches('\u{FFFD}').count() < 3 {
            return content;
        }
    }

    // Use specified encoding or detect
    let encoding = match encoding_override {
        Some("gbk") | Some("gb2312") | Some("gb18030") => encoding_rs::GBK,
        Some("utf-8") => encoding_rs::UTF_8,
        Some("shift_jis") | Some("shift-jis") => encoding_rs::SHIFT_JIS,
        Some("euc-kr") | Some("euckr") => encoding_rs::EUC_KR,
        _ => detect_encoding(file_path),
    };

    let file_bytes = match std::fs::read(file_path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };

    let (decoded, _used_decoder, had_errors) = encoding.decode(&file_bytes);
    if had_errors {
        // Log warning but still return decoded string
        eprintln!("Warning: encoding errors detected in {}", file_path);
    }
    decoded.into_owned()
}

/// Read a file line-by-line with encoding detection
pub fn read_lines(file_path: &str, encoding_override: Option<&str>) -> Vec<String> {
    let content = read_file_to_utf8(file_path, encoding_override);
    content.lines().map(|l| l.to_string()).collect()
}

/// Read a gzip-compressed file and return decompressed UTF-8 content.
/// Returns the entire decompressed content — compressed files cannot be
/// incrementally indexed.
pub fn read_gz_to_utf8(file_path: &str) -> anyhow::Result<String> {
    use std::io::Read;

    let file = std::fs::File::open(file_path)
        .map_err(|e| anyhow::anyhow!("Failed to open gzip file {}: {}", file_path, e))?;

    let mut decoder = flate2::read::GzDecoder::new(file);
    let mut bytes = Vec::new();
    decoder
        .read_to_end(&mut bytes)
        .map_err(|e| anyhow::anyhow!("Failed to decompress {}: {}", file_path, e))?;

    // Detect encoding on the decompressed bytes
    let encoding = {
        let mut detector = chardetng::EncodingDetector::new();
        let sample = &bytes[..bytes.len().min(4096)];
        detector.feed(sample, true);
        detector.guess(None, true)
    };

    let (decoded, _used_decoder, had_errors) = encoding.decode(&bytes);
    if had_errors {
        eprintln!("Warning: encoding errors detected in {}", file_path);
    }
    Ok(decoded.into_owned())
}

/// Read file content starting from a byte offset (for incremental indexing).
/// Seeks to `byte_offset` and reads only the remaining bytes.
pub fn read_file_from_offset(file_path: &str, byte_offset: u64) -> anyhow::Result<String> {
    let mut file = File::open(file_path)
        .map_err(|e| anyhow::anyhow!("Failed to open {}: {}", file_path, e))?;
    file.seek(SeekFrom::Start(byte_offset))
        .map_err(|e| anyhow::anyhow!("Failed to seek in {}: {}", file_path, e))?;
    let mut content = String::new();
    BufReader::new(file).read_to_string(&mut content)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", file_path, e))?;
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Generate a unique temp file path with the given suffix.
    fn tmp_path(suffix: &str) -> String {
        format!("/tmp/loggerlog_test_{}_{}", std::process::id(), suffix)
    }

    // ── read_file_to_utf8 ──────────────────────────────────────────────

    #[test]
    fn test_read_utf8_file() {
        let path = tmp_path("utf8_file");
        let content = "hello world\nline two\nline three";
        std::fs::write(&path, content).unwrap();
        let result = read_file_to_utf8(&path, None);
        assert_eq!(result, content);
    }

    #[test]
    fn test_read_utf8_with_bom() {
        let path = tmp_path("utf8_bom");
        let payload = "BOM test content";
        let mut bytes = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        bytes.extend_from_slice(payload.as_bytes());
        std::fs::write(&path, &bytes).unwrap();
        let result = read_file_to_utf8(&path, None);
        // The BOM bytes decode as U+FEFF (ZERO WIDTH NO-BREAK SPACE)
        assert!(result.starts_with('\u{FEFD}') || result.starts_with('\u{FEFF}') || result.contains(payload));
        // Key assertion: the readable payload is preserved
        assert!(result.contains("BOM test content"));
    }

    #[test]
    fn test_read_nonexistent_file() {
        let path = tmp_path("nonexistent");
        let result = read_file_to_utf8(&path, None);
        assert_eq!(result, "");
    }

    #[test]
    fn test_read_empty_file() {
        let path = tmp_path("empty_file");
        std::fs::write(&path, "").unwrap();
        let result = read_file_to_utf8(&path, None);
        assert_eq!(result, "");
    }

    // ── read_lines ────────────────────────────────────────────────────

    #[test]
    fn test_read_lines_basic() {
        let path = tmp_path("lines_basic");
        std::fs::write(&path, "first\nsecond\nthird\n").unwrap();
        let lines = read_lines(&path, None);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "first");
        assert_eq!(lines[1], "second");
        assert_eq!(lines[2], "third");
    }

    #[test]
    fn test_read_lines_empty_file() {
        let path = tmp_path("lines_empty");
        std::fs::write(&path, "").unwrap();
        let lines = read_lines(&path, None);
        assert!(lines.is_empty());
    }

    // ── read_gz_to_utf8 ──────────────────────────────────────────────

    #[test]
    fn test_read_gz_valid() {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let path = tmp_path("valid.gz");
        let content = b"hello world gzip test";
        let file = std::fs::File::create(&path).unwrap();
        let mut encoder = GzEncoder::new(file, Compression::fast());
        encoder.write_all(content).unwrap();
        encoder.finish().unwrap();

        let result = read_gz_to_utf8(&path).unwrap();
        assert_eq!(result, "hello world gzip test");
    }

    #[test]
    fn test_read_gz_not_gz() {
        let path = tmp_path("not_really.gz");
        std::fs::write(&path, "this is not gzip data").unwrap();

        let result = read_gz_to_utf8(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_gz_nonexistent() {
        let path = tmp_path("nonexistent.gz");
        let result = read_gz_to_utf8(&path);
        assert!(result.is_err());
    }

    // ── read_file_from_offset ─────────────────────────────────────────

    #[test]
    fn test_read_from_offset_zero() {
        let path = tmp_path("offset_zero");
        let content = "hello\nworld\n";
        std::fs::write(&path, content).unwrap();

        let result = read_file_from_offset(&path, 0).unwrap();
        assert_eq!(result, content);
    }

    #[test]
    fn test_read_from_offset_middle() {
        let path = tmp_path("offset_middle");
        let content = "hello\nworld\n";
        std::fs::write(&path, content).unwrap();
        // "hello\n" is 6 bytes, so offset 6 should give "world\n"
        let result = read_file_from_offset(&path, 6).unwrap();
        assert_eq!(result, "world\n");
    }

    #[test]
    fn test_read_from_offset_past_end() {
        let path = tmp_path("offset_past_end");
        std::fs::write(&path, "hi").unwrap();
        let result = read_file_from_offset(&path, 100).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_read_from_offset_nonexistent() {
        let path = tmp_path("nonexistent_offset");
        let result = read_file_from_offset(&path, 0);
        assert!(result.is_err());
    }
}
