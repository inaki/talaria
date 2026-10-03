//! Live gateway session: JSON-RPC commands on a spawned tui_gateway.

use std::time::Duration;

use serde_json::{json, Value};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::gateway::{GatewayClient, GatewayError};

use super::parse::{
    map_gateway_event, match_saved_session, ordinary_submit_params, parse_active_list,
    parse_catalog, parse_command_result, parse_config_set, parse_delegation, parse_mcp_catalog,
    parse_mcp_servers, parse_model_disconnected, parse_model_key_saved, parse_model_options,
    parse_plugins_list, parse_saved_list, parse_saved_sessions, parse_skills_list,
    parse_spawn_trees, parse_survivor_user_row_ids, parse_transcript_messages, parse_usage,
    resize_params, rewind_submit_params,
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
            create_session(client, session_id, cwd, cols, ev_tx).await
        }
        SessionCommand::ResumeLatest { cwd, cols } => {
            resume_or_create(client, session_id, None, cwd, cols, ev_tx).await
        }
        SessionCommand::ResumeQuery { query, cwd, cols } => {
            resume_or_create(client, session_id, Some(query), cwd, cols, ev_tx).await
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
            apply_switch_result(client, session_id, &result, ev_tx).await;
            Ok(())
        }
        SessionCommand::Activate { session_id: sid } => {
            let result = client
                .request("session.activate", json!({ "session_id": sid }))
                .await?;
            apply_switch_result(client, session_id, &result, ev_tx).await;
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
            question_id,
            answers,
        } => {
            let mut params = json!({ "request_id": request_id, "answers": answers });
            params["answer"] = answers.clone();
            if let Some(qid) = question_id {
                params["question_id"] = json!(qid);
            }
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
            // Gateway `_sess_nowait` looks up params.session_id. A missing
            // id is "session not found" — and a drag-resize would spam that
            // into the transcript if we treated it as a hard error.
            let Some(sid) = session_id.as_deref() else {
                return Ok(());
            };
            let _ = client
                .request("terminal.resize", resize_params(sid, cols, rows))
                .await;
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
            apply_switch_result(client, session_id, &result, ev_tx).await;
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
        SessionCommand::SteerSubagent { subagent_id, text } => {
            let mut params = json!({ "subagent_id": subagent_id, "text": text });
            if let Some(sid) = session_id.as_deref() {
                params["session_id"] = json!(sid);
            }
            let result = client.request("subagent.steer", params).await?;
            let status = result
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("queued");
            let _ = ev_tx
                .send(SessionEvent::Status(format!(
                    "subagent {subagent_id} steer {status}"
                )))
                .await;
            Ok(())
        }
        SessionCommand::FetchUsage => {
            let mut params = json!({});
            if let Some(sid) = session_id.as_deref() {
                params["session_id"] = json!(sid);
            }
            match client.request("session.usage", params).await {
                Ok(result) => {
                    let _ = ev_tx.send(parse_usage(&result)).await;
                }
                Err(e) => {
                    let _ = ev_tx
                        .send(SessionEvent::Error {
                            message: crate::user_messages::gateway_failed(&e),
                        })
                        .await;
                }
            }
            Ok(())
        }
        SessionCommand::CloseSession {
            session_id: sid,
            cols,
        } => {
            let _ = client
                .request("session.close", json!({ "session_id": sid }))
                .await;
            let closing_current = session_id.as_deref() == Some(sid.as_str());
            let _ = ev_tx
                .send(SessionEvent::Status(format!("closed {sid}")))
                .await;
            if closing_current {
                *session_id = None;
                *agent_ready = false;
                return create_session(client, session_id, None, cols, ev_tx).await;
            }
            Ok(())
        }
        SessionCommand::ShellExec { command, cwd } => {
            let result = run_bang_command(&command, &cwd).await;
            let _ = ev_tx.send(result).await;
            Ok(())
        }
        SessionCommand::AttachImage { path, label } => {
            let Some(sid) = session_id.as_deref() else {
                return Err(GatewayError::Protocol("no session_id yet".into()));
            };
            // `label` is additive — gateways that ignore unknown params still
            // attach, and the ordinal then survives as attach order alone.
            let result = client
                .request(
                    "image.attach",
                    json!({ "session_id": sid, "path": path, "label": label }),
                )
                .await?;
            let notice = result
                .get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{label} attached"));
            let _ = ev_tx.send(SessionEvent::Status(notice)).await;
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
        SessionCommand::SaveSpawnTree => {
            let mut params = json!({});
            if let Some(sid) = session_id.as_deref() {
                params["session_id"] = json!(sid);
            }
            let result = client.request("spawn_tree.save", params).await?;
            let label = result
                .get("label")
                .or_else(|| result.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("tree");
            let _ = ev_tx
                .send(SessionEvent::Status(format!("saved spawn tree {label}")))
                .await;
            if let Ok(listed) = client.request("spawn_tree.list", json!({})).await {
                let _ = ev_tx.send(parse_spawn_trees(&listed)).await;
            }
            Ok(())
        }
        SessionCommand::FetchModelOptions { refresh } => {
            let mut params = json!({ "include_unconfigured": true });
            if let Some(sid) = session_id.as_deref() {
                params["session_id"] = json!(sid);
            }
            if refresh {
                params["refresh"] = json!(true);
            }
            match client.request("model.options", params).await {
                Ok(result) => {
                    let _ = ev_tx.send(parse_model_options(&result)).await;
                }
                Err(e) => {
                    let _ = ev_tx
                        .send(SessionEvent::ModelOptions {
                            providers: Vec::new(),
                            model: String::new(),
                            error: Some(crate::user_messages::gateway_failed(&e)),
                        })
                        .await;
                }
            }
            Ok(())
        }
        SessionCommand::SaveModelKey { slug, api_key } => {
            let mut params = json!({ "slug": slug, "api_key": api_key });
            if let Some(sid) = session_id.as_deref() {
                params["session_id"] = json!(sid);
            }
            match client.request("model.save_key", params).await {
                Ok(result) => {
                    let _ = ev_tx.send(parse_model_key_saved(&result)).await;
                }
                Err(e) => {
                    let _ = ev_tx
                        .send(SessionEvent::ModelKeySaved {
                            provider: crate::session::ModelProvider {
                                slug,
                                name: String::new(),
                                authenticated: false,
                                is_current: false,
                                auth_type: String::new(),
                                key_env: None,
                                models: Vec::new(),
                                total_models: 0,
                                warning: None,
                            },
                            error: Some(crate::user_messages::gateway_failed(&e)),
                        })
                        .await;
                }
            }
            Ok(())
        }
        SessionCommand::DisconnectModel { slug } => {
            let mut params = json!({ "slug": slug });
            if let Some(sid) = session_id.as_deref() {
                params["session_id"] = json!(sid);
            }
            match client.request("model.disconnect", params).await {
                Ok(result) => {
                    let _ = ev_tx.send(parse_model_disconnected(slug, &result)).await;
                }
                Err(_) => {
                    let _ = ev_tx
                        .send(SessionEvent::ModelDisconnected { slug, ok: false })
                        .await;
                }
            }
            Ok(())
        }
        SessionCommand::SetConfig {
            key,
            value,
            confirm_expensive_model,
        } => {
            let mut params = json!({
                "key": key,
                "value": value,
                "confirm_expensive_model": confirm_expensive_model,
            });
            if let Some(sid) = session_id.as_deref() {
                params["session_id"] = json!(sid);
            }
            let result = client.request("config.set", params).await?;
            let _ = ev_tx.send(parse_config_set(key, &result)).await;
            Ok(())
        }
        SessionCommand::FetchSkills => {
            match client
                .request("skills.manage", json!({ "action": "list" }))
                .await
            {
                Ok(result) => {
                    let _ = ev_tx.send(parse_skills_list(&result)).await;
                }
                Err(e) => {
                    let _ = ev_tx
                        .send(SessionEvent::SkillsList {
                            groups: Vec::new(),
                            error: Some(crate::user_messages::gateway_failed(&e)),
                        })
                        .await;
                }
            }
            Ok(())
        }
        SessionCommand::InstallSkill { query } => {
            match client
                .request(
                    "skills.manage",
                    json!({ "action": "install", "query": query }),
                )
                .await
            {
                Ok(result) => {
                    let ok = result
                        .get("installed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    let name = result
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&query)
                        .to_string();
                    let _ = ev_tx
                        .send(SessionEvent::SkillInstalled {
                            name,
                            ok,
                            error: None,
                        })
                        .await;
                }
                Err(e) => {
                    let _ = ev_tx
                        .send(SessionEvent::SkillInstalled {
                            name: query,
                            ok: false,
                            error: Some(crate::user_messages::gateway_failed(&e)),
                        })
                        .await;
                }
            }
            Ok(())
        }
        SessionCommand::FetchPlugins => {
            match client
                .request("plugins.manage", json!({ "action": "list" }))
                .await
            {
                Ok(result) => {
                    let _ = ev_tx.send(parse_plugins_list(&result)).await;
                }
                Err(e) => {
                    let _ = ev_tx
                        .send(SessionEvent::PluginsList {
                            plugins: Vec::new(),
                            error: Some(crate::user_messages::gateway_failed(&e)),
                        })
                        .await;
                }
            }
            Ok(())
        }
        SessionCommand::TogglePlugin { key, enable } => {
            match client
                .request(
                    "plugins.manage",
                    json!({ "action": "toggle", "key": key, "enable": enable }),
                )
                .await
            {
                Ok(result) => {
                    let plugin = result
                        .get("plugin")
                        .and_then(super::parse::parse_plugin_row);
                    let _ = ev_tx
                        .send(SessionEvent::PluginToggled {
                            plugin,
                            ok: result.get("ok").and_then(|v| v.as_bool()).unwrap_or(true),
                        })
                        .await;
                }
                Err(_) => {
                    let _ = ev_tx
                        .send(SessionEvent::PluginToggled {
                            plugin: None,
                            ok: false,
                        })
                        .await;
                }
            }
            Ok(())
        }
        SessionCommand::FetchMcpServers => {
            match client.request("mcp.servers.list", json!({})).await {
                Ok(result) => {
                    let _ = ev_tx.send(parse_mcp_servers(&result)).await;
                }
                Err(e) => {
                    let _ = ev_tx
                        .send(SessionEvent::McpServers {
                            servers: Vec::new(),
                            error: Some(crate::user_messages::gateway_failed(&e)),
                        })
                        .await;
                }
            }
            Ok(())
        }
        SessionCommand::FetchMcpCatalog => {
            match client.request("mcp.catalog", json!({})).await {
                Ok(result) => {
                    let _ = ev_tx.send(parse_mcp_catalog(&result)).await;
                }
                Err(e) => {
                    let _ = ev_tx
                        .send(SessionEvent::McpCatalog {
                            servers: Vec::new(),
                            error: Some(crate::user_messages::gateway_failed(&e)),
                        })
                        .await;
                }
            }
            Ok(())
        }
        SessionCommand::AddMcp { name, preset } => {
            match client
                .request("mcp.servers.add", json!({ "name": name, "preset": preset }))
                .await
            {
                Ok(_) => {
                    let _ = ev_tx
                        .send(SessionEvent::McpChanged {
                            name,
                            ok: true,
                            error: None,
                        })
                        .await;
                }
                Err(e) => {
                    let _ = ev_tx
                        .send(SessionEvent::McpChanged {
                            name,
                            ok: false,
                            error: Some(crate::user_messages::gateway_failed(&e)),
                        })
                        .await;
                }
            }
            Ok(())
        }
        SessionCommand::RemoveMcp { name } => {
            match client
                .request("mcp.servers.remove", json!({ "name": name }))
                .await
            {
                Ok(_) => {
                    let _ = ev_tx
                        .send(SessionEvent::McpChanged {
                            name,
                            ok: true,
                            error: None,
                        })
                        .await;
                }
                Err(e) => {
                    let _ = ev_tx
                        .send(SessionEvent::McpChanged {
                            name,
                            ok: false,
                            error: Some(crate::user_messages::gateway_failed(&e)),
                        })
                        .await;
                }
            }
            Ok(())
        }
    }
}

async fn run_bang_command(command: &str, cwd: &str) -> SessionEvent {
    let mut cmd = {
        #[cfg(windows)]
        {
            let mut c = tokio::process::Command::new("cmd");
            c.arg("/C").arg(command);
            c
        }
        #[cfg(not(windows))]
        {
            let mut c = tokio::process::Command::new("sh");
            c.arg("-c").arg(command);
            c
        }
    };
    if !cwd.is_empty() {
        cmd.current_dir(cwd);
    }
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }
    cmd.kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let started = std::time::Instant::now();
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return SessionEvent::ShellResult {
                command: command.to_string(),
                output: format!("spawn error: {e}"),
                code: Some(1),
                duration_ms: started.elapsed().as_millis() as u64,
            };
        }
    };
    let pid = child.id();
    match tokio::time::timeout(
        Duration::from_secs(crate::shell::TIMEOUT_SECS),
        child.wait_with_output(),
    )
    .await
    {
        Ok(Ok(out)) => SessionEvent::ShellResult {
            command: command.to_string(),
            output: crate::shell::format_output(&out.stdout, &out.stderr),
            code: out.status.code(),
            duration_ms: started.elapsed().as_millis() as u64,
        },
        Ok(Err(e)) => SessionEvent::ShellResult {
            command: command.to_string(),
            output: format!("spawn error: {e}"),
            code: Some(1),
            duration_ms: started.elapsed().as_millis() as u64,
        },
        Err(_) => {
            if let Some(pid) = pid {
                crate::shell::kill_process_group(pid);
            }
            SessionEvent::ShellResult {
                command: command.to_string(),
                output: format!("timed out after {}s", crate::shell::TIMEOUT_SECS),
                code: Some(124),
                duration_ms: started.elapsed().as_millis() as u64,
            }
        }
    }
}

async fn create_session(
    client: &GatewayClient,
    session_id: &mut Option<String>,
    cwd: Option<String>,
    cols: u16,
    ev_tx: &mpsc::Sender<SessionEvent>,
) -> Result<(), GatewayError> {
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
    client: &GatewayClient,
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
    if let Ok(catalog) = client.request("commands.catalog", json!({})).await {
        let _ = ev_tx.send(parse_catalog(&catalog)).await;
    }
}

async fn resume_or_create(
    client: &GatewayClient,
    session_id: &mut Option<String>,
    query: Option<String>,
    cwd: Option<String>,
    cols: u16,
    ev_tx: &mpsc::Sender<SessionEvent>,
) -> Result<(), GatewayError> {
    let listed = client
        .request("session.list", json!({ "limit": 80 }))
        .await
        .unwrap_or(json!({}));
    let sessions = parse_saved_sessions(&listed);
    let picked = match query.as_deref() {
        None => sessions.first().map(|s| (s.id.clone(), s.title.clone())),
        Some(q) => match_saved_session(&sessions, q).map(|id| {
            let title = sessions
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.title.clone())
                .unwrap_or_default();
            (id, title)
        }),
    };
    let Some((id, title)) = picked else {
        if let Some(q) = query {
            let _ = ev_tx
                .send(SessionEvent::Status(format!(
                    "no session matching {q}; starting fresh"
                )))
                .await;
        }
        return create_session(client, session_id, cwd, cols, ev_tx).await;
    };
    let result = client
        .request("session.resume", json!({ "session_id": id, "cols": cols }))
        .await?;
    apply_switch_result(client, session_id, &result, ev_tx).await;
    let label = if title.is_empty() { id } else { title };
    let _ = ev_tx
        .send(SessionEvent::Status(format!("resumed {label}")))
        .await;
    Ok(())
}
