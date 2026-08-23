//! On-disk state for this host. Never writes under `~/.hermes`.
//!
//! Unix: `~/.hermes-rust/`
//! Windows: `%LOCALAPPDATA%\hermes-rust\`
//!
//! Path policy is hardcoded (no `dirs` crate). `dirs::state_dir()` on macOS
//! would land in `~/Library/Application Support`, which we do not want.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct HermesRustPaths {
    pub root: PathBuf,
}

impl HermesRustPaths {
    pub fn from_env() -> Self {
        Self {
            root: default_root(),
        }
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn log_file(&self) -> PathBuf {
        self.logs_dir().join("hermes-rust.log")
    }

    pub fn history_file(&self) -> PathBuf {
        self.root.join("history")
    }
}

impl Default for HermesRustPaths {
    fn default() -> Self {
        Self::from_env()
    }
}

fn default_root() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            if !local.is_empty() {
                return PathBuf::from(local).join("hermes-rust");
            }
        }
        if let Ok(home) = std::env::var("USERPROFILE") {
            return PathBuf::from(home).join("hermes-rust");
        }
        PathBuf::from(r"C:\hermes-rust")
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".hermes-rust")
    }
}

/// Best-effort 0700 directory. Logging must never fail the process.
pub fn create_private_dir_all(path: &Path) {
    let _ = std::fs::create_dir_all(path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o700);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_root_is_dot_hermes_rust() {
        #[cfg(not(windows))]
        {
            let p = HermesRustPaths::from_env();
            assert!(p.root.ends_with(".hermes-rust"));
            assert!(p.log_file().ends_with("logs/hermes-rust.log"));
        }
    }
}
