//! Live gateway session: JSON-RPC commands on a spawned tui_gateway.

use std::time::Duration;

use serde_json::{json, Value};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::gateway::{GatewayClient, GatewayError};

use super::parse::{
    map_gateway_event, ordinary_submit_params, parse_active_list, parse_catalog,
    parse_command_result, parse_delegation, parse_saved_list, parse_spawn_trees,
    parse_survivor_user_row_ids, parse_transcript_messages, rewind_submit_params,
};
use super::{SessionApi, SessionCommand, SessionEvent};

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
        SessionCommand::FetchHistory => {
            let Some(sid) = session_id.as_deref() else {
                return Err(GatewayError::Protocol("no session_id yet".into()));
            };
            let result = client
                .request("session.history", json!({ "session_id": sid }))
                .await?;
            let _ = ev_tx
                .send(SessionEvent::History {
                    messages: parse_transcript_messages(&result),
                })
                .await;
            Ok(())
        }
        SessionCommand::Rewind {
            text,
            truncate_before_row_id,
            confirm_empty_truncate,
        } => {
            let Some(sid) = session_id.as_deref() else {
                return Err(GatewayError::Protocol("no session_id yet".into()));
            };
            let result = client
                .request(
                    "prompt.submit",
                    rewind_submit_params(
                        sid,
                        &text,
                        truncate_before_row_id,
                        confirm_empty_truncate,
                    ),
                )
                .await?;
            let _ = ev_tx
                .send(SessionEvent::RewindApplied {
                    survivor_user_row_ids: parse_survivor_user_row_ids(&result),
                })
                .await;
            Ok(())
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
        SessionCommand::ClipboardPaste => {
            let Some(sid) = session_id.as_deref() else {
                return Err(GatewayError::Protocol("no session_id yet".into()));
            };
            let result = client
                .request("clipboard.paste", json!({ "session_id": sid }))
                .await?;
            let attached = result
                .get("attached")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let msg = result
                .get("message")
                .or_else(|| result.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or(if attached {
                    "clipboard image attached"
                } else {
                    "no image on clipboard"
                });
            let _ = ev_tx.send(SessionEvent::Status(msg.to_string())).await;
            Ok(())
        }
        SessionCommand::ListSpawnTrees => {
            let result = client.request("spawn_tree.list", json!({})).await?;
            let _ = ev_tx.send(parse_spawn_trees(&result)).await;
            Ok(())
        }
        SessionCommand::LoadSpawnTree { path } => {
            let result = client
                .request("spawn_tree.load", json!({ "path": path }))
                .await?;
            let _ = ev_tx
                .send(SessionEvent::Status(format!(
                    "loaded spawn tree {}",
                    result
                        .get("label")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&path)
                )))
                .await;
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
        .request("prompt.submit", ordinary_submit_params(sid, text))
        .await?;
    Ok(())
}

async fn emit_err(ev_tx: &mpsc::Sender<SessionEvent>, e: GatewayError) {
    let message = crate::user_messages::gateway_failed(&e);
    let _ = ev_tx.send(SessionEvent::Error { message }).await;
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
