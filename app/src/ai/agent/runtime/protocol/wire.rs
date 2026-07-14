use std::fmt;
use std::sync::LazyLock;

use serde::Deserialize;
use thiserror::Error;

macro_rules! impl_content_free_debug {
    ($type:ty, $name:literal) => {
        impl fmt::Debug for $type {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str($name)
            }
        }
    };
}

pub(super) const CORE_SCHEMA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tools/warp-bridge/protocol/core-v1.schema.json"
));

static CORE_PROTOCOL_VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    let schema =
        serde_json::from_slice(CORE_SCHEMA).expect("Core Protocol schema must be valid JSON");
    jsonschema::validator_for(&schema).expect("Core Protocol schema must compile")
});

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

#[derive(Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct TranscriptSyncItem {
    pub(super) sync_id: String,
    pub(super) index: u32,
    item: TranscriptItem,
}

impl_content_free_debug!(TranscriptSyncItem, "TranscriptSyncItem");

#[derive(Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum TranscriptItem {
    Message {
        message_id: String,
        role: TranscriptMessageRole,
        content: Vec<RuntimeContentBlock>,
    },
    ResourceSnapshot {
        resource_id: String,
        name: String,
        content: Vec<RuntimeContentBlock>,
    },
    ToolRequest {
        tool_call_id: String,
        tool_id: String,
        tool_name: String,
        arguments: serde_json::Map<String, serde_json::Value>,
    },
    ToolResult {
        tool_call_id: String,
        result: TranscriptToolResult,
    },
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum TranscriptToolResult {
    Success {
        content: Vec<RuntimeContentBlock>,
        truncated: bool,
    },
    Denied {
        denied_by: ToolDenialSource,
        content: Vec<RuntimeContentBlock>,
        truncated: bool,
    },
    Error {
        error_code: ToolErrorCode,
        may_have_executed: bool,
        content: Vec<RuntimeContentBlock>,
        truncated: bool,
    },
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum TranscriptMessageRole {
    User,
    Assistant,
}

#[derive(Deserialize, PartialEq, Eq)]
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

impl_content_free_debug!(RuntimeContentBlock, "RuntimeContentBlock");

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

#[derive(Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct RunStart {
    conversation_id: String,
    pub(super) run_id: String,
    pub(super) transcript_revision: u64,
    configuration: RunConfiguration,
}

impl_content_free_debug!(RunStart, "RunStart");

#[derive(Deserialize, PartialEq, Eq)]
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

#[derive(Deserialize, PartialEq, Eq)]
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

#[derive(Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ToolCatalogEntry {
    id: String,
    name: String,
    description: String,
    input_schema: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize, PartialEq, Eq)]
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

#[derive(Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct TextDelta {
    conversation_id: String,
    run_id: String,
    event_id: String,
    delta: String,
}

impl_content_free_debug!(TextDelta, "TextDelta");

#[derive(Deserialize, PartialEq, Eq)]
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

impl_content_free_debug!(AssistantMessageCommit, "AssistantMessageCommit");

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

impl RunFinished {
    pub(super) fn cancelled_identity(&self) -> Option<(&str, &str)> {
        match self {
            Self::Cancelled {
                conversation_id,
                run_id,
            } => Some((conversation_id, run_id)),
            Self::Completed { .. } | Self::Failed { .. } | Self::LimitReached { .. } => None,
        }
    }
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

#[derive(Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct ToolRequest {
    conversation_id: String,
    pub(super) run_id: String,
    pub(super) tool_call_id: String,
    tool_id: String,
    tool_name: String,
    arguments: serde_json::Map<String, serde_json::Value>,
}

impl_content_free_debug!(ToolRequest, "ToolRequest");

#[derive(Deserialize, PartialEq, Eq)]
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

impl_content_free_debug!(ToolResult, "ToolResult");

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

#[derive(Deserialize, PartialEq, Eq)]
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
        let value = serde_json::from_str(line).map_err(|_| ProtocolError::InvalidMessage)?;
        if !CORE_PROTOCOL_VALIDATOR.is_valid(&value) {
            return Err(ProtocolError::InvalidMessage);
        }
        serde_json::from_value(value).map_err(|_| ProtocolError::InvalidMessage)
    }
}

impl fmt::Debug for ProtocolMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message_type = match self {
            Self::BridgeHello(_) => "bridge_hello",
            Self::HandshakeResult(_) => "handshake_result",
            Self::TranscriptSyncBegin(_) => "transcript_sync_begin",
            Self::TranscriptSyncItem(_) => "transcript_sync_item",
            Self::TranscriptSyncCommit(_) => "transcript_sync_commit",
            Self::TranscriptSyncResult(_) => "transcript_sync_result",
            Self::RunStart(_) => "run_start",
            Self::RunStatus(_) => "run_status",
            Self::TextDelta(_) => "text_delta",
            Self::AssistantMessageCommit(_) => "assistant_message_commit",
            Self::CommitResult(_) => "commit_result",
            Self::RunCancel(_) => "run_cancel",
            Self::RunFinished(_) => "run_finished",
            Self::ToolRequest(_) => "tool_request",
            Self::ToolResult(_) => "tool_result",
        };
        formatter
            .debug_tuple("ProtocolMessage")
            .field(&message_type)
            .finish()
    }
}
