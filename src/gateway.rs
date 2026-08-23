//! Stdio JSON-RPC client for `python -m tui_gateway.entry`.
//!
//! `kill_on_drop` is the panic net. Happy-path teardown is [`GatewayClient::shutdown`]:
//! optional `process.stop`, close stdin (genuine EOF), wait
//! `HERMES_TUI_GATEWAY_SHUTDOWN_GRACE_S` (default 1s), then kill+reap.
//!
//! Do not treat the first empty stdout read as death — `entry.py` has
//! `handle_spurious_eof` because MCP children can look like stdin EOF. We wait
//! for the child `wait()` (or shutdown) after stdout closes.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::{timeout, Instant};

use crate::logging::{self, log_line, log_rpc, preview};
use crate::protocol::{
    classify, make_request, Incoming, WireEvent, MAX_STDERR_LINE, MAX_STDOUT_LINE,
};

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("failed to spawn python: {0}")]
    Spawn(std::io::Error),
    #[error("gateway did not send gateway.ready in time")]
    StartupTimeout { stderr_tail: String },
    #[error("gateway process exited")]
    ChildExited {
        code: Option<i32>,
        stderr_tail: String,
    },
    #[error("rpc `{method}` failed: {message}")]
    Rpc { method: String, message: String },
    #[error("rpc `{method}` timed out")]
    Timeout { method: String },
    #[error("gateway connection closed")]
    Closed,
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub enum GatewayEvent {
    Event(WireEvent),
    ProtocolError { preview: String },
    Stderr { line: String },
    ChildExited { code: Option<i32> },
}

#[derive(Debug, Clone)]
pub struct SpawnOptions {
    pub python: PathBuf,
    pub args: Vec<String>,
    pub extra_env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
}

impl Default for SpawnOptions {
    fn default() -> Self {
        Self {
            python: PathBuf::from(if cfg!(windows) { "python" } else { "python3" }),
            args: vec!["-u".into(), "-m".into(), "tui_gateway.entry".into()],
            extra_env: {
                let mut m = HashMap::new();
                m.insert("PYTHONUNBUFFERED".into(), "1".into());
                m
            },
            cwd: None,
        }
    }
}

struct Inner {
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    next_id: AtomicU64,
    pending: Mutex<HashMap<String, oneshot::Sender<Result<Value, GatewayError>>>>,
    events_tx: mpsc::Sender<GatewayEvent>,
    stderr_tail: Mutex<VecDeque<String>>,
    shutting_down: AtomicBool,
    ready: AtomicBool,
    ready_tx: Mutex<Option<oneshot::Sender<()>>>,
}

pub struct GatewayClient {
    inner: Arc<Inner>,
    events_rx: Option<mpsc::Receiver<GatewayEvent>>,
}

impl GatewayClient {
    /// Spawn the child and wait for `gateway.ready` (startup timeout).
    pub async fn spawn(opts: SpawnOptions) -> Result<Self, GatewayError> {
        let (events_tx, events_rx) = mpsc::channel(100);

        let mut cmd = Command::new(&opts.python);
        cmd.args(&opts.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // Child ignores SIGINT. New process group so a cooked-mode Ctrl+C
        // (dump example) is not delivered to Python. In the TUI, raw mode
        // already turns Ctrl+C into a key.
        #[cfg(unix)]
        {
            cmd.process_group(0);
        }
        for (k, v) in &opts.extra_env {
            cmd.env(k, v);
        }
        if let Some(cwd) = &opts.cwd {
            cmd.current_dir(cwd);
        }

        log_line(&format!(
            "spawn {} {}",
            opts.python.display(),
            opts.args.join(" ")
        ));

        let mut child = cmd.spawn().map_err(GatewayError::Spawn)?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let (ready_tx, ready_rx) = oneshot::channel();
        let inner = Arc::new(Inner {
            child: Mutex::new(Some(child)),
            stdin: Mutex::new(stdin),
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            events_tx: events_tx.clone(),
            stderr_tail: Mutex::new(VecDeque::with_capacity(32)),
            shutting_down: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            ready_tx: Mutex::new(Some(ready_tx)),
        });

        if let Some(stdout) = stdout {
            tokio::spawn(read_stdout(inner.clone(), stdout));
        }
        if let Some(stderr) = stderr {
            tokio::spawn(read_stderr(inner.clone(), stderr));
        }
        tokio::spawn(wait_child(inner.clone()));

        let client = GatewayClient {
            inner: inner.clone(),
            events_rx: Some(events_rx),
        };

        match timeout(startup_timeout(), ready_rx).await {
            Ok(Ok(())) => {
                inner.ready.store(true, Ordering::SeqCst);
                Ok(client)
            }
            Ok(Err(_)) => {
                let tail = client.stderr_tail_text().await;
                let _ = client.shutdown().await;
                Err(GatewayError::ChildExited {
                    code: None,
                    stderr_tail: tail,
                })
            }
            Err(_) => {
                let tail = client.stderr_tail_text().await;
                let _ = client.shutdown().await;
                Err(GatewayError::StartupTimeout { stderr_tail: tail })
            }
        }
    }

    pub fn take_events(&mut self) -> Option<mpsc::Receiver<GatewayEvent>> {
        self.events_rx.take()
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value, GatewayError> {
        if self.inner.shutting_down.load(Ordering::SeqCst) {
            return Err(GatewayError::Closed);
        }
        let n = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        let id = format!("r{n}");
        let req = make_request(&id, method, params.clone());
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.inner.pending.lock().await;
            pending.insert(id.clone(), tx);
        }
        log_rpc("→", method, &params);
        if let Err(e) = write_frame(&self.inner, &req).await {
            self.inner.pending.lock().await.remove(&id);
            return Err(e);
        }
        match timeout(rpc_timeout(), rx).await {
            Ok(Ok(result)) => match result {
                Ok(v) => {
                    log_rpc("←", method, &v);
                    Ok(v)
                }
                Err(e) => Err(e),
            },
            Ok(Err(_)) => Err(GatewayError::Closed),
            Err(_) => {
                self.inner.pending.lock().await.remove(&id);
                Err(GatewayError::Timeout {
                    method: method.into(),
                })
            }
        }
    }

    /// Happy-path teardown. Takes the `Child` so Drop/`kill_on_drop` does not
    /// race the grace wait.
    pub async fn shutdown(&self) -> Result<(), GatewayError> {
        if self.inner.shutting_down.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        log_line("gateway shutdown begin");
        // Best-effort process.stop — do not wait the full RPC timeout.
        let stop = timeout(Duration::from_millis(400), async {
            self.request_during_shutdown("process.stop", json!({}))
                .await
        })
        .await;
        match stop {
            Ok(Ok(_)) => log_line("process.stop ok"),
            Ok(Err(e)) => log_line(&format!("process.stop: {e}")),
            Err(_) => log_line("process.stop timed out"),
        }

        // Genuine stdin EOF.
        {
            let mut stdin = self.inner.stdin.lock().await;
            *stdin = None;
        }

        let grace = shutdown_grace();
        let start = Instant::now();
        while start.elapsed() < grace {
            let mut slot = self.inner.child.lock().await;
            if let Some(child) = slot.as_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        *slot = None;
                        log_line(&format!("child exited during grace: {status:?}"));
                        self.reject_pending(GatewayError::Closed).await;
                        return Ok(());
                    }
                    Ok(None) => {}
                    Err(e) => {
                        log_line(&format!("try_wait: {e}"));
                        break;
                    }
                }
            } else {
                self.reject_pending(GatewayError::Closed).await;
                return Ok(());
            }
            drop(slot);
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        if let Some(mut child) = self.inner.child.lock().await.take() {
            log_line("grace elapsed; start_kill");
            let _ = child.start_kill();
            let _ = timeout(Duration::from_secs(2), child.wait()).await;
        }
        self.reject_pending(GatewayError::Closed).await;
        log_line("gateway shutdown done");
        Ok(())
    }

    async fn request_during_shutdown(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, GatewayError> {
        // shutting_down is already true; bypass the Closed check.
        let n = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        let id = format!("r{n}");
        let req = make_request(&id, method, params);
        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().await.insert(id.clone(), tx);
        write_frame(&self.inner, &req).await?;
        timeout(Duration::from_millis(350), rx)
            .await
            .map_err(|_| GatewayError::Timeout {
                method: method.into(),
            })?
            .map_err(|_| GatewayError::Closed)?
    }

    async fn stderr_tail_text(&self) -> String {
        let tail = self.inner.stderr_tail.lock().await;
        tail.iter().cloned().collect::<Vec<_>>().join("\n")
    }

    async fn reject_pending(&self, err: GatewayError) {
        let mut pending = self.inner.pending.lock().await;
        for (_, tx) in pending.drain() {
            let _ = tx.send(Err(match &err {
                GatewayError::Closed => GatewayError::Closed,
                other => GatewayError::Rpc {
                    method: "pending".into(),
                    message: other.to_string(),
                },
            }));
        }
    }
}

impl Drop for GatewayClient {
    fn drop(&mut self) {
        // Safety net: if shutdown() never ran, Child still sits in the mutex
        // with kill_on_drop(true). Dropping Inner (last Arc) drops Child.
        if !self.inner.shutting_down.load(Ordering::SeqCst) {
            log_line("GatewayClient dropped without shutdown(); kill_on_drop will reap");
        }
    }
}

async fn write_frame(
    inner: &Inner,
    req: &crate::protocol::JsonRpcRequest,
) -> Result<(), GatewayError> {
    let mut line = serde_json::to_string(req).map_err(|e| GatewayError::Protocol(e.to_string()))?;
    line.push('\n');
    let mut stdin = inner.stdin.lock().await;
    let stdin = stdin.as_mut().ok_or(GatewayError::Closed)?;
    stdin.write_all(line.as_bytes()).await?;
    stdin.flush().await?;
    Ok(())
}

async fn read_stdout(inner: Arc<Inner>, stdout: tokio::process::ChildStdout) {
    let mut reader = BufReader::new(stdout);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) => {
                // Stdout closed. Do not treat as death — wait_child owns that.
                log_line("stdout EOF (not treating as child death)");
                break;
            }
            Ok(_) => {
                if buf.len() > MAX_STDOUT_LINE {
                    log_line("stdout line exceeded 1 MiB; dropped");
                    let _ = inner.events_tx.try_send(GatewayEvent::ProtocolError {
                        preview: "stdout line exceeded 1 MiB".into(),
                    });
                    continue;
                }
                let line = match std::str::from_utf8(&buf) {
                    Ok(s) => s.trim_end_matches(['\n', '\r']).to_string(),
                    Err(_) => {
                        let _ = inner.events_tx.try_send(GatewayEvent::ProtocolError {
                            preview: "invalid utf-8 on stdout".into(),
                        });
                        continue;
                    }
                };
                if line.is_empty() {
                    continue;
                }
                handle_line(&inner, &line).await;
            }
            Err(e) => {
                log_line(&format!("stdout read error: {e}"));
                break;
            }
        }
    }
}

async fn handle_line(inner: &Inner, line: &str) {
    let v: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            let prev = preview(line);
            log_line(&format!("malformed frame: {e} | {prev}"));
            let _ = inner
                .events_tx
                .try_send(GatewayEvent::ProtocolError { preview: prev });
            return;
        }
    };
    match classify(v) {
        Incoming::Result { id, result } => {
            let tx = inner.pending.lock().await.remove(&id);
            match tx {
                Some(tx) => {
                    let _ = tx.send(Ok(result));
                }
                None => {
                    // Ink ignores unknown result ids.
                    log_line(&format!("unknown result id {id} (ignored)"));
                }
            }
        }
        Incoming::Error { id, message } => {
            if let Some(id) = id {
                if let Some(tx) = inner.pending.lock().await.remove(&id) {
                    let _ = tx.send(Err(GatewayError::Rpc {
                        method: id,
                        message,
                    }));
                    return;
                }
            }
            // id: null parse-error — RPC error, not an event.
            log_line(&format!("rpc error (no id): {message}"));
            let _ = inner
                .events_tx
                .try_send(GatewayEvent::ProtocolError { preview: message });
        }
        Incoming::Event(ev) => {
            if ev.type_name == "gateway.ready" {
                if let Some(tx) = inner.ready_tx.lock().await.take() {
                    log_line(&format!(
                        "gateway.ready skin={} change_events={}",
                        ev.payload
                            .get("skin")
                            .map(|s| preview(&s.to_string()))
                            .unwrap_or_else(|| "?".into()),
                        ev.payload
                            .get("change_events")
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "?".into())
                    ));
                    let _ = tx.send(());
                }
            }
            if logging::is_sensitive(&ev.type_name) {
                log_line(&format!("event {} <redacted>", ev.type_name));
            } else if ev.type_name != "gateway.ready" {
                log_line(&format!("event {}", ev.type_name));
            }
            // Never drop terminal events. Deltas may drop under backpressure.
            let reserved = matches!(
                ev.type_name.as_str(),
                "message.complete"
                    | "approval.request"
                    | "approval.pending"
                    | "clarify.request"
                    | "sudo.request"
                    | "sudo.expire"
                    | "secret.request"
                    | "secret.expire"
                    | "gateway.ready"
            );
            if reserved {
                let _ = inner.events_tx.send(GatewayEvent::Event(ev)).await;
            } else if inner.events_tx.try_send(GatewayEvent::Event(ev)).is_err() {
                log_line("event channel full; dropping non-reserved event");
            }
        }
        Incoming::ProtocolError { preview } => {
            let _ = inner
                .events_tx
                .try_send(GatewayEvent::ProtocolError { preview });
        }
    }
}

async fn read_stderr(inner: Arc<Inner>, stderr: tokio::process::ChildStderr) {
    let mut reader = BufReader::new(stderr);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) => break,
            Ok(_) => {
                if buf.len() > MAX_STDERR_LINE {
                    buf.truncate(MAX_STDERR_LINE);
                }
                let line = String::from_utf8_lossy(&buf)
                    .trim_end_matches(['\n', '\r'])
                    .to_string();
                if line.is_empty() {
                    continue;
                }
                log_line(&format!("gw-stderr {line}"));
                {
                    let mut tail = inner.stderr_tail.lock().await;
                    if tail.len() >= 32 {
                        tail.pop_front();
                    }
                    tail.push_back(line.clone());
                }
                let _ = inner.events_tx.try_send(GatewayEvent::Stderr { line });
            }
            Err(_) => break,
        }
    }
}

async fn wait_child(inner: Arc<Inner>) {
    loop {
        if inner.shutting_down.load(Ordering::SeqCst) {
            // shutdown() owns the Child.
            return;
        }
        {
            let mut slot = inner.child.lock().await;
            if let Some(child) = slot.as_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        let code = status.code();
                        *slot = None;
                        log_line(&format!("child wait: {status:?}"));
                        drop(slot);
                        if let Some(tx) = inner.ready_tx.lock().await.take() {
                            drop(tx);
                        }
                        let mut pending = inner.pending.lock().await;
                        for (_, tx) in pending.drain() {
                            let _ = tx.send(Err(GatewayError::Closed));
                        }
                        drop(pending);
                        let _ = inner
                            .events_tx
                            .send(GatewayEvent::ChildExited { code })
                            .await;
                        return;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        log_line(&format!("child wait error: {e}"));
                        return;
                    }
                }
            } else {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
}

pub fn rpc_timeout() -> Duration {
    let ms = std::env::var("HERMES_TUI_RPC_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120_000u64);
    Duration::from_millis(ms.max(30_000))
}

pub fn startup_timeout() -> Duration {
    let ms = std::env::var("HERMES_TUI_STARTUP_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(15_000u64);
    Duration::from_millis(ms.max(5_000))
}

pub fn shutdown_grace() -> Duration {
    let s = std::env::var("HERMES_TUI_GATEWAY_SHUTDOWN_GRACE_S")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0);
    Duration::from_secs_f64(s.max(0.05))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    fn fake_script() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_gateway.py")
    }

    fn python() -> PathBuf {
        which::which("python3")
            .or_else(|_| which::which("python"))
            .expect("python3 for fake-child tests")
    }

    fn fake_opts() -> SpawnOptions {
        SpawnOptions {
            python: python(),
            args: vec!["-u".into(), fake_script().display().to_string()],
            extra_env: {
                let mut m = HashMap::new();
                m.insert("PYTHONUNBUFFERED".into(), "1".into());
                m
            },
            cwd: None,
        }
    }

    #[tokio::test]
    async fn spawn_ready_create_shutdown() {
        std::env::set_var("HERMES_TUI_STARTUP_TIMEOUT_MS", "8000");
        let mut client = GatewayClient::spawn(fake_opts())
            .await
            .expect("fake gateway should become ready");
        let created = client
            .request("session.create", json!({"cwd": ".", "cols": 80}))
            .await
            .expect("session.create");
        assert_eq!(created["session_id"], "sess-fake");
        assert_eq!(created["stored_session_id"], "store-fake");
        assert_eq!(created["info"]["lazy"], true);

        let _rx = client.take_events();
        assert!(client.take_events().is_none(), "take_events is once");

        client.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn unknown_result_id_is_ignored_not_panic() {
        // Covered by handle_line; this just ensures spawn still works after
        // the fake gateway emits an extra result id.
        let client = GatewayClient::spawn(fake_opts()).await.unwrap();
        let _ = client
            .request("session.close", json!({"session_id": "sess-fake"}))
            .await;
        client.shutdown().await.unwrap();
    }
}
