//! Local `!command` escape (herald_v2). Does not call the model.
//!
//! The UI confirms before spawn. The process is started in its own group so a
//! timeout can kill children, not just `sh`.

use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

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

pub fn kill_process_group(pid: u32) {
    if pid == 0 {
        return;
    }
    #[cfg(unix)]
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn configure(command: &str, cwd: &str) -> Command {
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
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

pub fn run_shell_command(command: &str, cwd: &str) -> ShellResult {
    run_shell_command_timeout(command, cwd, Duration::from_secs(TIMEOUT_SECS))
}

pub fn run_shell_command_timeout(command: &str, cwd: &str, timeout: Duration) -> ShellResult {
    let start = Instant::now();
    let mut cmd = configure(command, cwd);
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return ShellResult {
                command: command.to_string(),
                output: format!("spawn error: {e}"),
                code: Some(1),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    };
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(out)) => ShellResult {
            command: command.to_string(),
            output: format_output(&out.stdout, &out.stderr),
            code: out.status.code(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Ok(Err(e)) => ShellResult {
            command: command.to_string(),
            output: format!("spawn error: {e}"),
            code: Some(1),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(_) => {
            kill_process_group(pid);
            ShellResult {
                command: command.to_string(),
                output: format!("timed out after {}s", timeout.as_secs()),
                code: Some(124),
                duration_ms: start.elapsed().as_millis() as u64,
            }
        }
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
    let body = crate::logging::strip_controls(&body);
    let mut body = crate::logging::redact_secrets(&body);
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
    fn output_redacts_secrets_and_strips_csi() {
        let out = format_output(b"token=abcdefghijk\x1b[31mred", b"");
        assert!(out.contains("<redacted>"), "{out}");
        assert!(!out.contains("abcdefghijk"), "{out}");
        assert!(!out.contains('\u{1b}'), "{out}");
    }

    #[test]
    fn echo_runs() {
        let r = run_shell_command("echo hermes-bang", ".");
        assert_eq!(r.code, Some(0));
        assert!(r.output.contains("hermes-bang"), "{}", r.output);
    }

    #[test]
    fn timeout_kills_sleep() {
        let r = run_shell_command_timeout("sleep 30", ".", Duration::from_millis(400));
        assert_eq!(r.code, Some(124), "{}", r.output);
        assert!(r.duration_ms < 5_000, "waited {}ms", r.duration_ms);
    }

    #[cfg(unix)]
    #[test]
    fn timeout_kills_background_child() {
        // A child that would outlive `sh` if we only killed the shell.
        let r = run_shell_command_timeout("sleep 30 & wait", ".", Duration::from_millis(400));
        assert_eq!(r.code, Some(124), "{}", r.output);
        assert!(r.duration_ms < 5_000, "waited {}ms", r.duration_ms);
    }
}
