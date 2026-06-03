use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Root configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub sources: SourcesConfig,
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

/// Parse a human-readable size string like "2GB" to bytes
pub fn parse_size(size_str: &str) -> u64 {
    let s = size_str.trim().to_uppercase();
    let (num_str, multiplier) = if s.ends_with("GB") {
        (&s[..s.len() - 2], 1024u64 * 1024 * 1024)
    } else if s.ends_with("MB") {
        (&s[..s.len() - 2], 1024u64 * 1024)
    } else if s.ends_with("KB") {
        (&s[..s.len() - 2], 1024u64)
    } else if s.ends_with("B") {
        (&s[..s.len() - 1], 1)
    } else {
        (s.as_str(), 1)
    };
    num_str.trim().parse::<u64>().unwrap_or(0) * multiplier
}
