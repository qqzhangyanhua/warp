use std::fmt;

use serde::Deserialize;
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct ProtocolCapability {
    pub name: String,
    pub version: u32,
    pub schema_hash: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct BridgeHello {
    pub protocol_version: u32,
    pub core_schema_hash: String,
    pub bridge_version: String,
    pub capabilities: Vec<ProtocolCapability>,
    pub prompt_version: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum HandshakeResult {
    Accepted {
        max_frame_bytes: u32,
        max_transcript_bytes: u32,
    },
    Rejected {
        error_code: HandshakeRejectionCode,
        diagnostic_id: String,
    },
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum HandshakeRejectionCode {
    ProtocolVersionMismatch,
    CoreSchemaMismatch,
    MissingRequiredCapability,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct TranscriptSyncBegin {
    pub(super) sync_id: String,
    conversation_id: String,
    pub(super) revision: u64,
    pub(super) item_count: u32,
    total_bytes: u64,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct TranscriptSyncItem {
    pub(super) sync_id: String,
    pub(super) index: u32,
    item: TranscriptItem,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum TranscriptItem {
    Message {
        message_id: String,
        role: TranscriptMessageRole,
        content: Vec<RuntimeContentBlock>,
    },
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum TranscriptMessageRole {
    User,
    Assistant,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum RuntimeContentBlock {
    Text {
        text: String,
    },
    Image {
        mime_type: ImageMimeType,
        data_base64: String,
    },
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(super) enum ImageMimeType {
    #[serde(rename = "image/gif")]
    Gif,
    #[serde(rename = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/webp")]
    Webp,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct TranscriptSyncCommit {
    pub(super) sync_id: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum TranscriptSyncResult {
    Accepted { sync_id: String, revision: u64 },
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct RunStart {
    conversation_id: String,
    pub(super) run_id: String,
    pub(super) transcript_revision: u64,
    configuration: RunConfiguration,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RunConfiguration {
    provider: ProviderConfiguration,
    working_directory: String,
    context_limit: u64,
    reasoning_effort: ReasoningEffort,
    tool_request_limit: u32,
    tools: Vec<ToolCatalogEntry>,
    resources: Vec<AgentResource>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ProviderConfiguration {
    protocol: ProviderProtocol,
    base_url: String,
    provider_origin: String,
    model: String,
    api_key: ProviderApiKey,
    max_provider_attempts: u8,
    max_redirects: u8,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(transparent)]
struct ProviderApiKey(String);

impl fmt::Debug for ProviderApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ProviderProtocol {
    ChatCompletions,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ToolCatalogEntry {
    id: String,
    name: String,
    description: String,
    input_schema: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AgentResource {
    id: String,
    name: String,
    content: Vec<RuntimeContentBlock>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct RunStatus {
    conversation_id: String,
    run_id: String,
    status: RunState,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RunState {
    Running,
    WaitingForCommit,
    WaitingForToolResult,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct TextDelta {
    conversation_id: String,
    run_id: String,
    event_id: String,
    delta: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct AssistantMessageCommit {
    conversation_id: String,
    run_id: String,
    event_id: String,
    pub(super) commit_id: String,
    message_id: String,
    pub(super) expected_revision: u64,
    content: Vec<RuntimeContentBlock>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum CommitResult {
    Committed {
        conversation_id: String,
        run_id: String,
        commit_id: String,
        revision: u64,
    },
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct RunCancel {
    conversation_id: String,
    run_id: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum RunFinished {
    Completed {
        conversation_id: String,
        run_id: String,
    },
    Cancelled {
        conversation_id: String,
        run_id: String,
    },
    Failed {
        conversation_id: String,
        run_id: String,
        error_code: RunFailureCode,
        diagnostic_id: String,
    },
    LimitReached {
        conversation_id: String,
        run_id: String,
        tool_request_limit: u32,
    },
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum RunFailureCode {
    BridgeProtocolError,
    CommitTimeout,
    ProviderHttpError,
    ProviderProtocolError,
    ProviderTransportError,
    RevisionConflict,
    RuntimeFailure,
    TranscriptSyncError,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct ToolRequest {
    conversation_id: String,
    pub(super) run_id: String,
    pub(super) tool_call_id: String,
    tool_id: String,
    tool_name: String,
    arguments: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ToolResult {
    Success {
        conversation_id: String,
        run_id: String,
        tool_call_id: String,
        content: Vec<RuntimeContentBlock>,
        truncated: bool,
    },
    Denied {
        conversation_id: String,
        run_id: String,
        tool_call_id: String,
        denied_by: ToolDenialSource,
        content: Vec<RuntimeContentBlock>,
        truncated: bool,
    },
    Error {
        conversation_id: String,
        run_id: String,
        tool_call_id: String,
        error_code: ToolErrorCode,
        may_have_executed: bool,
        content: Vec<RuntimeContentBlock>,
        truncated: bool,
    },
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ToolDenialSource {
    Policy,
    User,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ToolErrorCode {
    InvalidToolRequest,
    ToolExecutionFailed,
    ToolRequestLimitExceeded,
    ToolOutcomeUnknown,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ProtocolMessage {
    BridgeHello(BridgeHello),
    HandshakeResult(HandshakeResult),
    TranscriptSyncBegin(TranscriptSyncBegin),
    TranscriptSyncItem(TranscriptSyncItem),
    TranscriptSyncCommit(TranscriptSyncCommit),
    TranscriptSyncResult(TranscriptSyncResult),
    RunStart(RunStart),
    RunStatus(RunStatus),
    TextDelta(TextDelta),
    AssistantMessageCommit(AssistantMessageCommit),
    CommitResult(CommitResult),
    RunCancel(RunCancel),
    RunFinished(RunFinished),
    ToolRequest(ToolRequest),
    ToolResult(ToolResult),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum ProtocolError {
    #[error("Invalid Bridge Protocol message")]
    InvalidMessage,
    #[error("Bridge Protocol frame exceeds the {max_frame_bytes}-byte limit")]
    FrameTooLarge { max_frame_bytes: usize },
}

impl ProtocolMessage {
    pub(super) fn parse_line(line: &str) -> Result<Self, ProtocolError> {
        serde_json::from_str(line).map_err(|_| ProtocolError::InvalidMessage)
    }
}
