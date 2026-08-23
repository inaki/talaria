//! Copy text out of the TUI: OSC 52 + native clipboard + `~/.talaria/last-copy.txt`.
//!
//! Same contract as Grok Build `/copy`: always write a backup file; toast `Copied!`
//! when a native clipboard tool confirms; otherwise name the backup path so SSH
//! sessions can still recover the text. `TALARIA_COPY_FILE` overrides the backup.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::paths::{create_private_dir_all, ensure_private_file, TalariaPaths};

const OSC52_MAX: usize = 100_000;

#[derive(Debug, Clone)]
pub struct CopyResult {
    pub verified: bool,
    pub backup: PathBuf,
    pub dest_file: Option<PathBuf>,
}

impl CopyResult {
    pub fn toast(&self) -> String {
        if let Some(path) = &self.dest_file {
            return format!("Wrote {}", display_path(path));
        }
        if self.verified {
            "Copied!".into()
        } else {
            format!("Copied · {}", display_path(&self.backup))
        }
    }
}

/// `/copy` args: `/copy`, `/copy 2`, `/copy out.txt`, `/copy 2 ~/exports/last-reply.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyArgs {
    /// 1-based index into latest-first assistant responses.
    pub n: usize,
    pub path: Option<String>,
}

pub fn parse_copy_args(rest: &str) -> CopyArgs {
    let rest = rest.trim();
    if rest.is_empty() {
        return CopyArgs { n: 1, path: None };
    }
    if let Some((first, more)) = rest.split_once(char::is_whitespace) {
        if let Ok(n) = first.parse::<usize>() {
            let path = more.trim();
            return CopyArgs {
                n: n.max(1),
                path: if path.is_empty() {
                    None
                } else {
                    Some(path.to_string())
                },
            };
        }
    } else if let Ok(n) = rest.parse::<usize>() {
        return CopyArgs {
            n: n.max(1),
            path: None,
        };
    }
    CopyArgs {
        n: 1,
        path: Some(rest.to_string()),
    }
}

pub fn last_copy_path() -> PathBuf {
    if let Ok(p) = std::env::var("TALARIA_COPY_FILE") {
        if !p.is_empty() {
            return expand_tilde(&p);
        }
    }
    TalariaPaths::from_env().last_copy_file()
}

pub fn copy_text(text: &str) -> CopyResult {
    let backup = write_backup(text);
    let verified = copy_native(text);
    let _ = write_osc52(text);
    CopyResult {
        verified,
        backup,
        dest_file: None,
    }
}

pub fn copy_to_file(text: &str, path: &str) -> Result<CopyResult, String> {
    let dest = expand_tilde(path);
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
        }
    }
    std::fs::write(&dest, text).map_err(|e| format!("write: {e}"))?;
    let backup = write_backup(text);
    Ok(CopyResult {
        verified: true,
        backup,
        dest_file: Some(dest),
    })
}

pub fn expand_tilde(p: &str) -> PathBuf {
    if p == "~" {
        return home_dir();
    }
    if let Some(rest) = p.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    PathBuf::from(p)
}

fn write_backup(text: &str) -> PathBuf {
    let path = last_copy_path();
    if let Some(parent) = path.parent() {
        create_private_dir_all(parent);
    }
    let _ = std::fs::write(&path, text);
    ensure_private_file(&path);
    path
}

fn copy_native(text: &str) -> bool {
    if cfg!(target_os = "macos") {
        return pipe_to(&["pbcopy"], text);
    }
    if cfg!(target_os = "windows") {
        return pipe_to(&["clip"], text);
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_some() && pipe_to(&["wl-copy"], text) {
        return true;
    }
    if pipe_to(&["xclip", "-selection", "clipboard"], text) {
        return true;
    }
    pipe_to(&["xsel", "--clipboard", "--input"], text)
}

fn pipe_to(argv: &[&str], text: &str) -> bool {
    let Some((bin, args)) = argv.split_first() else {
        return false;
    };
    let mut child = match Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(text.as_bytes()).is_err() {
            return false;
        }
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}

fn write_osc52(text: &str) -> bool {
    if text.len() > OSC52_MAX {
        return false;
    }
    let seq = osc52_sequence(&b64(text.as_bytes()));
    let mut out = std::io::stdout();
    out.write_all(seq.as_bytes()).is_ok() && out.flush().is_ok()
}

pub(crate) fn osc52_sequence(b64: &str) -> String {
    let inner = format!("\x1b]52;c;{b64}\x07");
    if std::env::var_os("TMUX").is_some() {
        format!("\x1bPtmux;\x1b{inner}\x1b\\")
    } else {
        inner
    }
}

pub(crate) fn b64(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().saturating_add(2) / 3 * 4);
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i];
        let b1 = if i + 1 < data.len() { data[i + 1] } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] } else { 0 };
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if i + 1 < data.len() {
            out.push(T[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < data.len() {
            out.push(T[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}

fn home_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(home) = std::env::var("USERPROFILE") {
            if !home.is_empty() {
                return PathBuf::from(home);
            }
        }
        PathBuf::from(".")
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

fn display_path(path: &Path) -> String {
    let s = path.display().to_string();
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            if let Some(rest) = s.strip_prefix(&home) {
                return format!("~{rest}");
            }
        }
    }
    s
}

/// Display-column slice `[from, to)` of `s`. `to == u16::MAX` means end of line.
pub fn slice_display_cols(s: &str, from: u16, to: u16) -> String {
    let mut col = 0u16;
    let mut out = String::new();
    for ch in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
        if cw == 0 {
            if col >= from && col < to {
                out.push(ch);
            }
            continue;
        }
        if col >= to {
            break;
        }
        if col >= from {
            out.push(ch);
        }
        col = col.saturating_add(cw);
    }
    out
}

/// Inclusive mouse cells `(row, col)` → plain text across wrapped visual rows.
pub fn extract_selection(rows: &[String], start: (u16, u16), end: (u16, u16)) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let (s, e) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    let sr = s.0 as usize;
    let er = e.0 as usize;
    let sc = s.1;
    let ec = e.1.saturating_add(1);
    if sr >= rows.len() {
        return String::new();
    }
    if sr == er {
        return slice_display_cols(&rows[sr], sc, ec);
    }
    let mut out = slice_display_cols(&rows[sr], sc, u16::MAX);
    let last = er.min(rows.len().saturating_sub(1));
    for row in &rows[sr + 1..last] {
        out.push('\n');
        out.push_str(&slice_display_cols(row, 0, u16::MAX));
    }
    if last > sr && last < rows.len() {
        out.push('\n');
        out.push_str(&slice_display_cols(&rows[last], 0, ec));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_args_match_grok() {
        assert_eq!(parse_copy_args(""), CopyArgs { n: 1, path: None });
        assert_eq!(parse_copy_args("2"), CopyArgs { n: 2, path: None });
        assert_eq!(
            parse_copy_args("out.txt"),
            CopyArgs {
                n: 1,
                path: Some("out.txt".into())
            }
        );
        assert_eq!(
            parse_copy_args("2 ~/exports/last-reply.md"),
            CopyArgs {
                n: 2,
                path: Some("~/exports/last-reply.md".into())
            }
        );
        assert_eq!(parse_copy_args("0"), CopyArgs { n: 1, path: None });
    }

    #[test]
    fn b64_known_vectors() {
        assert_eq!(b64(b""), "");
        assert_eq!(b64(b"Man"), "TWFu");
        assert_eq!(b64(b"Ma"), "TWE=");
        assert_eq!(b64(b"M"), "TQ==");
        assert_eq!(b64(b"hello"), "aGVsbG8=");
    }

    #[test]
    fn osc52_is_bel_terminated() {
        let seq = {
            let inner = format!("\x1b]52;c;TQ==\x07");
            let _ = inner;
            osc52_sequence("TQ==")
        };
        assert!(seq.contains("]52;c;TQ=="), "{seq:?}");
        assert!(seq.contains('\u{07}'), "{seq:?}");
    }

    #[test]
    fn selection_extracts_inclusive_cells() {
        let rows = vec!["abcdef".into(), "ghijkl".into()];
        assert_eq!(extract_selection(&rows, (0, 1), (0, 3)), "bcd");
        assert_eq!(extract_selection(&rows, (0, 4), (1, 1)), "ef\ngh");
        assert_eq!(extract_selection(&rows, (1, 1), (0, 4)), "ef\ngh");
        assert_eq!(extract_selection(&rows, (0, 0), (0, 0)), "a");
    }

    #[test]
    fn tilde_expands_to_home() {
        let p = expand_tilde("~/exports/last-reply.md");
        assert!(p.ends_with("exports/last-reply.md"), "{p:?}");
    }
}
