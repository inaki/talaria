//! Chat screen. State lives here so child modules can touch private fields.

use crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::session::{
    rewind_turns_from_messages, SessionEvent, SubagentKind, SubagentRow, TranscriptMessage,
};
use crate::ui::screens::{Screen, ScreenAction};
use crate::ui::widgets::{KeyHints, SlashItem, SlashMenu, Spinner, TextComposer};

mod hubs;
mod input;
mod model_picker;
mod overlay;
mod render;
mod tick;

pub(crate) use overlay::Overlay;

#[derive(Debug, Clone)]
pub enum TimelineItem {
    User {
        text: String,
        row_id: Option<i64>,
    },
    Assistant {
        text: String,
        streaming: bool,
    },
    Tool {
        tool_id: String,
        name: String,
        args: String,
        preview: String,
        result: String,
        error: Option<String>,
        done: bool,
        expanded: bool,
    },
    Thinking {
        text: String,
        live: bool,
    },
    Status(String),
    Error(String),
    Subagent {
        id: String,
        label: String,
    },
}

pub struct Chat {
    pub(crate) items: Vec<TimelineItem>,
    pub(crate) composer: TextComposer,
    pub(crate) spinner: Spinner,
    pub(crate) scroll: u16,
    pub(crate) streaming: bool,
    pub(crate) gateway_alive: bool,
    pub(crate) model: String,
    pub(crate) session_id: String,
    pub(crate) confirm_quit: bool,
    pub(crate) notice: Option<String>,
    pub(crate) overlay: Overlay,
    pub(crate) slash: SlashMenu,
    pub(crate) pending_send: Option<String>,
    pub(crate) follow: bool,
    pub(crate) thinking: bool,
    pub(crate) agents: Vec<SubagentRow>,
    pub(crate) cwd: String,
    pub(crate) stored_session_id: String,
    pub(crate) version: String,
    pub(crate) release_date: String,
    pub(crate) tools: Vec<(String, Vec<String>)>,
    pub(crate) skills: Vec<(String, Vec<String>)>,
}

impl Chat {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            composer: TextComposer::with_persisted_history(),
            spinner: Spinner::default(),
            scroll: 0,
            streaming: false,
            gateway_alive: false,
            model: "…".into(),
            session_id: "—".into(),
            confirm_quit: false,
            notice: Some("starting…".into()),
            overlay: Overlay::None,
            slash: SlashMenu::default(),
            pending_send: None,
            follow: true,
            thinking: false,
            agents: Vec::new(),
            cwd: std::env::current_dir()
                .ok()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| ".".into()),
            stored_session_id: String::new(),
            version: String::new(),
            release_date: String::new(),
            tools: Vec::new(),
            skills: Vec::new(),
        }
    }

    pub fn apply_event(&mut self, ev: SessionEvent) {
        match ev {
            SessionEvent::GatewayReady { skin, .. } => {
                self.gateway_alive = true;
                self.notice = Some("gateway ready".into());
                if let Some(skin) = skin {
                    crate::theme::apply_skin(&skin);
                }
            }
            SessionEvent::SessionCreated {
                session_id,
                stored_session_id,
                info,
            } => {
                self.session_id = session_id;
                if let Some(s) = stored_session_id {
                    self.stored_session_id = s;
                }
                if let Some(info) = info {
                    self.apply_session_info(info);
                }
            }
            SessionEvent::SessionInfo {
                info, session_id, ..
            } => {
                if let Some(id) = session_id {
                    self.session_id = id;
                }
                self.apply_session_info(info);
                self.notice = None;
            }
            SessionEvent::MessageDelta { text, .. } => {
                self.streaming = true;
                self.thinking = false;
                self.seal_thinking();
                self.notice = None;
                if let Some(i) = self.last_streaming_assistant() {
                    if let TimelineItem::Assistant {
                        text: buf,
                        streaming,
                    } = &mut self.items[i]
                    {
                        buf.push_str(&text);
                        *streaming = true;
                    }
                } else {
                    self.items.push(TimelineItem::Assistant {
                        text,
                        streaming: true,
                    });
                }
                self.scroll_to_bottom();
            }
            SessionEvent::MessageComplete { text, .. } => {
                self.streaming = false;
                self.thinking = false;
                self.seal_thinking();
                if matches!(self.overlay, Overlay::Clarify { .. }) {
                    self.overlay.close();
                }
                if let Some(i) = self
                    .last_streaming_assistant()
                    .or_else(|| self.last_assistant())
                {
                    if let TimelineItem::Assistant {
                        text: buf,
                        streaming,
                    } = &mut self.items[i]
                    {
                        if buf.is_empty() {
                            if let Some(t) = text {
                                *buf = t;
                            }
                        }
                        *streaming = false;
                    }
                } else if let Some(t) = text {
                    self.items.push(TimelineItem::Assistant {
                        text: t,
                        streaming: false,
                    });
                }
                self.scroll_to_bottom();
            }
            SessionEvent::ToolStart {
                tool_id,
                name,
                args,
            } => {
                self.streaming = true;
                let name = name.unwrap_or_else(|| "tool".into());
                let args = crate::logging::sanitize_tool_text(&name, args.as_deref().unwrap_or(""));
                self.items.push(TimelineItem::Tool {
                    tool_id,
                    name,
                    args,
                    preview: String::new(),
                    result: String::new(),
                    error: None,
                    done: false,
                    expanded: false,
                });
                self.scroll_to_bottom();
            }
            SessionEvent::ToolProgress {
                tool_id,
                preview,
                name,
                ..
            } => {
                if let Some(i) = self.tool_index(tool_id.as_deref(), name.as_deref()) {
                    if let TimelineItem::Tool {
                        preview: p,
                        name: n,
                        ..
                    } = &mut self.items[i]
                    {
                        if let Some(prev) = preview {
                            *p = crate::logging::sanitize_tool_text(n, &prev);
                        }
                    }
                }
            }
            SessionEvent::ToolComplete {
                tool_id,
                name,
                result,
                error,
            } => {
                self.streaming = false;
                if matches!(self.overlay, Overlay::Clarify { .. }) {
                    self.overlay.close();
                }
                if let Some(i) = self.tool_index(Some(&tool_id), name.as_deref()) {
                    if let TimelineItem::Tool {
                        name: n,
                        result: r,
                        error: e,
                        done,
                        ..
                    } = &mut self.items[i]
                    {
                        *done = true;
                        if let Some(text) = result {
                            *r = crate::logging::sanitize_tool_text(n, &text);
                        }
                        *e = error.map(|err| crate::logging::preview(&err));
                    }
                }
            }
            SessionEvent::ApprovalRequest {
                command,
                description,
                choices,
                allow_permanent,
                request_id,
                ..
            } => {
                self.streaming = false;
                let choices =
                    choices.unwrap_or_else(|| vec!["once".into(), "always".into(), "deny".into()]);
                self.overlay = Overlay::Approval {
                    command,
                    description,
                    choices,
                    selected: 0,
                    request_id,
                    allow_permanent: allow_permanent.unwrap_or(true),
                };
            }
            SessionEvent::ApprovalPending => {
                if !self.overlay.is_open() {
                    self.notice = Some("approval pending…".into());
                }
            }
            SessionEvent::ClarifyRequest {
                request_id,
                payload,
            } => {
                let question = payload
                    .get("question")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Hermes has a question")
                    .to_string();
                let choices = payload.get("choices").and_then(|v| {
                    v.as_array().map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                });
                let choices = match choices {
                    Some(c) if c.is_empty() => None,
                    other => other,
                };
                self.overlay = Overlay::Clarify {
                    request_id,
                    question,
                    choices,
                    selected: 0,
                    draft: String::new(),
                };
            }
            SessionEvent::SudoRequest { request_id } => {
                self.overlay = Overlay::Sudo {
                    request_id,
                    draft: String::new(),
                };
            }
            SessionEvent::SecretRequest {
                request_id,
                env_var,
                prompt,
            } => {
                self.overlay = Overlay::Secret {
                    request_id,
                    env_var,
                    prompt,
                    draft: String::new(),
                };
            }
            SessionEvent::SudoExpired { request_id } => {
                let clear = matches!(
                    &self.overlay,
                    Overlay::Sudo { request_id: id, .. } if *id == request_id
                );
                if clear {
                    self.overlay.close();
                    self.notice = Some("sudo prompt expired".into());
                }
            }
            SessionEvent::SecretExpired { request_id } => {
                let clear = matches!(
                    &self.overlay,
                    Overlay::Secret { request_id: id, .. } if *id == request_id
                );
                if clear {
                    self.overlay.close();
                    self.notice = Some("secret prompt expired".into());
                }
            }
            SessionEvent::Catalog { commands, warning } => {
                let mut items: Vec<SlashItem> = commands
                    .into_iter()
                    .map(|c| SlashItem {
                        name: c.name,
                        help: c.help,
                    })
                    .collect();
                merge_host_slash_commands(&mut items);
                self.slash.set_commands(items);
                if let Some(w) = warning {
                    self.notice = Some(w);
                }
            }
            SessionEvent::SavedList { sessions } => {
                if let Overlay::Sessions {
                    saved,
                    loading,
                    notice,
                    ..
                } = &mut self.overlay
                {
                    *saved = sessions;
                    *loading = false;
                    *notice = None;
                }
            }
            SessionEvent::ActiveList { sessions } => {
                if let Overlay::Sessions {
                    live,
                    loading,
                    notice,
                    ..
                } = &mut self.overlay
                {
                    *live = sessions;
                    *loading = false;
                    *notice = None;
                }
            }
            SessionEvent::CommandResult {
                output,
                send,
                prefill,
            } => {
                if let Some(out) = output {
                    self.items.push(TimelineItem::Status(out));
                }
                if let Some(text) = prefill {
                    self.composer.clear();
                    self.composer.insert_str(&text);
                }
                if let Some(text) = send {
                    self.pending_send = Some(text);
                }
            }
            SessionEvent::Transcript {
                session_id,
                messages,
            } => {
                self.session_id = session_id;
                self.items.clear();
                for m in messages {
                    match m.role.as_str() {
                        "user" => self.items.push(TimelineItem::User {
                            text: m.text,
                            row_id: m.row_id,
                        }),
                        "assistant" => self.items.push(TimelineItem::Assistant {
                            text: m.text,
                            streaming: false,
                        }),
                        "tool" => self.items.push(TimelineItem::Tool {
                            tool_id: "resume".into(),
                            name: "tool".into(),
                            args: String::new(),
                            preview: String::new(),
                            result: crate::logging::sanitize_tool_text("tool", &m.text),
                            error: None,
                            done: true,
                            expanded: false,
                        }),
                        _ => self.items.push(TimelineItem::Status(m.text)),
                    }
                }
                self.scroll_to_bottom();
                self.overlay.close();
            }
            SessionEvent::Status(s) => {
                self.notice = Some(s.clone());
                self.items.push(TimelineItem::Status(s));
            }
            SessionEvent::Error { message } => {
                self.streaming = false;
                self.thinking = false;
                self.notice = Some(message.clone());
                self.items.push(TimelineItem::Error(message));
            }
            SessionEvent::ChildExited { code } => {
                self.gateway_alive = false;
                self.streaming = false;
                self.items
                    .push(TimelineItem::Error(crate::user_messages::child_exited(
                        code,
                    )));
            }
            SessionEvent::ProtocolError { preview } => {
                self.items
                    .push(TimelineItem::Status(format!("protocol: {preview}")));
            }
            SessionEvent::Stderr { .. } => {}
            SessionEvent::Unhandled { .. } => {}
            SessionEvent::Thinking { text } => {
                self.thinking = true;
                self.streaming = true;
                if !text.is_empty() {
                    self.notice = Some(truncate_notice(&text));
                }
                match self.items.last_mut() {
                    Some(TimelineItem::Thinking { text: buf, live }) if *live => {
                        buf.push_str(&text);
                    }
                    _ if text.is_empty() => {}
                    _ => self.items.push(TimelineItem::Thinking { text, live: true }),
                }
                self.scroll_to_bottom();
            }
            SessionEvent::SpawnTrees { entries } => {
                if let Overlay::SpawnTrees {
                    entries: list,
                    loading,
                    ..
                } = &mut self.overlay
                {
                    *list = entries;
                    *loading = false;
                }
            }
            SessionEvent::History { messages } => {
                self.stamp_user_row_ids(&messages);
                if let Overlay::Rewind {
                    turns,
                    loading,
                    selected,
                    ..
                } = &mut self.overlay
                {
                    *turns = rewind_turns_from_messages(&messages);
                    *loading = false;
                    if !turns.is_empty() {
                        *selected = (*selected).min(turns.len() - 1);
                    } else {
                        *selected = 0;
                    }
                }
            }
            SessionEvent::RewindApplied {
                survivor_user_row_ids,
            } => {
                if let Some(ids) = survivor_user_row_ids {
                    self.rebind_survivor_row_ids(&ids);
                }
            }
            SessionEvent::Subagent {
                kind,
                id,
                goal,
                text,
            } => {
                let label = match kind {
                    SubagentKind::Start => format!(
                        "subagent {id} started{}",
                        goal.as_deref()
                            .map(|g| format!(" — {g}"))
                            .unwrap_or_default()
                    ),
                    SubagentKind::Tool => {
                        format!("subagent {id} tool {}", text.as_deref().unwrap_or(""))
                    }
                    SubagentKind::Progress => format!("subagent {id}…"),
                    SubagentKind::Complete => format!(
                        "subagent {id} done{}",
                        text.as_deref()
                            .map(|t| format!(" — {t}"))
                            .unwrap_or_default()
                    ),
                };
                self.items.push(TimelineItem::Subagent {
                    id: id.clone(),
                    label,
                });
                match kind {
                    SubagentKind::Start => {
                        if !self.agents.iter().any(|a| a.id == id) {
                            self.agents.push(SubagentRow {
                                id,
                                goal: goal.unwrap_or_default(),
                                status: "running".into(),
                            });
                        }
                    }
                    SubagentKind::Complete => {
                        self.agents.retain(|a| a.id != id);
                    }
                    _ => {}
                }
                self.scroll_to_bottom();
            }
            SessionEvent::Delegation { agents } => {
                self.agents = agents.clone();
                if let Overlay::Agents {
                    agents: list,
                    loading,
                    ..
                } = &mut self.overlay
                {
                    *list = agents;
                    *loading = false;
                }
            }
            SessionEvent::ModelOptions {
                providers,
                model,
                error,
            } => {
                if let Overlay::Model(picker) = &mut self.overlay {
                    if let Some(err) = error {
                        picker.loading = false;
                        picker.error = Some(err);
                    } else {
                        picker.apply_options(providers, model);
                    }
                }
            }
            SessionEvent::ModelKeySaved { provider, error } => {
                if let Overlay::Model(picker) = &mut self.overlay {
                    picker.key_saving = false;
                    if let Some(err) = error {
                        picker.key_error = Some(err);
                    } else if !provider.slug.is_empty() {
                        let slug = provider.slug.clone();
                        if let Some(existing) = picker.providers.iter_mut().find(|p| p.slug == slug)
                        {
                            *existing = provider;
                        } else {
                            picker.providers.push(provider);
                        }
                        if let Some(i) = picker.providers.iter().position(|p| p.slug == slug) {
                            picker.provider_idx = i;
                        }
                        picker.stage = model_picker::ModelStage::Model;
                        picker.model_idx = 0;
                        picker.key_input.clear();
                        picker.filter.clear();
                    }
                }
            }
            SessionEvent::ModelDisconnected { slug, ok } => {
                if let Overlay::Model(picker) = &mut self.overlay {
                    picker.key_saving = false;
                    picker.stage = model_picker::ModelStage::Provider;
                    if ok {
                        if let Some(existing) = picker.providers.iter_mut().find(|p| p.slug == slug)
                        {
                            existing.authenticated = false;
                            existing.models.clear();
                            existing.total_models = 0;
                            existing.warning = existing
                                .key_env
                                .as_ref()
                                .map(|e| format!("paste {e} to activate"))
                                .or_else(|| Some("OAuth provider — not a paste-key setup".into()));
                        }
                    }
                }
            }
            SessionEvent::ConfigSet {
                value,
                warning,
                deferred,
                confirm_required,
                confirm_message,
                info,
                ..
            } => {
                if confirm_required {
                    let msg = confirm_message
                        .unwrap_or_else(|| "This model has unusually high known pricing.".into());
                    if let Overlay::Model(picker) = &mut self.overlay {
                        picker.confirm_message = Some(msg);
                        if picker.pending_value.is_none() {
                            picker.pending_value = value;
                        }
                    } else {
                        let mut picker = model_picker::ModelPicker::loading();
                        picker.loading = false;
                        picker.pending_value = value;
                        picker.confirm_message = Some(msg);
                        self.overlay = Overlay::Model(picker);
                    }
                    return;
                }
                if let Some(info) = info {
                    self.apply_session_info(info);
                } else if let Some(v) = &value {
                    self.model = v.clone();
                }
                let shown = value.as_deref().unwrap_or("?");
                let msg = if deferred {
                    format!("model → {shown} (applies next turn)")
                } else {
                    format!("model → {shown}")
                };
                self.notice = Some(msg.clone());
                self.items.push(TimelineItem::Status(msg));
                if let Some(w) = warning {
                    self.items.push(TimelineItem::Status(w));
                }
                if matches!(self.overlay, Overlay::Model(_)) {
                    self.overlay.close();
                }
            }
            SessionEvent::SkillsList { groups, error } => {
                if let Overlay::Skills(hub) = &mut self.overlay {
                    hub.loading = false;
                    hub.error = error;
                    hub.groups = groups;
                }
            }
            SessionEvent::SkillInstalled { name, ok, error } => {
                let msg = if let Some(e) = error {
                    format!("skill {name}: {e}")
                } else if ok {
                    format!("installed {name}")
                } else {
                    format!("install failed: {name}")
                };
                self.notice = Some(msg.clone());
                self.items.push(TimelineItem::Status(msg.clone()));
                if let Overlay::Skills(hub) = &mut self.overlay {
                    hub.notice = Some(msg);
                }
            }
            SessionEvent::PluginsList { plugins, error } => {
                if let Overlay::Plugins(hub) = &mut self.overlay {
                    hub.loading = false;
                    hub.error = error;
                    hub.plugins = plugins;
                    if hub.selected >= hub.plugins.len() {
                        hub.selected = hub.plugins.len().saturating_sub(1);
                    }
                }
            }
            SessionEvent::PluginToggled { plugin, ok } => {
                if let Overlay::Plugins(hub) = &mut self.overlay {
                    if let Some(row) = plugin {
                        let key = row.key.clone();
                        let status = row.status.clone();
                        if let Some(existing) = hub.plugins.iter_mut().find(|p| p.key == key) {
                            *existing = row;
                        }
                        hub.notice = Some(if ok {
                            format!("{status} {key}")
                        } else {
                            format!("toggle failed: {key}")
                        });
                    } else if !ok {
                        hub.notice = Some("toggle failed".into());
                    }
                }
            }
            SessionEvent::McpServers { servers, error } => {
                if let Overlay::Mcp(hub) = &mut self.overlay {
                    hub.loading = false;
                    if error.is_some() {
                        hub.error = error;
                    }
                    hub.installed = servers;
                }
            }
            SessionEvent::McpCatalog { servers, error } => {
                if let Overlay::Mcp(hub) = &mut self.overlay {
                    hub.loading = false;
                    if error.is_some() {
                        hub.error = error;
                    }
                    hub.catalog = servers;
                }
            }
            SessionEvent::McpChanged { name, ok, error } => {
                if let Overlay::Mcp(hub) = &mut self.overlay {
                    hub.loading = false;
                    if let Some(e) = error {
                        hub.error = Some(e);
                    } else if ok {
                        hub.notice = Some(format!("updated {name}"));
                        match hub.tab {
                            hubs::McpTab::Catalog => {
                                if !hub.installed.iter().any(|s| s.name == name) {
                                    hub.installed.push(crate::session::McpServer {
                                        name: name.clone(),
                                        transport: String::new(),
                                        enabled: true,
                                        auth: String::new(),
                                    });
                                }
                                if let Some(c) = hub.catalog.iter_mut().find(|c| c.name == name) {
                                    c.installed = true;
                                }
                            }
                            hubs::McpTab::Installed => {
                                hub.installed.retain(|s| s.name != name);
                            }
                        }
                    }
                }
                self.notice = Some(if ok {
                    format!("mcp {name}")
                } else {
                    format!("mcp {name} failed")
                });
            }
        }
    }

    fn apply_session_info(&mut self, info: serde_json::Value) {
        if let Some(m) = info.get("model").and_then(|v| v.as_str()) {
            self.model = m.to_string();
        }
        if let Some(c) = info.get("cwd").and_then(|v| v.as_str()) {
            self.cwd = c.to_string();
        }
        if let Some(v) = info.get("version").and_then(|v| v.as_str()) {
            self.version = v.to_string();
        }
        if let Some(v) = info.get("release_date").and_then(|v| v.as_str()) {
            self.release_date = v.to_string();
        }
        let tools = grouped_map(info.get("tools"));
        if !tools.is_empty() {
            self.tools = tools;
        }
        let skills = grouped_map(info.get("skills"));
        if !skills.is_empty() {
            self.skills = skills;
        }
    }

    pub fn push_user(&mut self, text: String) {
        self.items.push(TimelineItem::User { text, row_id: None });
        self.scroll_to_bottom();
    }

    pub fn open_rewind(&mut self) {
        self.slash.close();
        let turns = rewind_turns_from_timeline(&self.items);
        self.overlay = Overlay::Rewind {
            turns,
            selected: 0,
            loading: true,
            confirming: false,
        };
    }

    pub fn apply_rewind_locally(&mut self, row_id: i64, text: String) {
        if let Some(i) = self.items.iter().position(
            |it| matches!(it, TimelineItem::User { row_id: Some(id), .. } if *id == row_id),
        ) {
            self.items.truncate(i);
        } else if let Some(i) = self
            .items
            .iter()
            .rposition(|it| matches!(it, TimelineItem::User { text: t, .. } if t == &text))
        {
            self.items.truncate(i);
        }
        self.push_user(text);
        self.streaming = false;
        self.thinking = false;
    }

    fn stamp_user_row_ids(&mut self, messages: &[TranscriptMessage]) {
        let durable: Vec<(i64, &str)> = messages
            .iter()
            .filter(|m| m.role == "user" && m.display_kind.as_deref().unwrap_or("").is_empty())
            .filter_map(|m| Some((m.row_id?, m.text.as_str())))
            .collect();
        for item in &mut self.items {
            let TimelineItem::User { text, row_id } = item else {
                continue;
            };
            if row_id.is_some() {
                continue;
            }
            let matches: Vec<i64> = durable
                .iter()
                .filter(|(_, t)| *t == text.as_str())
                .map(|(id, _)| *id)
                .collect();
            if matches.len() == 1 {
                *row_id = Some(matches[0]);
            }
        }
    }

    fn rebind_survivor_row_ids(&mut self, survivors: &[Option<i64>]) {
        let mut ordinal = 0usize;
        for item in &mut self.items {
            if let TimelineItem::User { row_id, .. } = item {
                if ordinal < survivors.len() {
                    *row_id = survivors[ordinal];
                } else {
                    *row_id = None;
                }
                ordinal += 1;
            }
        }
    }

    fn seal_thinking(&mut self) {
        if let Some(TimelineItem::Thinking { live, .. }) = self.items.last_mut() {
            *live = false;
        }
    }

    fn last_streaming_assistant(&self) -> Option<usize> {
        self.items.iter().rposition(|it| {
            matches!(
                it,
                TimelineItem::Assistant {
                    streaming: true,
                    ..
                }
            )
        })
    }

    fn last_assistant(&self) -> Option<usize> {
        self.items
            .iter()
            .rposition(|it| matches!(it, TimelineItem::Assistant { .. }))
    }

    pub fn toggle_last_tool(&mut self) {
        if let Some(i) = self
            .items
            .iter()
            .rposition(|it| matches!(it, TimelineItem::Tool { .. }))
        {
            if let TimelineItem::Tool { expanded, .. } = &mut self.items[i] {
                *expanded = !*expanded;
            }
        }
    }

    pub fn open_spawn_trees(&mut self) {
        self.slash.close();
        self.overlay = Overlay::SpawnTrees {
            entries: Vec::new(),
            selected: 0,
            loading: true,
        };
    }

    fn tool_index(&self, id: Option<&str>, name: Option<&str>) -> Option<usize> {
        if let Some(want) = id {
            if let Some(i) = self
                .items
                .iter()
                .rposition(|it| matches!(it, TimelineItem::Tool { tool_id, .. } if tool_id == want))
            {
                return Some(i);
            }
        }
        self.items.iter().rposition(|it| match it {
            TimelineItem::Tool { done, name: n, .. } => {
                !*done && name.map(|want| n == want).unwrap_or(true)
            }
            _ => false,
        })
    }

    fn scroll_to_bottom(&mut self) {
        self.follow = true;
        self.scroll = u16::MAX;
    }

    fn render_screen(&mut self, f: &mut Frame, area: Rect) {
        render::draw(self, f, area);
    }

    fn on_key(&mut self, key: KeyEvent) -> Option<ScreenAction> {
        input::on_key(self, key)
    }

    fn on_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> Option<ScreenAction> {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.follow = false;
                self.scroll = self.scroll.saturating_sub(1);
            }
            MouseEventKind::ScrollDown => {
                self.scroll = self.scroll.saturating_add(1);
            }
            _ => {}
        }
        None
    }

    fn tick_screen(&mut self) {
        tick::on_tick(self);
    }

    fn key_hints_impl(&self) -> KeyHints<'static> {
        if self.confirm_quit {
            KeyHints::confirm_quit()
        } else if self.overlay.is_open() {
            KeyHints::overlay()
        } else if self.slash.is_active() {
            KeyHints::slash()
        } else if self.streaming || self.thinking {
            KeyHints::streaming()
        } else {
            KeyHints::idle()
        }
    }

    pub fn take_pending_send(&mut self) -> Option<String> {
        self.pending_send.take()
    }

    pub fn open_model(&mut self) {
        self.slash.close();
        self.overlay = Overlay::Model(model_picker::ModelPicker::loading());
    }

    pub fn open_skills(&mut self) {
        self.slash.close();
        self.overlay = Overlay::Skills(hubs::SkillsHub::loading());
    }

    pub fn open_plugins(&mut self) {
        self.slash.close();
        self.overlay = Overlay::Plugins(hubs::PluginsHub::loading());
    }

    pub fn open_mcp(&mut self) {
        self.slash.close();
        self.overlay = Overlay::Mcp(hubs::McpHub::loading());
    }

    pub fn open_theme(&mut self) {
        self.slash.close();
        let saved_id = crate::theme::current_theme_id();
        let selected = crate::theme::themes()
            .iter()
            .position(|t| t.id == saved_id)
            .unwrap_or(0);
        self.overlay = Overlay::Theme { selected, saved_id };
    }

    pub fn open_help(&mut self) {
        self.slash.close();
        self.overlay = Overlay::Help;
    }

    pub fn open_agents(&mut self) {
        self.slash.close();
        self.overlay = Overlay::Agents {
            agents: self.agents.clone(),
            selected: 0,
            loading: true,
        };
    }

    pub fn open_sessions(&mut self) {
        self.slash.close();
        self.overlay = Overlay::Sessions {
            tab: overlay::SessionTab::Saved,
            saved: Vec::new(),
            live: Vec::new(),
            selected: 0,
            loading: true,
            notice: None,
        };
    }
}

fn grouped_map(v: Option<&serde_json::Value>) -> Vec<(String, Vec<String>)> {
    let Some(obj) = v.and_then(|x| x.as_object()) else {
        return Vec::new();
    };
    let mut out: Vec<(String, Vec<String>)> = obj
        .iter()
        .map(|(k, val)| {
            let mut name = k.clone();
            if let Some(stripped) = name.strip_suffix("_tools") {
                name = stripped.to_string();
            }
            let members = match val {
                serde_json::Value::Array(a) => a
                    .iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect(),
                serde_json::Value::String(s) => vec![s.clone()],
                _ => Vec::new(),
            };
            (name, members)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn merge_host_slash_commands(items: &mut Vec<SlashItem>) {
    for (name, help) in [
        ("theme", "color theme"),
        ("model", "switch or add models"),
        ("skills", "browse and install skills"),
        ("plugins", "toggle plugins"),
        ("mcp", "add or remove MCP servers"),
        ("rewind", "regenerate from a past user turn"),
        ("trees", "saved spawn trees"),
        ("help", "keyboard help"),
    ] {
        if !items.iter().any(|c| c.name == name) {
            items.push(SlashItem {
                name: name.into(),
                help: help.into(),
            });
        }
    }
}

fn rewind_turns_from_timeline(items: &[TimelineItem]) -> Vec<crate::session::RewindTurn> {
    let mut turns = Vec::new();
    for item in items {
        if let TimelineItem::User {
            text,
            row_id: Some(row_id),
        } = item
        {
            let first = turns.is_empty();
            turns.push(crate::session::RewindTurn {
                row_id: *row_id,
                text: text.clone(),
                first,
            });
        }
    }
    turns
}

fn truncate_notice(s: &str) -> String {
    let one = s.replace('\n', " ");
    if one.chars().count() > 48 {
        format!("{}…", one.chars().take(48).collect::<String>())
    } else {
        one
    }
}

impl Default for Chat {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_rewind_drops_from_row_id_and_rebinds_survivors() {
        let mut chat = Chat::new();
        chat.items = vec![
            TimelineItem::User {
                text: "a".into(),
                row_id: Some(1),
            },
            TimelineItem::Assistant {
                text: "b".into(),
                streaming: false,
            },
            TimelineItem::User {
                text: "c".into(),
                row_id: Some(2),
            },
            TimelineItem::Assistant {
                text: "d".into(),
                streaming: false,
            },
        ];
        chat.apply_rewind_locally(2, "c-edit".into());
        assert_eq!(chat.items.len(), 3);
        match &chat.items[2] {
            TimelineItem::User { text, row_id } => {
                assert_eq!(text, "c-edit");
                assert_eq!(*row_id, None);
            }
            other => panic!("{other:?}"),
        }
        chat.rebind_survivor_row_ids(&[Some(101)]);
        match &chat.items[0] {
            TimelineItem::User { row_id, .. } => assert_eq!(*row_id, Some(101)),
            other => panic!("{other:?}"),
        }
        match &chat.items[2] {
            TimelineItem::User { row_id, .. } => assert_eq!(*row_id, None),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn complete_seals_streaming_assistant_even_if_thinking_is_last() {
        let mut chat = Chat::new();
        chat.apply_event(SessionEvent::MessageDelta {
            session_id: None,
            text: "hello **world**".into(),
            rendered: None,
        });
        chat.apply_event(SessionEvent::Thinking {
            text: String::new(),
        });
        chat.apply_event(SessionEvent::Thinking { text: "hm".into() });
        chat.apply_event(SessionEvent::MessageComplete {
            session_id: None,
            text: Some("hello **world**".into()),
        });
        let assistants: Vec<_> = chat
            .items
            .iter()
            .filter(|it| matches!(it, TimelineItem::Assistant { .. }))
            .collect();
        assert_eq!(assistants.len(), 1, "{:?}", chat.items.len());
        match &chat.items[0] {
            TimelineItem::Assistant { text, streaming } => {
                assert_eq!(text, "hello **world**");
                assert!(!*streaming);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn config_set_updates_model_and_closes_picker() {
        let mut chat = Chat::new();
        chat.open_model();
        chat.apply_event(SessionEvent::ConfigSet {
            key: "model".into(),
            value: Some("openrouter/foo".into()),
            warning: None,
            deferred: false,
            confirm_required: false,
            confirm_message: None,
            info: Some(serde_json::json!({"model": "openrouter/foo"})),
        });
        assert_eq!(chat.model, "openrouter/foo");
        assert!(!chat.overlay.is_open());
    }

    #[test]
    fn config_set_confirm_keeps_picker_open() {
        let mut chat = Chat::new();
        chat.apply_event(SessionEvent::ConfigSet {
            key: "model".into(),
            value: Some("opus --global".into()),
            warning: None,
            deferred: false,
            confirm_required: true,
            confirm_message: Some("pricey".into()),
            info: None,
        });
        match &chat.overlay {
            Overlay::Model(p) => {
                assert_eq!(p.confirm_message.as_deref(), Some("pricey"));
                assert_eq!(p.pending_value.as_deref(), Some("opus --global"));
            }
            other => panic!("{other:?}"),
        }
    }
}

impl Screen for Chat {
    fn render(&mut self, f: &mut Frame, area: Rect) {
        self.render_screen(f, area);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<ScreenAction> {
        self.on_key(key)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> Option<ScreenAction> {
        self.on_mouse(mouse, area)
    }

    fn handle_paste(&mut self, text: String) -> Option<ScreenAction> {
        self.handle_paste_inner(text)
    }

    fn tick(&mut self) {
        self.tick_screen();
    }

    fn key_hints(&self) -> KeyHints<'static> {
        self.key_hints_impl()
    }
}
