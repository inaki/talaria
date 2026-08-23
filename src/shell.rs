//! Local `!command` escape (herald_v2). Does not call the model.

use std::process::{Command, Stdio};
use std::time::Instant;

pub const TIMEOUT_SECS: u64 = 30;
const MAX_CHARS: usize = 4000;

#[derive(Debug, Clone)]
pub struct ShellResult {
    pub command: String,
    pub output: String,
    pub code: Option<i32>,
    pub duration_ms: u64,
}

/// Extract a shell command from a `!…` draft. `! pwd` and `!pwd` both work.
pub fn command_from_input(input: &str) -> Option<&str> {
    let trimmed = input.trim();
    let rest = trimmed.strip_prefix('!')?;
    let cmd = rest.trim();
    if cmd.is_empty() {
        None
    } else {
        Some(cmd)
    }
}

pub fn run_shell_command(command: &str, cwd: &str) -> ShellResult {
    let start = Instant::now();
    let mut cmd = {
        #[cfg(windows)]
        {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(command);
            c
        }
        #[cfg(not(windows))]
        {
            let mut c = Command::new("sh");
            c.arg("-c").arg(command);
            c
        }
    };
    if !cwd.is_empty() {
        cmd.current_dir(cwd);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = match cmd.output() {
        Ok(out) => out,
        Err(e) => {
            return ShellResult {
                command: command.to_string(),
                output: format!("spawn error: {e}"),
                code: Some(1),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    let body = format_output(&output.stdout, &output.stderr);

    ShellResult {
        command: command.to_string(),
        output: body,
        code: output.status.code(),
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

pub fn format_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut body = String::from_utf8_lossy(stdout).into_owned();
    let err = String::from_utf8_lossy(stderr);
    if !err.trim().is_empty() {
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str("stderr:\n");
        body.push_str(&err);
    }
    if body.chars().count() > MAX_CHARS {
        let keep: String = body.chars().take(MAX_CHARS).collect();
        body = format!("{keep}\n… truncated");
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bang_with_optional_space() {
        assert_eq!(command_from_input("! pwd"), Some("pwd"));
        assert_eq!(command_from_input("!pwd"), Some("pwd"));
        assert_eq!(command_from_input("  !  ls -la  "), Some("ls -la"));
        assert_eq!(command_from_input("!"), None);
        assert_eq!(command_from_input("hello!"), None);
        assert_eq!(command_from_input("fix the bug!"), None);
    }

    #[test]
    fn echo_runs() {
        let r = run_shell_command("echo hermes-bang", ".");
        assert_eq!(r.code, Some(0));
        assert!(r.output.contains("hermes-bang"), "{}", r.output);
    }
}
