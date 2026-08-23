//! Rotating file log under `~/.hermes-rust/logs/hermes-rust.log`.
//!
//! Always on. `--verbose` only echoes a pre-TUI notice to stderr; writing to
//! stderr after the alternate screen is up corrupts the display.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::paths::{create_private_dir_all, ensure_private_file, HermesRustPaths};

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
            ensure_private_file(&older);
        }
    }
    ensure_private_file(path);
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
                ensure_private_file(path);
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
    k.contains("secret")
        || k.contains("sudo")
        || k.contains("approval")
        || k.contains("clarify")
        || k.contains("save_key")
        || k.contains("api_key")
        || k.contains("password")
        || k.contains("credential")
        || k.contains("authorization")
        || k.contains("token")
        || k.contains("prompt.submit")
        || k.contains("steer")
}

/// Remove C0/C1 controls and OSC/CSI so untrusted text cannot drive the terminal.
/// Keeps newline and tab.
pub fn strip_controls(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            match it.peek().copied() {
                Some('[') => {
                    it.next();
                    for x in it.by_ref() {
                        if ('@'..='~').contains(&x) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    it.next();
                    while let Some(x) = it.next() {
                        if x == '\u{07}' {
                            break;
                        }
                        if x == '\u{1b}' && it.peek() == Some(&'\\') {
                            it.next();
                            break;
                        }
                    }
                }
                Some(_) => {
                    let _ = it.next();
                }
                None => {}
            }
            continue;
        }
        if c == '\n' || c == '\t' {
            out.push(c);
            continue;
        }
        if c.is_control() || ('\u{80}'..='\u{9f}').contains(&c) {
            continue;
        }
        out.push(c);
    }
    out
}

pub fn tool_is_sensitive(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("secret")
        || n.contains("sudo")
        || n.contains("password")
        || n.contains("credential")
        || n.contains("token")
}

fn key_is_sensitive(key: &str) -> bool {
    let k = key.trim().to_ascii_lowercase().replace('-', "_");
    matches!(
        k.as_str(),
        "password"
            | "passwd"
            | "secret"
            | "token"
            | "api_key"
            | "apikey"
            | "authorization"
            | "access_token"
            | "refresh_token"
            | "private_key"
            | "client_secret"
            | "credential"
            | "bearer"
            | "api_token"
            | "auth_token"
    ) || k.ends_with("_password")
        || k.ends_with("_secret")
        || k.ends_with("_token")
        || k.ends_with("_credential")
        || k.ends_with("_api_key")
        || k.ends_with("_private_key")
}

/// True when a composer line must not be written to `~/.hermes-rust/history`.
pub fn should_skip_history(text: &str) -> bool {
    let t = text.trim();
    t.starts_with('!') || text_looks_secret(t)
}

pub fn text_looks_secret(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if assignment_is_secret(&lower) {
        return true;
    }
    if lower.contains("bearer ") {
        return true;
    }
    token_has_secret_prefix(text)
}

fn assignment_is_secret(lower: &str) -> bool {
    const MARKERS: &[&str] = &[
        "password=",
        "passwd=",
        "secret=",
        "token=",
        "api_key=",
        "apikey=",
        "authorization=",
        "access_token=",
        "private_key=",
        "client_secret=",
        "credential=",
    ];
    MARKERS.iter().any(|m| lower.contains(m))
}

fn token_has_secret_prefix(text: &str) -> bool {
    for token in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')) {
        if secret_prefix_token(token) {
            return true;
        }
    }
    false
}

fn secret_prefix_token(token: &str) -> bool {
    let t = token.to_ascii_lowercase();
    const PREFIXES: &[&str] = &[
        "sk-",
        "xai-",
        "ghp_",
        "github_pat_",
        "glpat-",
        "akia",
        "xoxb-",
        "xoxp-",
        "xoxa-",
    ];
    PREFIXES
        .iter()
        .any(|p| t.starts_with(p) && t.len() >= p.len() + 8)
}

/// Strip secrets from tool cards, logs, and stderr. Never panics.
pub fn redact_secrets(text: &str) -> String {
    let s = redact_quoted_json_values(text);
    let s = redact_assignments(&s);
    let s = redact_bearer(&s);
    redact_known_prefixes(&s)
}

fn redact_quoted_json_values(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        if rest.starts_with('"') || rest.starts_with('\'') {
            if let Some((key, after_key)) = parse_quoted(text, i) {
                let colon = skip_ws(text, after_key);
                if colon < text.len() && text.as_bytes()[colon] == b':' {
                    let val_at = skip_ws(text, colon + 1);
                    if val_at < text.len()
                        && (text[val_at..].starts_with('"') || text[val_at..].starts_with('\''))
                    {
                        if key_is_sensitive(key) {
                            if let Some((_, after_val)) = parse_quoted(text, val_at) {
                                out.push_str(&text[i..val_at]);
                                let q = text[val_at..].chars().next().unwrap();
                                out.push(q);
                                out.push_str("<redacted>");
                                out.push(q);
                                i = after_val;
                                continue;
                            }
                        }
                    }
                }
            }
        }
        let c = text[i..].chars().next().unwrap();
        out.push(c);
        i += c.len_utf8();
    }
    out
}

fn parse_quoted(s: &str, start: usize) -> Option<(&str, usize)> {
    let quote = s[start..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let content_start = start + quote.len_utf8();
    let mut escaped = false;
    for (off, c) in s[content_start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == quote {
            let content_end = content_start + off;
            return Some((
                &s[content_start..content_end],
                content_end + quote.len_utf8(),
            ));
        }
    }
    None
}

fn skip_ws(s: &str, mut i: usize) -> usize {
    while i < s.len() {
        let c = s[i..].chars().next().unwrap();
        if !c.is_whitespace() {
            break;
        }
        i += c.len_utf8();
    }
    i
}

fn redact_assignments(text: &str) -> String {
    let mut out = String::new();
    for (i, raw) in text.split_whitespace().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let lower = raw.to_ascii_lowercase();
        if assignment_is_secret(&lower) || lower.contains("password=") || lower.contains("token=") {
            if let Some(eq) = raw.find('=') {
                out.push_str(&raw[..=eq]);
                out.push_str("<redacted>");
            } else {
                out.push_str("<redacted>");
            }
        } else {
            out.push_str(raw);
        }
    }
    out
}

fn redact_bearer(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::new();
    let mut last = 0usize;
    let mut search = 0usize;
    while let Some(pos) = lower[search..].find("bearer ") {
        let at = search + pos;
        let value_start = at + "bearer ".len();
        let value_end = text[value_start..]
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',')
            .map(|n| value_start + n)
            .unwrap_or(text.len());
        out.push_str(&text[last..value_start]);
        if value_end > value_start {
            out.push_str("<redacted>");
        }
        last = value_end;
        search = value_end.max(search + 1);
    }
    out.push_str(&text[last..]);
    out
}

fn redact_known_prefixes(text: &str) -> String {
    let mut out = String::new();
    let mut last = 0usize;
    let mut idx = 0usize;
    while idx < text.len() {
        let c = text[idx..].chars().next().unwrap();
        let is_tok = c.is_ascii_alphanumeric() || c == '-' || c == '_';
        if !is_tok {
            idx += c.len_utf8();
            continue;
        }
        let start = idx;
        while idx < text.len() {
            let n = text[idx..].chars().next().unwrap();
            if !(n.is_ascii_alphanumeric() || n == '-' || n == '_') {
                break;
            }
            idx += n.len_utf8();
        }
        let token = &text[start..idx];
        if secret_prefix_token(token) {
            out.push_str(&text[last..start]);
            out.push_str("<redacted>");
            last = idx;
        }
    }
    out.push_str(&text[last..]);
    out
}

/// Truncate and strip secrets for tool cards.
pub fn sanitize_tool_text(name: &str, text: &str) -> String {
    if tool_is_sensitive(name) || is_sensitive(name) {
        return "<redacted>".into();
    }
    preview(&redact_secrets(&strip_controls(text)))
}

pub fn should_redact_rpc(method: &str, body: &serde_json::Value) -> bool {
    is_sensitive(method) || json_contains_secret(body)
}

fn json_contains_secret(body: &serde_json::Value) -> bool {
    match body {
        serde_json::Value::Object(map) => map
            .iter()
            .any(|(k, v)| key_is_sensitive(k) || json_contains_secret(v)),
        serde_json::Value::Array(arr) => arr.iter().any(json_contains_secret),
        serde_json::Value::String(s) => text_looks_secret(s),
        _ => false,
    }
}

pub fn rpc_log_line(direction: &str, method: &str, body: &serde_json::Value) -> String {
    if should_redact_rpc(method, body) {
        format!("{direction} {method} <redacted>")
    } else {
        format!("{direction} {method} {}", preview(&body.to_string()))
    }
}

pub fn stderr_log_line(line: &str) -> String {
    format!("gw-stderr {}", redact_secrets(&strip_controls(line)))
}

pub fn log_rpc(direction: &str, method: &str, body: &serde_json::Value) {
    log_line(&rpc_log_line(direction, method, body));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_redacts_secret_and_approval() {
        assert!(is_sensitive("secret.request"));
        assert!(is_sensitive("sudo.respond"));
        assert!(is_sensitive("approval.request"));
        assert!(is_sensitive("model.save_key"));
        assert!(!is_sensitive("session.create"));
        assert!(is_sensitive("prompt.submit"));
        assert!(is_sensitive("session.steer"));
        assert!(tool_is_sensitive("read_secret"));
        assert_eq!(sanitize_tool_text("sudo", "anything"), "<redacted>");
        assert!(sanitize_tool_text("web_search", "q=hi token=abc").contains("<redacted>"));
    }

    #[test]
    fn save_key_rpc_never_logs_body() {
        let body = serde_json::json!({
            "slug": "openai",
            "api_key": "sk-live-super-secret-value"
        });
        let line = rpc_log_line("→", "model.save_key", &body);
        assert_eq!(line, "→ model.save_key <redacted>");
        assert!(!line.contains("sk-live"));
        assert!(!line.contains("super-secret"));
    }

    #[test]
    fn config_set_with_key_value_is_redacted() {
        let body = serde_json::json!({
            "key": "openai_api_key",
            "value": "sk-live-abcdefghijklmnopqrstuvwxyz"
        });
        // method name is not sensitive; body string contains a key prefix.
        let line = rpc_log_line("→", "config.set", &body);
        assert!(
            line.contains("<redacted>") || !line.contains("sk-live-abcdefgh"),
            "{line}"
        );
        assert!(
            !line.contains("sk-live-abcdefghijklmnopqrstuvwxyz"),
            "{line}"
        );
    }

    #[test]
    fn ordinary_rpc_still_logs_preview() {
        let body = serde_json::json!({ "session_id": "abc", "cols": 80 });
        let line = rpc_log_line("→", "session.create", &body);
        assert!(line.contains("session.create"), "{line}");
        assert!(line.contains("abc"), "{line}");
        assert!(!line.contains("<redacted>"), "{line}");
    }

    #[test]
    fn json_tool_args_redact_api_key() {
        let raw = r#"{"api_key": "sk-live-should-not-appear", "q": "weather"}"#;
        let out = sanitize_tool_text("web_search", raw);
        assert!(out.contains("<redacted>"), "{out}");
        assert!(!out.contains("sk-live-should-not-appear"), "{out}");
        assert!(out.contains("weather"), "{out}");
    }

    #[test]
    fn bearer_and_prefixes_redacted() {
        let out = redact_secrets("Authorization: Bearer abcdefghijklmnop and sk-live-abcdefghijk");
        assert!(out.contains("<redacted>"), "{out}");
        assert!(!out.contains("abcdefghijklmnop"), "{out}");
        assert!(!out.contains("sk-live-abcdefghijk"), "{out}");
    }

    #[test]
    fn stderr_strips_secrets() {
        let line = stderr_log_line(r#"auth api_key="hunter2secretxx""#);
        assert!(line.starts_with("gw-stderr "));
        assert!(!line.contains("hunter2secretxx"), "{line}");
    }

    #[test]
    fn history_skips_bang_and_secrets() {
        assert!(should_skip_history("! pwd"));
        assert!(should_skip_history("  !rm -rf /"));
        assert!(should_skip_history("api_key=sk-live-abcdefghijk"));
        assert!(should_skip_history("password=hunter2"));
        assert!(!should_skip_history(
            "how do I reset a password in this app"
        ));
        assert!(!should_skip_history("please list the files"));
    }

    #[test]
    fn prompt_submit_rpc_redacts_body() {
        let body = serde_json::json!({ "session_id": "s1", "text": "here is sk-live-abcdefghijklmnopqrstuvwxyz" });
        let line = rpc_log_line("→", "prompt.submit", &body);
        assert_eq!(line, "→ prompt.submit <redacted>");
        assert!(!line.contains("sk-live"));
    }

    #[test]
    fn strip_controls_drops_csi_and_osc() {
        assert_eq!(strip_controls("hello\u{1b}[31mred"), "hellored");
        assert_eq!(strip_controls("x\u{1b}]0;evil-title\u{07}y"), "xy");
        assert_eq!(strip_controls("a\nb\tc"), "a\nb\tc");
        let link = strip_controls("go\u{1b}]8;;http://evil.example\u{07}here");
        assert!(!link.contains('\u{1b}'), "{link}");
        assert_eq!(link, "gohere");
    }

    #[test]
    fn preview_caps_at_240() {
        let long = "x".repeat(500);
        let p = preview(&long);
        assert!(p.ends_with('…'));
        assert!(p.chars().count() <= 241);
    }
}
