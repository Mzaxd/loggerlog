use encoding_rs::Encoding;
use std::fs::File;
use std::io::{BufReader, Read};

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
