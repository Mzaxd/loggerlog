pub fn run() {
    #[cfg(all(feature = "cli", feature = "gui"))]
    {
        if cli::is_cli_mode() {
            cli::run();
        } else {
            gui::run();
        }
    }

    #[cfg(all(feature = "cli", not(feature = "gui")))]
    {
        cli::run();
    }

    #[cfg(all(feature = "gui", not(feature = "cli")))]
    {
        gui::run();
    }

    #[cfg(not(any(feature = "cli", feature = "gui")))]
    {
        eprintln!("LoggerLog: no features enabled. Build with --features cli or --features gui");
    }
}

pub mod cli;
pub mod core;
pub mod gui;
