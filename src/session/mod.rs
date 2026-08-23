//! Session control surface. Chat screens never name Live vs Mock.

mod live;
mod mock;
pub(crate) mod parse;
mod types;

use tokio::sync::mpsc;

pub use live::GatewaySession;
pub use mock::{MockScenario, MockSession};
pub(crate) use parse::rewind_turns_from_messages;
pub use types::{
    ActiveSession, McpCatalogEntry, McpServer, ModelProvider, PluginRow, RewindTurn, SavedSession,
    SessionCommand, SessionEvent, SlashCommand, SpawnTreeEntry, SubagentKind, SubagentRow,
    TranscriptMessage,
};

pub trait SessionApi {
    fn send(&self, cmd: SessionCommand);
    fn take_events(&mut self) -> Option<mpsc::Receiver<SessionEvent>>;
}

pub enum SessionKind {
    Live(GatewaySession),
    Mock(MockSession),
}

impl SessionApi for SessionKind {
    fn send(&self, cmd: SessionCommand) {
        match self {
            SessionKind::Live(s) => s.send(cmd),
            SessionKind::Mock(s) => s.send(cmd),
        }
    }

    fn take_events(&mut self) -> Option<mpsc::Receiver<SessionEvent>> {
        match self {
            SessionKind::Live(s) => s.take_events(),
            SessionKind::Mock(s) => s.take_events(),
        }
    }
}
