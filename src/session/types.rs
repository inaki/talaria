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
    FetchModelOptions {
        refresh: bool,
    },
    SaveModelKey {
        slug: String,
        api_key: String,
    },
    DisconnectModel {
        slug: String,
    },
    SetConfig {
        key: String,
        value: String,
        confirm_expensive_model: bool,
    },
    FetchSkills,
    InstallSkill {
        query: String,
    },
    FetchPlugins,
    TogglePlugin {
        key: String,
        enable: bool,
    },
    FetchMcpServers,
    FetchMcpCatalog,
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
    FetchUsage,
    CloseSession {
        session_id: String,
        cols: u16,
    },
    ShellExec {
        command: String,
        cwd: String,
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
    ModelOptions {
        providers: Vec<ModelProvider>,
        model: String,
        error: Option<String>,
    },
    ModelKeySaved {
        provider: ModelProvider,
        error: Option<String>,
    },
    ModelDisconnected {
        slug: String,
        ok: bool,
    },
    ConfigSet {
        key: String,
        value: Option<String>,
        warning: Option<String>,
        deferred: bool,
        confirm_required: bool,
        confirm_message: Option<String>,
        info: Option<Value>,
    },
    SkillsList {
        groups: Vec<(String, Vec<String>)>,
        error: Option<String>,
    },
    SkillInstalled {
        name: String,
        ok: bool,
        error: Option<String>,
    },
    PluginsList {
        plugins: Vec<PluginRow>,
        error: Option<String>,
    },
    PluginToggled {
        plugin: Option<PluginRow>,
        ok: bool,
    },
    McpServers {
        servers: Vec<McpServer>,
        error: Option<String>,
    },
    McpCatalog {
        servers: Vec<McpCatalogEntry>,
        error: Option<String>,
    },
    McpChanged {
        name: String,
        ok: bool,
        error: Option<String>,
    },
    Usage(UsageSnapshot),
    ShellResult {
        command: String,
        output: String,
        code: Option<i32>,
        duration_ms: u64,
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

#[derive(Debug, Clone, Default)]
pub struct UsageSnapshot {
    pub calls: u64,
    pub input: u64,
    pub output: u64,
    pub total: u64,
    pub context_used: u64,
    pub context_max: u64,
    pub context_percent: u64,
    pub cost_usd: Option<f64>,
    pub model: String,
    pub credits_lines: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PluginRow {
    pub name: String,
    pub key: String,
    pub version: String,
    pub description: String,
    pub source: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct McpServer {
    pub name: String,
    pub transport: String,
    pub enabled: bool,
    pub auth: String,
}

#[derive(Debug, Clone)]
pub struct McpCatalogEntry {
    pub name: String,
    pub description: String,
    pub installed: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct ModelProvider {
    pub slug: String,
    pub name: String,
    pub authenticated: bool,
    pub is_current: bool,
    pub auth_type: String,
    pub key_env: Option<String>,
    pub models: Vec<String>,
    pub total_models: u64,
    pub warning: Option<String>,
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
