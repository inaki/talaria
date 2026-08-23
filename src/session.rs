//! Session control surface. Chat screens never name Live vs Mock.

use std::time::Duration;

use serde_json::{json, Value};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::gateway::{GatewayClient, GatewayError, GatewayEvent};
use crate::protocol::{delta_text, is_unhandled_v1, WireEvent};

#[derive(Debug, Clone)]
pub enum SessionCommand {
    Create {
        cwd: Option<String>,
        cols: u16,
    },
    Submit {
        text: String,
    },
    Interrupt,
    Resume {
        session_id: String,
        cols: u16,
    },
    Activate {
        session_id: String,
    },
    Dispatch {
        command: String,
    },
    RespondApproval {
        choice: String,
        request_id: Option<String>,
    },
    RespondClarify {
        request_id: String,
        answers: Value,
    },
    RespondSudo {
        request_id: String,
        password: String,
    },
    RespondSecret {
        request_id: String,
        value: String,
    },
    Resize {
        cols: u16,
        rows: u16,
    },
    Close,
    Shutdown,
    FetchCatalog,
    ListSaved {
        limit: u32,
    },
    ListActive,
    Steer {
        text: String,
    },
    Branch {
        title: Option<String>,
    },
    FetchDelegation,
    InterruptSubagent {
        subagent_id: String,
    },
    AttachImage {
        path: String,
    },
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    GatewayReady {
        skin: Option<Value>,
        change_events: Option<bool>,
    },
    SessionCreated {
        session_id: String,
        stored_session_id: Option<String>,
        info: Option<Value>,
    },
    SessionInfo {
        session_id: Option<String>,
        info: Value,
    },
    MessageDelta {
        session_id: Option<String>,
        text: String,
        rendered: Option<String>,
    },
    MessageComplete {
        session_id: Option<String>,
        text: Option<String>,
    },
    ToolStart {
        tool_id: String,
        name: Option<String>,
        args: Option<String>,
    },
    ToolProgress {
        tool_id: Option<String>,
        name: Option<String>,
        preview: Option<String>,
    },
    ToolComplete {
        tool_id: String,
        name: Option<String>,
        result: Option<String>,
        error: Option<String>,
    },
    ApprovalRequest {
        command: String,
        description: String,
        choices: Option<Vec<String>>,
        allow_permanent: Option<bool>,
        smart_denied: Option<bool>,
        request_id: Option<String>,
    },
    ApprovalPending,
    ClarifyRequest {
        request_id: String,
        payload: Value,
    },
    SudoRequest {
        request_id: String,
    },
    SecretRequest {
        request_id: String,
        env_var: String,
        prompt: String,
    },
    SudoExpired {
        request_id: String,
    },
    SecretExpired {
        request_id: String,
    },
    Status(String),
    Error {
        message: String,
    },
    ChildExited {
        code: Option<i32>,
    },
    ProtocolError {
        preview: String,
    },
    Stderr {
        line: String,
    },
    Unhandled {
        type_name: String,
        payload: Value,
    },
    Catalog {
        commands: Vec<SlashCommand>,
        warning: Option<String>,
    },
    SavedList {
        sessions: Vec<SavedSession>,
    },
    ActiveList {
        sessions: Vec<ActiveSession>,
    },
    CommandResult {
        output: Option<String>,
        send: Option<String>,
        prefill: Option<String>,
    },
    Transcript {
        session_id: String,
        messages: Vec<TranscriptMessage>,
    },
    Thinking {
        text: String,
    },
    Subagent {
        kind: SubagentKind,
        id: String,
        goal: Option<String>,
        text: Option<String>,
    },
    Delegation {
        agents: Vec<SubagentRow>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentKind {
    Start,
    Progress,
    Tool,
    Complete,
}

#[derive(Debug, Clone)]
pub struct SubagentRow {
    pub id: String,
    pub goal: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct SlashCommand {
    pub name: String,
    pub help: String,
}

#[derive(Debug, Clone)]
pub struct SavedSession {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub source: String,
    pub message_count: u64,
}

#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub id: String,
    pub title: Option<String>,
    pub status: String,
    pub current: bool,
}

#[derive(Debug, Clone)]
pub struct TranscriptMessage {
    pub role: String,
    pub text: String,
}

pub trait SessionApi {
    fn send(&self, cmd: SessionCommand);
    fn take_events(&mut self) -> Option<mpsc::Receiver<SessionEvent>>;
}

pub enum SessionKind {
    Live(GatewaySession),
    Mock(MockSession),
}

impl SessionApi for SessionKind {
    fn send(&self, cmd: SessionCommand) {
        match self {
            SessionKind::Live(s) => s.send(cmd),
            SessionKind::Mock(s) => s.send(cmd),
        }
    }

    fn take_events(&mut self) -> Option<mpsc::Receiver<SessionEvent>> {
        match self {
            SessionKind::Live(s) => s.take_events(),
            SessionKind::Mock(s) => s.take_events(),
        }
    }
}

pub struct GatewaySession {
    cmd_tx: mpsc::Sender<SessionCommand>,
    events_rx: Option<mpsc::Receiver<SessionEvent>>,
}

impl GatewaySession {
    pub fn start(client: GatewayClient, handle: &Handle) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let (ev_tx, ev_rx) = mpsc::channel(100);
        handle.spawn(live_loop(client, cmd_rx, ev_tx));
        Self {
            cmd_tx,
            events_rx: Some(ev_rx),
        }
    }
}

impl SessionApi for GatewaySession {
    fn send(&self, cmd: SessionCommand) {
        let _ = self.cmd_tx.try_send(cmd);
    }

    fn take_events(&mut self) -> Option<mpsc::Receiver<SessionEvent>> {
        self.events_rx.take()
    }
}

async fn live_loop(
    mut client: GatewayClient,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    ev_tx: mpsc::Sender<SessionEvent>,
) {
    let mut gw_events = match client.take_events() {
        Some(rx) => rx,
        None => {
            let _ = ev_tx
                .send(SessionEvent::Error {
                    message: "gateway event channel already taken".into(),
                })
                .await;
            return;
        }
    };

    let mut session_id: Option<String> = None;
    let mut agent_ready = false;
    let mut queued_submit: Option<String> = None;
    let ready_deadline = tokio::time::Instant::now() + Duration::from_secs(15);

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                let Some(cmd) = cmd else { break };
                if matches!(cmd, SessionCommand::Shutdown) {
                    let _ = client.shutdown().await;
                    break;
                }
                if let Err(e) = handle_live_cmd(
                    &client,
                    cmd,
                    &mut session_id,
                    &mut agent_ready,
                    &mut queued_submit,
                    ready_deadline,
                    &ev_tx,
                ).await {
                    emit_err(&ev_tx, e).await;
                }
            }
            event = gw_events.recv() => {
                let Some(event) = event else { break };
                let se = map_gateway_event(event);
                if let SessionEvent::SessionInfo { .. } = &se {
                    agent_ready = true;
                    if let Some(text) = queued_submit.take() {
                        if let Err(e) = do_submit(&client, session_id.as_deref(), &text).await {
                            emit_err(&ev_tx, e).await;
                        }
                    }
                }
                if ev_tx.send(se).await.is_err() {
                    break;
                }
            }
        }
    }
}

async fn handle_live_cmd(
    client: &GatewayClient,
    cmd: SessionCommand,
    session_id: &mut Option<String>,
    agent_ready: &mut bool,
    queued_submit: &mut Option<String>,
    ready_deadline: tokio::time::Instant,
    ev_tx: &mpsc::Sender<SessionEvent>,
) -> Result<(), GatewayError> {
    match cmd {
        SessionCommand::Create { cwd, cols } => {
            let mut params = json!({ "cols": cols });
            if let Some(cwd) = cwd {
                params["cwd"] = json!(cwd);
            }
            let result = client.request("session.create", params).await?;
            let sid = result
                .get("session_id")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            *session_id = Some(sid.clone());
            let stored = result
                .get("stored_session_id")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            let info = result.get("info").cloned();
            let _ = ev_tx
                .send(SessionEvent::SessionCreated {
                    session_id: sid,
                    stored_session_id: stored,
                    info,
                })
                .await;
            if let Ok(catalog) = client.request("commands.catalog", json!({})).await {
                let _ = ev_tx.send(parse_catalog(&catalog)).await;
            }
            Ok(())
        }
        SessionCommand::Submit { text } => {
            if !*agent_ready && tokio::time::Instant::now() < ready_deadline {
                *queued_submit = Some(text);
                let _ = ev_tx
                    .send(SessionEvent::Status(
                        "waiting for Hermes to finish starting…".into(),
                    ))
                    .await;
                return Ok(());
            }
            if !*agent_ready {
                let _ = ev_tx
                    .send(SessionEvent::Status(
                        "Hermes still starting; sending anyway.".into(),
                    ))
                    .await;
            }
            do_submit(client, session_id.as_deref(), &text).await
        }
        SessionCommand::Interrupt => {
            if let Some(sid) = session_id.as_deref() {
                let _ = client
                    .request("session.interrupt", json!({ "session_id": sid }))
                    .await;
            }
            Ok(())
        }
        SessionCommand::Resume {
            session_id: sid,
            cols,
        } => {
            let result = client
                .request("session.resume", json!({ "session_id": sid, "cols": cols }))
                .await?;
            apply_switch_result(session_id, &result, ev_tx).await;
            Ok(())
        }
        SessionCommand::Activate { session_id: sid } => {
            let result = client
                .request("session.activate", json!({ "session_id": sid }))
                .await?;
            apply_switch_result(session_id, &result, ev_tx).await;
            Ok(())
        }
        SessionCommand::Dispatch { command } => {
            let result = client
                .request("command.dispatch", json!({ "command": command }))
                .await?;
            let _ = ev_tx.send(parse_command_result(&result)).await;
            Ok(())
        }
        SessionCommand::FetchCatalog => {
            let result = client.request("commands.catalog", json!({})).await?;
            let _ = ev_tx.send(parse_catalog(&result)).await;
            Ok(())
        }
        SessionCommand::ListSaved { limit } => {
            let result = client
                .request("session.list", json!({ "limit": limit }))
                .await?;
            let _ = ev_tx.send(parse_saved_list(&result)).await;
            Ok(())
        }
        SessionCommand::ListActive => {
            let result = client
                .request(
                    "session.active_list",
                    json!({ "current_session_id": session_id.clone() }),
                )
                .await?;
            let _ = ev_tx.send(parse_active_list(&result)).await;
            Ok(())
        }
        SessionCommand::RespondApproval { choice, request_id } => {
            let mut params = json!({ "choice": choice });
            if let Some(id) = request_id {
                params["request_id"] = json!(id);
            }
            client.request("approval.respond", params).await?;
            Ok(())
        }
        SessionCommand::RespondClarify {
            request_id,
            answers,
        } => {
            let mut params = json!({ "request_id": request_id, "answers": answers });
            params["answer"] = answers.clone();
            client.request("clarify.respond", params).await?;
            Ok(())
        }
        SessionCommand::RespondSudo {
            request_id,
            password,
        } => {
            client
                .request(
                    "sudo.respond",
                    json!({ "request_id": request_id, "password": password }),
                )
                .await?;
            Ok(())
        }
        SessionCommand::RespondSecret { request_id, value } => {
            client
                .request(
                    "secret.respond",
                    json!({ "request_id": request_id, "value": value }),
                )
                .await?;
            Ok(())
        }
        SessionCommand::Resize { cols, rows } => {
            client
                .request("terminal.resize", json!({ "cols": cols, "rows": rows }))
                .await?;
            Ok(())
        }
        SessionCommand::Close => {
            if let Some(sid) = session_id.as_deref() {
                let _ = client
                    .request("session.close", json!({ "session_id": sid }))
                    .await;
            }
            Ok(())
        }
        SessionCommand::Shutdown => Ok(()),
        SessionCommand::Steer { text } => {
            if let Some(sid) = session_id.as_deref() {
                client
                    .request("session.steer", json!({ "session_id": sid, "text": text }))
                    .await?;
            }
            Ok(())
        }
        SessionCommand::Branch { title } => {
            let Some(sid) = session_id.as_deref() else {
                return Err(GatewayError::Protocol("no session_id yet".into()));
            };
            let mut params = json!({ "session_id": sid });
            if let Some(title) = title {
                params["title"] = json!(title);
            }
            let result = client.request("session.branch", params).await?;
            apply_switch_result(session_id, &result, ev_tx).await;
            Ok(())
        }
        SessionCommand::FetchDelegation => {
            let mut params = json!({});
            if let Some(sid) = session_id.as_deref() {
                params["session_id"] = json!(sid);
            }
            let result = client.request("delegation.status", params).await?;
            let _ = ev_tx.send(parse_delegation(&result)).await;
            Ok(())
        }
        SessionCommand::InterruptSubagent { subagent_id } => {
            let mut params = json!({ "subagent_id": subagent_id });
            if let Some(sid) = session_id.as_deref() {
                params["session_id"] = json!(sid);
            }
            client.request("subagent.interrupt", params).await?;
            Ok(())
        }
        SessionCommand::AttachImage { path } => {
            let Some(sid) = session_id.as_deref() else {
                return Err(GatewayError::Protocol("no session_id yet".into()));
            };
            let result = client
                .request("image.attach", json!({ "session_id": sid, "path": path }))
                .await?;
            let notice = result
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("image attached");
            let _ = ev_tx.send(SessionEvent::Status(notice.to_string())).await;
            Ok(())
        }
    }
}

async fn do_submit(
    client: &GatewayClient,
    session_id: Option<&str>,
    text: &str,
) -> Result<(), GatewayError> {
    let Some(sid) = session_id else {
        return Err(GatewayError::Protocol("no session_id yet".into()));
    };
    // Ordinary submit is {session_id, text} only — never truncation params.
    let _ = client
        .request("prompt.submit", json!({ "session_id": sid, "text": text }))
        .await?;
    Ok(())
}

async fn emit_err(ev_tx: &mpsc::Sender<SessionEvent>, e: GatewayError) {
    let message = crate::user_messages::gateway_failed(&e);
    let _ = ev_tx.send(SessionEvent::Error { message }).await;
}

pub fn map_gateway_event(event: GatewayEvent) -> SessionEvent {
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

fn parse_catalog(result: &Value) -> SessionEvent {
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

fn parse_saved_list(result: &Value) -> SessionEvent {
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

fn parse_active_list(result: &Value) -> SessionEvent {
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

fn parse_command_result(result: &Value) -> SessionEvent {
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

fn parse_transcript_messages(result: &Value) -> Vec<TranscriptMessage> {
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
                    Some(TranscriptMessage { role, text })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_delegation(result: &Value) -> SessionEvent {
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

async fn apply_switch_result(
    session_id: &mut Option<String>,
    result: &Value,
    ev_tx: &mpsc::Sender<SessionEvent>,
) {
    let sid = result
        .get("session_id")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    if !sid.is_empty() {
        *session_id = Some(sid.clone());
    }
    let stored = result
        .get("session_key")
        .or_else(|| result.get("stored_session_id"))
        .or_else(|| result.get("resumed"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());
    let info = result.get("info").cloned();
    let _ = ev_tx
        .send(SessionEvent::SessionCreated {
            session_id: sid.clone(),
            stored_session_id: stored,
            info,
        })
        .await;
    if let Some(info) = result.get("info") {
        let _ = ev_tx
            .send(SessionEvent::SessionInfo {
                session_id: Some(sid.clone()),
                info: info.clone(),
            })
            .await;
    }
    let _ = ev_tx
        .send(SessionEvent::Transcript {
            session_id: sid,
            messages: parse_transcript_messages(result),
        })
        .await;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MockScenario {
    Streaming,
    Tools,
    Approval,
    Error,
    Subagent,
}

impl MockScenario {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "tools" => Self::Tools,
            "approval" => Self::Approval,
            "error" => Self::Error,
            "subagent" | "agents" => Self::Subagent,
            _ => Self::Streaming,
        }
    }
}

pub struct MockSession {
    cmd_tx: mpsc::Sender<SessionCommand>,
    events_rx: Option<mpsc::Receiver<SessionEvent>>,
}

impl MockSession {
    pub fn start(scenario: MockScenario, handle: &Handle) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let (ev_tx, ev_rx) = mpsc::channel(64);
        handle.spawn(mock_loop(scenario, cmd_rx, ev_tx));
        Self {
            cmd_tx,
            events_rx: Some(ev_rx),
        }
    }
}

impl SessionApi for MockSession {
    fn send(&self, cmd: SessionCommand) {
        let _ = self.cmd_tx.try_send(cmd);
    }

    fn take_events(&mut self) -> Option<mpsc::Receiver<SessionEvent>> {
        self.events_rx.take()
    }
}

async fn mock_loop(
    scenario: MockScenario,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    ev_tx: mpsc::Sender<SessionEvent>,
) {
    let _ = ev_tx
        .send(SessionEvent::GatewayReady {
            skin: Some(json!({"name": "mock"})),
            change_events: Some(true),
        })
        .await;
    let _ = ev_tx
        .send(SessionEvent::SessionCreated {
            session_id: "mock-session".into(),
            stored_session_id: Some("mock-store".into()),
            info: Some(json!({"lazy": false, "model": "mock-model"})),
        })
        .await;
    let _ = ev_tx
        .send(SessionEvent::SessionInfo {
            session_id: Some("mock-session".into()),
            info: json!({"model": "mock-model", "cwd": ".", "lazy": false}),
        })
        .await;
    let _ = ev_tx.send(mock_catalog()).await;

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            SessionCommand::Submit { text } => {
                play_scenario(scenario, &text, &ev_tx).await;
            }
            SessionCommand::Interrupt => {
                let _ = ev_tx
                    .send(SessionEvent::Status("interrupted (mock)".into()))
                    .await;
            }
            SessionCommand::FetchCatalog => {
                let _ = ev_tx.send(mock_catalog()).await;
            }
            SessionCommand::ListSaved { .. } => {
                let _ = ev_tx
                    .send(SessionEvent::SavedList {
                        sessions: vec![
                            SavedSession {
                                id: "sess-alpha".into(),
                                title: "Yesterday's chat".into(),
                                preview: "we talked about rust".into(),
                                source: "tui".into(),
                                message_count: 6,
                            },
                            SavedSession {
                                id: "sess-beta".into(),
                                title: "CLI notes".into(),
                                preview: "cargo test".into(),
                                source: "cli".into(),
                                message_count: 3,
                            },
                        ],
                    })
                    .await;
            }
            SessionCommand::ListActive => {
                let _ = ev_tx
                    .send(SessionEvent::ActiveList {
                        sessions: vec![ActiveSession {
                            id: "mock-session".into(),
                            title: Some("current mock".into()),
                            status: "idle".into(),
                            current: true,
                        }],
                    })
                    .await;
            }
            SessionCommand::Resume {
                session_id: sid, ..
            }
            | SessionCommand::Activate { session_id: sid } => {
                let _ = ev_tx
                    .send(SessionEvent::Transcript {
                        session_id: sid.clone(),
                        messages: vec![
                            TranscriptMessage {
                                role: "user".into(),
                                text: format!("(resumed {sid})"),
                            },
                            TranscriptMessage {
                                role: "assistant".into(),
                                text: "Welcome back. This is mock history.".into(),
                            },
                        ],
                    })
                    .await;
                let _ = ev_tx
                    .send(SessionEvent::SessionCreated {
                        session_id: sid.clone(),
                        stored_session_id: Some(sid),
                        info: Some(json!({"model": "mock-model"})),
                    })
                    .await;
            }
            SessionCommand::Dispatch { command } => {
                let _ = ev_tx
                    .send(SessionEvent::CommandResult {
                        output: Some(format!("mock dispatch {command}")),
                        send: None,
                        prefill: None,
                    })
                    .await;
            }
            SessionCommand::RespondApproval { choice, .. } => {
                let _ = ev_tx
                    .send(SessionEvent::Status(format!("approval: {choice}")))
                    .await;
            }
            SessionCommand::RespondClarify { .. } => {
                let _ = ev_tx
                    .send(SessionEvent::Status("clarify answered (mock)".into()))
                    .await;
            }
            SessionCommand::RespondSudo { .. } | SessionCommand::RespondSecret { .. } => {
                let _ = ev_tx
                    .send(SessionEvent::Status("secret submitted (mock)".into()))
                    .await;
            }
            SessionCommand::Steer { text } => {
                let _ = ev_tx
                    .send(SessionEvent::Status(format!("steering: {text}")))
                    .await;
            }
            SessionCommand::Branch { .. } => {
                let _ = ev_tx
                    .send(SessionEvent::Status("branched (mock)".into()))
                    .await;
                let _ = ev_tx
                    .send(SessionEvent::SessionCreated {
                        session_id: "mock-branch".into(),
                        stored_session_id: Some("mock-branch".into()),
                        info: Some(json!({"model": "mock-model"})),
                    })
                    .await;
            }
            SessionCommand::FetchDelegation => {
                let _ = ev_tx
                    .send(SessionEvent::Delegation {
                        agents: vec![SubagentRow {
                            id: "sa-0".into(),
                            goal: "mock child task".into(),
                            status: "running".into(),
                        }],
                    })
                    .await;
            }
            SessionCommand::InterruptSubagent { subagent_id } => {
                let _ = ev_tx
                    .send(SessionEvent::Status(format!(
                        "interrupted subagent {subagent_id}"
                    )))
                    .await;
            }
            SessionCommand::AttachImage { path } => {
                let _ = ev_tx
                    .send(SessionEvent::Status(format!("attached {path}")))
                    .await;
            }
            SessionCommand::Shutdown | SessionCommand::Close => break,
            _ => {}
        }
    }
}

fn mock_catalog() -> SessionEvent {
    SessionEvent::Catalog {
        commands: vec![
            SlashCommand {
                name: "help".into(),
                help: "show help".into(),
            },
            SlashCommand {
                name: "clear".into(),
                help: "clear the transcript".into(),
            },
            SlashCommand {
                name: "resume".into(),
                help: "resume a saved session".into(),
            },
            SlashCommand {
                name: "sessions".into(),
                help: "browse saved and live sessions".into(),
            },
            SlashCommand {
                name: "model".into(),
                help: "switch model".into(),
            },
            SlashCommand {
                name: "branch".into(),
                help: "fork this session".into(),
            },
            SlashCommand {
                name: "agents".into(),
                help: "list live subagents".into(),
            },
            SlashCommand {
                name: "help".into(),
                help: "keyboard help".into(),
            },
        ],
        warning: None,
    }
}

async fn play_scenario(scenario: MockScenario, text: &str, ev_tx: &mpsc::Sender<SessionEvent>) {
    match scenario {
        MockScenario::Streaming => {
            let _ = ev_tx
                .send(SessionEvent::Thinking {
                    text: "thinking…".into(),
                })
                .await;
            tokio::time::sleep(Duration::from_millis(120)).await;
            let reply = format!(
                "# Mock reply\n\nYou said **{text}**.\n\n- item one\n- item two\n\nUse `Esc` to interrupt.\n"
            );
            for chunk in chunk_chars(&reply, 4) {
                let _ = ev_tx
                    .send(SessionEvent::MessageDelta {
                        session_id: Some("mock-session".into()),
                        text: chunk,
                        rendered: None,
                    })
                    .await;
                tokio::time::sleep(Duration::from_millis(40)).await;
            }
            let _ = ev_tx
                .send(SessionEvent::MessageComplete {
                    session_id: Some("mock-session".into()),
                    text: Some(reply),
                })
                .await;
        }
        MockScenario::Tools => {
            let _ = ev_tx
                .send(SessionEvent::ToolStart {
                    tool_id: "t1".into(),
                    name: Some("web_search".into()),
                    args: Some("query=\"hermes tui_gateway\"".into()),
                })
                .await;
            tokio::time::sleep(Duration::from_millis(80)).await;
            let _ = ev_tx
                .send(SessionEvent::ToolProgress {
                    tool_id: Some("t1".into()),
                    name: Some("web_search".into()),
                    preview: Some("searching…".into()),
                })
                .await;
            tokio::time::sleep(Duration::from_millis(80)).await;
            let _ = ev_tx
                .send(SessionEvent::ToolComplete {
                    tool_id: "t1".into(),
                    name: Some("web_search".into()),
                    result: Some("1 hit: programmatic integration docs".into()),
                    error: None,
                })
                .await;
            let _ = ev_tx
                .send(SessionEvent::MessageDelta {
                    session_id: Some("mock-session".into()),
                    text: "Tool finished. Here's a mock answer.".into(),
                    rendered: None,
                })
                .await;
            let _ = ev_tx
                .send(SessionEvent::MessageComplete {
                    session_id: Some("mock-session".into()),
                    text: Some("Tool finished. Here's a mock answer.".into()),
                })
                .await;
        }
        MockScenario::Approval => {
            let _ = ev_tx
                .send(SessionEvent::ApprovalRequest {
                    command: "rm -rf /tmp/mock".into(),
                    description: "Mock dangerous command (no live gateway).".into(),
                    choices: Some(vec!["once".into(), "always".into(), "deny".into()]),
                    allow_permanent: Some(true),
                    smart_denied: Some(false),
                    request_id: Some("mock-approval".into()),
                })
                .await;
        }
        MockScenario::Subagent => {
            let _ = ev_tx
                .send(SessionEvent::Subagent {
                    kind: SubagentKind::Start,
                    id: "sa-0".into(),
                    goal: Some("explore the repo".into()),
                    text: None,
                })
                .await;
            tokio::time::sleep(Duration::from_millis(80)).await;
            let _ = ev_tx
                .send(SessionEvent::Subagent {
                    kind: SubagentKind::Tool,
                    id: "sa-0".into(),
                    goal: Some("explore the repo".into()),
                    text: Some("list files".into()),
                })
                .await;
            tokio::time::sleep(Duration::from_millis(80)).await;
            let _ = ev_tx
                .send(SessionEvent::Subagent {
                    kind: SubagentKind::Complete,
                    id: "sa-0".into(),
                    goal: Some("explore the repo".into()),
                    text: Some("done".into()),
                })
                .await;
            let _ = ev_tx
                .send(SessionEvent::MessageComplete {
                    session_id: Some("mock-session".into()),
                    text: Some("Child finished exploring.".into()),
                })
                .await;
        }
        MockScenario::Error => {
            let _ = ev_tx
                .send(SessionEvent::Error {
                    message: "Mock provider error: the model is unavailable.".into(),
                })
                .await;
        }
    }
}

fn chunk_chars(s: &str, n: usize) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    chars.chunks(n).map(|c| c.iter().collect()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[tokio::test]
    async fn mock_streaming_emits_complete() {
        let rt = tokio::runtime::Handle::current();
        let mut mock = MockSession::start(MockScenario::Streaming, &rt);
        let mut rx = mock.take_events().unwrap();
        assert!(mock.take_events().is_none());
        // drain bootstrap
        let mut saw_info = false;
        for _ in 0..5 {
            if matches!(rx.recv().await, Some(SessionEvent::SessionInfo { .. })) {
                saw_info = true;
                break;
            }
        }
        assert!(saw_info);
        mock.send(SessionCommand::Submit { text: "hi".into() });
        let mut complete = false;
        while let Some(ev) = rx.recv().await {
            if matches!(ev, SessionEvent::MessageComplete { .. }) {
                complete = true;
                break;
            }
        }
        assert!(complete);
        mock.send(SessionCommand::Shutdown);
    }
}
