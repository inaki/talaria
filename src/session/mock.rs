//! Mock session for `--dev --mock=`. Same SessionEvent surface as live.

use std::time::Duration;

use serde_json::json;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use super::{
    ActiveSession, SavedSession, SessionApi, SessionCommand, SessionEvent, SlashCommand,
    SpawnTreeEntry, SubagentKind, SubagentRow, TranscriptMessage,
};

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
            info: json!({
                "model": "z-ai/glm-5.2",
                "cwd": ".",
                "lazy": false,
                "version": "0.20.5",
                "release_date": "2026.8.19",
                "tools": {
                    "browser": ["browser_back", "browser_click", "browser_exec"],
                    "clarify": ["clarify"],
                    "code_execution": ["execute_code"],
                    "delegation": ["delegate_task"]
                },
                "skills": {
                    "apple": ["apple-notes", "findmy"],
                    "github": ["codebase-inspection", "github-auth"],
                    "software-development": ["dogfood"]
                }
            }),
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
                            SavedSession {
                                id: "sess-gamma".into(),
                                title: "Planning".into(),
                                preview: "phase B returning workspace".into(),
                                source: "tui".into(),
                                message_count: 9,
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
            SessionCommand::ResumeLatest { .. } => {
                mock_resume(&ev_tx, "sess-alpha").await;
            }
            SessionCommand::ResumeQuery { query, .. } => {
                let id = if query == "sess-beta" || query.eq_ignore_ascii_case("cli notes") {
                    "sess-beta"
                } else {
                    "sess-alpha"
                };
                mock_resume(&ev_tx, id).await;
            }
            SessionCommand::Resume {
                session_id: sid, ..
            }
            | SessionCommand::Activate { session_id: sid } => {
                mock_resume(&ev_tx, &sid).await;
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
            SessionCommand::AttachImage { path, label } => {
                let _ = ev_tx
                    .send(SessionEvent::Status(format!("{label} attached · {path}")))
                    .await;
            }
            SessionCommand::ClipboardPaste => {
                let _ = ev_tx
                    .send(SessionEvent::Status("clipboard paste (mock)".into()))
                    .await;
            }
            SessionCommand::ListSpawnTrees => {
                let _ = ev_tx
                    .send(SessionEvent::SpawnTrees {
                        entries: vec![SpawnTreeEntry {
                            path: "mock-tree.json".into(),
                            label: "mock run".into(),
                            count: 2,
                        }],
                    })
                    .await;
            }
            SessionCommand::LoadSpawnTree { path } => {
                let _ = ev_tx
                    .send(SessionEvent::Status(format!("loaded {path} (mock)")))
                    .await;
            }
            SessionCommand::SaveSpawnTree => {
                let _ = ev_tx
                    .send(SessionEvent::Status(
                        "saved spawn tree mock-tree.json".into(),
                    ))
                    .await;
                let _ = ev_tx
                    .send(SessionEvent::SpawnTrees {
                        entries: vec![SpawnTreeEntry {
                            path: "mock-tree.json".into(),
                            label: "mock run".into(),
                            count: 2,
                        }],
                    })
                    .await;
            }
            SessionCommand::FetchHistory => {
                let _ = ev_tx
                    .send(SessionEvent::History {
                        messages: vec![
                            TranscriptMessage {
                                role: "user".into(),
                                text: "first question".into(),
                                row_id: Some(11),
                                display_kind: None,
                            },
                            TranscriptMessage {
                                role: "assistant".into(),
                                text: "first answer".into(),
                                row_id: Some(12),
                                display_kind: None,
                            },
                            TranscriptMessage {
                                role: "user".into(),
                                text: "second question".into(),
                                row_id: Some(13),
                                display_kind: None,
                            },
                        ],
                    })
                    .await;
            }
            SessionCommand::Rewind { text, .. } => {
                let _ = ev_tx
                    .send(SessionEvent::RewindApplied {
                        survivor_user_row_ids: Some(vec![Some(101)]),
                    })
                    .await;
                play_scenario(scenario, &text, &ev_tx).await;
            }
            SessionCommand::FetchModelOptions { .. } => {
                let _ = ev_tx.send(mock_model_options()).await;
            }
            SessionCommand::SaveModelKey { slug, .. } => {
                let _ = ev_tx
                    .send(SessionEvent::ModelKeySaved {
                        provider: crate::session::ModelProvider {
                            slug: slug.clone(),
                            name: slug,
                            authenticated: true,
                            is_current: false,
                            auth_type: "api_key".into(),
                            key_env: Some("MOCK_API_KEY".into()),
                            models: vec!["mock/alpha".into(), "mock/beta".into()],
                            total_models: 2,
                            warning: None,
                        },
                        error: None,
                    })
                    .await;
            }
            SessionCommand::DisconnectModel { slug } => {
                let _ = ev_tx
                    .send(SessionEvent::ModelDisconnected { slug, ok: true })
                    .await;
            }
            SessionCommand::FetchSkills => {
                let _ = ev_tx
                    .send(SessionEvent::SkillsList {
                        groups: vec![("bundled".into(), vec!["plan".into(), "review".into()])],
                        error: None,
                    })
                    .await;
            }
            SessionCommand::InstallSkill { query } => {
                let _ = ev_tx
                    .send(SessionEvent::SkillInstalled {
                        name: query,
                        ok: true,
                        error: None,
                    })
                    .await;
            }
            SessionCommand::FetchPlugins => {
                let _ = ev_tx
                    .send(SessionEvent::PluginsList {
                        plugins: vec![crate::session::PluginRow {
                            name: "demo".into(),
                            key: "demo".into(),
                            version: "1".into(),
                            description: "mock plugin".into(),
                            source: "user".into(),
                            status: "enabled".into(),
                        }],
                        error: None,
                    })
                    .await;
            }
            SessionCommand::TogglePlugin { key, enable } => {
                let _ = ev_tx
                    .send(SessionEvent::PluginToggled {
                        plugin: Some(crate::session::PluginRow {
                            name: key.clone(),
                            key,
                            version: "1".into(),
                            description: String::new(),
                            source: "user".into(),
                            status: if enable { "enabled" } else { "disabled" }.into(),
                        }),
                        ok: true,
                    })
                    .await;
            }
            SessionCommand::FetchMcpServers => {
                let _ = ev_tx
                    .send(SessionEvent::McpServers {
                        servers: vec![crate::session::McpServer {
                            name: "filesystem".into(),
                            transport: "stdio".into(),
                            enabled: true,
                            auth: String::new(),
                        }],
                        error: None,
                    })
                    .await;
            }
            SessionCommand::FetchMcpCatalog => {
                let _ = ev_tx
                    .send(SessionEvent::McpCatalog {
                        servers: vec![crate::session::McpCatalogEntry {
                            name: "n8n".into(),
                            description: "n8n automation".into(),
                            installed: false,
                            enabled: false,
                        }],
                        error: None,
                    })
                    .await;
            }
            SessionCommand::AddMcp { name, .. } | SessionCommand::RemoveMcp { name } => {
                let _ = ev_tx
                    .send(SessionEvent::McpChanged {
                        name,
                        ok: true,
                        error: None,
                    })
                    .await;
            }
            SessionCommand::SetConfig { key, value, .. } => {
                let model = value
                    .split_whitespace()
                    .next()
                    .unwrap_or(&value)
                    .to_string();
                let _ = ev_tx
                    .send(SessionEvent::ConfigSet {
                        key,
                        value: Some(model.clone()),
                        warning: None,
                        deferred: false,
                        confirm_required: false,
                        confirm_message: None,
                        info: Some(json!({ "model": model })),
                    })
                    .await;
            }
            SessionCommand::ShellExec { command, cwd } => {
                let r = crate::shell::run_shell_command(&command, &cwd);
                let _ = ev_tx
                    .send(SessionEvent::ShellResult {
                        command: r.command,
                        output: r.output,
                        code: r.code,
                        duration_ms: r.duration_ms,
                    })
                    .await;
            }
            SessionCommand::Create { .. } => {
                let _ = ev_tx
                    .send(SessionEvent::SessionCreated {
                        session_id: "mock-session-new".into(),
                        stored_session_id: Some("mock-store-new".into()),
                        info: Some(json!({"model": "mock-model"})),
                    })
                    .await;
            }
            SessionCommand::FetchUsage => {
                let _ = ev_tx
                    .send(SessionEvent::Usage(crate::session::UsageSnapshot {
                        calls: 2,
                        input: 800,
                        output: 120,
                        total: 920,
                        context_used: 4000,
                        context_max: 128000,
                        context_percent: 3,
                        cost_usd: Some(0.01),
                        model: "mock-model".into(),
                        credits_lines: vec!["$10.00 remaining".into()],
                    }))
                    .await;
            }
            SessionCommand::SteerSubagent { subagent_id, text } => {
                let _ = ev_tx
                    .send(SessionEvent::Status(format!(
                        "subagent {subagent_id} steer queued: {text}"
                    )))
                    .await;
            }
            SessionCommand::CloseSession {
                session_id: sid, ..
            } => {
                let _ = ev_tx
                    .send(SessionEvent::Status(format!("closed {sid}")))
                    .await;
                let _ = ev_tx
                    .send(SessionEvent::SessionCreated {
                        session_id: "mock-session-new".into(),
                        stored_session_id: Some("mock-store-new".into()),
                        info: Some(json!({"model": "mock-model"})),
                    })
                    .await;
            }
            SessionCommand::Shutdown | SessionCommand::Close => break,
            _ => {}
        }
    }
}

async fn mock_resume(ev_tx: &mpsc::Sender<SessionEvent>, sid: &str) {
    let _ = ev_tx
        .send(SessionEvent::Status(format!("resumed {sid}")))
        .await;
    let _ = ev_tx
        .send(SessionEvent::Transcript {
            session_id: sid.to_string(),
            messages: vec![
                TranscriptMessage {
                    role: "user".into(),
                    text: format!("(resumed {sid})"),
                    row_id: Some(11),
                    display_kind: None,
                },
                TranscriptMessage {
                    role: "assistant".into(),
                    text: "Welcome back. This is mock history.".into(),
                    row_id: Some(12),
                    display_kind: None,
                },
            ],
        })
        .await;
    let _ = ev_tx
        .send(SessionEvent::SessionCreated {
            session_id: sid.to_string(),
            stored_session_id: Some(sid.to_string()),
            info: Some(json!({"model": "mock-model"})),
        })
        .await;
}

fn mock_model_options() -> SessionEvent {
    SessionEvent::ModelOptions {
        providers: vec![
            crate::session::ModelProvider {
                slug: "openrouter".into(),
                name: "OpenRouter".into(),
                authenticated: true,
                is_current: true,
                auth_type: "api_key".into(),
                key_env: Some("OPENROUTER_API_KEY".into()),
                models: vec![
                    "anthropic/claude-sonnet-4.6".into(),
                    "openai/gpt-5.4".into(),
                ],
                total_models: 2,
                warning: None,
            },
            crate::session::ModelProvider {
                slug: "anthropic".into(),
                name: "Anthropic".into(),
                authenticated: false,
                is_current: false,
                auth_type: "api_key".into(),
                key_env: Some("ANTHROPIC_API_KEY".into()),
                models: Vec::new(),
                total_models: 0,
                warning: Some("paste ANTHROPIC_API_KEY to activate".into()),
            },
        ],
        model: "anthropic/claude-sonnet-4.6".into(),
        error: None,
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
            SlashCommand {
                name: "trees".into(),
                help: "saved spawn trees".into(),
            },
            SlashCommand {
                name: "rewind".into(),
                help: "regenerate from a past user turn".into(),
            },
            SlashCommand {
                name: "skin".into(),
                help: "color skin".into(),
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
                    preview: None,
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

    #[tokio::test]
    async fn mock_streaming_emits_complete() {
        let rt = tokio::runtime::Handle::current();
        let mut mock = MockSession::start(MockScenario::Streaming, &rt);
        let mut rx = mock.take_events().unwrap();
        assert!(mock.take_events().is_none());
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
