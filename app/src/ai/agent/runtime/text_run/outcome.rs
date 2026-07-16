use super::{TextRunOutcome, ToolExecutionResult};
use crate::ai::agent::runtime::protocol::{
    RuntimeFailureCode, RuntimeRunOutcome, RuntimeRunStatus,
};
use crate::ai::agent::runtime::tool_catalog::TOOL_REQUEST_LIMIT;
use crate::ai::agent::runtime::transcript::{ToolErrorCode, ToolResultProjection};
use crate::persistence::model::{AgentRuntimeRunState, AgentRuntimeTerminalOutcome};

pub(super) fn persistence_state(status: RuntimeRunStatus) -> AgentRuntimeRunState {
    match status {
        RuntimeRunStatus::Running => AgentRuntimeRunState::Running,
        RuntimeRunStatus::WaitingForCommit => AgentRuntimeRunState::WaitingForCommit,
        RuntimeRunStatus::WaitingForToolResult => AgentRuntimeRunState::WaitingForToolResult,
    }
}

pub(super) fn text_run_outcome(outcome: RuntimeRunOutcome) -> TextRunOutcome {
    match outcome {
        RuntimeRunOutcome::Completed => TextRunOutcome::Completed,
        RuntimeRunOutcome::Cancelled => TextRunOutcome::Cancelled,
        RuntimeRunOutcome::Failed {
            error_code,
            diagnostic_id,
        } => TextRunOutcome::Failed {
            error_code,
            diagnostic_id,
        },
        RuntimeRunOutcome::LimitReached { tool_request_limit } => {
            TextRunOutcome::LimitReached { tool_request_limit }
        }
    }
}

pub(super) fn terminal_outcome(outcome: &TextRunOutcome) -> AgentRuntimeTerminalOutcome {
    match outcome {
        TextRunOutcome::Completed => AgentRuntimeTerminalOutcome::Completed,
        TextRunOutcome::Cancelled => AgentRuntimeTerminalOutcome::Cancelled,
        TextRunOutcome::Failed { .. } => AgentRuntimeTerminalOutcome::Failed,
        TextRunOutcome::LimitReached { .. } => AgentRuntimeTerminalOutcome::LimitReached,
    }
}

pub(super) fn terminal_outcome_for_tool_result(result: &ToolExecutionResult) -> TextRunOutcome {
    match &result.projection {
        ToolResultProjection::Error {
            error_code: ToolErrorCode::ToolRequestLimitExceeded,
            ..
        } => TextRunOutcome::LimitReached {
            tool_request_limit: TOOL_REQUEST_LIMIT,
        },
        ToolResultProjection::Error { .. }
        | ToolResultProjection::Success { .. }
        | ToolResultProjection::Denied { .. } => TextRunOutcome::Failed {
            error_code: RuntimeFailureCode::RuntimeFailure,
            diagnostic_id: "tool_run_ended".to_string(),
        },
    }
}
