//! On-disk chrome for this host. Never writes under `~/.hermes`.
//!
//! Unix: `~/.talaria/`
//! Windows: `%LOCALAPPDATA%\talaria\`
//!
//! Path policy is hardcoded (no `dirs` crate). `dirs::state_dir()` on macOS
//! would land in `~/Library/Application Support`, which we do not want.
//!
//! A leftover `~/.hermes-rust` from the pre-rebrand name is renamed once
//! onto `~/.talaria` (see [`migrate_legacy_chrome`]).

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TalariaPaths {
    pub root: PathBuf,
}

impl TalariaPaths {
    pub fn from_env() -> Self {
        Self {
            root: default_root(),
        }
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn log_file(&self) -> PathBuf {
        self.logs_dir().join("talaria.log")
    }

    pub fn history_file(&self) -> PathBuf {
        self.root.join("history")
    }

    pub fn theme_file(&self) -> PathBuf {
        self.root.join("theme")
    }

    pub fn custom_file(&self) -> PathBuf {
        self.root.join("custom")
    }

    pub fn last_copy_file(&self) -> PathBuf {
        self.root.join("last-copy.txt")
    }
}

impl Default for TalariaPaths {
    fn default() -> Self {
        Self::from_env()
    }
}

fn default_root() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            if !local.is_empty() {
                return PathBuf::from(local).join("talaria");
            }
        }
        if let Ok(home) = std::env::var("USERPROFILE") {
            return PathBuf::from(home).join("talaria");
        }
        PathBuf::from(r"C:\talaria")
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".talaria")
    }
}

fn legacy_root() -> PathBuf {
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

/// If `~/.talaria` is missing and `~/.hermes-rust` exists, rename it.
/// No-op when the new dir already exists. Never touches `~/.hermes`.
pub fn migrate_legacy_chrome() {
    migrate_legacy_dir(&legacy_root(), &default_root());
}

fn migrate_legacy_dir(old: &Path, new: &Path) {
    if new.exists() || !old.is_dir() {
        return;
    }
    if let Some(parent) = new.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::rename(old, new);
}

/// Best-effort 0600 on an existing file. `OpenOptions.mode` only applies to create.
pub fn ensure_private_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let Ok(meta) = std::fs::metadata(path) else {
            return;
        };
        if !meta.is_file() {
            return;
        }
        if meta.permissions().mode() & 0o777 == 0o600 {
            return;
        }
        let mut perms = meta.permissions();
        perms.set_mode(0o600);
        let _ = std::fs::set_permissions(path, perms);
    }
    #[cfg(not(unix))]
    {
        let _ = path;
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
    fn unix_root_is_dot_talaria() {
        #[cfg(not(windows))]
        {
            let p = TalariaPaths::from_env();
            assert!(p.root.ends_with(".talaria"));
            assert!(p.log_file().ends_with("logs/talaria.log"));
            assert!(p.custom_file().ends_with("custom"));
            assert!(p.last_copy_file().ends_with("last-copy.txt"));
        }
    }

    #[test]
    fn migrate_renames_legacy_once() {
        let pid = std::process::id();
        let base = std::env::temp_dir().join(format!("talaria-migrate-{pid}"));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let old = base.join("hermes-rust");
        let new = base.join("talaria");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("theme"), "github").unwrap();

        migrate_legacy_dir(&old, &new);
        assert!(new.join("theme").is_file());
        assert!(!old.exists());

        std::fs::write(new.join("theme"), "keep").unwrap();
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("theme"), "stale").unwrap();
        migrate_legacy_dir(&old, &new);
        assert_eq!(std::fs::read_to_string(new.join("theme")).unwrap(), "keep");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn ensure_private_file_tightens_mode() {
        use std::os::unix::fs::PermissionsExt;
        let p =
            std::env::temp_dir().join(format!("talaria-mode-{}-{}", std::process::id(), "hist"));
        std::fs::write(&p, "x").unwrap();
        let mut perms = std::fs::metadata(&p).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&p, perms).unwrap();
        assert_eq!(
            std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o644
        );
        ensure_private_file(&p);
        assert_eq!(
            std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let _ = std::fs::remove_file(&p);
    }
}
