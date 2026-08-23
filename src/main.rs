//! Thin binary. All logic lives in `hermes_rust`.

use clap::Parser;

use hermes_rust::app::{run_tui_with_options, RunOptions};
use hermes_rust::cli::Cli;

fn main() {
    let cli = Cli::parse();
    hermes_rust::logging::init_file_logging();

    if cli.verbose {
        eprintln!(
            "[hermes-rust] verbose; log {}",
            hermes_rust::logging::log_path().display()
        );
        if cli.dev || cli.mock.is_some() {
            eprintln!("[hermes-rust] mock session (no Python child)");
        }
    }

    if let Err(e) = run_tui_with_options(RunOptions {
        dev: cli.dev || cli.mock.is_some(),
        mock: cli.mock.clone(),
        verbose: cli.verbose,
    }) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
