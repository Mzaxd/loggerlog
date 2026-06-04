pub mod commands;

use clap::Parser;

#[derive(Parser)]
#[command(name = "loggerlog")]
#[command(about = "Lightweight log search tool", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Config file path
    #[arg(long, global = true)]
    config: Option<String>,

    /// Output format
    #[arg(long, global = true, value_enum)]
    output: Option<OutputFormat>,
}

#[derive(clap::Subcommand)]
enum Commands {
        /// Search indexed log entries
    Search {
        /// Search query (FTS5 syntax or regex: prefix)
        query: String,

        /// Filter by log level
        #[arg(short, long)]
        level: Vec<String>,

        /// Filter by source file glob
        #[arg(short, long)]
        source: Option<String>,

        /// Filter by project name
        #[arg(long)]
        project: Option<String>,

        /// Filter by module name (subdirectory within project)
        #[arg(long)]
        module: Option<String>,

        /// Only entries after this timestamp
        #[arg(long)]
        after: Option<String>,

        /// Only entries before this timestamp
        #[arg(long)]
        before: Option<String>,

        /// Filter by thread name
        #[arg(long)]
        thread: Option<String>,

        /// Use regex search instead of FTS
        #[arg(long)]
        regex: bool,

        /// Max results
        #[arg(short = 'n', long, default_value = "100")]
        limit: u32,

        /// Output format
        #[arg(short, long, default_value = "table")]
        output: OutputFormat,

        /// Include surrounding context lines
        #[arg(short = 'C', long)]
        context: Option<u32>,

        /// Skip automatic incremental sync before search
        #[arg(long)]
        no_sync: bool,
    },

    /// Tail / follow log files in real-time
    Tail {
        /// Source file or directory to tail
        source: Option<String>,

        /// Filter by level
        #[arg(short, long)]
        level: Vec<String>,

        /// Filter by FTS query
        #[arg(short, long)]
        filter: Option<String>,

        /// Output format
        #[arg(short, long, default_value = "raw")]
        output: OutputFormat,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Manage the search index
    Index {
        #[command(subcommand)]
        action: IndexAction,
    },

    /// Manage projects and modules
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Json,
    Table,
    Raw,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Raw => write!(f, "raw"),
        }
    }
}

#[derive(clap::Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Open config in $EDITOR
    Edit,
    /// Add a log directory
    AddDir {
        /// Directory path
        path: String,
        /// Recursive scan
        #[arg(long, default_value = "true")]
        recursive: bool,
        /// Encoding (auto, utf-8, gbk)
        #[arg(long, default_value = "auto")]
        encoding: String,
    },
    /// Remove a log directory
    RemoveDir {
        /// Directory path
        path: String,
    },
}

#[derive(clap::Subcommand)]
pub enum IndexAction {
    /// Full re-index of all configured sources
    Rebuild,
    /// Incremental update (index new/changed files)
    Update,
    /// Optimize FTS index
    Compact,
    /// Show index statistics
    Stats,
}

#[derive(clap::Subcommand)]
pub enum ProjectAction {
    /// Add a new project
    Add {
        /// Project name
        name: String,
        /// Root log directory path
        path: String,
        /// Recursive scan
        #[arg(long, default_value = "true")]
        recursive: bool,
    },
    /// Remove a project
    Remove {
        /// Project name
        name: String,
    },
    /// List all projects and their modules
    List,
}

pub fn run() {
    let cli = Cli::parse();
    let config_path = cli.config.clone();
    let global_output = cli.output.clone();

    match cli.command {
        Some(Commands::Search {
            query,
            level,
            source,
            project,
            module,
            after,
            before,
            thread,
            regex,
            limit,
            output,
            context,
            no_sync,
        }) => {
            let output = global_output.unwrap_or(output);
            if let Err(e) = commands::search::run(&query, &level, source.as_deref(),
                project.as_deref(), module.as_deref(),
                after.as_deref(), before.as_deref(), thread.as_deref(),
                regex, limit, context, &output, config_path.as_deref(), no_sync) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Tail { source, level, filter, output }) => {
            let output = global_output.unwrap_or(output);
            if let Err(e) = commands::tail::run(source.as_deref(), &level, filter.as_deref(), &output, config_path.as_deref()) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Config { action }) => {
            if let Err(e) = commands::config::run(action, config_path.as_deref()) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Index { action }) => {
            if let Err(e) = commands::index::run(action, config_path.as_deref()) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Project { action }) => {
            if let Err(e) = commands::project::run(action, config_path.as_deref()) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        None => {
            Cli::parse_from(["loggerlog", "--help"]);
        }
    }
}
