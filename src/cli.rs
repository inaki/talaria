//! Clap surface. Dump is an example, not a subcommand — keeps this bin TUI-shaped.

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "talaria",
    version,
    about = "Native Rust TUI client for Hermes Agent"
)]
pub struct Cli {
    /// Extra stderr notices before the alternate screen. File log is always on.
    #[arg(short, long)]
    pub verbose: bool,

    /// Offline UI: MockSession only, no Python child.
    #[arg(long)]
    pub dev: bool,

    /// Canned MockSession script: streaming | tools | approval | error
    #[arg(long)]
    pub mock: Option<String>,

    /// Color skin: github | default | ares | mono | slate | daylight | warm-lightmode | poseidon | sisyphus | charizard
    #[arg(long)]
    pub theme: Option<String>,

    /// Start even if another Hermes TUI / talaria / tui_gateway is already running.
    #[arg(long)]
    pub force: bool,
}
