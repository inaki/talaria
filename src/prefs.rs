//! Chrome prefs (`/custom`). Stored under `~/.talaria/custom`, never `~/.hermes`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::paths::{create_private_dir_all, ensure_private_file, TalariaPaths};

fn on() -> bool {
    true
}

fn off() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChromePrefs {
    #[serde(default = "on")]
    pub status_bar: bool,
    #[serde(default = "on")]
    pub key_hints: bool,
    /// Right-hand ambient rail. Opt-in: it costs columns the document wants.
    #[serde(default = "off")]
    pub rail: bool,
}

impl Default for ChromePrefs {
    fn default() -> Self {
        Self {
            status_bar: true,
            key_hints: true,
            rail: false,
        }
    }
}

static PREFS: RwLock<ChromePrefs> = RwLock::new(ChromePrefs {
    status_bar: true,
    key_hints: true,
    rail: false,
});

pub fn load() {
    let prefs = ChromePrefs::read_file(&prefs_path());
    if let Ok(mut g) = PREFS.write() {
        *g = prefs;
    }
}

pub fn get() -> ChromePrefs {
    PREFS.read().map(|g| *g).unwrap_or_default()
}

pub fn status_bar() -> bool {
    get().status_bar
}

pub fn key_hints() -> bool {
    get().key_hints
}

pub fn rail() -> bool {
    get().rail
}

pub fn toggle_rail() -> bool {
    let mut p = get();
    p.rail = !p.rail;
    replace(p);
    p.rail
}

pub fn toggle_status_bar() -> bool {
    let mut p = get();
    p.status_bar = !p.status_bar;
    replace(p);
    p.status_bar
}

pub fn toggle_key_hints() -> bool {
    let mut p = get();
    p.key_hints = !p.key_hints;
    replace(p);
    p.key_hints
}

fn replace(prefs: ChromePrefs) {
    if let Ok(mut g) = PREFS.write() {
        *g = prefs;
    }
    prefs.write_file(&prefs_path());
}

fn prefs_path() -> PathBuf {
    TalariaPaths::from_env().custom_file()
}

impl ChromePrefs {
    pub fn read_file(path: &Path) -> Self {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(raw.trim()).unwrap_or_default()
    }

    pub fn write_file(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            create_private_dir_all(parent);
        }
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).write(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let Ok(mut f) = opts.open(path) else {
            return;
        };
        ensure_private_file(path);
        if let Ok(body) = serde_json::to_string_pretty(self) {
            let _ = writeln!(f, "{body}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("talaria-prefs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn missing_file_is_all_on() {
        let p = temp_path("missing");
        assert_eq!(ChromePrefs::read_file(&p), ChromePrefs::default());
    }

    #[test]
    fn round_trip_and_partial() {
        let p = temp_path("round");
        ChromePrefs {
            status_bar: false,
            key_hints: true,
            rail: true,
        }
        .write_file(&p);
        let loaded = ChromePrefs::read_file(&p);
        assert!(!loaded.status_bar);
        assert!(loaded.key_hints);
        assert!(loaded.rail);

        std::fs::write(&p, "{\"status_bar\":false}").unwrap();
        let partial = ChromePrefs::read_file(&p);
        assert!(!partial.status_bar);
        assert!(partial.key_hints, "omitted key_hints stays on");
        assert!(
            !partial.rail,
            "rail is opt-in, so an old file leaves it off"
        );
        let _ = std::fs::remove_file(&p);
    }
}
