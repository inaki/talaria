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
    fn key_hints(&self) -> KeyHints<'static>;
}

#[derive(Debug)]
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
    AttachImage {
        path: String,
    },
    ClipboardPaste,
    OpenSpawnTrees,
    LoadSpawnTree {
        path: String,
    },
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
