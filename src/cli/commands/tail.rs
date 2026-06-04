use crate::cli::OutputFormat;
use crate::core::config;
use anyhow::Result;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek};

pub fn run(
    source: Option<&str>,
    levels: &[String],
    filter: Option<&str>,
    _output: &OutputFormat,
    config_path: Option<&str>,
) -> Result<()> {
    let cfg = config::load(config_path)?;

    let file_path = match source {
        Some(s) => s.to_string(),
        None => {
            if cfg.sources.directories.is_empty() {
                anyhow::bail!("No log directories configured. Use 'loggerlog config add-dir <path>' first.");
            }
            let dir = std::path::Path::new(&cfg.sources.directories[0].path);
            find_most_recent_log(dir)?
        }
    };

    println!("Tailing: {} (press Ctrl+C to stop)", file_path);

    let file = File::open(&file_path)?;
    let _file_size = file.metadata()?.len();
    let mut reader = BufReader::new(file);

    reader.get_mut().seek(std::io::SeekFrom::End(0))?;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;

        if bytes_read > 0 {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if !levels.is_empty() {
                let matches_level = levels.iter().any(|l| {
                    trimmed.to_uppercase().contains(&format!(" {} ", l))
                        || trimmed.to_uppercase().starts_with(&format!("{} ", l))
                });
                if !matches_level {
                    continue;
                }
            }

            if let Some(f) = filter {
                if !trimmed.contains(f) {
                    continue;
                }
            }

            println!("{}", trimmed);
        } else {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
}

fn find_most_recent_log(dir: &std::path::Path) -> Result<String> {
    let mut most_recent = None;
    let mut most_recent_time: std::time::SystemTime = std::time::UNIX_EPOCH;

    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "log" || ext == "txt" || ext == "out" {
                        let meta = entry.metadata()?;
                        if let Ok(modified) = meta.modified() {
                            if modified > most_recent_time {
                                most_recent_time = modified;
                                most_recent = Some(path.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    most_recent.ok_or_else(|| anyhow::anyhow!("No log files found in {}", dir.display()))
}
