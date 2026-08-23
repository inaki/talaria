//! Rotating file log under `~/.hermes-rust/logs/hermes-rust.log`.
//!
//! Always on. `--verbose` only echoes a pre-TUI notice to stderr; writing to
//! stderr after the alternate screen is up corrupts the display.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::paths::{create_private_dir_all, HermesRustPaths};

static LOG_FILE: Mutex<Option<PathBuf>> = Mutex::new(None);

const MAX_BYTES: u64 = 10 * 1024 * 1024;
const MAX_FILES: usize = 5;

pub fn log_path() -> PathBuf {
    HermesRustPaths::from_env().log_file()
}

/// Idempotent. Safe to call from `main` and from `run_tui`.
pub fn init_file_logging() {
    {
        let mut guard = match LOG_FILE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if guard.is_some() {
            return;
        }
        let path = log_path();
        if let Some(parent) = path.parent() {
            create_private_dir_all(parent);
        }
        rotate_if_large(&path);
        *guard = Some(path);
    }
    log_line("hermes-rust logging enabled");
}

fn generation(path: &Path, n: usize) -> PathBuf {
    let mut name: OsString = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{n}"));
    path.with_file_name(name)
}

fn rotate_if_large(path: &Path) {
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() >= MAX_BYTES => {}
        _ => return,
    }
    for i in (1..MAX_FILES).rev() {
        let older = generation(path, i);
        let newer = if i == 1 {
            path.to_path_buf()
        } else {
            generation(path, i - 1)
        };
        if newer.exists() {
            let _ = std::fs::rename(&newer, &older);
        }
    }
}

pub fn log_line(message: &str) {
    let timestamp = chrono::Utc::now().to_rfc3339();
    let line = format!("[{timestamp}] {message}\n");
    if let Ok(guard) = LOG_FILE.lock() {
        if let Some(path) = guard.as_ref() {
            let mut opts = OpenOptions::new();
            opts.create(true).append(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            if let Ok(mut file) = opts.open(path) {
                let _ = file.write_all(line.as_bytes());
            }
        }
    }
    tracing::debug!("{message}");
}

/// Truncate a protocol preview to 240 chars (Ink protocol-error preview).
pub fn preview(s: &str) -> String {
    const LIMIT: usize = 240;
    if s.chars().count() <= LIMIT {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(LIMIT).collect();
        format!("{truncated}…")
    }
}

/// Methods / event types whose params must never hit the log.
pub fn is_sensitive(kind: &str) -> bool {
    let k = kind.to_ascii_lowercase();
    k.contains("secret") || k.contains("sudo") || k.contains("approval") || k.contains("clarify")
}

pub fn tool_is_sensitive(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("secret")
        || n.contains("sudo")
        || n.contains("password")
        || n.contains("credential")
        || n.contains("token")
}

/// Truncate and strip obvious secret-looking assignments for tool cards.
pub fn sanitize_tool_text(name: &str, text: &str) -> String {
    if tool_is_sensitive(name) || is_sensitive(name) {
        return "<redacted>".into();
    }
    let mut out = String::new();
    for (i, raw) in text.split_whitespace().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let lower = raw.to_ascii_lowercase();
        if lower.contains("password=")
            || lower.contains("token=")
            || lower.contains("secret=")
            || lower.contains("api_key=")
        {
            out.push_str("<redacted>");
        } else {
            out.push_str(raw);
        }
    }
    preview(&out)
}

pub fn log_rpc(direction: &str, method: &str, body: &serde_json::Value) {
    if is_sensitive(method) {
        log_line(&format!("{direction} {method} <redacted>"));
    } else {
        log_line(&format!(
            "{direction} {method} {}",
            preview(&body.to_string())
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_redacts_secret_and_approval() {
        assert!(is_sensitive("secret.request"));
        assert!(is_sensitive("sudo.respond"));
        assert!(is_sensitive("approval.request"));
        assert!(!is_sensitive("session.create"));
        assert!(!is_sensitive("prompt.submit"));
        assert!(tool_is_sensitive("read_secret"));
        assert_eq!(sanitize_tool_text("sudo", "anything"), "<redacted>");
        assert!(sanitize_tool_text("web_search", "q=hi token=abc").contains("<redacted>"));
    }

    #[test]
    fn preview_caps_at_240() {
        let long = "x".repeat(500);
        let p = preview(&long);
        assert!(p.ends_with('…'));
        assert!(p.chars().count() <= 241);
    }
}
