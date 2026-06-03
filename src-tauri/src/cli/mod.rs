pub mod commands;

#[cfg(feature = "cli")]
use clap::Parser;

#[cfg(feature = "cli")]
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

#[cfg(feature = "cli")]
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

    /// Launch the GUI
    Gui,
}

#[cfg(feature = "cli")]
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Json,
    Table,
    Raw,
}

#[cfg(feature = "cli")]
impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Raw => write!(f, "raw"),
        }
    }
}

#[cfg(feature = "cli")]
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

#[cfg(feature = "cli")]
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

/// Check if the command line arguments indicate CLI mode
#[cfg(feature = "cli")]
pub fn is_cli_mode() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let second = args[1].to_lowercase();
        matches!(
            second.as_str(),
            "search" | "tail" | "config" | "index" | "--help" | "-h" | "--version" | "-v"
        )
    } else {
        false
    }
}

#[cfg(feature = "cli")]
pub fn run() {
    let cli = Cli::parse();
    let config_path = cli.config.clone();
    let global_output = cli.output.clone();

    match cli.command {
        Some(Commands::Search {
            query,
            level,
            source,
            after,
            before,
            thread,
            regex,
            limit,
            output,
            context,
        }) => {
            let output = global_output.unwrap_or(output);
            if let Err(e) = commands::search::run(&query, &level, source.as_deref(),
                after.as_deref(), before.as_deref(), thread.as_deref(),
                regex, limit, context, &output, config_path.as_deref()) {
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
        Some(Commands::Gui) => {
            #[cfg(feature = "gui")]
            crate::gui::run();
            #[cfg(not(feature = "gui"))]
            eprintln!("GUI feature not enabled. Rebuild with --features cli,gui");
        }
        None => {
            Cli::parse_from(["loggerlog", "--help"]);
        }
    }
}

#[cfg(not(feature = "cli"))]
pub fn is_cli_mode() -> bool {
    false
}

#[cfg(not(feature = "cli"))]
pub fn run() {
    eprintln!("CLI feature not enabled. Build with --features cli");
}
