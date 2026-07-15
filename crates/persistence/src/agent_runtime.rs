use chrono::NaiveDateTime;
use diesel::prelude::*;

use super::schema::{agent_runtime_runs, agent_tool_execution_records};

pub const COMPLETE_TOOL_OUTCOME_ENCODING_VERSION: i32 = 1;
pub const TOOL_RESULT_PROJECTION_ENCODING_VERSION: i32 = 1;
pub const TOOL_REQUEST_ENCODING_VERSION: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRuntimeRunState {
    Starting,
    Running,
    WaitingForCommit,
    WaitingForToolResult,
    Finished,
}

impl AgentRuntimeRunState {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::WaitingForCommit => "waiting_for_commit",
            Self::WaitingForToolResult => "waiting_for_tool_result",
            Self::Finished => "finished",
        }
    }

    fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "starting" => Some(Self::Starting),
            "running" => Some(Self::Running),
            "waiting_for_commit" => Some(Self::WaitingForCommit),
            "waiting_for_tool_result" => Some(Self::WaitingForToolResult),
            "finished" => Some(Self::Finished),
            _ => None,
        }
    }
}

#[derive(Debug, Insertable)]
#[diesel(table_name = agent_runtime_runs)]
pub struct NewAgentRuntimeRunRecord<'a> {
    conversation_id: &'a str,
    run_id: &'a str,
    retry_of_run_id: Option<&'a str>,
    starting_revision: i64,
    state: &'static str,
}

impl<'a> NewAgentRuntimeRunRecord<'a> {
    pub fn starting(
        conversation_id: &'a str,
        run_id: &'a str,
        retry_of_run_id: Option<&'a str>,
        starting_revision: i64,
    ) -> Self {
        Self {
            conversation_id,
            run_id,
            retry_of_run_id,
            starting_revision,
            state: AgentRuntimeRunState::Starting.as_database_value(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRuntimeTerminalOutcome {
    Completed,
    Cancelled,
    Failed,
    LimitReached,
}

impl AgentRuntimeTerminalOutcome {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::LimitReached => "limit_reached",
        }
    }

    fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "completed" => Some(Self::Completed),
            "cancelled" => Some(Self::Cancelled),
            "failed" => Some(Self::Failed),
            "limit_reached" => Some(Self::LimitReached),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Queryable, Selectable, Clone)]
#[diesel(table_name = agent_runtime_runs)]
#[diesel(primary_key(id))]
pub struct AgentRuntimeRunRecord {
    pub id: i32,
    pub conversation_id: String,
    pub run_id: String,
    pub retry_of_run_id: Option<String>,
    pub starting_revision: i64,
    state: String,
    terminal_outcome: Option<String>,
    pub last_commit_id: Option<String>,
    pub last_committed_revision: Option<i64>,
    pub created_at: NaiveDateTime,
    pub last_modified_at: NaiveDateTime,
    pub last_commit_payload_fingerprint: Option<Vec<u8>>,
}

impl AgentRuntimeRunRecord {
    pub fn state(&self) -> Option<AgentRuntimeRunState> {
        AgentRuntimeRunState::from_database_value(&self.state)
    }

    pub fn terminal_outcome(&self) -> Option<AgentRuntimeTerminalOutcome> {
        self.terminal_outcome
            .as_deref()
            .and_then(AgentRuntimeTerminalOutcome::from_database_value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentToolExecutionState {
    Pending,
    Executing,
    Completed,
}

impl AgentToolExecutionState {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Executing => "executing",
            Self::Completed => "completed",
        }
    }

    pub fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "executing" => Some(Self::Executing),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionedCompleteToolOutcome<'a> {
    encoding_version: i32,
    bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionedToolRequest<'a> {
    encoding_version: i32,
    bytes: &'a [u8],
}

impl<'a> VersionedToolRequest<'a> {
    pub fn current(bytes: &'a [u8]) -> Self {
        Self {
            encoding_version: TOOL_REQUEST_ENCODING_VERSION,
            bytes,
        }
    }

    pub fn from_parts(encoding_version: i32, bytes: &'a [u8]) -> Option<Self> {
        (encoding_version == TOOL_REQUEST_ENCODING_VERSION).then_some(Self {
            encoding_version,
            bytes,
        })
    }

    pub fn encoding_version(self) -> i32 {
        self.encoding_version
    }

    pub fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

impl<'a> VersionedCompleteToolOutcome<'a> {
    pub fn current(bytes: &'a [u8]) -> Self {
        Self {
            encoding_version: COMPLETE_TOOL_OUTCOME_ENCODING_VERSION,
            bytes,
        }
    }

    pub fn from_parts(encoding_version: i32, bytes: &'a [u8]) -> Option<Self> {
        (encoding_version == COMPLETE_TOOL_OUTCOME_ENCODING_VERSION).then_some(Self {
            encoding_version,
            bytes,
        })
    }

    pub fn encoding_version(self) -> i32 {
        self.encoding_version
    }

    pub fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionedToolResultProjection<'a> {
    encoding_version: i32,
    bytes: &'a [u8],
}

impl<'a> VersionedToolResultProjection<'a> {
    pub fn current(bytes: &'a [u8]) -> Self {
        Self {
            encoding_version: TOOL_RESULT_PROJECTION_ENCODING_VERSION,
            bytes,
        }
    }

    pub fn from_parts(encoding_version: i32, bytes: &'a [u8]) -> Option<Self> {
        (encoding_version == TOOL_RESULT_PROJECTION_ENCODING_VERSION).then_some(Self {
            encoding_version,
            bytes,
        })
    }

    pub fn encoding_version(self) -> i32 {
        self.encoding_version
    }

    pub fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

#[derive(Debug, Insertable)]
#[diesel(table_name = agent_tool_execution_records)]
pub struct NewAgentToolExecutionRecord<'a> {
    conversation_id: &'a str,
    run_id: &'a str,
    tool_call_id: &'a str,
    request_fingerprint: &'a [u8],
    request_encoding_version: i32,
    request_payload: &'a [u8],
    state: &'static str,
}

impl<'a> NewAgentToolExecutionRecord<'a> {
    pub fn pending(
        conversation_id: &'a str,
        run_id: &'a str,
        tool_call_id: &'a str,
        request_fingerprint: &'a [u8; 32],
        request: VersionedToolRequest<'a>,
    ) -> Self {
        Self {
            conversation_id,
            run_id,
            tool_call_id,
            request_fingerprint,
            request_encoding_version: request.encoding_version,
            request_payload: request.bytes,
            state: AgentToolExecutionState::Pending.as_database_value(),
        }
    }
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = agent_tool_execution_records)]
pub struct CompleteAgentToolExecution<'a> {
    state: &'static str,
    complete_outcome_encoding_version: i32,
    complete_outcome: &'a [u8],
    tool_result_projection_encoding_version: i32,
    tool_result_projection: &'a [u8],
}

impl<'a> CompleteAgentToolExecution<'a> {
    pub fn new(
        complete_outcome: VersionedCompleteToolOutcome<'a>,
        tool_result_projection: VersionedToolResultProjection<'a>,
    ) -> Self {
        Self {
            state: AgentToolExecutionState::Completed.as_database_value(),
            complete_outcome_encoding_version: complete_outcome.encoding_version,
            complete_outcome: complete_outcome.bytes,
            tool_result_projection_encoding_version: tool_result_projection.encoding_version,
            tool_result_projection: tool_result_projection.bytes,
        }
    }
}

#[derive(Debug, PartialEq, Queryable, Selectable)]
#[diesel(table_name = agent_tool_execution_records)]
#[diesel(primary_key(id))]
pub struct AgentToolExecutionRecord {
    pub id: i32,
    pub conversation_id: String,
    pub run_id: String,
    pub tool_call_id: String,
    pub request_fingerprint: Vec<u8>,
    request_encoding_version: i32,
    request_payload: Vec<u8>,
    state: String,
    complete_outcome_encoding_version: Option<i32>,
    complete_outcome: Option<Vec<u8>>,
    tool_result_projection_encoding_version: Option<i32>,
    tool_result_projection: Option<Vec<u8>>,
    pub created_at: NaiveDateTime,
    pub last_modified_at: NaiveDateTime,
}

impl AgentToolExecutionRecord {
    pub fn tool_request(&self) -> Option<VersionedToolRequest<'_>> {
        VersionedToolRequest::from_parts(self.request_encoding_version, &self.request_payload)
    }

    pub fn state(&self) -> Option<AgentToolExecutionState> {
        AgentToolExecutionState::from_database_value(&self.state)
    }

    pub fn complete_outcome(&self) -> Option<VersionedCompleteToolOutcome<'_>> {
        VersionedCompleteToolOutcome::from_parts(
            self.complete_outcome_encoding_version?,
            self.complete_outcome.as_deref()?,
        )
    }

    pub fn tool_result_projection(&self) -> Option<VersionedToolResultProjection<'_>> {
        VersionedToolResultProjection::from_parts(
            self.tool_result_projection_encoding_version?,
            self.tool_result_projection.as_deref()?,
        )
    }
}
