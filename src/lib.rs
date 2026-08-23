//! Native ratatui host for official Hermes `tui_gateway`.
//!
//! Library-first: tests and the binary share this crate. `main.rs` stays thin.

pub mod app;
pub mod cli;
pub mod discover;
pub mod gateway;
pub mod logging;
pub mod paths;
pub mod protocol;
pub mod session;
pub mod shell;
pub mod theme;
pub mod ui;
pub mod user_messages;

pub use app::{run_tui, run_tui_with_options, RunOptions};
