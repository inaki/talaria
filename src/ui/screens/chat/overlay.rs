//! Approval / clarify / sudo / secret / session-picker overlays.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::hubs::{self, McpHub, PluginsHub, SkillsHub};
use super::model_picker::{self, ModelPicker};
use crate::session::{ActiveSession, RewindTurn, SavedSession, SpawnTreeEntry, SubagentRow};
use crate::theme;
use crate::ui::keys::{is_ctrl_c, typed_char};
use crate::ui::screens::ScreenAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTab {
    Saved,
    Live,
}

#[derive(Debug, Clone, Default)]
pub enum Overlay {
    #[default]
    None,
    Approval {
        command: String,
        description: String,
        choices: Vec<String>,
        selected: usize,
        request_id: Option<String>,
        allow_permanent: bool,
    },
    Clarify {
        request_id: String,
        question: String,
        choices: Option<Vec<String>>,
        selected: usize,
        draft: String,
    },
    Sudo {
        request_id: String,
        draft: String,
    },
    Secret {
        request_id: String,
        env_var: String,
        prompt: String,
        draft: String,
    },
    Sessions {
        tab: SessionTab,
        saved: Vec<SavedSession>,
        live: Vec<ActiveSession>,
        selected: usize,
        loading: bool,
        notice: Option<String>,
    },
    Help,
    Agents {
        agents: Vec<SubagentRow>,
        selected: usize,
        loading: bool,
    },
    SpawnTrees {
        entries: Vec<SpawnTreeEntry>,
        selected: usize,
        loading: bool,
    },
    Rewind {
        turns: Vec<RewindTurn>,
        selected: usize,
        loading: bool,
        confirming: bool,
    },
    Theme {
        selected: usize,
        saved_id: &'static str,
    },
    Model(ModelPicker),
    Skills(SkillsHub),
    Plugins(PluginsHub),
    Mcp(McpHub),
}

impl Overlay {
    pub fn is_open(&self) -> bool {
        !matches!(self, Overlay::None)
    }

    pub fn close(&mut self) {
        *self = Overlay::None;
    }
}

pub fn handle_overlay_key(overlay: &mut Overlay, key: KeyEvent) -> Option<ScreenAction> {
    if is_ctrl_c(&key) {
        return None; // caller handles quit confirm
    }
    match overlay {
        Overlay::None => None,
        Overlay::Approval {
            choices,
            selected,
            request_id,
            ..
        } => {
            if key.code == KeyCode::Esc {
                let choice = deny_choice(choices).unwrap_or("deny").to_string();
                let id = request_id.clone();
                overlay.close();
                return Some(ScreenAction::RespondApproval {
                    choice,
                    request_id: id,
                });
            }
            if key.code == KeyCode::Up {
                *selected = selected.saturating_sub(1);
                return None;
            }
            if key.code == KeyCode::Down {
                if !choices.is_empty() {
                    *selected = (*selected + 1).min(choices.len() - 1);
                }
                return None;
            }
            if let KeyCode::Char(c) = key.code {
                if let Some(d) = c.to_digit(10) {
                    let i = d as usize;
                    if i >= 1 && i <= choices.len() {
                        let choice = choices[i - 1].clone();
                        let id = request_id.clone();
                        overlay.close();
                        return Some(ScreenAction::RespondApproval {
                            choice,
                            request_id: id,
                        });
                    }
                }
            }
            if key.code == KeyCode::Enter && !choices.is_empty() {
                let choice = choices[*selected].clone();
                let id = request_id.clone();
                overlay.close();
                return Some(ScreenAction::RespondApproval {
                    choice,
                    request_id: id,
                });
            }
            None
        }
        Overlay::Clarify {
            request_id,
            choices,
            selected,
            draft,
            ..
        } => {
            if key.code == KeyCode::Esc {
                overlay.close();
                return None;
            }
            if let Some(choices) = choices {
                if key.code == KeyCode::Up {
                    *selected = selected.saturating_sub(1);
                    return None;
                }
                if key.code == KeyCode::Down && !choices.is_empty() {
                    *selected = (*selected + 1).min(choices.len() - 1);
                    return None;
                }
                if key.code == KeyCode::Enter && !choices.is_empty() {
                    let answer = choices[*selected].clone();
                    let id = request_id.clone();
                    overlay.close();
                    return Some(ScreenAction::RespondClarify {
                        request_id: id,
                        answers: serde_json::json!(answer),
                    });
                }
            } else {
                match key.code {
                    KeyCode::Enter => {
                        if draft.trim().is_empty() {
                            return None;
                        }
                        let answer = draft.clone();
                        let id = request_id.clone();
                        overlay.close();
                        return Some(ScreenAction::RespondClarify {
                            request_id: id,
                            answers: serde_json::json!(answer),
                        });
                    }
                    KeyCode::Backspace => {
                        draft.pop();
                    }
                    _ => {
                        if let Some(c) = typed_char(&key) {
                            draft.push(c);
                        }
                    }
                }
            }
            None
        }
        Overlay::Sudo { request_id, draft } => {
            if key.code == KeyCode::Esc {
                overlay.close();
                return None;
            }
            if key.code == KeyCode::Backspace {
                draft.pop();
                return None;
            }
            if key.code == KeyCode::Enter {
                let password = std::mem::take(draft);
                let id = request_id.clone();
                overlay.close();
                return Some(ScreenAction::RespondSudo {
                    request_id: id,
                    password,
                });
            }
            if let Some(c) = typed_char(&key) {
                draft.push(c);
            }
            None
        }
        Overlay::Secret {
            request_id, draft, ..
        } => {
            if key.code == KeyCode::Esc {
                overlay.close();
                return None;
            }
            if key.code == KeyCode::Backspace {
                draft.pop();
                return None;
            }
            if key.code == KeyCode::Enter {
                let value = std::mem::take(draft);
                let id = request_id.clone();
                overlay.close();
                return Some(ScreenAction::RespondSecret {
                    request_id: id,
                    value,
                });
            }
            if let Some(c) = typed_char(&key) {
                draft.push(c);
            }
            None
        }
        Overlay::Sessions {
            tab,
            saved,
            live,
            selected,
            loading,
            ..
        } => {
            if key.code == KeyCode::Esc {
                overlay.close();
                return None;
            }
            if matches!(key.code, KeyCode::Tab | KeyCode::Left | KeyCode::Right) {
                *tab = match tab {
                    SessionTab::Saved => SessionTab::Live,
                    SessionTab::Live => SessionTab::Saved,
                };
                *selected = 0;
                return None;
            }
            let len = match tab {
                SessionTab::Saved => saved.len(),
                SessionTab::Live => live.len(),
            };
            if key.code == KeyCode::Up {
                *selected = selected.saturating_sub(1);
                return None;
            }
            if key.code == KeyCode::Down && len > 0 {
                *selected = (*selected + 1).min(len - 1);
                return None;
            }
            if key.code == KeyCode::Enter && !*loading && len > 0 {
                let i = *selected;
                let action = match tab {
                    SessionTab::Saved => saved.get(i).map(|s| ScreenAction::ResumeSaved {
                        session_id: s.id.clone(),
                    }),
                    SessionTab::Live => live.get(i).map(|s| ScreenAction::ActivateLive {
                        session_id: s.id.clone(),
                    }),
                };
                overlay.close();
                return action;
            }
            None
        }
        Overlay::Help => {
            if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
                overlay.close();
            }
            None
        }
        Overlay::Agents {
            agents,
            selected,
            loading,
        } => {
            if key.code == KeyCode::Esc {
                overlay.close();
                return None;
            }
            if key.code == KeyCode::Up {
                *selected = selected.saturating_sub(1);
                return None;
            }
            if key.code == KeyCode::Down && !agents.is_empty() {
                *selected = (*selected + 1).min(agents.len() - 1);
                return None;
            }
            if key.code == KeyCode::Enter && !*loading {
                if let Some(a) = agents.get(*selected) {
                    let id = a.id.clone();
                    overlay.close();
                    return Some(ScreenAction::InterruptSubagent { subagent_id: id });
                }
            }
            None
        }
        Overlay::SpawnTrees {
            entries,
            selected,
            loading,
        } => {
            if key.code == KeyCode::Esc {
                overlay.close();
                return None;
            }
            if key.code == KeyCode::Up {
                *selected = selected.saturating_sub(1);
                return None;
            }
            if key.code == KeyCode::Down && !entries.is_empty() {
                *selected = (*selected + 1).min(entries.len() - 1);
                return None;
            }
            if key.code == KeyCode::Enter && !*loading {
                if let Some(e) = entries.get(*selected) {
                    let path = e.path.clone();
                    overlay.close();
                    return Some(ScreenAction::LoadSpawnTree { path });
                }
            }
            None
        }
        Overlay::Rewind {
            turns,
            selected,
            loading,
            confirming,
        } => {
            if *confirming {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                        *confirming = false;
                        None
                    }
                    KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                        let turn = turns.get(*selected).cloned();
                        overlay.close();
                        turn.map(|t| ScreenAction::Rewind {
                            text: t.text,
                            truncate_before_row_id: t.row_id,
                            confirm_empty_truncate: t.first,
                        })
                    }
                    _ => None,
                }
            } else {
                if key.code == KeyCode::Esc {
                    overlay.close();
                    return None;
                }
                if key.code == KeyCode::Up {
                    *selected = selected.saturating_sub(1);
                    return None;
                }
                if key.code == KeyCode::Down && !turns.is_empty() {
                    *selected = (*selected + 1).min(turns.len() - 1);
                    return None;
                }
                if key.code == KeyCode::Enter && !turns.is_empty() && !*loading {
                    *confirming = true;
                }
                None
            }
        }
        Overlay::Theme { selected, saved_id } => {
            if key.code == KeyCode::Esc {
                let _ = theme::apply_theme(saved_id);
                theme::sync_terminal_canvas();
                overlay.close();
                return None;
            }
            let n = theme::themes().len();
            if key.code == KeyCode::Up {
                *selected = selected.saturating_sub(1);
                preview_theme(*selected);
                return None;
            }
            if key.code == KeyCode::Down && n > 0 {
                *selected = (*selected + 1).min(n - 1);
                preview_theme(*selected);
                return None;
            }
            if key.code == KeyCode::Enter {
                if let Some(t) = theme::themes().get(*selected) {
                    let _ = theme::apply_theme(t.id);
                    theme::save_theme_id(t.id);
                    theme::sync_terminal_canvas_hard();
                }
                overlay.close();
            }
            None
        }
        Overlay::Model(picker) => match model_picker::on_key(picker, key) {
            model_picker::ModelKey::None => None,
            model_picker::ModelKey::Close => {
                overlay.close();
                None
            }
            model_picker::ModelKey::Action(action) => Some(action),
        },
        Overlay::Skills(hub) => match hubs::skills_key(hub, key) {
            (hubs::HubKey::Close, _) => {
                overlay.close();
                None
            }
            (hubs::HubKey::None, action) => action,
        },
        Overlay::Plugins(hub) => match hubs::plugins_key(hub, key) {
            (hubs::HubKey::Close, _) => {
                overlay.close();
                None
            }
            (hubs::HubKey::None, action) => action,
        },
        Overlay::Mcp(hub) => match hubs::mcp_key(hub, key) {
            (hubs::HubKey::Close, _) => {
                overlay.close();
                None
            }
            (hubs::HubKey::None, action) => action,
        },
    }
}

fn preview_theme(index: usize) {
    if let Some(t) = theme::themes().get(index) {
        let _ = theme::apply_theme(t.id);
        theme::sync_terminal_canvas();
    }
}

fn deny_choice(choices: &[String]) -> Option<&str> {
    choices
        .iter()
        .find(|c| {
            let n = c.to_ascii_lowercase();
            n == "deny" || n == "n" || n == "no" || n.contains("deny")
        })
        .map(|s| s.as_str())
}

pub fn draw_overlay(overlay: &Overlay, f: &mut Frame, area: Rect) {
    match overlay {
        Overlay::None => {}
        Overlay::Approval {
            command,
            description,
            choices,
            selected,
            allow_permanent,
            ..
        } => {
            let mut lines = vec![
                Line::from(Span::styled(
                    "approval needed",
                    Style::default()
                        .fg(theme::WARNING())
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(command.clone(), theme::text())),
            ];
            if !description.is_empty() {
                lines.push(Line::from(Span::styled(description.clone(), theme::dim())));
            }
            if !*allow_permanent {
                lines.push(Line::from(Span::styled(
                    "permanent allow is not available",
                    theme::dim(),
                )));
            }
            lines.push(Line::from(""));
            for (i, c) in choices.iter().enumerate() {
                let mark = if i == *selected { "▸ " } else { "  " };
                let style = if i == *selected {
                    theme::accent()
                } else {
                    theme::text()
                };
                lines.push(Line::from(Span::styled(
                    format!("{mark}{}. {c}", i + 1),
                    style,
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Enter select  ·  1-9  ·  Esc deny",
                theme::dim(),
            )));
            paint_modal(f, area, " approval ", lines, 16);
        }
        Overlay::Clarify {
            question,
            choices,
            selected,
            draft,
            ..
        } => {
            let mut lines = vec![
                Line::from(Span::styled(question.clone(), theme::text())),
                Line::from(""),
            ];
            if let Some(choices) = choices {
                for (i, c) in choices.iter().enumerate() {
                    let mark = if i == *selected { "▸ " } else { "  " };
                    lines.push(Line::from(Span::styled(
                        format!("{mark}{c}"),
                        if i == *selected {
                            theme::accent()
                        } else {
                            theme::text()
                        },
                    )));
                }
                lines.push(Line::from(Span::styled(
                    "Enter to answer  ·  Esc to dismiss",
                    theme::dim(),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("› {draft}▍"),
                    theme::accent(),
                )));
                lines.push(Line::from(Span::styled(
                    "Enter to send  ·  Esc dismiss",
                    theme::dim(),
                )));
            }
            paint_modal(f, area, " clarify ", lines, 14);
        }
        Overlay::Sudo { draft, .. } => {
            let mask = "*".repeat(draft.chars().count());
            let lines = vec![
                Line::from(Span::styled(
                    "Hermes needs your sudo password for this turn.",
                    theme::text(),
                )),
                Line::from(""),
                Line::from(Span::styled(format!("password: {mask}▍"), theme::accent())),
                Line::from(""),
                Line::from(Span::styled("Enter submit  ·  Esc dismiss", theme::dim())),
            ];
            paint_modal(f, area, " sudo ", lines, 9);
        }
        Overlay::Secret {
            env_var,
            prompt,
            draft,
            ..
        } => {
            let mask = "*".repeat(draft.chars().count());
            let lines = vec![
                Line::from(Span::styled(prompt.clone(), theme::text())),
                Line::from(Span::styled(format!("env: {env_var}"), theme::dim())),
                Line::from(""),
                Line::from(Span::styled(format!("secret: {mask}▍"), theme::accent())),
                Line::from(""),
                Line::from(Span::styled("Enter submit  ·  Esc dismiss", theme::dim())),
            ];
            paint_modal(f, area, " secret ", lines, 10);
        }
        Overlay::Sessions {
            tab,
            saved,
            live,
            selected,
            loading,
            notice,
        } => {
            let tab_line = match tab {
                SessionTab::Saved => "[saved]   live",
                SessionTab::Live => " saved   [live]",
            };
            let mut lines = vec![
                Line::from(Span::styled(tab_line, theme::accent())),
                Line::from(Span::styled(
                    "Tab switches lists  ·  Enter opens  ·  Esc closes",
                    theme::dim(),
                )),
                Line::from(""),
            ];
            if *loading {
                lines.push(Line::from(Span::styled("loading…", theme::dim())));
            }
            if let Some(n) = notice {
                lines.push(Line::from(Span::styled(n.clone(), theme::dim())));
            }
            match tab {
                SessionTab::Saved => {
                    if saved.is_empty() && !*loading {
                        lines.push(Line::from(Span::styled("no saved sessions", theme::dim())));
                    }
                    for (i, s) in saved.iter().enumerate() {
                        let mark = if i == *selected { "▸ " } else { "  " };
                        let title = if s.title.is_empty() {
                            s.id.clone()
                        } else {
                            s.title.clone()
                        };
                        let preview = if s.preview.is_empty() {
                            String::new()
                        } else {
                            format!(" — {}", truncate(&s.preview, 40))
                        };
                        let src = if s.source.is_empty() {
                            String::new()
                        } else {
                            format!(" [{}]", s.source)
                        };
                        lines.push(Line::from(Span::styled(
                            format!("{mark}{title}{src}{preview}"),
                            if i == *selected {
                                theme::accent()
                            } else {
                                theme::text()
                            },
                        )));
                    }
                }
                SessionTab::Live => {
                    if live.is_empty() && !*loading {
                        lines.push(Line::from(Span::styled(
                            "no other live sessions in this process",
                            theme::dim(),
                        )));
                    }
                    for (i, s) in live.iter().enumerate() {
                        let mark = if i == *selected { "▸ " } else { "  " };
                        let cur = if s.current { " (current)" } else { "" };
                        let title = s.title.clone().unwrap_or_else(|| s.id.clone());
                        lines.push(Line::from(Span::styled(
                            format!("{mark}{title}{cur}  {}", s.status),
                            if i == *selected {
                                theme::accent()
                            } else {
                                theme::text()
                            },
                        )));
                    }
                }
            }
            paint_modal(f, area, " sessions ", lines, 18);
        }
        Overlay::Help => {
            let lines = vec![
                Line::from(Span::styled("hermes-rust", theme::accent())),
                Line::from(""),
                Line::from("Enter            send (steer if a turn is running)"),
                Line::from("Shift+Enter      newline"),
                Line::from("/                command catalog"),
                Line::from("Esc              interrupt / dismiss overlay"),
                Line::from("Ctrl+C then y    quit"),
                Line::from("PageUp/Down      scroll transcript"),
                Line::from(""),
                Line::from("/sessions  /resume     saved vs live sessions"),
                Line::from("/branch                fork this session"),
                Line::from("/agents                live subagents (Enter interrupts)"),
                Line::from("/trees                 spawn-tree snapshots"),
                Line::from("/rewind               regenerate from a user turn (row_id)"),
                Line::from("/model                provider + model picker"),
                Line::from("/skills  /plugins     install / toggle"),
                Line::from("/mcp                  add / remove MCP servers"),
                Line::from("/theme                gold / hermes / github palettes"),
                Line::from("/clear  /quit"),
                Line::from("Ctrl+O           expand last tool card"),
                Line::from("Ctrl+V           paste clipboard image (gateway)"),
                Line::from(""),
                Line::from(Span::styled(
                    "Paste a path ending in .png/.jpg/.webp to attach.",
                    theme::dim(),
                )),
                Line::from(Span::styled("Esc or Enter closes this help.", theme::dim())),
            ];
            paint_modal(f, area, " help ", lines, 23);
        }
        Overlay::Agents {
            agents,
            selected,
            loading,
        } => {
            let mut lines = vec![
                Line::from(Span::styled(
                    "Enter interrupts the selected child",
                    theme::dim(),
                )),
                Line::from(""),
            ];
            if *loading {
                lines.push(Line::from(Span::styled("loading…", theme::dim())));
            }
            if agents.is_empty() && !*loading {
                lines.push(Line::from(Span::styled("no live subagents", theme::dim())));
            }
            for (i, a) in agents.iter().enumerate() {
                let mark = if i == *selected { "▸ " } else { "  " };
                lines.push(Line::from(Span::styled(
                    format!("{mark}{}  {}  {}", a.id, a.status, a.goal),
                    if i == *selected {
                        theme::accent()
                    } else {
                        theme::text()
                    },
                )));
            }
            paint_modal(f, area, " agents ", lines, 14);
        }
        Overlay::SpawnTrees {
            entries,
            selected,
            loading,
        } => {
            let mut lines = vec![
                Line::from(Span::styled("Enter loads the selected tree", theme::dim())),
                Line::from(""),
            ];
            if *loading {
                lines.push(Line::from(Span::styled("loading…", theme::dim())));
            }
            if entries.is_empty() && !*loading {
                lines.push(Line::from(Span::styled(
                    "no spawn trees saved",
                    theme::dim(),
                )));
            }
            for (i, e) in entries.iter().enumerate() {
                let mark = if i == *selected { "▸ " } else { "  " };
                lines.push(Line::from(Span::styled(
                    format!("{mark}{}  ({} nodes)", e.label, e.count),
                    if i == *selected {
                        theme::accent()
                    } else {
                        theme::text()
                    },
                )));
            }
            paint_modal(f, area, " spawn trees ", lines, 14);
        }
        Overlay::Rewind {
            turns,
            selected,
            loading,
            confirming,
        } => {
            if *confirming {
                let preview = turns.get(*selected).map(|t| t.text.as_str()).unwrap_or("");
                let lines = vec![
                    Line::from(Span::styled(
                        "This drops that turn and everything after it.",
                        theme::error(),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(truncate(preview, 80), theme::text())),
                    Line::from(""),
                    Line::from(Span::styled(
                        "y / Enter confirm   ·   n / Esc cancel",
                        theme::dim(),
                    )),
                ];
                paint_modal(f, area, " confirm rewind ", lines, 10);
            } else {
                let mut lines = vec![
                    Line::from(Span::styled(
                        "Enter selects a user turn with a durable row_id",
                        theme::dim(),
                    )),
                    Line::from(""),
                ];
                if *loading {
                    lines.push(Line::from(Span::styled("loading history…", theme::dim())));
                }
                if turns.is_empty() && !*loading {
                    lines.push(Line::from(Span::styled(
                        "no rewind targets (need session.history row_id)",
                        theme::dim(),
                    )));
                }
                for (i, t) in turns.iter().enumerate() {
                    let mark = if i == *selected { "▸ " } else { "  " };
                    lines.push(Line::from(Span::styled(
                        format!("{mark}#{}  {}", t.row_id, truncate(&t.text, 56)),
                        if i == *selected {
                            theme::accent()
                        } else {
                            theme::text()
                        },
                    )));
                }
                paint_modal(f, area, " rewind ", lines, 16);
            }
        }
        Overlay::Theme { selected, saved_id } => {
            let mut lines = vec![
                Line::from(Span::styled(
                    "↑↓ preview   ·   Enter save   ·   Esc restore",
                    theme::dim(),
                )),
                Line::from(""),
            ];
            for (i, t) in theme::themes().iter().enumerate() {
                let mark = if i == *selected { "› " } else { "  " };
                let saved = if t.id == *saved_id { "  (saved)" } else { "" };
                lines.push(Line::from(Span::styled(
                    format!("{mark}{}{saved}", t.label),
                    if i == *selected {
                        theme::accent().add_modifier(Modifier::BOLD)
                    } else {
                        theme::text()
                    },
                )));
                lines.push(Line::from(Span::styled(
                    format!("    {}", t.description),
                    theme::dim(),
                )));
            }
            paint_modal(f, area, " theme ", lines, 14);
        }
        Overlay::Model(picker) => {
            let lines = model_picker::lines(picker);
            let h = (lines.len() as u16).saturating_add(2).clamp(8, 24);
            paint_modal(f, area, " model ", lines, h);
        }
        Overlay::Skills(hub) => {
            let lines = hubs::skills_lines(hub);
            let h = (lines.len() as u16).saturating_add(2).clamp(8, 22);
            paint_modal(f, area, " skills ", lines, h);
        }
        Overlay::Plugins(hub) => {
            let lines = hubs::plugins_lines(hub);
            let h = (lines.len() as u16).saturating_add(2).clamp(8, 22);
            paint_modal(f, area, " plugins ", lines, h);
        }
        Overlay::Mcp(hub) => {
            let lines = hubs::mcp_lines(hub);
            let h = (lines.len() as u16).saturating_add(2).clamp(8, 22);
            paint_modal(f, area, " mcp ", lines, h);
        }
    }
}

fn paint_modal(f: &mut Frame, area: Rect, title: &str, lines: Vec<Line>, want_h: u16) {
    let w = area.width.saturating_sub(4).min(72).max(24);
    let h = want_h.min(area.height.saturating_sub(2)).max(6);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(theme::accent())
        .style(Style::default().bg(theme::SURFACE()).fg(theme::TEXT()));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn truncate(s: &str, n: usize) -> String {
    let mut it = s.chars();
    let head: String = it.by_ref().take(n).collect();
    if it.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sudo_expire_only_clears_matching_id() {
        let mut overlay = Overlay::Sudo {
            request_id: "abc".into(),
            draft: "x".into(),
        };
        // matching
        if let Overlay::Sudo { request_id, .. } = &overlay {
            if request_id == "abc" {
                overlay.close();
            }
        }
        assert!(!overlay.is_open());
        overlay = Overlay::Sudo {
            request_id: "abc".into(),
            draft: "x".into(),
        };
        if let Overlay::Sudo { request_id, .. } = &overlay {
            if request_id == "zzz" {
                overlay.close();
            }
        }
        assert!(overlay.is_open());
    }

    #[test]
    fn rewind_confirm_sends_row_id_not_ordinal() {
        let mut overlay = Overlay::Rewind {
            turns: vec![
                RewindTurn {
                    row_id: 11,
                    text: "first".into(),
                    first: true,
                },
                RewindTurn {
                    row_id: 13,
                    text: "second".into(),
                    first: false,
                },
            ],
            selected: 1,
            loading: false,
            confirming: false,
        };
        let enter = KeyEvent::from(KeyCode::Enter);
        assert!(handle_overlay_key(&mut overlay, enter).is_none());
        assert!(matches!(
            overlay,
            Overlay::Rewind {
                confirming: true,
                selected: 1,
                ..
            }
        ));
        let yes = KeyEvent::from(KeyCode::Char('y'));
        match handle_overlay_key(&mut overlay, yes) {
            Some(ScreenAction::Rewind {
                text,
                truncate_before_row_id,
                confirm_empty_truncate,
            }) => {
                assert_eq!(text, "second");
                assert_eq!(truncate_before_row_id, 13);
                assert!(!confirm_empty_truncate);
            }
            other => panic!("{other:?}"),
        }
        assert!(!overlay.is_open());
    }

    #[test]
    fn rewind_first_turn_sets_empty_truncate() {
        let mut overlay = Overlay::Rewind {
            turns: vec![RewindTurn {
                row_id: 7,
                text: "only".into(),
                first: true,
            }],
            selected: 0,
            loading: false,
            confirming: true,
        };
        let enter = KeyEvent::from(KeyCode::Enter);
        match handle_overlay_key(&mut overlay, enter) {
            Some(ScreenAction::Rewind {
                truncate_before_row_id,
                confirm_empty_truncate,
                ..
            }) => {
                assert_eq!(truncate_before_row_id, 7);
                assert!(confirm_empty_truncate);
            }
            other => panic!("{other:?}"),
        }
    }
}
