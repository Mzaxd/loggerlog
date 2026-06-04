use crate::core::config::DirectorySource;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Discovered log file information
#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub size: u64,
    pub group_name: String,  // base name without rotation suffix
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
            let file_name = path.file_name()
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
            // Find the last dot (which should be after second_last_dot)
            let after_first = &stripped[..second_last_dot];
            // Count dots before second_last_dot
            let dot_count = after_first.chars().filter(|&c| c == '.').count();

            // If there are no more dots before this one (e.g., "app.log.1"), check suffix
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

    /// Simple glob matching
    fn matches_glob(text: &str, pattern: &str) -> bool {
        let p = pattern.trim_start_matches('*').trim_start_matches('.');
        text.contains(p)
    }

    /// Group files by their logical source (rotation group)
    pub fn group_files(files: &[DiscoveredFile]) -> BTreeMap<String, Vec<&DiscoveredFile>> {
        let mut groups: BTreeMap<String, Vec<&DiscoveredFile>> = BTreeMap::new();

        for file in files {
            groups.entry(file.group_name.clone())
                .or_default()
                .push(file);
        }

        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
