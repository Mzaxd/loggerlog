use crate::cli::ProjectAction;
use crate::core::config;
use crate::core::index::IndexManager;
use anyhow::Result;

pub fn run(action: ProjectAction, config_path: Option<&str>) -> Result<()> {
    match action {
        ProjectAction::Add {
            name,
            path,
            recursive,
        } => add_project(&name, &path, recursive, config_path),
        ProjectAction::Remove { name } => remove_project(&name, config_path),
        ProjectAction::List => list_projects(config_path),
    }
}

fn add_project(name: &str, path: &str, recursive: bool, config_path: Option<&str>) -> Result<()> {
    let mut cfg = config::load(config_path)?;
    let added = config::add_project(&mut cfg, name, path);

    if added {
        // Set recursive if not default
        if let Some(project) = cfg.projects.projects.iter_mut().find(|p| p.name == name) {
            project.recursive = recursive;
        }
        config::save(&cfg, config_path)?;
        println!("Added project '{}' -> {}", name, path);

        // Scan subdirectories and show detected modules
        let project_path = std::path::Path::new(path);
        if project_path.is_dir() {
            let mut modules: Vec<String> = Vec::new();
            if let Ok(entries) = std::fs::read_dir(project_path) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            modules.push(name.to_string());
                        }
                    }
                }
            }
            modules.sort();
            if !modules.is_empty() {
                println!(
                    "  Detected modules ({}): {}",
                    modules.len(),
                    modules.join(", ")
                );
            } else {
                println!("  No subdirectories detected as modules.");
            }
        }
    } else {
        println!("Project '{}' or path '{}' already exists.", name, path);
    }

    Ok(())
}

fn remove_project(name: &str, config_path: Option<&str>) -> Result<()> {
    let mut cfg = config::load(config_path)?;
    let removed = config::remove_project(&mut cfg, name);

    if removed {
        config::save(&cfg, config_path)?;
        // Also clean up project from database
        if let Ok(idx) = IndexManager::open(&cfg.general.database_path) {
            idx.remove_project(name)?;
        }
        println!("Removed project: {}", name);
    } else {
        println!("Project not found: {}", name);
    }

    Ok(())
}

fn list_projects(config_path: Option<&str>) -> Result<()> {
    let cfg = config::load(config_path)?;
    let idx = IndexManager::open(&cfg.general.database_path)?;

    if cfg.projects.projects.is_empty() {
        println!("No projects configured. Use 'loggerlog project add <name> <path>' to add one.");
        return Ok(());
    }

    // Sync projects first to ensure DB is up to date
    idx.sync_projects(&cfg.projects.projects)?;

    println!("{:<20} {:<50} {:<15}", "NAME", "PATH", "MODULES");
    println!("{}", "-".repeat(85));

    for project in &cfg.projects.projects {
        let modules = idx.get_modules_for_project(&project.name)?;
        let modules_str = if modules.is_empty() {
            "-".to_string()
        } else {
            format!("{} module(s)", modules.len())
        };
        let path_short = if project.path.len() > 47 {
            format!("...{}", &project.path[project.path.len() - 47..])
        } else {
            project.path.clone()
        };
        println!(
            "{:<20} {:<50} {:<15}",
            project.name, path_short, modules_str
        );

        // Show modules if any
        if !modules.is_empty() {
            println!("  Modules: {}", modules.join(", "));
        }
    }

    Ok(())
}
