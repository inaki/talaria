//! Talaria Client: native Rust TUI for Hermes Agent `tui_gateway`.
//!
//! Library-first: tests and the binary share this crate. `main.rs` stays thin.
//! Not affiliated with Nous Research. See `NOTICE.md`.

pub mod app;
pub mod cli;
pub mod discover;
pub mod gateway;
pub mod logging;
pub mod paths;
pub mod peer;
pub mod prefs;
pub mod protocol;
pub mod session;
pub mod shell;
pub mod theme;
pub mod ui;
pub mod update;
pub mod user_messages;

pub use app::{run_tui, run_tui_with_options, RunOptions};
