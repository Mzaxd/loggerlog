use crate::core::config::DirectorySource;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Discovered log file information
#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub size: u64,
    pub group_name: String, // base name without rotation suffix
    pub is_compressed: bool,
    pub is_rotated: bool,
}

/// Discover log files from configured directories
pub struct FileDiscovery;

impl FileDiscovery {
    /// Scan all configured directories and return discovered files
    pub fn scan_directories(directories: &[DirectorySource]) -> Vec<DiscoveredFile> {
        let mut all_files = Vec::new();

        for dir in directories {
            let path = Path::new(&dir.path);
            if !path.exists() {
                eprintln!("Warning: directory does not exist: {}", dir.path);
                continue;
            }

            let files = Self::scan_directory(path, dir.recursive, &dir.exclude_patterns);
            all_files.extend(files);
        }

        all_files
    }

    /// Scan a single directory for log files
    pub fn scan_directory(
        dir: &Path,
        recursive: bool,
        exclude_patterns: &[String],
    ) -> Vec<DiscoveredFile> {
        let mut files = Vec::new();

        // log file extensions to match

        let walker = if recursive {
            walkdir::WalkDir::new(dir).into_iter()
        } else {
            walkdir::WalkDir::new(dir).max_depth(1).into_iter()
        };

        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            // Check extension
            let ext = path.extension().and_then(|e| e.to_str());
            let has_log_ext = ext.map_or(false, |e| ["log", "txt", "out", "app"].contains(&e));

            // Also accept files without extension that look like logs
            let no_ext = path.extension().is_none();

            if !has_log_ext && !no_ext {
                continue;
            }

            // Check exclude patterns
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            let mut excluded = false;
            for pattern in exclude_patterns {
                if Self::matches_glob(&file_name, pattern) {
                    excluded = true;
                    break;
                }
            }
            if excluded {
                continue;
            }

            let metadata = std::fs::metadata(path).ok();
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

            let (group_name, is_compressed, is_rotated) = Self::analyze_filename(&file_name);

            files.push(DiscoveredFile {
                path: path.to_path_buf(),
                size,
                group_name,
                is_compressed,
                is_rotated,
            });
        }

        files.sort_by(|a, b| a.path.cmp(&b.path));
        files
    }

    /// Analyze a filename to determine its group, compression status, and rotation status
    fn analyze_filename(filename: &str) -> (String, bool, bool) {
        let mut is_compressed = false;
        let mut is_rotated = false;

        // Check compression
        if filename.ends_with(".gz") || filename.ends_with(".zip") || filename.ends_with(".bz2") {
            is_compressed = true;
        }

        // Check rotation: pattern like app.log.1, app.log.2024-01-01, app.log.2024-01-01.gz
        let mut stripped = filename;

        // Remove compression extension
        for ext in [".gz", ".zip", ".bz2"] {
            if stripped.ends_with(ext) {
                stripped = &stripped[..stripped.len() - ext.len()];
            }
        }

        // Check rotation by looking at dots from right to left
        // e.g., "app.log.1" -> suffix "1" is a number -> rotated, group = "app.log"
        // e.g., "app.log.2024-01-01" -> suffix "2024-01-01" is a date -> rotated
        // We need at least 2 dots for this to be a rotated log (e.g., "app.log.1")
        let group_name = if let Some(second_last_dot) = stripped.rfind('.') {
            // Check suffix after last dot
            let suffix = &stripped[second_last_dot + 1..];
            let is_number = suffix.parse::<u32>().is_ok();
            let is_date = chrono::NaiveDate::parse_from_str(suffix, "%Y-%m-%d").is_ok()
                || chrono::NaiveDate::parse_from_str(suffix, "%Y-%m-%d-%H").is_ok();

            if is_number || is_date {
                is_rotated = true;
                stripped[..second_last_dot].to_string()
            } else {
                stripped.to_string()
            }
        } else {
            stripped.to_string()
        };

        (group_name, is_compressed, is_rotated)
    }

    /// Simple glob matching for `*.ext` style patterns.
    /// Only supports the `*` prefix wildcard (e.g. `*.gz`, `*.tmp`).
    fn matches_glob(text: &str, pattern: &str) -> bool {
        if let Some(suffix) = pattern.strip_prefix("*.") {
            text.ends_with(suffix)
        } else if let Some(suffix) = pattern.strip_prefix('*') {
            text.ends_with(suffix)
        } else {
            text == pattern
        }
    }

    /// Group files by their logical source (rotation group)
    pub fn group_files(files: &[DiscoveredFile]) -> BTreeMap<String, Vec<&DiscoveredFile>> {
        let mut groups: BTreeMap<String, Vec<&DiscoveredFile>> = BTreeMap::new();

        for file in files {
            groups
                .entry(file.group_name.clone())
                .or_default()
                .push(file);
        }

        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    #[test]
    fn test_analyze_filename() {
        let (group, compressed, rotated) = FileDiscovery::analyze_filename("app.log");
        assert_eq!(group, "app.log");
        assert!(!compressed);
        assert!(!rotated);

        let (group, compressed, rotated) = FileDiscovery::analyze_filename("app.log.1");
        assert_eq!(group, "app.log");
        assert!(!compressed);
        assert!(rotated);

        let (group, compressed, rotated) = FileDiscovery::analyze_filename("app.log.2024-01-15.gz");
        assert_eq!(group, "app.log");
        assert!(compressed);
        assert!(rotated);
    }

    // --- analyze_filename additional cases ---

    #[test]
    fn test_analyze_filename_no_ext() {
        let (group, compressed, rotated) = FileDiscovery::analyze_filename("syslog");
        assert_eq!(group, "syslog");
        assert!(!compressed);
        assert!(!rotated);
    }

    #[test]
    fn test_analyze_filename_date_rotation() {
        let (group, compressed, rotated) = FileDiscovery::analyze_filename("app.log.2024-01-15");
        assert_eq!(group, "app.log");
        assert!(!compressed);
        assert!(rotated);
    }

    #[test]
    fn test_analyze_filename_date_hour() {
        // chrono::NaiveDate::parse_from_str is lenient: "%Y-%m-%d-%H" successfully parses
        // "2024-01-15-08" by consuming just the date portion, so it IS detected as rotated.
        let (group, compressed, rotated) = FileDiscovery::analyze_filename("app.log.2024-01-15-08");
        assert_eq!(group, "app.log");
        assert!(!compressed);
        assert!(rotated);
    }

    #[test]
    fn test_analyze_filename_zip() {
        let (group, compressed, rotated) = FileDiscovery::analyze_filename("app.log.1.zip");
        assert_eq!(group, "app.log");
        assert!(compressed);
        assert!(rotated);
    }

    #[test]
    fn test_analyze_filename_bz2() {
        let (group, compressed, rotated) =
            FileDiscovery::analyze_filename("app.log.2024-01-01.bz2");
        assert_eq!(group, "app.log");
        assert!(compressed);
        assert!(rotated);
    }

    #[test]
    fn test_analyze_filename_single_dot() {
        // "app.log" has one dot; suffix "log" is neither a number nor a date.
        let (group, compressed, rotated) = FileDiscovery::analyze_filename("app.log");
        assert_eq!(group, "app.log");
        assert!(!compressed);
        assert!(!rotated);
    }

    #[test]
    fn test_analyze_filename_not_number() {
        let (group, compressed, rotated) = FileDiscovery::analyze_filename("app.log.backup");
        assert_eq!(group, "app.log.backup");
        assert!(!compressed);
        assert!(!rotated);
    }

    #[test]
    fn test_analyze_filename_no_ext_number() {
        let (group, compressed, rotated) = FileDiscovery::analyze_filename("server.out.10");
        assert_eq!(group, "server.out");
        assert!(!compressed);
        assert!(rotated);
    }

    // --- matches_glob ---

    #[test]
    fn test_matches_glob_star_ext() {
        assert!(FileDiscovery::matches_glob("file.gz", "*.gz"));
        assert!(FileDiscovery::matches_glob("archive.tar.gz", "*.gz"));
        assert!(!FileDiscovery::matches_glob("file.txt", "*.gz"));
    }

    #[test]
    fn test_matches_glob_star_only() {
        // pattern "*" → strip_prefix('*') → Some("") → ends_with("") → always true
        assert!(FileDiscovery::matches_glob("anything", "*"));
        assert!(FileDiscovery::matches_glob("", "*"));
    }

    #[test]
    fn test_matches_glob_exact() {
        assert!(FileDiscovery::matches_glob("file.gz", "file.gz"));
        assert!(!FileDiscovery::matches_glob("file.tar.gz", "file.gz"));
    }

    #[test]
    fn test_matches_glob_no_match() {
        assert!(!FileDiscovery::matches_glob("file.txt", "*.gz"));
        assert!(!FileDiscovery::matches_glob("notes.md", "*.gz"));
    }

    // --- group_files ---

    #[test]
    fn test_group_files_basic() {
        let files = vec![
            DiscoveredFile {
                path: PathBuf::from("/a/app.log"),
                size: 100,
                group_name: "app.log".to_string(),
                is_compressed: false,
                is_rotated: false,
            },
            DiscoveredFile {
                path: PathBuf::from("/a/app.log.1"),
                size: 200,
                group_name: "app.log".to_string(),
                is_compressed: false,
                is_rotated: true,
            },
            DiscoveredFile {
                path: PathBuf::from("/a/app.log.2.gz"),
                size: 50,
                group_name: "app.log".to_string(),
                is_compressed: true,
                is_rotated: true,
            },
        ];
        let groups = FileDiscovery::group_files(&files);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups["app.log"].len(), 3);
    }

    #[test]
    fn test_group_files_empty() {
        let files: Vec<DiscoveredFile> = vec![];
        let groups = FileDiscovery::group_files(&files);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_group_files_sorted() {
        let make = |path: &str, group: &str| DiscoveredFile {
            path: PathBuf::from(path),
            size: 0,
            group_name: group.to_string(),
            is_compressed: false,
            is_rotated: false,
        };
        let files = vec![
            make("/a/syslog", "syslog"),
            make("/a/auth.log", "auth.log"),
            make("/a/app.log", "app.log"),
        ];
        let groups = FileDiscovery::group_files(&files);
        let keys: Vec<&String> = groups.keys().collect();
        assert_eq!(keys, vec!["app.log", "auth.log", "syslog"]);
    }

    // --- scan_directory ---

    #[test]
    fn test_scan_directory_recursive() {
        let tmp = std::env::temp_dir().join("loggerlog_test_scan_recursive");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("sub/deep")).unwrap();
        File::create(tmp.join("root.log")).unwrap();
        File::create(tmp.join("sub/nested.log")).unwrap();
        File::create(tmp.join("sub/deep/inner.txt")).unwrap();

        let files = FileDiscovery::scan_directory(&tmp, true, &[]);
        assert_eq!(files.len(), 3);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_scan_directory_exclude() {
        let tmp = std::env::temp_dir().join("loggerlog_test_scan_exclude");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        File::create(tmp.join("app.log")).unwrap();
        File::create(tmp.join("debug.log.gz")).unwrap();
        File::create(tmp.join("archive.txt")).unwrap();

        // Exclude .gz files (but .gz is a compression ext, not a log ext —
        // scan_directory only accepts log/out/txt/app/no-ext, so .gz is already
        // filtered out. Let's use a meaningful exclusion.)
        let files = FileDiscovery::scan_directory(&tmp, false, &["*.txt".to_string()]);
        // app.log passes, debug.log.gz is excluded by extension filter, archive.txt excluded by pattern
        assert_eq!(files.len(), 1);
        assert!(files[0]
            .path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("app.log"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_scan_directory_nonexistent() {
        let path = PathBuf::from("/tmp/loggerlog_nonexistent_dir_12345");
        let files = FileDiscovery::scan_directory(&path, true, &[]);
        assert!(files.is_empty());
    }
}
