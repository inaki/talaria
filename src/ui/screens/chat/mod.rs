//! Chat screen. State lives here so child modules can touch private fields.

use crossterm::event::{KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::session::{SessionEvent, SubagentKind, SubagentRow};
use crate::ui::screens::{Screen, ScreenAction};
use crate::ui::widgets::{KeyHints, SlashItem, SlashMenu, Spinner, TextComposer};

mod input;
mod overlay;
mod render;
mod tick;

pub(crate) use overlay::Overlay;

#[derive(Debug, Clone)]
pub enum TimelineItem {
    User(String),
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
            SessionEvent::SessionCreated { session_id, .. } => {
                self.session_id = session_id;
            }
            SessionEvent::SessionInfo {
                info, session_id, ..
            } => {
                if let Some(id) = session_id {
                    self.session_id = id;
                }
                if let Some(m) = info.get("model").and_then(|v| v.as_str()) {
                    self.model = m.to_string();
                }
                self.notice = None;
            }
            SessionEvent::MessageDelta { text, .. } => {
                self.streaming = true;
                self.thinking = false;
                self.notice = None;
                match self.items.last_mut() {
                    Some(TimelineItem::Assistant {
                        text: buf,
                        streaming,
                    }) if *streaming => {
                        buf.push_str(&text);
                    }
                    _ => self.items.push(TimelineItem::Assistant {
                        text,
                        streaming: true,
                    }),
                }
                self.scroll_to_bottom();
            }
            SessionEvent::MessageComplete { text, .. } => {
                self.streaming = false;
                self.thinking = false;
                if matches!(self.overlay, Overlay::Clarify { .. }) {
                    self.overlay.close();
                }
                match self.items.last_mut() {
                    Some(TimelineItem::Assistant {
                        text: buf,
                        streaming,
                    }) => {
                        if let Some(t) = text {
                            if buf.is_empty() {
                                *buf = t;
                            }
                        }
                        *streaming = false;
                    }
                    _ => {
                        if let Some(t) = text {
                            self.items.push(TimelineItem::Assistant {
                                text: t,
                                streaming: false,
                            });
                        }
                    }
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
                self.slash.set_commands(
                    commands
                        .into_iter()
                        .map(|c| SlashItem {
                            name: c.name,
                            help: c.help,
                        })
                        .collect(),
                );
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
                        "user" => self.items.push(TimelineItem::User(m.text)),
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
        }
    }

    pub fn push_user(&mut self, text: String) {
        self.items.push(TimelineItem::User(text));
        self.scroll_to_bottom();
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
