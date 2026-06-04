use crate::cli::ConfigAction;
use crate::core::config;
use anyhow::Result;

pub fn run(action: ConfigAction, config_path: Option<&str>) -> Result<()> {
    match action {
        ConfigAction::Show => show_config(config_path),
        ConfigAction::Edit => edit_config(),
        ConfigAction::AddDir { path, recursive, encoding } => {
            add_dir(&path, recursive, &encoding, config_path)
        }
        ConfigAction::RemoveDir { path } => remove_dir(&path, config_path),
    }
}

fn show_config(config_path: Option<&str>) -> Result<()> {
    let cfg = config::load(config_path)?;
    let path = config::config_path();
    println!("Config file: {}", path.display());
    println!();
    println!("Database: {}", cfg.general.database_path);
    println!("Max file size: {}", cfg.general.max_file_size);
    println!("Watch interval: {}", cfg.general.watch_interval);
    println!();

    if cfg.sources.directories.is_empty() {
        println!("No log directories configured.");
    } else {
        println!("Log directories:");
        for (i, dir) in cfg.sources.directories.iter().enumerate() {
            println!("  {}. {} (recursive={}, encoding={})",
                i + 1, dir.path, dir.recursive, dir.encoding);
        }
    }

    Ok(())
}

fn edit_config() -> Result<()> {
    let path = config::config_path();
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    std::process::Command::new(editor)
        .arg(path)
        .status()?;
    Ok(())
}

fn add_dir(path: &str, recursive: bool, encoding: &str, config_path: Option<&str>) -> Result<()> {
    let mut cfg = config::load(config_path)?;
    let added = config::add_directory(&mut cfg, path, recursive, encoding);

    if added {
        config::save(&cfg, config_path)?;
        println!("Added log directory: {} (recursive={}, encoding={})", path, recursive, encoding);
    } else {
        println!("Directory already configured: {}", path);
    }

    Ok(())
}

fn remove_dir(path: &str, config_path: Option<&str>) -> Result<()> {
    let mut cfg = config::load(config_path)?;
    let removed = config::remove_directory(&mut cfg, path);

    if removed {
        config::save(&cfg, config_path)?;
        println!("Removed log directory: {}", path);
    } else {
        println!("Directory not found: {}", path);
    }

    Ok(())
}
