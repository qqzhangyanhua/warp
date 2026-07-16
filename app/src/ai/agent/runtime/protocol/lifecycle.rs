use super::wire::{RunFailureCode, RunFinished};
use crate::ai::agent::runtime::transcript::RuntimeContentBlock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ai::agent::runtime) enum RuntimeRunStatus {
    Running,
    WaitingForCommit,
    WaitingForToolResult,
}

pub(in crate::ai::agent::runtime) struct RuntimeTextDelta {
    pub conversation_id: String,
    pub run_id: String,
    pub event_id: String,
    pub delta: String,
}

impl std::fmt::Debug for RuntimeTextDelta {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RuntimeTextDelta")
    }
}

pub(in crate::ai::agent::runtime) struct RuntimeAssistantCommit {
    pub conversation_id: String,
    pub run_id: String,
    pub event_id: String,
    pub commit_id: String,
    pub message_id: String,
    pub expected_revision: u64,
    pub content: Vec<RuntimeContentBlock>,
}

impl std::fmt::Debug for RuntimeAssistantCommit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RuntimeAssistantCommit")
    }
}

pub(in crate::ai::agent::runtime) struct RuntimeToolRequest {
    pub frame_fingerprint: [u8; 32],
    pub conversation_id: String,
    pub run_id: String,
    pub tool_call_id: String,
    pub tool_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Map<String, serde_json::Value>,
}

impl std::fmt::Debug for RuntimeToolRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RuntimeToolRequest")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ai::agent::runtime) enum RuntimeFailureCode {
    BridgeProtocolError,
    CommitTimeout,
    ProviderHttpError,
    ProviderProtocolError,
    ProviderRedirectNotAllowed,
    ProviderTransportError,
    RevisionConflict,
    RuntimeFailure,
    TranscriptSyncError,
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::ai::agent::runtime) enum RuntimeRunOutcome {
    Completed,
    Cancelled,
    Failed {
        error_code: RuntimeFailureCode,
        diagnostic_id: String,
    },
    LimitReached {
        tool_request_limit: u32,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::ai::agent::runtime) struct RuntimeRunFinished {
    pub conversation_id: String,
    pub run_id: String,
    pub outcome: RuntimeRunOutcome,
}

pub(in crate::ai::agent::runtime) enum LifecycleMessage {
    BridgeHello,
    TranscriptSyncAccepted {
        sync_id: String,
        revision: u64,
    },
    RunStatus {
        conversation_id: String,
        run_id: String,
        status: RuntimeRunStatus,
    },
    TextDelta(RuntimeTextDelta),
    AssistantMessageCommit(RuntimeAssistantCommit),
    ToolRequest(RuntimeToolRequest),
    RunFinished(RuntimeRunFinished),
    Other,
}

pub(super) fn map_run_finished(finished: RunFinished) -> RuntimeRunFinished {
    let (conversation_id, run_id, outcome) = match finished {
        RunFinished::Completed {
            conversation_id,
            run_id,
        } => (conversation_id, run_id, RuntimeRunOutcome::Completed),
        RunFinished::Cancelled {
            conversation_id,
            run_id,
        } => (conversation_id, run_id, RuntimeRunOutcome::Cancelled),
        RunFinished::Failed {
            conversation_id,
            run_id,
            error_code,
            diagnostic_id,
        } => (
            conversation_id,
            run_id,
            RuntimeRunOutcome::Failed {
                error_code: map_failure_code(error_code),
                diagnostic_id,
            },
        ),
        RunFinished::LimitReached {
            conversation_id,
            run_id,
            tool_request_limit,
        } => (
            conversation_id,
            run_id,
            RuntimeRunOutcome::LimitReached { tool_request_limit },
        ),
    };
    RuntimeRunFinished {
        conversation_id,
        run_id,
        outcome,
    }
}

fn map_failure_code(error_code: RunFailureCode) -> RuntimeFailureCode {
    match error_code {
        RunFailureCode::BridgeProtocolError => RuntimeFailureCode::BridgeProtocolError,
        RunFailureCode::CommitTimeout => RuntimeFailureCode::CommitTimeout,
        RunFailureCode::ProviderHttpError => RuntimeFailureCode::ProviderHttpError,
        RunFailureCode::ProviderProtocolError => RuntimeFailureCode::ProviderProtocolError,
        RunFailureCode::ProviderRedirectNotAllowed => {
            RuntimeFailureCode::ProviderRedirectNotAllowed
        }
        RunFailureCode::ProviderTransportError => RuntimeFailureCode::ProviderTransportError,
        RunFailureCode::RevisionConflict => RuntimeFailureCode::RevisionConflict,
        RunFailureCode::RuntimeFailure => RuntimeFailureCode::RuntimeFailure,
        RunFailureCode::TranscriptSyncError => RuntimeFailureCode::TranscriptSyncError,
    }
}
