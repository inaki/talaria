//! Optional GitHub Releases check so the TUI can say a newer talaria exists.
//!
//! Never auto-upgrades. Failures are silent. Cache 24h under `~/.talaria`.
//! Skip with `TALARIA_NO_UPDATE_CHECK=1`.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::paths::{create_private_dir_all, ensure_private_file, TalariaPaths};

const CURRENT: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_REPO: &str = "inaki/talaria";
const CACHE_MAX_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Cache {
    checked_unix: u64,
    latest: String,
}

pub fn disabled() -> bool {
    matches!(
        std::env::var("TALARIA_NO_UPDATE_CHECK").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes")
    )
}

/// Latest version newer than this binary, if GitHub (or cache) says so.
pub fn check_blocking() -> Option<String> {
    if disabled() {
        return None;
    }
    let current = CURRENT;
    if let Some(cache) = load_cache() {
        let age = now_unix().saturating_sub(cache.checked_unix);
        if age < CACHE_MAX_SECS {
            return newer(current, &cache.latest).then_some(cache.latest);
        }
    }
    let latest = fetch_latest().or_else(|| load_cache().map(|c| c.latest))?;
    save_cache(&latest);
    newer(current, &latest).then_some(latest)
}

pub fn notice_line(latest: &str) -> String {
    format!("Talaria {latest} is out · brew upgrade / curl | bash")
}

pub fn newer(current: &str, latest: &str) -> bool {
    match (parse_ver(current), parse_ver(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

fn parse_ver(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches('v');
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn repo() -> String {
    std::env::var("TALARIA_REPO")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_REPO.to_string())
}

fn fetch_latest() -> Option<String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo());
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "4",
            "-A",
            concat!("talaria/", env!("CARGO_PKG_VERSION")),
            &url,
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let tag = v.get("tag_name")?.as_str()?;
    let ver = tag.trim().trim_start_matches('v').to_string();
    if ver.is_empty() {
        None
    } else {
        Some(ver)
    }
}

fn cache_path() -> std::path::PathBuf {
    TalariaPaths::from_env().root.join("update-check")
}

fn load_cache() -> Option<Cache> {
    let raw = std::fs::read_to_string(cache_path()).ok()?;
    serde_json::from_str(raw.trim()).ok()
}

fn save_cache(latest: &str) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        create_private_dir_all(parent);
    }
    let body = Cache {
        checked_unix: now_unix(),
        latest: latest.to_string(),
    };
    if let Ok(json) = serde_json::to_string(&body) {
        let _ = std::fs::write(&path, json);
        ensure_private_file(&path);
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_compares_semver() {
        assert!(newer("0.1.0", "0.1.1"));
        assert!(newer("0.1.1", "0.2.0"));
        assert!(!newer("0.1.1", "0.1.1"));
        assert!(!newer("0.1.1", "0.1.0"));
        assert!(newer("v0.1.0", "v0.1.1"));
    }

    #[test]
    fn notice_names_latest_and_install() {
        let s = notice_line("0.1.1");
        assert!(s.contains("0.1.1"), "{s}");
        assert!(s.contains("brew"), "{s}");
        assert!(s.contains("curl"), "{s}");
    }
}
