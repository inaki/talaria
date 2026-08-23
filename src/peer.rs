//! Detect another live Hermes TUI / talaria / `tui_gateway` before we spawn ours.
//!
//! Two gateways on one `HERMES_HOME` race the session SQLite. Mock/dev skips this.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::discover;
use crate::paths::{create_private_dir_all, ensure_private_file, TalariaPaths};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerKind {
    HermesTui,
    TuiGateway,
    Talaria,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub pid: u32,
    pub kind: PeerKind,
    pub summary: String,
}

impl PeerKind {
    pub fn label(self) -> &'static str {
        match self {
            PeerKind::HermesTui => "hermes --tui",
            PeerKind::TuiGateway => "tui_gateway",
            PeerKind::Talaria => "talaria",
        }
    }
}

fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes")
    )
}

pub fn allow_concurrent(force: bool) -> bool {
    if force {
        return true;
    }
    env_truthy("TALARIA_ALLOW_CONCURRENT") || env_truthy("HERMES_RUST_ALLOW_CONCURRENT")
}

pub fn find_conflicts() -> Vec<Peer> {
    let me = std::process::id();
    let mut out = Vec::new();
    for peer in list_from_ps() {
        if peer.pid != me {
            push_unique(&mut out, peer);
        }
    }
    if let Some(peer) = list_from_pid_file(me) {
        push_unique(&mut out, peer);
    }
    out
}

fn push_unique(out: &mut Vec<Peer>, peer: Peer) {
    if out.iter().any(|p| p.pid == peer.pid) {
        return;
    }
    out.push(peer);
}

pub fn classify_args(args: &str) -> Option<PeerKind> {
    let args = args.trim();
    if args.is_empty() {
        return None;
    }
    let lower = args.to_ascii_lowercase();
    if lower.contains("import tui_gateway") && !lower.contains("tui_gateway.entry") {
        return None;
    }
    let argv0 = args.split_whitespace().next().unwrap_or("");
    let base = std::path::Path::new(argv0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if is_talaria_bin(base) {
        return Some(PeerKind::Talaria);
    }
    if lower.contains("tui_gateway.entry") || lower.contains("-m tui_gateway") {
        return Some(PeerKind::TuiGateway);
    }
    if looks_like_hermes_tui(args, base, &lower) {
        return Some(PeerKind::HermesTui);
    }
    None
}

fn is_talaria_bin(base: &str) -> bool {
    base == "talaria"
        || base.starts_with("talaria-")
        || base == "hermes-rust"
        || base.starts_with("hermes-rust-")
}

fn looks_like_hermes_tui(args: &str, argv0_base: &str, lower: &str) -> bool {
    let has_tui_flag = args.split_whitespace().any(|t| t == "--tui");
    if has_tui_flag && (argv0_base == "hermes" || argv0_base == "hermes.exe") {
        return true;
    }
    if has_tui_flag && lower.contains("hermes-agent") {
        return true;
    }
    lower.contains("ui-tui") && (lower.contains("node") || lower.contains("tsx"))
}

fn list_from_ps() -> Vec<Peer> {
    let output = Command::new("ps")
        .args(["-ax", "-o", "pid=,args="])
        .output();
    let Ok(out) = output else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    parse_ps_table(&String::from_utf8_lossy(&out.stdout))
}

pub fn parse_ps_table(table: &str) -> Vec<Peer> {
    let mut out = Vec::new();
    for line in table.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((pid_s, args)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let Ok(pid) = pid_s.trim().parse::<u32>() else {
            continue;
        };
        let args = args.trim();
        let Some(kind) = classify_args(args) else {
            continue;
        };
        out.push(Peer {
            pid,
            kind,
            summary: truncate_args(args),
        });
    }
    out
}

fn truncate_args(args: &str) -> String {
    const MAX: usize = 80;
    if args.chars().count() <= MAX {
        args.to_string()
    } else {
        format!("{}…", args.chars().take(MAX).collect::<String>())
    }
}

fn pid_file() -> PathBuf {
    TalariaPaths::from_env().root.join("tui.pid")
}

fn list_from_pid_file(me: u32) -> Option<Peer> {
    let raw = fs::read_to_string(pid_file()).ok()?;
    let pid = raw.lines().next()?.trim().parse::<u32>().ok()?;
    if pid == me || pid == 0 {
        return None;
    }
    if !pid_is_alive(pid) {
        return None;
    }
    if let Some(args) = args_for_pid(pid) {
        let kind = classify_args(&args)?;
        return Some(Peer {
            pid,
            kind,
            summary: truncate_args(&args),
        });
    }
    Some(Peer {
        pid,
        kind: PeerKind::Talaria,
        summary: "talaria (pid file)".into(),
    })
}

fn args_for_pid(pid: u32) -> Option<String> {
    let out = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "args="])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn pid_is_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let rc = unsafe { libc::kill(pid as i32, 0) };
        rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// Held while this TUI is live. Removes a stale pid file on drop.
pub struct TuiLock {
    path: PathBuf,
    pid: u32,
}

impl TuiLock {
    pub fn acquire() -> Self {
        let path = pid_file();
        if let Some(parent) = path.parent() {
            create_private_dir_all(parent);
        }
        let pid = std::process::id();
        let _ = fs::write(&path, format!("{pid}\n"));
        ensure_private_file(&path);
        Self { path, pid }
    }
}

impl Drop for TuiLock {
    fn drop(&mut self) {
        if let Ok(raw) = fs::read_to_string(&self.path) {
            if raw.lines().next().and_then(|s| s.parse::<u32>().ok()) == Some(self.pid) {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

pub fn conflict_message(peers: &[Peer]) -> String {
    let home = discover::hermes_home();
    let mut s = format!(
        "Another Hermes Agent TUI is already using {}.\n\n",
        home.display()
    );
    for p in peers {
        s.push_str(&format!(
            "  pid {:>6}  {}  {}\n",
            p.pid,
            p.kind.label(),
            p.summary
        ));
    }
    s.push_str(
        "\nQuit that instance first, then start talaria.\n\
         Two gateways on one HERMES_HOME can corrupt the session database.\n\n\
         Override: talaria --force   or   TALARIA_ALLOW_CONCURRENT=1\n",
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_hermes_tui_and_gateway() {
        assert_eq!(
            classify_args("/Users/me/.local/bin/hermes --tui"),
            Some(PeerKind::HermesTui)
        );
        assert_eq!(
            classify_args("python3 -u -m tui_gateway.entry"),
            Some(PeerKind::TuiGateway)
        );
        assert_eq!(
            classify_args("/opt/homebrew/bin/talaria"),
            Some(PeerKind::Talaria)
        );
        assert_eq!(
            classify_args("target/debug/talaria --theme default"),
            Some(PeerKind::Talaria)
        );
        assert_eq!(
            classify_args("/opt/homebrew/bin/hermes-rust"),
            Some(PeerKind::Talaria)
        );
        assert_eq!(classify_args("python3 -c import tui_gateway"), None);
        assert_eq!(classify_args("vim README.md"), None);
        assert_eq!(
            classify_args("node /app/ui-tui/dist/index.js"),
            Some(PeerKind::HermesTui)
        );
    }

    #[test]
    fn parse_ps_skips_noise() {
        let table = "\
  11 /sbin/launchd
  42 /Users/me/.local/bin/hermes --tui
  99 python3 -c import tui_gateway
 100 python3 -u -m tui_gateway.entry
 101 /opt/homebrew/bin/talaria\n";
        let peers = parse_ps_table(table);
        assert_eq!(peers.len(), 3);
        assert_eq!(peers[0].pid, 42);
        assert_eq!(peers[0].kind, PeerKind::HermesTui);
        assert_eq!(peers[1].kind, PeerKind::TuiGateway);
        assert_eq!(peers[2].kind, PeerKind::Talaria);
    }

    #[test]
    fn conflict_message_names_home_and_override() {
        let msg = conflict_message(&[Peer {
            pid: 7,
            kind: PeerKind::HermesTui,
            summary: "hermes --tui".into(),
        }]);
        assert!(msg.contains("hermes --tui"), "{msg}");
        assert!(msg.contains("pid"));
        assert!(msg.contains("--force"));
        assert!(msg.contains("TALARIA_ALLOW_CONCURRENT"));
        assert!(msg.contains("talaria"));
    }
}
