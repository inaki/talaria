use tokio::sync::mpsc;

use crate::session::{SessionEvent, SessionKind};
use crate::ui::screens::{Chat, CurrentScreen};

pub mod dispatch;
pub mod input;
pub mod render;
pub mod run;
pub mod tick;

pub use run::{run_tui, run_tui_with_options};

#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    pub dev: bool,
    pub mock: Option<String>,
    pub verbose: bool,
    pub theme: Option<String>,
    pub force: bool,
}

pub struct App {
    pub(crate) screen: CurrentScreen,
    pub(crate) session: Option<SessionKind>,
    pub(crate) events: Option<mpsc::Receiver<SessionEvent>>,
    pub(crate) should_quit: bool,
    pub(crate) tokio_handle: Option<tokio::runtime::Handle>,
    pub(crate) last_cols: u16,
    pub(crate) last_rows: u16,
    pub(crate) update_rx: Option<mpsc::Receiver<String>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: CurrentScreen::Chat(Chat::new()),
            session: None,
            events: None,
            should_quit: false,
            tokio_handle: None,
            last_cols: 80,
            last_rows: 24,
            update_rx: None,
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
