//! JSON-RPC 2.0 framing for `tui_gateway`.
//!
//! **Live dump:** 2026-08-22, official install at
//! `~/.hermes/hermes-agent/venv/bin/python` (src_root `~/.hermes/hermes-agent`).
//!
//! Observed `session.create` result:
//! `{ session_id, stored_session_id, message_count, messages, info: { lazy,
//! model, cwd, branch, project, skills, tools, desktop_contract, profile_name } }`
//!
//! Observed `gateway.ready` payload: `{ skin: object (branding + banners),
//! change_events: true }`. Skin is not a string. Hex color overlay is optional.
//!
//! `message.delta` payload in TS is `{ text?, rendered? }`. Older notes used
//! `delta` — we accept all three.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Stdout line cap. Larger → drop, log, local `gateway.protocol_error`.
pub const MAX_STDOUT_LINE: usize = 1024 * 1024;
/// Ink stderr truncate.
pub const MAX_STDERR_LINE: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: Option<String>,
    #[serde(default)]
    pub id: Option<Value>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    #[serde(default)]
    pub code: i64,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

/// A pushed event (`method == "event"`).
#[derive(Debug, Clone)]
pub struct WireEvent {
    pub type_name: String,
    pub payload: Value,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Incoming {
    /// Matching pending request id.
    Result {
        id: String,
        result: Value,
    },
    /// Matching pending request id, or `id: null` parse error.
    Error {
        id: Option<String>,
        message: String,
    },
    Event(WireEvent),
    ProtocolError {
        preview: String,
    },
}

pub fn make_request(id: &str, method: &str, params: Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: id.to_string(),
        method: method.to_string(),
        params,
    }
}

/// Classify one decoded JSON object.
pub fn classify(v: Value) -> Incoming {
    let parsed: JsonRpcResponse = match serde_json::from_value(v.clone()) {
        Ok(p) => p,
        Err(e) => {
            return Incoming::ProtocolError {
                preview: crate::logging::preview(&format!("{e} | {v}")),
            };
        }
    };

    if parsed.method.as_deref() == Some("event") {
        let params = parsed.params.unwrap_or(Value::Null);
        let type_name = params
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("unknown")
            .to_string();
        let payload = params
            .get("payload")
            .cloned()
            .unwrap_or_else(|| params.clone());
        let session_id = params
            .get("session_id")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());
        return Incoming::Event(WireEvent {
            type_name,
            payload,
            session_id,
        });
    }

    let id = match &parsed.id {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(other) => Some(other.to_string()),
    };

    if let Some(err) = parsed.error {
        return Incoming::Error {
            id,
            message: if err.message.is_empty() {
                format!("rpc error {}", err.code)
            } else {
                err.message
            },
        };
    }

    if let Some(id) = id {
        return Incoming::Result {
            id,
            result: parsed.result.unwrap_or(Value::Null),
        };
    }

    Incoming::ProtocolError {
        preview: crate::logging::preview(&v.to_string()),
    }
}

/// Events the v1 demo UI must not invent widgets from.
/// Assistant stream text from a `message.delta` payload.
pub fn delta_text(payload: &Value) -> (String, Option<String>) {
    let text = payload
        .get("text")
        .or_else(|| payload.get("delta"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let rendered = payload
        .get("rendered")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    (text, rendered)
}

/// `session.create` / resume RPC `session_id` (the live id we put on later RPCs).
pub fn rpc_session_id(result: &Value) -> Option<&str> {
    result.get("session_id").and_then(|s| s.as_str())
}

pub fn is_unhandled_v1(type_name: &str) -> bool {
    matches!(
        type_name,
        "reasoning.available" | "message.start" | "message.interim" | "reaction" | "review.summary"
    ) || type_name.starts_with("voice.")
        || type_name.starts_with("wake.")
        || type_name.starts_with("moa.")
        || type_name.starts_with("browser.")
        || type_name.starts_with("billing.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn delta_prefers_text_over_legacy_delta() {
        let (t, r) = delta_text(&json!({"text": "hello", "delta": "old", "rendered": "**hello**"}));
        assert_eq!(t, "hello");
        assert_eq!(r.as_deref(), Some("**hello**"));
    }

    #[test]
    fn classifies_event() {
        let v = json!({
            "jsonrpc": "2.0",
            "method": "event",
            "params": {
                "type": "gateway.ready",
                "payload": { "skin": { "name": "dark" }, "change_events": true }
            }
        });
        match classify(v) {
            Incoming::Event(e) => {
                assert_eq!(e.type_name, "gateway.ready");
                assert!(e.payload.get("skin").unwrap().is_object());
                assert_eq!(e.payload.get("change_events"), Some(&json!(true)));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn classifies_result() {
        let v = json!({"jsonrpc":"2.0","id":"r1","result":{"session_id":"s1"}});
        match classify(v) {
            Incoming::Result { id, result } => {
                assert_eq!(id, "r1");
                assert_eq!(result["session_id"], "s1");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn null_id_error_is_rpc_not_event() {
        let v = json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": { "code": -32700, "message": "parse error" }
        });
        match classify(v) {
            Incoming::Error { id, message } => {
                assert!(id.is_none());
                assert!(message.contains("parse"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
