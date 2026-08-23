//! JSON-RPC result/event mapping and submit param builders.

use serde_json::{json, Value};

use crate::gateway::GatewayEvent;
use crate::protocol::{delta_text, is_unhandled_v1, WireEvent};

use super::types::{
    ActiveSession, McpCatalogEntry, McpServer, ModelProvider, PluginRow, RewindTurn, SavedSession,
    SessionEvent, SlashCommand, SpawnTreeEntry, SubagentKind, SubagentRow, TranscriptMessage,
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

pub(crate) fn map_gateway_event(event: GatewayEvent) -> SessionEvent {
    match event {
        GatewayEvent::Event(ev) => map_wire(ev),
        GatewayEvent::ProtocolError { preview } => SessionEvent::ProtocolError { preview },
        GatewayEvent::Stderr { line } => SessionEvent::Stderr { line },
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
                text,
                rendered,
            }
        }
        "message.complete" => {
            let text = payload
                .get("text")
                .or_else(|| payload.get("rendered"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            SessionEvent::MessageComplete { session_id, text }
        }
        "thinking.delta" | "reasoning.delta" => SessionEvent::Thinking {
            text: payload
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
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
                goal: payload
                    .get("goal")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                text: payload
                    .get("text")
                    .or_else(|| payload.get("summary"))
                    .or_else(|| payload.get("tool_preview"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            }
        }
        "tool.start" => SessionEvent::ToolStart {
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
            args: payload
                .get("args_text")
                .or_else(|| payload.get("args"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        },
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
                .map(|s| s.to_string()),
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
            result: payload
                .get("result_text")
                .or_else(|| payload.get("summary"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            error: payload
                .get("error")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        },
        "approval.request" => SessionEvent::ApprovalRequest {
            command: payload
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            description: payload
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
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
                .unwrap_or("secret")
                .to_string(),
        },
        "error" => SessionEvent::Error {
            message: payload
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Hermes reported an error")
                .to_string(),
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

pub(super) fn parse_saved_list(result: &Value) -> SessionEvent {
    let sessions = result
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
        .unwrap_or_default();
    SessionEvent::SavedList { sessions }
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
}
