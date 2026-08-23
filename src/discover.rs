//! Find a Python that can `import tui_gateway`.
//!
//! Never spawn `python -m tui_gateway.entry` as a probe — that starts MCP
//! discovery and can write `~/.hermes`. Official Ink `resolvePython` also
//! never boots the gateway to see if Python works.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct DiscoveredPython {
    pub python: PathBuf,
    /// Set only when we know a Hermes source/install root (for PYTHONPATH).
    pub src_root: Option<PathBuf>,
    pub tried: Vec<String>,
}

#[derive(Debug, Error)]
pub struct DiscoveryError {
    pub tried: Vec<String>,
    pub last_error: Option<String>,
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "no Python on PATH could import tui_gateway")
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    python: PathBuf,
    src_root: Option<PathBuf>,
    why: String,
}

/// Locate an interpreter that imports `tui_gateway`.
pub fn discover() -> Result<DiscoveredPython, DiscoveryError> {
    let candidates = candidates();
    let mut tried = Vec::new();
    let mut last_error = None;

    for c in candidates {
        tried.push(c.why.clone());
        match import_probe(&c.python, c.src_root.as_deref()) {
            Ok(()) => {
                return Ok(DiscoveredPython {
                    python: c.python,
                    src_root: c.src_root,
                    tried,
                });
            }
            Err(e) => last_error = Some(e),
        }
    }

    Err(DiscoveryError { tried, last_error })
}

pub(crate) fn candidates() -> Vec<Candidate> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let push =
        |out: &mut Vec<Candidate>, seen: &mut std::collections::HashSet<PathBuf>, c: Candidate| {
            if seen.insert(c.python.clone()) {
                out.push(c);
            }
        };

    if let Ok(p) = std::env::var("HERMES_PYTHON") {
        if !p.is_empty() {
            push(
                &mut out,
                &mut seen,
                Candidate {
                    python: PathBuf::from(&p),
                    src_root: src_root_from_env(),
                    why: format!("HERMES_PYTHON={p}"),
                },
            );
        }
    }
    if let Ok(p) = std::env::var("PYTHON") {
        if !p.is_empty() {
            push(
                &mut out,
                &mut seen,
                Candidate {
                    python: PathBuf::from(&p),
                    src_root: src_root_from_env(),
                    why: format!("PYTHON={p}"),
                },
            );
        }
    }

    if let Ok(venv) = std::env::var("VIRTUAL_ENV") {
        if !venv.is_empty() {
            let python = venv_python(&venv);
            push(
                &mut out,
                &mut seen,
                Candidate {
                    python,
                    src_root: src_root_from_env(),
                    why: format!("VIRTUAL_ENV={venv}"),
                },
            );
        }
    }

    if let Some(root) = src_root_from_env() {
        push_venv(&mut out, &mut seen, &root, "HERMES_PYTHON_SRC_ROOT");
    }

    // Official install: ~/.local/bin/hermes is a launcher whose shebang (or
    // `exec ".../venv/bin/python"`) is the interpreter that can import tui_gateway.
    if let Ok(hermes) = which::which("hermes") {
        crate::logging::log_line(&format!("discover: hermes launcher {}", hermes.display()));
        if let Some((python, root)) = python_from_hermes_launcher(&hermes) {
            push(
                &mut out,
                &mut seen,
                Candidate {
                    python,
                    src_root: root,
                    why: format!("shebang/exec from {}", hermes.display()),
                },
            );
        }
    }

    // Official git installer layout even if `hermes` is not on this shell's PATH.
    //   code: $HERMES_HOME/hermes-agent
    //   venv: $HERMES_HOME/hermes-agent/venv  (or .venv)
    //   data: $HERMES_HOME  (usually ~/.hermes)
    let home = hermes_home();
    let agent = home.join("hermes-agent");
    push_venv(
        &mut out,
        &mut seen,
        &agent,
        &format!("HERMES_HOME {}", home.display()),
    );
    push_venv(
        &mut out,
        &mut seen,
        &home.join("venvs").join("hermes-dev"),
        "HERMES_HOME/venvs/hermes-dev",
    );

    let fallback = if cfg!(windows) { "python" } else { "python3" };
    if let Ok(p) = which::which(fallback) {
        push(
            &mut out,
            &mut seen,
            Candidate {
                python: p,
                src_root: src_root_from_env(),
                why: format!("which {fallback}"),
            },
        );
    } else {
        push(
            &mut out,
            &mut seen,
            Candidate {
                python: PathBuf::from(fallback),
                src_root: src_root_from_env(),
                why: format!("fallback {fallback}"),
            },
        );
    }

    // Last-ditch: `python` on Unix too.
    if !cfg!(windows) {
        if let Ok(p) = which::which("python") {
            push(
                &mut out,
                &mut seen,
                Candidate {
                    python: p,
                    src_root: src_root_from_env(),
                    why: "which python".into(),
                },
            );
        }
    }

    out
}

fn src_root_from_env() -> Option<PathBuf> {
    std::env::var("HERMES_PYTHON_SRC_ROOT")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

fn hermes_home() -> PathBuf {
    std::env::var("HERMES_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs_home()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".hermes")
        })
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn push_venv(
    out: &mut Vec<Candidate>,
    seen: &mut std::collections::HashSet<PathBuf>,
    root: &Path,
    why: &str,
) {
    for name in ["venv", ".venv"] {
        let python = venv_python_in(&root.join(name));
        if python.exists() {
            if seen.insert(python.clone()) {
                out.push(Candidate {
                    python,
                    src_root: Some(root.to_path_buf()),
                    why: format!("{why} / {name}"),
                });
            }
        }
    }
    // Root itself might already be a venv (…/venvs/hermes-dev).
    let python = venv_python_in(root);
    if python.exists() && seen.insert(python.clone()) {
        let src = root
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("hermes-agent"));
        out.push(Candidate {
            python,
            src_root: src
                .filter(|p| p.exists())
                .or_else(|| Some(root.to_path_buf())),
            why: why.to_string(),
        });
    }
}

/// Read the `hermes` launcher. Console scripts start with `#!/path/to/venv/python`.
/// Installer wrappers `exec "$INSTALL_DIR/venv/bin/python" …`.
fn python_from_hermes_launcher(hermes: &Path) -> Option<(PathBuf, Option<PathBuf>)> {
    let text = std::fs::read_to_string(hermes).ok()?;
    let first = text.lines().next().unwrap_or("");
    if let Some(rest) = first.strip_prefix("#!") {
        let rest = rest.trim();
        if !rest.contains("/env ") && rest.contains("python") {
            let python = PathBuf::from(rest.split_whitespace().next().unwrap_or(rest));
            if python.exists() {
                let root = python
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                    .map(|p| p.to_path_buf());
                return Some((python, root));
            }
        }
    }
    for line in text.lines() {
        let line = line.trim();
        if let Some(idx) = line.find("/venv/bin/python") {
            let start = line[..idx].rfind('"').map(|i| i + 1).unwrap_or(0);
            let end = idx + "/venv/bin/python".len();
            let python = PathBuf::from(&line[start..end]);
            if python.exists() {
                let root = python.parent()?.parent()?.parent()?.to_path_buf();
                return Some((python, Some(root)));
            }
        }
        if let Some(idx) = line.find("/.venv/bin/python") {
            let start = line[..idx].rfind('"').map(|i| i + 1).unwrap_or(0);
            let end = idx + "/.venv/bin/python".len();
            let python = PathBuf::from(&line[start..end]);
            if python.exists() {
                let root = python.parent()?.parent()?.parent()?.to_path_buf();
                return Some((python, Some(root)));
            }
        }
    }
    None
}

fn venv_python(venv: &str) -> PathBuf {
    venv_python_in(Path::new(venv))
}

fn venv_python_in(venv: &Path) -> PathBuf {
    if cfg!(windows) {
        venv.join("Scripts").join("python.exe")
    } else {
        venv.join("bin").join("python")
    }
}

/// `python -c "import tui_gateway"` with a short timeout. No gateway spawn.
pub fn import_probe(python: &Path, src_root: Option<&Path>) -> Result<(), String> {
    let mut cmd = Command::new(python);
    cmd.arg("-c")
        .arg("import tui_gateway")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(root) = src_root {
        cmd.env("PYTHONPATH", root);
        cmd.env("HERMES_PYTHON_SRC_ROOT", root);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("{}: {e}", python.display()))?;

    let timeout = Duration::from_secs(8);
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                }
                let stderr = child
                    .stderr
                    .as_mut()
                    .and_then(|s| {
                        use std::io::Read;
                        let mut buf = String::new();
                        s.read_to_string(&mut buf).ok()?;
                        Some(buf)
                    })
                    .unwrap_or_default();
                let preview = crate::logging::preview(stderr.trim());
                return Err(format!(
                    "{} exited {}: {preview}",
                    python.display(),
                    status.code().unwrap_or(-1)
                ));
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("{}: import probe timed out", python.display()));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(format!("{}: {e}", python.display())),
        }
    }
}

impl DiscoveredPython {
    pub fn spawn_args(&self) -> crate::gateway::SpawnOptions {
        let mut extra_env = std::collections::HashMap::new();
        extra_env.insert("PYTHONUNBUFFERED".into(), "1".into());
        if let Some(root) = &self.src_root {
            extra_env.insert("PYTHONPATH".into(), root.display().to_string());
            extra_env.insert("HERMES_PYTHON_SRC_ROOT".into(), root.display().to_string());
        }
        crate::gateway::SpawnOptions {
            python: self.python.clone(),
            args: vec!["-u".into(), "-m".into(), "tui_gateway.entry".into()],
            extra_env,
            cwd: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn hermes_python_is_first_candidate() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("HERMES_PYTHON", "/tmp/fake-hermes-python");
        let cs = candidates();
        std::env::remove_var("HERMES_PYTHON");
        assert_eq!(cs[0].python, PathBuf::from("/tmp/fake-hermes-python"));
        assert!(cs[0].why.contains("HERMES_PYTHON"));
    }

    #[test]
    fn hermes_home_venv_is_a_candidate() {
        let _g = ENV_LOCK.lock().unwrap();
        let tmp = std::env::temp_dir().join(format!(
            "hermes-rust-disc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let py = tmp.join("hermes-agent/venv/bin/python");
        std::fs::create_dir_all(py.parent().unwrap()).unwrap();
        std::fs::write(&py, "").unwrap();
        std::env::set_var("HERMES_HOME", &tmp);
        std::env::remove_var("HERMES_PYTHON");
        let cs = candidates();
        std::env::remove_var("HERMES_HOME");
        let _ = std::fs::remove_dir_all(&tmp);
        assert!(
            cs.iter()
                .any(|c| c.python == py && c.why.contains("HERMES_HOME")),
            "candidates were {cs:?}"
        );
    }

    #[test]
    fn launcher_shebang_extracts_python() {
        let tmp = std::env::temp_dir().join(format!("hermes-rust-launch-{}", std::process::id()));
        let venv_py = tmp.join("hermes-agent/venv/bin/python");
        std::fs::create_dir_all(venv_py.parent().unwrap()).unwrap();
        std::fs::write(&venv_py, "").unwrap();
        let launcher = tmp.join("hermes");
        std::fs::write(
            &launcher,
            format!("#!{}\n# hermes wrapper\n", venv_py.display()),
        )
        .unwrap();
        let (py, root) = python_from_hermes_launcher(&launcher).expect("shebang");
        let _ = std::fs::remove_dir_all(&tmp);
        assert_eq!(py, venv_py);
        assert!(root.unwrap().ends_with("hermes-agent"));
    }

    #[test]
    fn import_probe_does_not_spawn_entry() {
        let python = if cfg!(windows) { "python" } else { "python3" };
        let _ = import_probe(Path::new(python), None);
    }
}
