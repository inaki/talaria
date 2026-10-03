//! JSON-RPC result/event mapping and submit param builders.

use serde_json::{json, Value};

use crate::gateway::GatewayEvent;
use crate::protocol::{delta_text, is_unhandled_v1, WireEvent};

use super::types::{
    ActiveSession, McpCatalogEntry, McpServer, ModelProvider, PluginRow, RewindTurn, SavedSession,
    SessionEvent, SlashCommand, SpawnTreeEntry, SubagentKind, SubagentRow, TranscriptMessage,
    UsageSnapshot,
};

pub(crate) fn ordinary_submit_params(session_id: &str, text: &str) -> Value {
    json!({ "session_id": session_id, "text": text })
}

pub(crate) fn resize_params(session_id: &str, cols: u16, rows: u16) -> Value {
    json!({ "session_id": session_id, "cols": cols, "rows": rows })
}

pub(crate) fn rewind_submit_params(
    session_id: &str,
    text: &str,
    truncate_before_row_id: i64,
    confirm_empty_truncate: bool,
) -> Value {
    let mut params = json!({
        "session_id": session_id,
        "text": text,
        "confirm_truncate": true,
        "truncate_before_row_id": truncate_before_row_id,
    });
    if confirm_empty_truncate {
        params["confirm_empty_truncate"] = json!(true);
    }
    params
}

pub(crate) fn parse_survivor_user_row_ids(result: &Value) -> Option<Vec<Option<i64>>> {
    let raw = result.get("survivor_user_row_ids")?.as_array()?;
    Some(raw.iter().map(json_i64).collect())
}

fn json_i64(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_u64().and_then(|u| i64::try_from(u).ok()))
}

pub(crate) fn rewind_turns_from_messages(messages: &[TranscriptMessage]) -> Vec<RewindTurn> {
    let mut turns = Vec::new();
    for m in messages {
        if m.role != "user" {
            continue;
        }
        if m.display_kind.as_deref().is_some_and(|d| !d.is_empty()) {
            continue;
        }
        let Some(row_id) = m.row_id else { continue };
        let first = turns.is_empty();
        turns.push(RewindTurn {
            row_id,
            text: m.text.clone(),
            first,
        });
    }
    turns
}

fn clean(s: &str) -> String {
    crate::logging::strip_controls(s)
}

/// Gateway `args` / `result` are often JSON objects, not strings.
fn json_display(v: Option<&Value>) -> Option<String> {
    let v = v?;
    let s = match v {
        Value::Null => return None,
        Value::String(s) => clean(s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Object(map) => {
            for key in [
                "command", "cmd", "output", "stdout", "text", "path", "query", "url",
            ] {
                if let Some(Value::String(s)) = map.get(key) {
                    if map.len() == 1 {
                        return Some(clean(s)).filter(|s| !s.is_empty());
                    }
                    return Some(clean(&format!("{key}: {s}"))).filter(|s| !s.is_empty());
                }
            }
            serde_json::to_string(v)
                .ok()
                .map(|s| clean(&s))
                .unwrap_or_default()
        }
        Value::Array(_) => serde_json::to_string(v)
            .ok()
            .map(|s| clean(&s))
            .unwrap_or_default(),
    };
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

pub(crate) fn map_gateway_event(event: GatewayEvent) -> SessionEvent {
    match event {
        GatewayEvent::Event(ev) => map_wire(ev),
        GatewayEvent::ProtocolError { preview } => SessionEvent::ProtocolError {
            preview: clean(&preview),
        },
        GatewayEvent::Stderr { line } => SessionEvent::Stderr {
            line: crate::logging::redact_secrets(&clean(&line)),
        },
        GatewayEvent::ChildExited { code } => SessionEvent::ChildExited { code },
    }
}

fn map_wire(ev: WireEvent) -> SessionEvent {
    let payload = ev.payload;
    let session_id = ev.session_id;
    match ev.type_name.as_str() {
        "gateway.ready" => SessionEvent::GatewayReady {
            skin: payload.get("skin").cloned(),
            change_events: payload.get("change_events").and_then(|v| v.as_bool()),
        },
        "session.info" => SessionEvent::SessionInfo {
            session_id,
            info: payload,
        },
        "message.delta" => {
            let (text, rendered) = delta_text(&payload);
            SessionEvent::MessageDelta {
                session_id,
                text: clean(&text),
                rendered: rendered.map(|s| clean(&s)),
            }
        }
        "message.complete" => {
            let text = payload
                .get("text")
                .or_else(|| payload.get("rendered"))
                .and_then(|v| v.as_str())
                .map(clean);
            SessionEvent::MessageComplete { session_id, text }
        }
        "thinking.delta" | "reasoning.delta" => SessionEvent::Thinking {
            text: payload
                .get("text")
                .and_then(|v| v.as_str())
                .map(clean)
                .unwrap_or_default(),
        },
        t if t.starts_with("subagent.") => {
            let kind = match t {
                "subagent.start" | "subagent.spawn_requested" => SubagentKind::Start,
                "subagent.tool" => SubagentKind::Tool,
                "subagent.complete" => SubagentKind::Complete,
                _ => SubagentKind::Progress,
            };
            SessionEvent::Subagent {
                kind,
                id: payload
                    .get("subagent_id")
                    .or_else(|| payload.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("sa")
                    .to_string(),
                goal: payload.get("goal").and_then(|v| v.as_str()).map(clean),
                text: payload
                    .get("text")
                    .or_else(|| payload.get("summary"))
                    .or_else(|| payload.get("tool_preview"))
                    .and_then(|v| v.as_str())
                    .map(clean),
            }
        }
        "tool.start" => {
            let name = payload
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let preview = payload
                .get("context")
                .and_then(|v| v.as_str())
                .map(clean)
                .filter(|s| !s.is_empty());
            let args = json_display(payload.get("args_text").or_else(|| payload.get("args")));
            if name.as_deref() == Some("_thinking") {
                SessionEvent::Thinking {
                    text: preview.or(args).unwrap_or_default(),
                }
            } else {
                SessionEvent::ToolStart {
                    tool_id: payload
                        .get("tool_id")
                        .or_else(|| payload.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool")
                        .to_string(),
                    name,
                    args,
                    preview,
                }
            }
        }
        "tool.progress" => SessionEvent::ToolProgress {
            tool_id: payload
                .get("tool_id")
                .or_else(|| payload.get("id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            name: payload
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            preview: payload
                .get("preview")
                .or_else(|| payload.get("output"))
                .and_then(|v| v.as_str())
                .map(clean),
        },
        "tool.complete" => SessionEvent::ToolComplete {
            tool_id: payload
                .get("tool_id")
                .or_else(|| payload.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("tool")
                .to_string(),
            name: payload
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            result: json_display(
                payload
                    .get("result_text")
                    .or_else(|| payload.get("summary"))
                    .or_else(|| payload.get("result")),
            ),
            error: payload.get("error").and_then(|v| v.as_str()).map(clean),
        },
        "approval.request" => SessionEvent::ApprovalRequest {
            command: payload
                .get("command")
                .and_then(|v| v.as_str())
                .map(clean)
                .unwrap_or_default(),
            description: payload
                .get("description")
                .and_then(|v| v.as_str())
                .map(clean)
                .unwrap_or_default(),
            choices: payload.get("choices").and_then(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
            }),
            allow_permanent: payload
                .get("allowPermanent")
                .or(payload.get("allow_permanent"))
                .and_then(|v| v.as_bool()),
            smart_denied: payload
                .get("smartDenied")
                .or(payload.get("smart_denied"))
                .and_then(|v| v.as_bool()),
            request_id: payload
                .get("request_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        },
        "approval.pending" => SessionEvent::ApprovalPending,
        // ~1s ticker while a turn runs; payload is `{usage: <session.usage result>}`.
        "session.usage" => match payload.get("usage") {
            Some(usage) => parse_usage(usage),
            None => SessionEvent::Unhandled {
                type_name: "session.usage".into(),
                payload: payload.clone(),
            },
        },
        "clarify.request" => SessionEvent::ClarifyRequest {
            request_id: payload
                .get("request_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            payload,
        },
        "sudo.request" => SessionEvent::SudoRequest {
            request_id: payload
                .get("request_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
        "secret.request" => SessionEvent::SecretRequest {
            request_id: payload
                .get("request_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            env_var: payload
                .get("env_var")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            prompt: payload
                .get("prompt")
                .and_then(|v| v.as_str())
                .map(clean)
                .unwrap_or_else(|| "secret".into()),
        },
        "error" => SessionEvent::Error {
            message: payload
                .get("message")
                .and_then(|v| v.as_str())
                .map(clean)
                .unwrap_or_else(|| "Hermes reported an error".into()),
        },
        "sudo.expire" => SessionEvent::SudoExpired {
            request_id: payload
                .get("request_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
        "secret.expire" => SessionEvent::SecretExpired {
            request_id: payload
                .get("request_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
        "skin.changed" => SessionEvent::SkinChanged {
            skin: payload.get("skin").cloned().unwrap_or(payload),
        },
        "background.complete" => {
            SessionEvent::Status(format!("background complete{}", payload_suffix(&payload)))
        }
        "status.update" => SessionEvent::Status(
            payload_message(&payload).unwrap_or_else(|| "status update".into()),
        ),
        other if other.starts_with("notification.") => {
            SessionEvent::Status(payload_message(&payload).unwrap_or_else(|| other.to_string()))
        }
        other if is_unhandled_v1(other) => SessionEvent::Unhandled {
            type_name: other.to_string(),
            payload,
        },
        other => SessionEvent::Unhandled {
            type_name: other.to_string(),
            payload,
        },
    }
}

pub(super) fn parse_catalog(result: &Value) -> SessionEvent {
    let mut commands = Vec::new();
    if let Some(pairs) = result.get("pairs").and_then(|v| v.as_array()) {
        for p in pairs {
            if let Some(arr) = p.as_array() {
                let name = arr
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim_start_matches('/')
                    .to_string();
                let help = arr
                    .get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !name.is_empty() {
                    commands.push(SlashCommand { name, help });
                }
            }
        }
    }
    if commands.is_empty() {
        if let Some(cats) = result.get("categories").and_then(|v| v.as_array()) {
            for cat in cats {
                if let Some(pairs) = cat.get("pairs").and_then(|v| v.as_array()) {
                    for p in pairs {
                        if let Some(arr) = p.as_array() {
                            let name = arr
                                .first()
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .trim_start_matches('/')
                                .to_string();
                            let help = arr
                                .get(1)
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            if !name.is_empty() {
                                commands.push(SlashCommand { name, help });
                            }
                        }
                    }
                }
            }
        }
    }
    SessionEvent::Catalog {
        commands,
        warning: result
            .get("warning")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    }
}

pub(crate) fn parse_saved_sessions(result: &Value) -> Vec<SavedSession> {
    result
        .get("sessions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    let id = s.get("id").and_then(|v| v.as_str())?.to_string();
                    Some(SavedSession {
                        id,
                        title: s
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        preview: s
                            .get("preview")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        source: s
                            .get("source")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        message_count: s.get("message_count").and_then(|v| v.as_u64()).unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn parse_saved_list(result: &Value) -> SessionEvent {
    SessionEvent::SavedList {
        sessions: parse_saved_sessions(result),
    }
}

/// Exact id, then exact title (case-insensitive), then title substring.
pub(crate) fn match_saved_session(sessions: &[SavedSession], query: &str) -> Option<String> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    if let Some(s) = sessions.iter().find(|s| s.id == q) {
        return Some(s.id.clone());
    }
    let lower = q.to_ascii_lowercase();
    if let Some(s) = sessions.iter().find(|s| s.title.eq_ignore_ascii_case(q)) {
        return Some(s.id.clone());
    }
    sessions
        .iter()
        .find(|s| s.title.to_ascii_lowercase().contains(&lower))
        .map(|s| s.id.clone())
}

fn payload_message(payload: &Value) -> Option<String> {
    for key in ["text", "message", "summary", "status", "title", "body"] {
        if let Some(s) = payload.get(key).and_then(|v| v.as_str()) {
            let s = clean(s);
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

fn payload_suffix(payload: &Value) -> String {
    match payload_message(payload) {
        Some(s) => format!(": {s}"),
        None => String::new(),
    }
}

pub(super) fn parse_active_list(result: &Value) -> SessionEvent {
    let sessions = result
        .get("sessions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    let id = s
                        .get("id")
                        .or_else(|| s.get("session_id"))
                        .and_then(|v| v.as_str())?
                        .to_string();
                    Some(ActiveSession {
                        id,
                        title: s
                            .get("title")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        status: s
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("idle")
                            .to_string(),
                        current: s.get("current").and_then(|v| v.as_bool()).unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    SessionEvent::ActiveList { sessions }
}

pub(super) fn parse_command_result(result: &Value) -> SessionEvent {
    let kind = result.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match kind {
        "send" => SessionEvent::CommandResult {
            output: result
                .get("notice")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            send: result
                .get("message")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            prefill: None,
        },
        "prefill" => SessionEvent::CommandResult {
            output: result
                .get("notice")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            send: None,
            prefill: result
                .get("message")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        },
        _ => SessionEvent::CommandResult {
            output: result
                .get("output")
                .or_else(|| result.get("message"))
                .or_else(|| result.get("display"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    if result.is_null() {
                        None
                    } else {
                        Some(result.to_string())
                    }
                }),
            send: None,
            prefill: None,
        },
    }
}

pub(super) fn parse_transcript_messages(result: &Value) -> Vec<TranscriptMessage> {
    result
        .get("messages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let role = m.get("role").and_then(|v| v.as_str())?.to_string();
                    let text = m
                        .get("text")
                        .or_else(|| m.get("content"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let row_id = m.get("row_id").and_then(json_i64);
                    let display_kind = m
                        .get("display_kind")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string());
                    Some(TranscriptMessage {
                        role,
                        text,
                        row_id,
                        display_kind,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn parse_spawn_trees(result: &Value) -> SessionEvent {
    let entries = result
        .get("entries")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    let path = e.get("path").and_then(|v| v.as_str())?.to_string();
                    Some(SpawnTreeEntry {
                        path: path.clone(),
                        label: e
                            .get("label")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&path)
                            .to_string(),
                        count: e.get("count").and_then(|v| v.as_u64()).unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    SessionEvent::SpawnTrees { entries }
}

fn rewrite_setup_warning(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    if !(lower.contains("hermes model")
        || lower.contains("hermes setup")
        || lower.contains("`hermes"))
    {
        return s.to_string();
    }
    if lower.contains("oauth") {
        "OAuth provider — not a paste-key setup".into()
    } else if lower.contains("key") || lower.contains("paste") {
        "paste the API key to activate".into()
    } else {
        "not configured — API-key providers can be activated here".into()
    }
}

pub(super) fn parse_model_options(result: &Value) -> SessionEvent {
    let providers = result
        .get("providers")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(parse_model_provider).collect())
        .unwrap_or_default();
    SessionEvent::ModelOptions {
        providers,
        model: result
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        error: None,
    }
}

pub(super) fn parse_model_provider(v: &Value) -> Option<ModelProvider> {
    let slug = v.get("slug").and_then(|s| s.as_str())?.to_string();
    let name = v
        .get("name")
        .and_then(|s| s.as_str())
        .unwrap_or(&slug)
        .to_string();
    let models: Vec<String> = v
        .get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let total_models = v
        .get("total_models")
        .and_then(|n| n.as_u64())
        .unwrap_or(models.len() as u64);
    Some(ModelProvider {
        name,
        slug,
        authenticated: v
            .get("authenticated")
            .and_then(|b| b.as_bool())
            .unwrap_or(true),
        is_current: v
            .get("is_current")
            .and_then(|b| b.as_bool())
            .unwrap_or(false),
        auth_type: v
            .get("auth_type")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        key_env: v
            .get("key_env")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        models,
        total_models,
        warning: v
            .get("warning")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(rewrite_setup_warning),
    })
}

pub(super) fn parse_model_key_saved(result: &Value) -> SessionEvent {
    match result.get("provider").and_then(parse_model_provider) {
        Some(provider) => SessionEvent::ModelKeySaved {
            provider,
            error: None,
        },
        None => SessionEvent::ModelKeySaved {
            provider: ModelProvider {
                slug: String::new(),
                name: String::new(),
                authenticated: false,
                is_current: false,
                auth_type: String::new(),
                key_env: None,
                models: Vec::new(),
                total_models: 0,
                warning: None,
            },
            error: Some("failed to save key".into()),
        },
    }
}

pub(super) fn parse_model_disconnected(slug: String, result: &Value) -> SessionEvent {
    SessionEvent::ModelDisconnected {
        slug,
        ok: result
            .get("disconnected")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}

pub(super) fn parse_config_set(key: String, result: &Value) -> SessionEvent {
    SessionEvent::ConfigSet {
        key,
        value: result
            .get("value")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        warning: result
            .get("warning")
            .or_else(|| result.get("credential_warning"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        deferred: result
            .get("deferred")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        confirm_required: result
            .get("confirm_required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        confirm_message: result
            .get("confirm_message")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        info: result.get("info").cloned(),
    }
}

pub(super) fn parse_skills_list(result: &Value) -> SessionEvent {
    let groups = result
        .get("skills")
        .and_then(|v| v.as_object())
        .map(|obj| {
            let mut out: Vec<(String, Vec<String>)> = obj
                .iter()
                .map(|(k, v)| {
                    let members = v
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    (k.clone(), members)
                })
                .collect();
            out.sort_by(|a, b| a.0.cmp(&b.0));
            out
        })
        .unwrap_or_default();
    SessionEvent::SkillsList {
        groups,
        error: None,
    }
}

pub(super) fn parse_plugins_list(result: &Value) -> SessionEvent {
    let plugins = result
        .get("plugins")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(parse_plugin_row).collect())
        .unwrap_or_default();
    SessionEvent::PluginsList {
        plugins,
        error: None,
    }
}

pub(super) fn parse_plugin_row(v: &Value) -> Option<PluginRow> {
    let name = v.get("name").and_then(|s| s.as_str())?.to_string();
    Some(PluginRow {
        key: v
            .get("key")
            .and_then(|s| s.as_str())
            .unwrap_or(&name)
            .to_string(),
        name,
        version: v
            .get("version")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        description: v
            .get("description")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        source: v
            .get("source")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        status: v
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

pub(super) fn parse_mcp_servers(result: &Value) -> SessionEvent {
    let servers = result
        .get("servers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    let name = s.get("name").and_then(|v| v.as_str())?.to_string();
                    Some(McpServer {
                        transport: s
                            .get("transport")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        enabled: s.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                        auth: s
                            .get("auth")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        name,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    SessionEvent::McpServers {
        servers,
        error: None,
    }
}

pub(super) fn parse_mcp_catalog(result: &Value) -> SessionEvent {
    let servers = result
        .get("servers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    let name = s.get("name").and_then(|v| v.as_str())?.to_string();
                    Some(McpCatalogEntry {
                        description: s
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        installed: s
                            .get("installed")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        enabled: s.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
                        name,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    SessionEvent::McpCatalog {
        servers,
        error: None,
    }
}

pub(super) fn parse_usage(result: &Value) -> SessionEvent {
    let num = |k: &str| -> u64 {
        result
            .get(k)
            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
            .unwrap_or(0)
    };
    let credits = result
        .get("credits_lines")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    SessionEvent::Usage(UsageSnapshot {
        calls: num("calls"),
        input: num("input"),
        output: num("output"),
        total: num("total"),
        context_used: num("context_used"),
        context_max: num("context_max"),
        context_percent: num("context_percent"),
        cost_usd: result.get("cost_usd").and_then(|v| v.as_f64()),
        cost_status: result
            .get("cost_status")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        model: result
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        credits_lines: credits,
    })
}

pub(super) fn parse_delegation(result: &Value) -> SessionEvent {
    let agents = result
        .get("active")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    Some(SubagentRow {
                        id: a.get("subagent_id").and_then(|v| v.as_str())?.to_string(),
                        goal: a
                            .get("goal")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        status: a
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("running")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    SessionEvent::Delegation { agents }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use crate::gateway::GatewayEvent;
    use crate::protocol::WireEvent;

    #[test]
    fn parse_catalog_from_pairs() {
        let ev = parse_catalog(&json!({
            "pairs": [["help", "show help"], ["/resume", "resume a session"]]
        }));
        match ev {
            SessionEvent::Catalog { commands, .. } => {
                assert_eq!(commands.len(), 2);
                assert_eq!(commands[1].name, "resume");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn maps_skin_background_status_and_notifications() {
        let skin = map_gateway_event(GatewayEvent::Event(WireEvent {
            type_name: "skin.changed".into(),
            payload: json!({"skin": {"name": "ares"}}),
            session_id: None,
        }));
        match skin {
            SessionEvent::SkinChanged { skin } => {
                assert_eq!(skin.get("name").and_then(|v| v.as_str()), Some("ares"));
            }
            other => panic!("{other:?}"),
        }
        let bg = map_gateway_event(GatewayEvent::Event(WireEvent {
            type_name: "background.complete".into(),
            payload: json!({"summary": "done compiling"}),
            session_id: None,
        }));
        match bg {
            SessionEvent::Status(s) => assert!(s.contains("done compiling"), "{s}"),
            other => panic!("{other:?}"),
        }
        let st = map_gateway_event(GatewayEvent::Event(WireEvent {
            type_name: "status.update".into(),
            payload: json!({"message": "yolo on"}),
            session_id: None,
        }));
        match st {
            SessionEvent::Status(s) => assert_eq!(s, "yolo on"),
            other => panic!("{other:?}"),
        }
        let note = map_gateway_event(GatewayEvent::Event(WireEvent {
            type_name: "notification.info".into(),
            payload: json!({"text": "mcp ready"}),
            session_id: None,
        }));
        match note {
            SessionEvent::Status(s) => assert_eq!(s, "mcp ready"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn match_saved_prefers_id_then_title() {
        let sessions = vec![
            SavedSession {
                id: "abc".into(),
                title: "Auth refactor".into(),
                preview: String::new(),
                source: "tui".into(),
                message_count: 1,
            },
            SavedSession {
                id: "def".into(),
                title: "notes".into(),
                preview: String::new(),
                source: "cli".into(),
                message_count: 2,
            },
        ];
        assert_eq!(
            match_saved_session(&sessions, "abc").as_deref(),
            Some("abc")
        );
        assert_eq!(
            match_saved_session(&sessions, "auth refactor").as_deref(),
            Some("abc")
        );
        assert_eq!(
            match_saved_session(&sessions, "NOTE").as_deref(),
            Some("def")
        );
        assert!(match_saved_session(&sessions, "nope").is_none());
    }

    #[test]
    fn parse_saved_list_from_gateway_shape() {
        let ev = parse_saved_list(&json!({
            "sessions": [{
                "id": "abc",
                "title": "t",
                "preview": "p",
                "started_at": 1,
                "message_count": 3,
                "source": "tui"
            }]
        }));
        match ev {
            SessionEvent::SavedList { sessions } => {
                assert_eq!(sessions[0].id, "abc");
                assert_eq!(sessions[0].source, "tui");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn map_ready_skin_is_object() {
        let ev = map_gateway_event(GatewayEvent::Event(WireEvent {
            type_name: "gateway.ready".into(),
            payload: json!({"skin": {"name": "dark"}, "change_events": true}),
            session_id: None,
        }));
        match ev {
            SessionEvent::GatewayReady {
                skin,
                change_events,
            } => {
                assert!(skin.unwrap().is_object());
                assert_eq!(change_events, Some(true));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rewrite_warning_drops_hermes_cli() {
        assert_eq!(
            rewrite_setup_warning("run `hermes model` to configure"),
            "not configured — API-key providers can be activated here"
        );
        assert!(
            rewrite_setup_warning("paste OPENROUTER_API_KEY to activate").contains("OPENROUTER")
        );
    }

    #[test]
    fn parse_model_options_includes_unconfigured() {
        let ev = parse_model_options(&json!({
            "model": "openrouter/foo",
            "providers": [
                {
                    "slug": "openrouter",
                    "name": "OpenRouter",
                    "authenticated": true,
                    "is_current": true,
                    "models": ["openrouter/foo", "openrouter/bar"],
                    "total_models": 2
                },
                {
                    "slug": "anthropic",
                    "name": "Anthropic",
                    "authenticated": false,
                    "auth_type": "api_key",
                    "key_env": "ANTHROPIC_API_KEY",
                    "warning": "paste ANTHROPIC_API_KEY to activate"
                }
            ]
        }));
        match ev {
            SessionEvent::ModelOptions {
                providers, model, ..
            } => {
                assert_eq!(model, "openrouter/foo");
                assert_eq!(providers.len(), 2);
                assert!(providers[0].authenticated);
                assert!(!providers[1].authenticated);
                assert_eq!(providers[1].key_env.as_deref(), Some("ANTHROPIC_API_KEY"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parse_config_set_flags_expensive_confirm() {
        let ev = parse_config_set(
            "model".into(),
            &json!({
                "confirm_required": true,
                "confirm_message": "this is expensive",
                "value": "opus"
            }),
        );
        match ev {
            SessionEvent::ConfigSet {
                confirm_required,
                confirm_message,
                value,
                ..
            } => {
                assert!(confirm_required);
                assert_eq!(confirm_message.as_deref(), Some("this is expensive"));
                assert_eq!(value.as_deref(), Some("opus"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn resize_params_include_session_id() {
        let p = resize_params("sess-1", 120, 40);
        assert_eq!(p["session_id"], "sess-1");
        assert_eq!(p["cols"], 120);
        assert_eq!(p["rows"], 40);
    }

    #[test]
    fn parse_usage_reads_counts() {
        let ev = parse_usage(&json!({
            "calls": 3,
            "input": 1000,
            "output": 40,
            "total": 1040,
            "context_percent": 12,
            "cost_usd": 0.02,
            "model": "mock",
            "credits_lines": ["$10 remaining"]
        }));
        match ev {
            SessionEvent::Usage(u) => {
                assert_eq!(u.calls, 3);
                assert_eq!(u.total, 1040);
                assert_eq!(u.credits_lines.len(), 1);
                assert_eq!(u.cost_usd, Some(0.02));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn session_usage_event_carries_cost() {
        let ev = map_wire(WireEvent {
            type_name: "session.usage".into(),
            session_id: None,
            payload: json!({"usage": {
                "total": 900,
                "cost_usd": 0.0123,
                "cost_status": "estimated",
            }}),
        });
        match ev {
            SessionEvent::Usage(u) => {
                assert_eq!(u.total, 900);
                assert_eq!(u.cost_label().as_deref(), Some("~$0.012"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn ordinary_submit_is_session_and_text_only() {
        let p = ordinary_submit_params("sess-1", "hello");
        let obj = p.as_object().expect("object");
        assert_eq!(obj.len(), 2);
        assert_eq!(obj["session_id"], "sess-1");
        assert_eq!(obj["text"], "hello");
        for forbidden in [
            "confirm_truncate",
            "confirm_empty_truncate",
            "truncate_before_row_id",
            "truncate_before_user_ordinal",
            "truncate_before_message_id",
        ] {
            assert!(
                !obj.contains_key(forbidden),
                "{forbidden} must not ride along"
            );
        }
    }

    #[test]
    fn rewind_submit_sends_confirm_and_row_id() {
        let p = rewind_submit_params("sess-1", "hello", 42, false);
        assert_eq!(p["session_id"], "sess-1");
        assert_eq!(p["text"], "hello");
        assert_eq!(p["confirm_truncate"], true);
        assert_eq!(p["truncate_before_row_id"], 42);
        assert!(p.get("truncate_before_user_ordinal").is_none());
        assert!(p.get("truncate_before_message_id").is_none());
        assert!(p.get("confirm_empty_truncate").is_none());

        let empty = rewind_submit_params("sess-1", "hello", 7, true);
        assert_eq!(empty["confirm_empty_truncate"], true);
        assert_eq!(empty["truncate_before_row_id"], 7);
    }

    #[test]
    fn parse_transcript_row_ids() {
        let msgs = parse_transcript_messages(&json!({
            "messages": [
                {"role": "user", "text": "hi", "row_id": 13},
                {"role": "assistant", "text": "yo"},
                {"role": "user", "text": "skill", "row_id": 14, "display_kind": "skill_invocation"},
            ]
        }));
        assert_eq!(msgs[0].row_id, Some(13));
        assert_eq!(msgs[1].row_id, None);
        assert_eq!(msgs[2].display_kind.as_deref(), Some("skill_invocation"));
        let turns = rewind_turns_from_messages(&msgs);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].row_id, 13);
        assert!(turns[0].first);
    }

    #[test]
    fn parse_survivor_row_ids() {
        let ids = parse_survivor_user_row_ids(&json!({
            "status": "streaming",
            "survivor_user_row_ids": [7, null, 9]
        }));
        assert_eq!(ids, Some(vec![Some(7), None, Some(9)]));
        assert!(parse_survivor_user_row_ids(&json!({"status": "streaming"})).is_none());
    }

    #[test]
    fn tool_start_reads_json_args_and_context() {
        let ev = map_gateway_event(GatewayEvent::Event(WireEvent {
            type_name: "tool.start".into(),
            payload: json!({
                "tool_id": "tool-1",
                "name": "terminal",
                "context": "ls",
                "args": { "command": "ls" }
            }),
            session_id: None,
        }));
        match ev {
            SessionEvent::ToolStart {
                tool_id,
                name,
                args,
                preview,
            } => {
                assert_eq!(tool_id, "tool-1");
                assert_eq!(name.as_deref(), Some("terminal"));
                assert_eq!(args.as_deref(), Some("ls"));
                assert_eq!(preview.as_deref(), Some("ls"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn tool_complete_reads_json_result() {
        let ev = map_gateway_event(GatewayEvent::Event(WireEvent {
            type_name: "tool.complete".into(),
            payload: json!({
                "tool_id": "tool-1",
                "name": "terminal",
                "result": { "output": "ok" }
            }),
            session_id: None,
        }));
        match ev {
            SessionEvent::ToolComplete { result, .. } => {
                assert_eq!(result.as_deref(), Some("ok"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn thinking_tool_start_is_not_a_card() {
        let ev = map_gateway_event(GatewayEvent::Event(WireEvent {
            type_name: "tool.start".into(),
            payload: json!({
                "name": "_thinking",
                "context": "pondering"
            }),
            session_id: None,
        }));
        match ev {
            SessionEvent::Thinking { text } => assert_eq!(text, "pondering"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn thinking_delta_strips_csi() {
        let ev = map_gateway_event(GatewayEvent::Event(WireEvent {
            type_name: "thinking.delta".into(),
            payload: json!({ "text": "hi\u{1b}[31mred" }),
            session_id: None,
        }));
        match ev {
            SessionEvent::Thinking { text } => {
                assert_eq!(text, "hired");
                assert!(!text.contains('\u{1b}'));
            }
            other => panic!("{other:?}"),
        }
    }
}
