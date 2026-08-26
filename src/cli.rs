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

    /// Color skin: talaria | talaria-light | github | default | ares | mono | slate | daylight | warm-lightmode | poseidon | sisyphus | charizard
    #[arg(long)]
    pub theme: Option<String>,

    /// Start even if another Hermes TUI / talaria / tui_gateway is already running.
    #[arg(long)]
    pub force: bool,

    /// Resume the most recent saved session (same as `TALARIA_RESUME=1`).
    #[arg(short = 'c', long = "continue")]
    pub r#continue: bool,

    /// Resume a saved session by id or title (`latest` = most recent).
    #[arg(short = 'r', long = "resume")]
    pub resume: Option<String>,
}

/// How live mode should open a session. CLI flags beat `TALARIA_RESUME`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ResumeSpec {
    #[default]
    Fresh,
    Latest,
    Query(String),
}

impl ResumeSpec {
    pub fn resolve(continue_flag: bool, resume: Option<&str>, env: Option<&str>) -> Self {
        if let Some(q) = resume.map(str::trim).filter(|s| !s.is_empty()) {
            if q.eq_ignore_ascii_case("latest") {
                return Self::Latest;
            }
            return Self::Query(q.to_string());
        }
        if continue_flag {
            return Self::Latest;
        }
        let Some(raw) = env.map(str::trim).filter(|s| !s.is_empty()) else {
            return Self::Fresh;
        };
        match raw {
            "0" | "false" | "FALSE" | "off" | "OFF" | "no" | "NO" => Self::Fresh,
            "1" | "true" | "TRUE" | "on" | "ON" | "latest" | "LATEST" => Self::Latest,
            other => Self::Query(other.to_string()),
        }
    }

    pub fn from_env() -> Self {
        Self::resolve(false, None, std::env::var("TALARIA_RESUME").ok().as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::ResumeSpec;

    #[test]
    fn resume_flag_beats_continue_and_env() {
        assert_eq!(
            ResumeSpec::resolve(true, Some("abc"), Some("1")),
            ResumeSpec::Query("abc".into())
        );
        assert_eq!(
            ResumeSpec::resolve(false, Some("latest"), None),
            ResumeSpec::Latest
        );
        assert_eq!(ResumeSpec::resolve(true, None, None), ResumeSpec::Latest);
        assert_eq!(
            ResumeSpec::resolve(false, None, Some("1")),
            ResumeSpec::Latest
        );
        assert_eq!(
            ResumeSpec::resolve(false, None, Some("my-thread")),
            ResumeSpec::Query("my-thread".into())
        );
        assert_eq!(
            ResumeSpec::resolve(false, None, Some("0")),
            ResumeSpec::Fresh
        );
        assert_eq!(ResumeSpec::resolve(false, None, None), ResumeSpec::Fresh);
    }
}
