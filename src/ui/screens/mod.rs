use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use serde_json::Value;

use crate::ui::widgets::KeyHints;

pub mod chat;

pub use chat::Chat;

pub trait Screen {
    fn render(&mut self, f: &mut Frame, area: Rect);
    fn handle_key(&mut self, key: KeyEvent) -> Option<ScreenAction>;
    fn handle_mouse(&mut self, _mouse: MouseEvent, _area: Rect) -> Option<ScreenAction> {
        None
    }
    fn handle_paste(&mut self, _text: String) -> Option<ScreenAction> {
        None
    }
    fn tick(&mut self);
    fn key_hints(&self) -> KeyHints;
}

pub enum ScreenAction {
    Quit,
    SubmitPrompt(String),
    Interrupt,
    DispatchCommand {
        command: String,
    },
    OpenSessions,
    ResumeSaved {
        session_id: String,
    },
    ActivateLive {
        session_id: String,
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
    Steer {
        text: String,
    },
    Branch {
        title: Option<String>,
    },
    OpenAgents,
    InterruptSubagent {
        subagent_id: String,
    },
    AttachImages {
        items: Vec<crate::attachments::Attachment>,
    },
    ClipboardPaste,
    OpenSpawnTrees,
    LoadSpawnTree {
        path: String,
    },
    SaveSpawnTree,
    OpenRewind,
    Rewind {
        text: String,
        truncate_before_row_id: i64,
        confirm_empty_truncate: bool,
    },
    OpenTheme,
    OpenModel {
        refresh: bool,
    },
    SaveModelKey {
        slug: String,
        api_key: String,
    },
    DisconnectModel {
        slug: String,
    },
    SetModel {
        value: String,
        confirm_expensive_model: bool,
    },
    OpenSkills,
    InstallSkill {
        query: String,
    },
    OpenPlugins,
    TogglePlugin {
        key: String,
        enable: bool,
    },
    OpenMcp,
    AddMcp {
        name: String,
        preset: String,
    },
    RemoveMcp {
        name: String,
    },
    SteerSubagent {
        subagent_id: String,
        text: String,
    },
    OpenUsage,
    NewSession,
    CloseLive {
        session_id: String,
    },
    ShellExec {
        command: String,
    },
}

fn secret_dbg(s: &str) -> &'static str {
    if s.is_empty() {
        ""
    } else {
        "***"
    }
}

impl std::fmt::Debug for ScreenAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScreenAction::Quit => write!(f, "Quit"),
            ScreenAction::SubmitPrompt(_) => write!(f, "SubmitPrompt(<text>)"),
            ScreenAction::Interrupt => write!(f, "Interrupt"),
            ScreenAction::DispatchCommand { command } => f
                .debug_struct("DispatchCommand")
                .field("command", command)
                .finish(),
            ScreenAction::OpenSessions => write!(f, "OpenSessions"),
            ScreenAction::ResumeSaved { session_id } => f
                .debug_struct("ResumeSaved")
                .field("session_id", session_id)
                .finish(),
            ScreenAction::ActivateLive { session_id } => f
                .debug_struct("ActivateLive")
                .field("session_id", session_id)
                .finish(),
            ScreenAction::RespondApproval { choice, request_id } => f
                .debug_struct("RespondApproval")
                .field("choice", choice)
                .field("request_id", request_id)
                .finish(),
            ScreenAction::RespondClarify { request_id, .. } => f
                .debug_struct("RespondClarify")
                .field("request_id", request_id)
                .field("answers", &"<omitted>")
                .finish(),
            ScreenAction::RespondSudo {
                request_id,
                password,
            } => f
                .debug_struct("RespondSudo")
                .field("request_id", request_id)
                .field("password", &secret_dbg(password))
                .finish(),
            ScreenAction::RespondSecret { request_id, value } => f
                .debug_struct("RespondSecret")
                .field("request_id", request_id)
                .field("value", &secret_dbg(value))
                .finish(),
            ScreenAction::Steer { .. } => write!(f, "Steer(<text>)"),
            ScreenAction::Branch { title } => {
                f.debug_struct("Branch").field("title", title).finish()
            }
            ScreenAction::OpenAgents => write!(f, "OpenAgents"),
            ScreenAction::InterruptSubagent { subagent_id } => f
                .debug_struct("InterruptSubagent")
                .field("subagent_id", subagent_id)
                .finish(),
            ScreenAction::AttachImages { items } => f
                .debug_struct("AttachImages")
                .field("count", &items.len())
                .finish(),
            ScreenAction::ClipboardPaste => write!(f, "ClipboardPaste"),
            ScreenAction::OpenSpawnTrees => write!(f, "OpenSpawnTrees"),
            ScreenAction::LoadSpawnTree { path } => {
                f.debug_struct("LoadSpawnTree").field("path", path).finish()
            }
            ScreenAction::SaveSpawnTree => write!(f, "SaveSpawnTree"),
            ScreenAction::OpenRewind => write!(f, "OpenRewind"),
            ScreenAction::Rewind {
                truncate_before_row_id,
                confirm_empty_truncate,
                ..
            } => f
                .debug_struct("Rewind")
                .field("truncate_before_row_id", truncate_before_row_id)
                .field("confirm_empty_truncate", confirm_empty_truncate)
                .field("text", &"<omitted>")
                .finish(),
            ScreenAction::OpenTheme => write!(f, "OpenTheme"),
            ScreenAction::OpenModel { refresh } => f
                .debug_struct("OpenModel")
                .field("refresh", refresh)
                .finish(),
            ScreenAction::SaveModelKey { slug, api_key } => f
                .debug_struct("SaveModelKey")
                .field("slug", slug)
                .field("api_key", &secret_dbg(api_key))
                .finish(),
            ScreenAction::DisconnectModel { slug } => f
                .debug_struct("DisconnectModel")
                .field("slug", slug)
                .finish(),
            ScreenAction::SetModel {
                value,
                confirm_expensive_model,
            } => f
                .debug_struct("SetModel")
                .field("value", value)
                .field("confirm_expensive_model", confirm_expensive_model)
                .finish(),
            ScreenAction::OpenSkills => write!(f, "OpenSkills"),
            ScreenAction::InstallSkill { query } => f
                .debug_struct("InstallSkill")
                .field("query", query)
                .finish(),
            ScreenAction::OpenPlugins => write!(f, "OpenPlugins"),
            ScreenAction::TogglePlugin { key, enable } => f
                .debug_struct("TogglePlugin")
                .field("key", key)
                .field("enable", enable)
                .finish(),
            ScreenAction::OpenMcp => write!(f, "OpenMcp"),
            ScreenAction::AddMcp { name, preset } => f
                .debug_struct("AddMcp")
                .field("name", name)
                .field("preset", preset)
                .finish(),
            ScreenAction::RemoveMcp { name } => {
                f.debug_struct("RemoveMcp").field("name", name).finish()
            }
            ScreenAction::SteerSubagent { subagent_id, .. } => f
                .debug_struct("SteerSubagent")
                .field("subagent_id", subagent_id)
                .field("text", &"<omitted>")
                .finish(),
            ScreenAction::OpenUsage => write!(f, "OpenUsage"),
            ScreenAction::NewSession => write!(f, "NewSession"),
            ScreenAction::CloseLive { session_id } => f
                .debug_struct("CloseLive")
                .field("session_id", session_id)
                .finish(),
            ScreenAction::ShellExec { command } => f
                .debug_struct("ShellExec")
                .field("command", command)
                .finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_passwords_and_keys() {
        let sudo = ScreenAction::RespondSudo {
            request_id: "r1".into(),
            password: "hunter2-secret".into(),
        };
        let s = format!("{sudo:?}");
        assert!(!s.contains("hunter2-secret"), "{s}");
        assert!(s.contains("***"), "{s}");

        let key = ScreenAction::SaveModelKey {
            slug: "openai".into(),
            api_key: "sk-live-should-not-debug".into(),
        };
        let s = format!("{key:?}");
        assert!(!s.contains("sk-live-should-not-debug"), "{s}");

        let secret = ScreenAction::RespondSecret {
            request_id: "r2".into(),
            value: "super-secret-value".into(),
        };
        assert!(!format!("{secret:?}").contains("super-secret-value"));
    }
}

pub enum CurrentScreen {
    Chat(Chat),
}

impl CurrentScreen {
    pub fn as_screen_mut(&mut self) -> &mut dyn Screen {
        match self {
            CurrentScreen::Chat(c) => c,
        }
    }
}
