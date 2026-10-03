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
    Home,
    Tools,
    Approval,
    Error,
    Subagent,
}

impl MockScenario {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "home" | "empty" | "splash" => Self::Home,
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

    let mut user_turns: Vec<String> = Vec::new();
    // Grows on every usage fetch so the status-bar cost visibly ticks up.
    let mut spent_usd = 0.0_f64;
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            SessionCommand::Submit { text } => {
                user_turns.push(text.clone());
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
                let sessions = if matches!(scenario, MockScenario::Home) {
                    Vec::new()
                } else {
                    mock_saved_sessions()
                };
                let _ = ev_tx.send(SessionEvent::SavedList { sessions }).await;
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
                stream_reply(
                    &ev_tx,
                    &format!("Approved **{choice}**. Continuing the mock turn."),
                )
                .await;
            }
            SessionCommand::RespondClarify {
                answers,
                question_id,
                ..
            } => {
                let picked = clarify_pick(&answers);
                // Batch: Hermes only resumes once the last question is locked.
                if question_id
                    .as_deref()
                    .is_some_and(|q| q != MOCK_BATCH_LAST_QID)
                {
                    let _ = ev_tx
                        .send(SessionEvent::Status(format!("locked: {picked}")))
                        .await;
                } else if picked.trim().is_empty() {
                    let _ = ev_tx
                        .send(SessionEvent::Status("clarify cancelled".into()))
                        .await;
                } else {
                    stream_reply(
                        &ev_tx,
                        &format!("You answered **{picked}**.\n\nContinuing with that choice."),
                    )
                    .await;
                }
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
                        messages: mock_history(&user_turns),
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
                spent_usd += 0.0137;
                let _ = ev_tx
                    .send(SessionEvent::Usage(crate::session::UsageSnapshot {
                        calls: 2,
                        input: 800,
                        output: 120,
                        total: 920,
                        context_used: 4000,
                        context_max: 128000,
                        context_percent: 3,
                        cost_usd: Some(spent_usd),
                        cost_status: Some("estimated".into()),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MockScript {
    Help,
    Default,
    Code,
    Document,
    Clarify,
    ClarifyType,
    ClarifyBatch,
    Tools,
    ToolError,
    Error,
    Approval,
    Subagent,
}

fn script_from_text(text: &str, fallback: MockScenario) -> MockScript {
    let t = text.to_ascii_lowercase();
    let words: Vec<&str> = t
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let has = |keys: &[&str]| words.iter().any(|w| keys.contains(w));
    if has(&["help", "keywords"]) {
        return MockScript::Help;
    }
    if has(&["codeblock", "code", "rust"]) {
        return MockScript::Code;
    }
    if has(&["pdf", "doc", "docs", "document"]) {
        return MockScript::Document;
    }
    if has(&["batch", "questions"]) {
        return MockScript::ClarifyBatch;
    }
    if has(&["ask", "clarify", "select", "question", "choice"]) {
        return MockScript::Clarify;
    }
    if has(&["type", "typed", "input"]) {
        return MockScript::ClarifyType;
    }
    if has(&["toolerror"]) {
        return MockScript::ToolError;
    }
    if has(&["tools", "tool"]) {
        return MockScript::Tools;
    }
    if has(&["error", "fail", "crash"]) {
        return MockScript::Error;
    }
    if has(&["approval", "approve", "danger"]) {
        return MockScript::Approval;
    }
    if has(&["agent", "subagent", "agents"]) {
        return MockScript::Subagent;
    }
    match fallback {
        MockScenario::Streaming | MockScenario::Home => MockScript::Default,
        MockScenario::Tools => MockScript::Tools,
        MockScenario::Approval => MockScript::Approval,
        MockScenario::Error => MockScript::Error,
        MockScenario::Subagent => MockScript::Subagent,
    }
}

async fn play_scenario(scenario: MockScenario, text: &str, ev_tx: &mpsc::Sender<SessionEvent>) {
    play_script(script_from_text(text, scenario), text, ev_tx).await;
}

async fn play_script(script: MockScript, text: &str, ev_tx: &mpsc::Sender<SessionEvent>) {
    match script {
        MockScript::Help => stream_reply(ev_tx, HELP_REPLY).await,
        MockScript::Default => {
            let reply = format!(
                "You said **{text}**.\n\nThis is the streaming mock. Type a keyword:\n\n{HELP_LIST}"
            );
            stream_reply(ev_tx, &reply).await;
        }
        MockScript::Code => stream_reply(ev_tx, CODE_REPLY).await,
        MockScript::Document => play_document(ev_tx).await,
        MockScript::Clarify => play_clarify(ev_tx, true).await,
        MockScript::ClarifyType => play_clarify(ev_tx, false).await,
        MockScript::ClarifyBatch => play_clarify_batch(ev_tx).await,
        MockScript::Tools => play_tools(ev_tx).await,
        MockScript::ToolError => play_tool_error(ev_tx).await,
        MockScript::Error => {
            let _ = ev_tx
                .send(SessionEvent::Error {
                    message: "Mock provider error: the model is unavailable.".into(),
                })
                .await;
        }
        MockScript::Approval => {
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
        MockScript::Subagent => play_subagent(ev_tx).await,
    }
}

const HELP_LIST: &str = "\
- `code` — rust fence in the document\n\
- `pdf` — read a document, then summarize\n\
- `ask` — pick a choice to continue\n\
- `type` — type an answer to continue\n\
- `batch` — three clarify questions in a row\n\
- `tools` — tool chips\n\
- `error` — provider error\n\
- `toolerror` — failed tool\n\
- `approval` — danger modal\n\
- `agent` — subagent rollup\n\
- `help` — this list";

const HELP_REPLY: &str = "\
# Mock keywords\n\n\
Enter one of these in the composer:\n\n\
- `code` — rust fence in the document\n\
- `pdf` — read a document, then summarize\n\
- `ask` — pick a choice to continue\n\
- `type` — type an answer to continue\n\
- `batch` — three clarify questions in a row\n\
- `tools` — tool chips\n\
- `error` — provider error\n\
- `toolerror` — failed tool\n\
- `approval` — danger modal\n\
- `agent` — subagent rollup";

const CODE_REPLY: &str = "\
# Token verification\n\n\
The async path sits behind a bounded channel so the event loop is not blocked.\n\n\
```rust\npub async fn verify_token(token: &str) -> Result<Claims, AuthError> {\n    let decoded = jsonwebtoken::decode::<Claims>(\n        token,\n        &KEYS.decoding,\n        &Validation::default(),\n    )?;\n\n    // Validate expiration claim explicitly\n    Ok(decoded.claims)\n}\n```\n\nEvaluating security boundaries.";

const DOC_REPLY: &str = "\
# Auth flow (from `auth_flow.pdf`)\n\n\
## Overview\n\n\
The current implementation relies on a synchronous blocking call in the event loop. We need an asynchronous bounded channel to prevent stream starvation during high-load authentication spikes.\n\n\
## Claims\n\n\
- `sub` — user id\n\
- `exp` — unix expiry\n\
- `aud` — API audience\n\n\
## Next\n\n\
See the `code` mock for the Rust path.";

async fn stream_reply(ev_tx: &mpsc::Sender<SessionEvent>, reply: &str) {
    let _ = ev_tx
        .send(SessionEvent::Thinking {
            text: "thinking…".into(),
        })
        .await;
    tokio::time::sleep(Duration::from_millis(80)).await;
    for chunk in chunk_chars(reply, 6) {
        let _ = ev_tx
            .send(SessionEvent::MessageDelta {
                session_id: Some("mock-session".into()),
                text: chunk,
                rendered: None,
            })
            .await;
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let _ = ev_tx
        .send(SessionEvent::MessageComplete {
            session_id: Some("mock-session".into()),
            text: Some(reply.to_string()),
        })
        .await;
}

async fn play_document(ev_tx: &mpsc::Sender<SessionEvent>) {
    let _ = ev_tx
        .send(SessionEvent::ToolStart {
            tool_id: "read-1".into(),
            name: Some("read_file".into()),
            args: Some("path=\"docs/auth_flow.pdf\"".into()),
            preview: Some("auth_flow.pdf".into()),
        })
        .await;
    tokio::time::sleep(Duration::from_millis(90)).await;
    let _ = ev_tx
        .send(SessionEvent::ToolProgress {
            tool_id: Some("read-1".into()),
            name: Some("read_file".into()),
            preview: Some("extracting pages 1–3…".into()),
        })
        .await;
    tokio::time::sleep(Duration::from_millis(90)).await;
    let _ = ev_tx
        .send(SessionEvent::ToolComplete {
            tool_id: "read-1".into(),
            name: Some("read_file".into()),
            result: Some(
                "PDF 3 pages · Auth flow\n\nSynchronous blocking call in the event loop…".into(),
            ),
            error: None,
        })
        .await;
    stream_reply(ev_tx, DOC_REPLY).await;
}

const MOCK_BATCH_LAST_QID: &str = "q3";

/// Multi-question clarify in the shape current Hermes sends (`questions: [...]`).
async fn play_clarify_batch(ev_tx: &mpsc::Sender<SessionEvent>) {
    let _ = ev_tx
        .send(SessionEvent::ClarifyRequest {
            request_id: "mock-clarify-batch".into(),
            payload: json!({"questions": [
                {
                    "qid": "q1",
                    "question": "Which runtime should we target?",
                    "choices": ["Tokio", "async-std", "Keep blocking"],
                    "multi_select": false,
                },
                {
                    "qid": "q2",
                    "question": "What should the new crate be called?",
                    "choices": [],
                    "multi_select": false,
                },
                {
                    "qid": MOCK_BATCH_LAST_QID,
                    "question": "Ship behind a feature flag?",
                    "choices": ["Yes", "No"],
                    "multi_select": false,
                },
            ]}),
        })
        .await;
}

async fn play_clarify(ev_tx: &mpsc::Sender<SessionEvent>, with_choices: bool) {
    let payload = if with_choices {
        json!({
            "question": "Which runtime should we target for the rewrite?",
            "choices": ["Tokio", "async-std", "Keep blocking"]
        })
    } else {
        json!({
            "question": "Name the session title for this auth work."
        })
    };
    let _ = ev_tx
        .send(SessionEvent::ClarifyRequest {
            request_id: if with_choices {
                "mock-clarify".into()
            } else {
                "mock-clarify-type".into()
            },
            payload,
        })
        .await;
}

async fn play_tools(ev_tx: &mpsc::Sender<SessionEvent>) {
    let _ = ev_tx
        .send(SessionEvent::ToolStart {
            tool_id: "t1".into(),
            name: Some("web_search".into()),
            args: Some("query=\"hermes tui_gateway\"".into()),
            preview: None,
        })
        .await;
    tokio::time::sleep(Duration::from_millis(70)).await;
    let _ = ev_tx
        .send(SessionEvent::ToolProgress {
            tool_id: Some("t1".into()),
            name: Some("web_search".into()),
            preview: Some("searching…".into()),
        })
        .await;
    tokio::time::sleep(Duration::from_millis(70)).await;
    let _ = ev_tx
        .send(SessionEvent::ToolComplete {
            tool_id: "t1".into(),
            name: Some("web_search".into()),
            result: Some("1 hit: programmatic integration docs".into()),
            error: None,
        })
        .await;
    let _ = ev_tx
        .send(SessionEvent::ToolStart {
            tool_id: "t2".into(),
            name: Some("execute_code".into()),
            args: Some("lang=\"rust\" source=\"fn main() {}\"".into()),
            preview: Some("fn main() {}".into()),
        })
        .await;
    tokio::time::sleep(Duration::from_millis(70)).await;
    let _ = ev_tx
        .send(SessionEvent::ToolComplete {
            tool_id: "t2".into(),
            name: Some("execute_code".into()),
            result: Some("ok".into()),
            error: None,
        })
        .await;
    stream_reply(
        ev_tx,
        "Search and execute both finished. Type `code` to see a fenced block.",
    )
    .await;
}

async fn play_tool_error(ev_tx: &mpsc::Sender<SessionEvent>) {
    let _ = ev_tx
        .send(SessionEvent::ToolStart {
            tool_id: "t-err".into(),
            name: Some("execute_code".into()),
            args: Some("lang=\"rust\" source=\"panic!()\"".into()),
            preview: Some("panic!()".into()),
        })
        .await;
    tokio::time::sleep(Duration::from_millis(80)).await;
    let _ = ev_tx
        .send(SessionEvent::ToolComplete {
            tool_id: "t-err".into(),
            name: Some("execute_code".into()),
            result: None,
            error: Some("thread 'main' panicked at panic!(), src/lib.rs:1:1".into()),
        })
        .await;
    stream_reply(
        ev_tx,
        "The tool failed. Open the chip with **Ctrl+O** to read the error.",
    )
    .await;
}

async fn play_subagent(ev_tx: &mpsc::Sender<SessionEvent>) {
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
    stream_reply(ev_tx, "Child finished exploring.").await;
}

fn clarify_pick(answers: &serde_json::Value) -> String {
    if let Some(s) = answers.as_str() {
        return s.to_string();
    }
    if let Some(arr) = answers.as_array() {
        let joined: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
        if !joined.is_empty() {
            return joined.join(", ");
        }
    }
    if let Some(obj) = answers.as_object() {
        if let Some(s) = obj
            .get("text")
            .or_else(|| obj.get("answer"))
            .or_else(|| obj.get("choice"))
            .and_then(|v| v.as_str())
        {
            return s.to_string();
        }
    }
    answers.to_string()
}

fn mock_saved_sessions() -> Vec<SavedSession> {
    vec![
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
    ]
}

fn mock_history(user_turns: &[String]) -> Vec<TranscriptMessage> {
    let mut messages = Vec::new();
    let mut row = 1i64;
    for text in user_turns {
        messages.push(TranscriptMessage {
            role: "user".into(),
            text: text.clone(),
            row_id: Some(row),
            display_kind: None,
        });
        row += 1;
        messages.push(TranscriptMessage {
            role: "assistant".into(),
            text: "mock reply".into(),
            row_id: Some(row),
            display_kind: None,
        });
        row += 1;
    }
    messages
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

    #[test]
    fn keywords_route_in_streaming() {
        let fb = MockScenario::Streaming;
        assert_eq!(script_from_text("code", fb), MockScript::Code);
        assert_eq!(
            script_from_text("show a codeblock please", fb),
            MockScript::Code
        );
        assert_eq!(script_from_text("pdf", fb), MockScript::Document);
        assert_eq!(
            script_from_text("read this document", fb),
            MockScript::Document
        );
        assert_eq!(script_from_text("ask", fb), MockScript::Clarify);
        assert_eq!(script_from_text("type", fb), MockScript::ClarifyType);
        assert_eq!(script_from_text("batch", fb), MockScript::ClarifyBatch);
        assert_eq!(script_from_text("tools", fb), MockScript::Tools);
        assert_eq!(script_from_text("error", fb), MockScript::Error);
        assert_eq!(script_from_text("toolerror", fb), MockScript::ToolError);
        assert_eq!(script_from_text("hello there", fb), MockScript::Default);
        assert_eq!(
            script_from_text("hello", MockScenario::Tools),
            MockScript::Tools
        );
    }

    #[tokio::test]
    async fn keyword_code_streams_a_fence() {
        let rt = tokio::runtime::Handle::current();
        let mut mock = MockSession::start(MockScenario::Streaming, &rt);
        let mut rx = mock.take_events().unwrap();
        mock.send(SessionCommand::Submit {
            text: "code".into(),
        });
        let mut body = String::new();
        while let Some(ev) = rx.recv().await {
            if let SessionEvent::MessageComplete { text, .. } = ev {
                body = text.unwrap_or_default();
                break;
            }
        }
        assert!(body.contains("```rust"), "{body}");
        assert!(body.contains("verify_token"), "{body}");
        mock.send(SessionCommand::Shutdown);
    }

    #[tokio::test]
    async fn keyword_ask_emits_clarify() {
        let rt = tokio::runtime::Handle::current();
        let mut mock = MockSession::start(MockScenario::Streaming, &rt);
        let mut rx = mock.take_events().unwrap();
        mock.send(SessionCommand::Submit { text: "ask".into() });
        let mut saw = false;
        while let Some(ev) = rx.recv().await {
            if matches!(ev, SessionEvent::ClarifyRequest { .. }) {
                saw = true;
                break;
            }
        }
        assert!(saw);
        mock.send(SessionCommand::Shutdown);
    }

    #[tokio::test]
    async fn fetch_history_uses_submitted_prompts() {
        let rt = tokio::runtime::Handle::current();
        let mut mock = MockSession::start(MockScenario::Streaming, &rt);
        let mut rx = mock.take_events().unwrap();
        mock.send(SessionCommand::Submit {
            text: "code".into(),
        });
        while let Some(ev) = rx.recv().await {
            if matches!(ev, SessionEvent::MessageComplete { .. }) {
                break;
            }
        }
        mock.send(SessionCommand::FetchHistory);
        let mut users = Vec::new();
        while let Some(ev) = rx.recv().await {
            if let SessionEvent::History { messages } = ev {
                users = messages
                    .into_iter()
                    .filter(|m| m.role == "user")
                    .map(|m| m.text)
                    .collect();
                break;
            }
        }
        assert_eq!(users, vec!["code".to_string()]);
        mock.send(SessionCommand::Shutdown);
    }
}
