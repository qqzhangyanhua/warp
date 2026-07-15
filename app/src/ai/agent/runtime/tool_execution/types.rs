use futures::future::BoxFuture;
use thiserror::Error;
use warp_multi_agent_api as api;

use super::super::transcript::ToolResultProjection;
use crate::ai::agent::AIAgentAction;
use crate::persistence::model::AgentConversationData;
use crate::persistence::{
    AcceptAgentToolExecutionError, CommitAgentRuntimeMutationError,
    MarkAgentToolExecutionExecutingError, ReadExecutingAgentToolExecutionsError,
};

pub(in crate::ai::agent::runtime) trait RuntimeToolActionAdapter:
    Send + Sync
{
    fn request_permission(
        &self,
        action: AIAgentAction,
    ) -> BoxFuture<'static, ToolPermissionDecision>;

    fn execute(&self, action: AIAgentAction) -> BoxFuture<'static, ToolEffectOutcome>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ai::agent::runtime) enum ToolPermissionDecision {
    Approved,
    DeniedByPolicy,
    DeniedByUser,
}

pub(in crate::ai::agent::runtime) struct ToolEffectOutcome {
    pub complete_outcome: Vec<u8>,
    pub result: Option<api::message::tool_call_result::Result>,
    pub projection: ToolResultProjection,
}

pub(in crate::ai::agent::runtime) struct ToolRunState {
    pub revision: u64,
    pub tasks: Vec<api::Task>,
    pub conversation_data: AgentConversationData,
    pub task_id: String,
}

#[derive(Debug)]
pub(in crate::ai::agent::runtime) struct ToolExecutionResult {
    pub projection: ToolResultProjection,
    pub run_must_end: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum ToolExecutionError {
    #[error("Tool Execution persistence is unavailable")]
    PersistenceUnavailable,
    #[error("Tool Execution persistence acknowledgement was dropped")]
    PersistenceAcknowledgementDropped,
    #[error(transparent)]
    Accept(#[from] AcceptAgentToolExecutionError),
    #[error(transparent)]
    MarkExecuting(#[from] MarkAgentToolExecutionExecutingError),
    #[error(transparent)]
    Commit(#[from] CommitAgentRuntimeMutationError),
    #[error(transparent)]
    ReadExecuting(#[from] ReadExecutingAgentToolExecutionsError),
    #[error("Tool Execution Record has an invalid durable state")]
    InvalidPersistenceState,
    #[error("Stored Tool Result Projection is invalid")]
    InvalidStoredProjection,
    #[error("Stored Tool Request is invalid")]
    InvalidStoredRequest,
    #[error("Tool Request could not be converted to a typed Warp action")]
    InvalidTypedAction,
    #[error("Agent task for Tool Execution was not found")]
    TaskNotFound,
    #[error("Tool Execution payload serialization failed")]
    Serialization,
}

impl From<serde_json::Error> for ToolExecutionError {
    fn from(_: serde_json::Error) -> Self {
        Self::Serialization
    }
}
