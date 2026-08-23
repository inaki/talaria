//! Session command/event types. Chat screens never name Live vs Mock.

use serde_json::Value;

#[derive(Debug, Clone)]
pub enum SessionCommand {
    Create {
        cwd: Option<String>,
        cols: u16,
    },
    Submit {
        text: String,
    },
    Interrupt,
    Resume {
        session_id: String,
        cols: u16,
    },
    Activate {
        session_id: String,
    },
    Dispatch {
        command: String,
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
    Resize {
        cols: u16,
        rows: u16,
    },
    Close,
    Shutdown,
    FetchCatalog,
    ListSaved {
        limit: u32,
    },
    ListActive,
    Steer {
        text: String,
    },
    Branch {
        title: Option<String>,
    },
    FetchDelegation,
    InterruptSubagent {
        subagent_id: String,
    },
    AttachImage {
        path: String,
    },
    ClipboardPaste,
    ListSpawnTrees,
    LoadSpawnTree {
        path: String,
    },
    FetchHistory,
    Rewind {
        text: String,
        truncate_before_row_id: i64,
        confirm_empty_truncate: bool,
    },
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    GatewayReady {
        skin: Option<Value>,
        change_events: Option<bool>,
    },
    SessionCreated {
        session_id: String,
        stored_session_id: Option<String>,
        info: Option<Value>,
    },
    SessionInfo {
        session_id: Option<String>,
        info: Value,
    },
    MessageDelta {
        session_id: Option<String>,
        text: String,
        rendered: Option<String>,
    },
    MessageComplete {
        session_id: Option<String>,
        text: Option<String>,
    },
    ToolStart {
        tool_id: String,
        name: Option<String>,
        args: Option<String>,
    },
    ToolProgress {
        tool_id: Option<String>,
        name: Option<String>,
        preview: Option<String>,
    },
    ToolComplete {
        tool_id: String,
        name: Option<String>,
        result: Option<String>,
        error: Option<String>,
    },
    ApprovalRequest {
        command: String,
        description: String,
        choices: Option<Vec<String>>,
        allow_permanent: Option<bool>,
        smart_denied: Option<bool>,
        request_id: Option<String>,
    },
    ApprovalPending,
    ClarifyRequest {
        request_id: String,
        payload: Value,
    },
    SudoRequest {
        request_id: String,
    },
    SecretRequest {
        request_id: String,
        env_var: String,
        prompt: String,
    },
    SudoExpired {
        request_id: String,
    },
    SecretExpired {
        request_id: String,
    },
    Status(String),
    Error {
        message: String,
    },
    ChildExited {
        code: Option<i32>,
    },
    ProtocolError {
        preview: String,
    },
    Stderr {
        line: String,
    },
    Unhandled {
        type_name: String,
        payload: Value,
    },
    Catalog {
        commands: Vec<SlashCommand>,
        warning: Option<String>,
    },
    SavedList {
        sessions: Vec<SavedSession>,
    },
    ActiveList {
        sessions: Vec<ActiveSession>,
    },
    CommandResult {
        output: Option<String>,
        send: Option<String>,
        prefill: Option<String>,
    },
    Transcript {
        session_id: String,
        messages: Vec<TranscriptMessage>,
    },
    Thinking {
        text: String,
    },
    Subagent {
        kind: SubagentKind,
        id: String,
        goal: Option<String>,
        text: Option<String>,
    },
    Delegation {
        agents: Vec<SubagentRow>,
    },
    SpawnTrees {
        entries: Vec<SpawnTreeEntry>,
    },
    History {
        messages: Vec<TranscriptMessage>,
    },
    RewindApplied {
        survivor_user_row_ids: Option<Vec<Option<i64>>>,
    },
}

#[derive(Debug, Clone)]
pub struct SpawnTreeEntry {
    pub path: String,
    pub label: String,
    pub count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentKind {
    Start,
    Progress,
    Tool,
    Complete,
}

#[derive(Debug, Clone)]
pub struct SubagentRow {
    pub id: String,
    pub goal: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct SlashCommand {
    pub name: String,
    pub help: String,
}

#[derive(Debug, Clone)]
pub struct SavedSession {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub source: String,
    pub message_count: u64,
}

#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub id: String,
    pub title: Option<String>,
    pub status: String,
    pub current: bool,
}

#[derive(Debug, Clone)]
pub struct TranscriptMessage {
    pub role: String,
    pub text: String,
    pub row_id: Option<i64>,
    pub display_kind: Option<String>,
}

/// A user turn the rewind picker can address. `first` maps to gateway
/// `confirm_empty_truncate` (ordinal 0 / empty-transcript cut).
#[derive(Debug, Clone)]
pub struct RewindTurn {
    pub row_id: i64,
    pub text: String,
    pub first: bool,
}
