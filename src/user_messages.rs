//! User-facing copy. No panics, stack traces, or internal type names in the TUI.

use std::fmt::Write as _;

use crate::discover::DiscoveryError;
use crate::gateway::GatewayError;

pub fn discovery_failed(err: &DiscoveryError) -> String {
    let mut s = String::from("Could not find a Python that imports tui_gateway.\n");
    s.push_str(
        "The system python3 is not the Hermes venv. Official install lives at\n\
         ~/.hermes/hermes-agent/venv/bin/python and the `hermes` command is usually\n\
         ~/.local/bin/hermes.\n\n\
         If `hermes --tui` already works in this shell:\n\
           export HERMES_PYTHON=$(head -1 $(command -v hermes) | sed 's/^#!//')\n\
         If Hermes is not installed:\n\
           curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash\n\
           then reload the shell.\n\n",
    );
    s.push_str("Tried:\n");
    for line in &err.tried {
        let _ = writeln!(s, "  - {line}");
    }
    if let Some(last) = &err.last_error {
        let _ = writeln!(s, "\nLast error: {last}");
    }
    s
}

pub fn gateway_failed(err: &GatewayError) -> String {
    match err {
        GatewayError::StartupTimeout { stderr_tail } => {
            let mut s = String::from(
                "Hermes gateway did not become ready in time. Check ~/.hermes-rust/logs/hermes-rust.log.",
            );
            if !stderr_tail.is_empty() {
                s.push_str("\n\nLast gateway output:\n");
                s.push_str(stderr_tail);
            }
            s
        }
        GatewayError::ChildExited { code, stderr_tail } => {
            let code_s = code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "unknown".into());
            let mut s = format!(
                "Hermes gateway exited (code {code_s}). See ~/.hermes-rust/logs/hermes-rust.log."
            );
            if !stderr_tail.is_empty() {
                s.push_str("\n\nLast gateway output:\n");
                s.push_str(stderr_tail);
            }
            s
        }
        GatewayError::Spawn(e) => format!("Could not start Python: {e}"),
        GatewayError::Rpc { method, message } => match method.as_str() {
            "session.resume" => {
                format!("Could not resume that session. {message}")
            }
            "session.activate" => {
                format!("Could not switch to that live session. {message}")
            }
            "approval.respond" => format!("Could not send the approval. {message}"),
            "command.dispatch" => format!("That command did not run. {message}"),
            _ => format!("Hermes could not complete that request. {message}"),
        },
        GatewayError::Closed => "The Hermes gateway connection is closed.".into(),
        GatewayError::Timeout { method } => {
            format!(
                "Timed out waiting for `{method}`. The model call may still be running in Hermes."
            )
        }
        GatewayError::Protocol(msg) => format!("Gateway protocol error: {msg}"),
        GatewayError::Io(e) => format!("I/O error talking to the gateway: {e}"),
    }
}

pub fn child_exited(code: Option<i32>) -> String {
    let code_s = code
        .map(|c| c.to_string())
        .unwrap_or_else(|| "unknown".into());
    format!("Hermes gateway exited (code {code_s}). Last log: ~/.hermes-rust/logs/hermes-rust.log")
}
