use std::sync::Arc;

use warp_multi_agent_api as api;

use super::super::configuration::RunConfiguration;
use super::super::protocol::RuntimeFailureCode;
use super::super::tool_execution::ToolExecutionAuthority;
use super::super::transcript::RuntimeTranscript;
use crate::persistence::model::{AgentConversationData, AgentRuntimeRunState};

pub(in crate::ai::agent::runtime) struct TextRunRequest {
    pub(super) run_id: String,
    pub(super) retry_of_run_id: Option<String>,
    pub(super) transcript: RuntimeTranscript,
    pub(super) configuration: RunConfiguration,
    pub(super) tasks: Vec<api::Task>,
    pub(super) conversation_data: AgentConversationData,
    pub(super) output_task_id: String,
    pub(super) initial_input_commit_id: Option<String>,
    pub(super) resolve_retry_lineage: bool,
    pub(super) prepared: bool,
    pub(super) tool_execution_authority: Option<Arc<ToolExecutionAuthority>>,
}

impl TextRunRequest {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::ai::agent::runtime) fn new(
        run_id: impl Into<String>,
        retry_of_run_id: Option<impl Into<String>>,
        transcript: RuntimeTranscript,
        configuration: RunConfiguration,
        tasks: Vec<api::Task>,
        conversation_data: AgentConversationData,
        output_task_id: impl Into<String>,
    ) -> Self {
        Self {
            run_id: run_id.into(),
            retry_of_run_id: retry_of_run_id.map(Into::into),
            transcript,
            configuration,
            tasks,
            conversation_data,
            output_task_id: output_task_id.into(),
            initial_input_commit_id: None,
            resolve_retry_lineage: false,
            prepared: false,
            tool_execution_authority: None,
        }
    }

    pub(in crate::ai::agent::runtime) fn with_initial_input_commit(
        mut self,
        commit_id: impl Into<String>,
    ) -> Self {
        self.initial_input_commit_id = Some(commit_id.into());
        self
    }

    pub(in crate::ai::agent::runtime) fn with_retry_lineage_lookup(mut self) -> Self {
        self.resolve_retry_lineage = true;
        self
    }

    pub(in crate::ai::agent::runtime) fn with_tool_execution_authority(
        mut self,
        authority: Arc<ToolExecutionAuthority>,
    ) -> Self {
        self.tool_execution_authority = Some(authority);
        self
    }

    pub(in crate::ai::agent::runtime) fn revision(&self) -> u64 {
        self.transcript.revision()
    }

    pub(in crate::ai::agent::runtime) fn run_id(&self) -> &str {
        &self.run_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::ai::agent::runtime) enum TextRunOutcome {
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

#[expect(
    dead_code,
    reason = "GUI and TUI consume the typed Runtime events when Runtime Selection is enabled"
)]
pub(in crate::ai::agent::runtime) enum RuntimeEvent {
    RunStatus {
        run_id: String,
        state: AgentRuntimeRunState,
    },
    TextDelta {
        run_id: String,
        event_id: String,
        delta: String,
    },
    ConversationCommit {
        run_id: String,
        revision: u64,
        tasks: Vec<api::Task>,
        conversation_data: AgentConversationData,
    },
    RunFinished {
        run_id: String,
        outcome: TextRunOutcome,
    },
}

pub(in crate::ai::agent::runtime) struct TextRunResult {
    pub(super) outcome: TextRunOutcome,
    pub(super) revision: u64,
    pub(super) tasks: Vec<api::Task>,
    pub(super) conversation_data: AgentConversationData,
}

impl TextRunResult {
    pub(in crate::ai::agent::runtime) fn outcome(&self) -> &TextRunOutcome {
        &self.outcome
    }

    pub(in crate::ai::agent::runtime) fn revision(&self) -> u64 {
        self.revision
    }

    pub(in crate::ai::agent::runtime) fn tasks(&self) -> &[api::Task] {
        &self.tasks
    }

    pub(in crate::ai::agent::runtime) fn conversation_data(&self) -> &AgentConversationData {
        &self.conversation_data
    }

    pub(in crate::ai::agent::runtime) fn requires_process_rebuild(&self) -> bool {
        matches!(self.outcome, TextRunOutcome::Failed { .. })
    }
}
