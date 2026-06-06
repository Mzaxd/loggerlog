use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Root configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub sources: SourcesConfig,
    #[serde(default)]
    pub projects: ProjectsConfig,
    pub search: SearchConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_database_path")]
    pub database_path: String,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: String,
    #[serde(default = "default_max_index_size")]
    pub max_index_size: String,
    #[serde(default = "default_watch_interval")]
    pub watch_interval: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcesConfig {
    #[serde(default)]
    pub directories: Vec<DirectorySource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectorySource {
    pub path: String,
    #[serde(default = "default_true")]
    pub recursive: bool,
    #[serde(default = "default_formats")]
    pub formats: Vec<String>,
    #[serde(default = "default_auto")]
    pub encoding: String,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
}

/// Project configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectsConfig {
    #[serde(default)]
    pub projects: Vec<Project>,
}

/// A named project with a root log directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Unique project name
    pub name: String,
    /// Root directory path for this project's logs
    pub path: String,
    #[serde(default = "default_true")]
    pub recursive: bool,
    #[serde(default = "default_formats")]
    pub formats: Vec<String>,
    #[serde(default = "default_auto")]
    pub encoding: String,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    #[serde(default = "default_limit")]
    pub default_limit: u32,
    #[serde(default = "default_max_limit")]
    pub max_limit: u32,
}

fn default_database_path() -> String {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("loggerlog")
        .join("index.db");
    data_dir.to_string_lossy().to_string()
}

fn default_max_file_size() -> String {
    "2GB".to_string()
}

fn default_max_index_size() -> String {
    "5GB".to_string()
}

fn default_watch_interval() -> String {
    "3s".to_string()
}

fn default_true() -> bool {
    true
}

fn default_auto() -> String {
    "auto".to_string()
}

fn default_formats() -> Vec<String> {
    vec!["auto".to_string()]
}

fn default_limit() -> u32 {
    100
}

fn default_max_limit() -> u32 {
    10000
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                database_path: default_database_path(),
                max_file_size: default_max_file_size(),
                max_index_size: default_max_index_size(),
                watch_interval: default_watch_interval(),
            },
            sources: SourcesConfig {
                directories: Vec::new(),
            },
            projects: ProjectsConfig {
                projects: Vec::new(),
            },
            search: SearchConfig {
                default_limit: default_limit(),
                max_limit: default_max_limit(),
            },
        }
    }
}

/// Get the config file path
pub fn config_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("LoggerLog");
    fs::create_dir_all(&config_dir).ok();
    config_dir.join("config.toml")
}

/// Load configuration from file, or return default
pub fn load(override_path: Option<&str>) -> Result<Config> {
    let path = match override_path {
        Some(p) => PathBuf::from(p),
        None => config_path(),
    };

    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    } else {
        // Save default config
        let config = Config::default();
        save(&config, None)?;
        Ok(config)
    }
}

/// Save configuration to file
pub fn save(config: &Config, override_path: Option<&str>) -> Result<()> {
    let path = match override_path {
        Some(p) => PathBuf::from(p),
        None => config_path(),
    };
    fs::create_dir_all(path.parent().unwrap())?;
    let content = toml::to_string_pretty(config)?;
    fs::write(&path, content)?;
    Ok(())
}

/// Add a directory source to the config
pub fn add_directory(config: &mut Config, path: &str, recursive: bool, encoding: &str) -> bool {
    // Check if already exists
    if config.sources.directories.iter().any(|d| d.path == path) {
        return false;
    }
    config.sources.directories.push(DirectorySource {
        path: path.to_string(),
        recursive,
        formats: vec!["auto".to_string()],
        encoding: encoding.to_string(),
        exclude_patterns: vec!["*.gz".to_string(), "*.zip".to_string(), "*.tmp".to_string()],
    });
    true
}

/// Remove a directory source from the config
pub fn remove_directory(config: &mut Config, path: &str) -> bool {
    let len_before = config.sources.directories.len();
    config.sources.directories.retain(|d| d.path != path);
    config.sources.directories.len() != len_before
}

/// Parse a human-readable size string like "2GB" to bytes.
/// Returns `None` if the input cannot be parsed or would overflow.
pub fn parse_size(size_str: &str) -> Option<u64> {
    let s = size_str.trim().to_uppercase();
    let (num_str, multiplier) = if let Some(n) = s.strip_suffix("GB") {
        (n, 1024u64 * 1024 * 1024)
    } else if let Some(n) = s.strip_suffix("MB") {
        (n, 1024u64 * 1024)
    } else if let Some(n) = s.strip_suffix("KB") {
        (n, 1024u64)
    } else if let Some(n) = s.strip_suffix("B") {
        (n, 1u64)
    } else {
        (s.as_str(), 1u64)
    };
    num_str.trim().parse::<u64>().ok()?.checked_mul(multiplier)
}

/// Add a project to the config
pub fn add_project(config: &mut Config, name: &str, path: &str) -> bool {
    // Check if name or path already exists
    if config.projects.projects.iter().any(|p| p.name == name || p.path == path) {
        return false;
    }
    config.projects.projects.push(Project {
        name: name.to_string(),
        path: path.to_string(),
        recursive: true,
        formats: vec!["auto".to_string()],
        encoding: "auto".to_string(),
        exclude_patterns: vec!["*.gz".to_string(), "*.zip".to_string(), "*.tmp".to_string()],
    });
    true
}

/// Remove a project from the config by name
pub fn remove_project(config: &mut Config, name: &str) -> bool {
    let len_before = config.projects.projects.len();
    config.projects.projects.retain(|p| p.name != name);
    config.projects.projects.len() != len_before
}

/// Get a project by name
pub fn get_project_by_name<'a>(config: &'a Config, name: &str) -> Option<&'a Project> {
    config.projects.projects.iter().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------------
    // parse_size
    // ---------------------------------------------------------------------------

    #[test]
    fn test_parse_size_bytes() {
        assert_eq!(parse_size("500B"), Some(500));
    }

    #[test]
    fn test_parse_size_kb() {
        assert_eq!(parse_size("1024KB"), Some(1_048_576));
    }

    #[test]
    fn test_parse_size_mb() {
        assert_eq!(parse_size("2MB"), Some(2_097_152));
    }

    #[test]
    fn test_parse_size_gb() {
        assert_eq!(parse_size("1GB"), Some(1_073_741_824));
    }

    #[test]
    fn test_parse_size_overflow() {
        assert_eq!(parse_size("99999999999999999GB"), None);
    }

    #[test]
    fn test_parse_size_negative() {
        assert_eq!(parse_size("-5MB"), None);
    }

    #[test]
    fn test_parse_size_invalid() {
        assert_eq!(parse_size("abc"), None);
        assert_eq!(parse_size("5TB"), None);
    }

    #[test]
    fn test_parse_size_whitespace() {
        assert_eq!(parse_size("  2 MB  "), Some(2_097_152));
    }

    #[test]
    fn test_parse_size_lowercase() {
        assert_eq!(parse_size("2gb"), Some(2_147_483_648));
    }

    #[test]
    fn test_parse_size_bare_number() {
        assert_eq!(parse_size("4096"), Some(4096));
    }

    #[test]
    fn test_parse_size_zero() {
        assert_eq!(parse_size("0B"), Some(0));
    }

    #[test]
    fn test_parse_size_decimal() {
        // No decimal support — "1.5" fails u64 parse
        assert_eq!(parse_size("1.5GB"), None);
    }

    // ---------------------------------------------------------------------------
    // add_directory / remove_directory
    // ---------------------------------------------------------------------------

    #[test]
    fn test_add_directory_basic() {
        let mut config = Config::default();
        assert!(add_directory(&mut config, "/var/log/app", true, "auto"));
        assert_eq!(config.sources.directories.len(), 1);
        assert_eq!(config.sources.directories[0].path, "/var/log/app");
        assert!(config.sources.directories[0].recursive);
        assert_eq!(config.sources.directories[0].encoding, "auto");
    }

    #[test]
    fn test_add_directory_duplicate() {
        let mut config = Config::default();
        assert!(add_directory(&mut config, "/var/log/app", true, "auto"));
        assert!(!add_directory(&mut config, "/var/log/app", false, "utf-8"));
        assert_eq!(config.sources.directories.len(), 1);
    }

    #[test]
    fn test_remove_directory_exists() {
        let mut config = Config::default();
        add_directory(&mut config, "/var/log/app", true, "auto");
        assert!(remove_directory(&mut config, "/var/log/app"));
        assert!(config.sources.directories.is_empty());
    }

    #[test]
    fn test_remove_directory_not_exists() {
        let mut config = Config::default();
        assert!(!remove_directory(&mut config, "/var/log/nonexistent"));
    }

    // ---------------------------------------------------------------------------
    // add_project / remove_project / get_project_by_name
    // ---------------------------------------------------------------------------

    #[test]
    fn test_add_project_basic() {
        let mut config = Config::default();
        assert!(add_project(&mut config, "myproject", "/logs/myproject"));
        assert_eq!(config.projects.projects.len(), 1);
        let p = &config.projects.projects[0];
        assert_eq!(p.name, "myproject");
        assert_eq!(p.path, "/logs/myproject");
        assert!(p.recursive);
    }

    #[test]
    fn test_add_project_duplicate_name() {
        let mut config = Config::default();
        assert!(add_project(&mut config, "proj", "/logs/a"));
        assert!(!add_project(&mut config, "proj", "/logs/b"));
        assert_eq!(config.projects.projects.len(), 1);
    }

    #[test]
    fn test_add_project_duplicate_path() {
        let mut config = Config::default();
        assert!(add_project(&mut config, "proj-a", "/logs/shared"));
        assert!(!add_project(&mut config, "proj-b", "/logs/shared"));
        assert_eq!(config.projects.projects.len(), 1);
    }

    #[test]
    fn test_remove_project_exists() {
        let mut config = Config::default();
        add_project(&mut config, "myproject", "/logs/myproject");
        assert!(remove_project(&mut config, "myproject"));
        assert!(config.projects.projects.is_empty());
    }

    #[test]
    fn test_remove_project_not_exists() {
        let mut config = Config::default();
        assert!(!remove_project(&mut config, "nonexistent"));
    }

    #[test]
    fn test_get_project_by_name_found() {
        let mut config = Config::default();
        add_project(&mut config, "alpha", "/logs/alpha");
        add_project(&mut config, "beta", "/logs/beta");
        let found = get_project_by_name(&config, "beta");
        assert!(found.is_some());
        assert_eq!(found.unwrap().path, "/logs/beta");
    }

    #[test]
    fn test_get_project_by_name_not_found() {
        let mut config = Config::default();
        add_project(&mut config, "alpha", "/logs/alpha");
        assert!(get_project_by_name(&config, "gamma").is_none());
    }

    // ---------------------------------------------------------------------------
    // load / save
    // ---------------------------------------------------------------------------

    fn tmp_config_path(name: &str) -> String {
        std::env::temp_dir().join(name).to_string_lossy().to_string()
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let path = tmp_config_path("loggerlog_test_roundtrip.toml");
        let _ = fs::remove_file(&path); // clean up from previous run

        let mut config = Config::default();
        add_directory(&mut config, "/var/log/app", true, "utf-8");
        add_project(&mut config, "web", "/logs/web");

        save(&config, Some(&path)).expect("save should succeed");
        let loaded = load(Some(&path)).expect("load should succeed");

        assert_eq!(loaded.sources.directories.len(), 1);
        assert_eq!(loaded.sources.directories[0].path, "/var/log/app");
        assert_eq!(loaded.projects.projects.len(), 1);
        assert_eq!(loaded.projects.projects[0].name, "web");
        assert_eq!(loaded.projects.projects[0].path, "/logs/web");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_load_nonexistent_creates_default() {
        let path = tmp_config_path("loggerlog_test_nonexistent.toml");
        let _ = fs::remove_file(&path); // ensure it doesn't exist

        let config = load(Some(&path)).expect("load should succeed");
        // load() returns a default Config when the file doesn't exist
        assert!(config.sources.directories.is_empty());
        assert!(config.projects.projects.is_empty());
        // Note: load() saves the default to config_path() (not the override path),
        // so the override-path file itself is NOT created.
        assert!(!PathBuf::from(&path).exists());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_load_invalid_toml_errors() {
        let path = tmp_config_path("loggerlog_test_invalid.toml");
        fs::write(&path, "{{{{invalid toml!!!").expect("write should succeed");

        let result = load(Some(&path));
        assert!(result.is_err(), "loading invalid TOML should return an error");

        let _ = fs::remove_file(&path);
    }
}
