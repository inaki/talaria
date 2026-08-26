//! Thin binary. All logic lives in `talaria`.

use clap::Parser;

use talaria::app::{run_tui_with_options, RunOptions};
use talaria::cli::Cli;

fn main() {
    let cli = Cli::parse();
    talaria::logging::init_file_logging();

    if cli.verbose {
        eprintln!(
            "[talaria] verbose; log {}",
            talaria::logging::log_path().display()
        );
        if cli.dev || cli.mock.is_some() {
            eprintln!("[talaria] mock session (no Python child)");
        }
    }

    if let Err(e) = run_tui_with_options(RunOptions {
        dev: cli.dev || cli.mock.is_some(),
        mock: cli.mock.clone(),
        verbose: cli.verbose,
        theme: cli.theme.clone(),
        force: cli.force,
        resume: talaria::cli::ResumeSpec::resolve(
            cli.r#continue,
            cli.resume.as_deref(),
            std::env::var("TALARIA_RESUME").ok().as_deref(),
        ),
    }) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
