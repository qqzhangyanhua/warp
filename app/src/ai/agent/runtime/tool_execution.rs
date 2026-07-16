use std::sync::mpsc::SyncSender;
use std::sync::Arc;

use futures::channel::oneshot;
use warp_multi_agent_api as api;

#[cfg(not(target_family = "wasm"))]
pub(super) mod blocklist_adapter;
mod fault_injection;
mod messages;
mod projection;
mod recovery;
mod request;
mod types;

#[cfg(test)]
use fault_injection::ToolExecutionFaultInjector;
use fault_injection::ToolExecutionFaultPoint;
use messages::{tool_request_message, tool_result_message};
use projection::{error_projection, projection_ends_run, unknown_outcome_projection};
use request::{request_payload, typed_action};
pub(super) use types::{
    RuntimeToolActionAdapter, ToolEffectOutcome, ToolExecutionResult, ToolRunState,
};
pub(crate) use types::{ToolExecutionError, ToolPermissionDecision};

use super::protocol::RuntimeToolRequest;
use super::tool_catalog::{ToolCatalog, TOOL_REQUEST_LIMIT};
use super::transcript::{
    RuntimeContentBlock, ToolDenialSource, ToolErrorCode, ToolResultProjection,
};
use crate::persistence::model::{AgentRuntimeTerminalOutcome, AgentToolExecutionState};
use crate::persistence::{
    AcceptAgentToolExecution, AcceptAgentToolExecutionResult, AgentRuntimeSidecarMutation,
    CommitAgentRuntimeMutation, CompleteToolOutcomePayload, MarkAgentToolExecutionExecuting,
    ModelEvent, ToolRequestPayload, ToolResultProjectionPayload,
};

#[cfg(all(test, not(target_family = "wasm")))]
#[path = "tool_execution/blocklist_adapter_tests.rs"]
mod blocklist_adapter_tests;

#[cfg(all(test, not(target_family = "wasm")))]
#[path = "tool_execution/blocklist_adapter_cancel_tests.rs"]
mod blocklist_adapter_cancel_tests;

pub(super) struct ToolExecutionAuthority {
    catalog: ToolCatalog,
    adapter: Arc<dyn RuntimeToolActionAdapter>,
    persistence: SyncSender<ModelEvent>,
    #[cfg(test)]
    fault_injector: Option<Arc<dyn ToolExecutionFaultInjector>>,
}

impl ToolExecutionAuthority {
    pub(super) fn new(
        catalog: ToolCatalog,
        adapter: Arc<dyn RuntimeToolActionAdapter>,
        persistence: SyncSender<ModelEvent>,
    ) -> Self {
        Self {
            catalog,
            adapter,
            persistence,
            #[cfg(test)]
            fault_injector: None,
        }
    }

    #[cfg(test)]
    fn set_fault_injector(&mut self, fault_injector: Arc<dyn ToolExecutionFaultInjector>) {
        self.fault_injector = Some(fault_injector);
    }

    #[cfg(test)]
    fn clear_fault_injector(&mut self) {
        self.fault_injector = None;
    }

    pub(super) async fn cancel_run(&self, run_id: String) {
        self.adapter.cancel_run(run_id).await;
    }

    pub(super) async fn handle(
        &self,
        request: RuntimeToolRequest,
        state: &mut ToolRunState,
    ) -> Result<ToolExecutionResult, ToolExecutionError> {
        let fingerprint = request.frame_fingerprint;
        self.inject_fault(ToolExecutionFaultPoint::BeforePendingPersisted)?;
        match self.accept(&request, &state.task_id, fingerprint).await? {
            AcceptAgentToolExecutionResult::Completed {
                tool_result_projection,
                ..
            } => {
                let projection_bytes = tool_result_projection.bytes().to_vec();
                let projection = serde_json::from_slice(&projection_bytes)
                    .map_err(|_| ToolExecutionError::InvalidStoredProjection)?;
                return Ok(ToolExecutionResult {
                    run_must_end: projection_ends_run(&projection),
                    projection,
                    projection_bytes,
                });
            }
            AcceptAgentToolExecutionResult::LimitReached { .. } => {
                let projection = error_projection(
                    ToolErrorCode::ToolRequestLimitExceeded,
                    false,
                    "Tool Request Limit reached. No tool was executed.",
                );
                let projection_bytes = self
                    .complete(
                        &request,
                        state,
                        self.resolve_tool(&request),
                        None,
                        projection.clone(),
                        serde_json::to_vec(&projection)?,
                        CompletionState::LimitReached,
                    )
                    .await?;
                return Ok(ToolExecutionResult {
                    projection,
                    projection_bytes,
                    run_must_end: true,
                });
            }
            AcceptAgentToolExecutionResult::Executing => {
                let projection = unknown_outcome_projection();
                let projection_bytes = self
                    .complete(
                        &request,
                        state,
                        self.resolve_tool(&request),
                        None,
                        projection.clone(),
                        serde_json::to_vec(&projection)?,
                        CompletionState::RecoveringExecuting,
                    )
                    .await?;
                return Ok(ToolExecutionResult {
                    projection,
                    projection_bytes,
                    run_must_end: true,
                });
            }
            AcceptAgentToolExecutionResult::Pending {
                newly_inserted: false,
            } => {
                let projection = error_projection(
                    ToolErrorCode::ToolExecutionFailed,
                    false,
                    "The previous tool request stopped before execution and was not retried.",
                );
                let projection_bytes = self
                    .complete(
                        &request,
                        state,
                        self.resolve_tool(&request),
                        None,
                        projection.clone(),
                        serde_json::to_vec(&projection)?,
                        CompletionState::RecoveringPending,
                    )
                    .await?;
                return Ok(ToolExecutionResult {
                    projection,
                    projection_bytes,
                    run_must_end: true,
                });
            }
            AcceptAgentToolExecutionResult::Pending {
                newly_inserted: true,
            } => {}
        }
        self.inject_fault(ToolExecutionFaultPoint::AfterPendingPersisted)?;

        let tool =
            match self
                .catalog
                .resolve(&request.tool_id, &request.tool_name, &request.arguments)
            {
                Ok(tool) => tool,
                Err(_) => {
                    return self.complete_invalid(request, state).await;
                }
            };
        let action = typed_action(&request, &state.task_id, tool.clone())?;
        self.inject_fault(ToolExecutionFaultPoint::BeforePermissionDecision)?;
        let permission = self
            .adapter
            .request_permission(request.run_id.clone(), action.clone())
            .await;
        self.inject_fault(ToolExecutionFaultPoint::AfterPermissionDecision)?;
        match permission {
            ToolPermissionDecision::DeniedByPolicy => {
                self.complete_denial(request, state, tool, ToolDenialSource::Policy)
                    .await
            }
            ToolPermissionDecision::DeniedByUser => {
                self.complete_denial(request, state, tool, ToolDenialSource::User)
                    .await
            }
            ToolPermissionDecision::Approved => {
                self.inject_fault(ToolExecutionFaultPoint::BeforeExecutingPersisted)?;
                match self.mark_executing(&request, fingerprint).await? {
                    AcceptAgentToolExecutionResult::Executing => {}
                    AcceptAgentToolExecutionResult::Completed {
                        tool_result_projection,
                        ..
                    } => {
                        let projection_bytes = tool_result_projection.bytes().to_vec();
                        let projection = serde_json::from_slice(&projection_bytes)
                            .map_err(|_| ToolExecutionError::InvalidStoredProjection)?;
                        return Ok(ToolExecutionResult {
                            run_must_end: projection_ends_run(&projection),
                            projection,
                            projection_bytes,
                        });
                    }
                    AcceptAgentToolExecutionResult::Pending { .. }
                    | AcceptAgentToolExecutionResult::LimitReached { .. } => {
                        return Err(ToolExecutionError::InvalidPersistenceState);
                    }
                }
                self.inject_fault(ToolExecutionFaultPoint::AfterExecutingPersisted)?;
                self.inject_fault(ToolExecutionFaultPoint::BeforeEffect)?;
                let outcome = self.adapter.execute(request.run_id.clone(), action).await;
                self.inject_fault(ToolExecutionFaultPoint::AfterEffectReturned)?;
                let projection_bytes = self
                    .complete(
                        &request,
                        state,
                        Some(tool),
                        outcome.result,
                        outcome.projection.clone(),
                        outcome.complete_outcome,
                        CompletionState::Executing,
                    )
                    .await?;
                Ok(ToolExecutionResult {
                    run_must_end: projection_ends_run(&outcome.projection),
                    projection: outcome.projection,
                    projection_bytes,
                })
            }
        }
    }

    async fn complete_invalid(
        &self,
        request: RuntimeToolRequest,
        state: &mut ToolRunState,
    ) -> Result<ToolExecutionResult, ToolExecutionError> {
        let projection = error_projection(
            ToolErrorCode::InvalidToolRequest,
            false,
            "Invalid Tool Request. No permission was requested and no tool was executed.",
        );
        let projection_bytes = self
            .complete(
                &request,
                state,
                None,
                None,
                projection.clone(),
                serde_json::to_vec(&projection)?,
                CompletionState::Pending,
            )
            .await?;
        Ok(ToolExecutionResult {
            projection,
            projection_bytes,
            run_must_end: false,
        })
    }

    fn resolve_tool(&self, request: &RuntimeToolRequest) -> Option<api::message::tool_call::Tool> {
        self.catalog
            .resolve(&request.tool_id, &request.tool_name, &request.arguments)
            .ok()
    }

    async fn complete_denial(
        &self,
        request: RuntimeToolRequest,
        state: &mut ToolRunState,
        tool: api::message::tool_call::Tool,
        denied_by: ToolDenialSource,
    ) -> Result<ToolExecutionResult, ToolExecutionError> {
        let projection = ToolResultProjection::Denied {
            denied_by,
            content: vec![RuntimeContentBlock::Text {
                text: "Warp denied this tool request. No tool was executed.".to_string(),
            }],
            truncated: false,
        };
        let projection_bytes = self
            .complete(
                &request,
                state,
                Some(tool),
                Some(api::message::tool_call_result::Result::Cancel(())),
                projection.clone(),
                serde_json::to_vec(&projection)?,
                CompletionState::Pending,
            )
            .await?;
        Ok(ToolExecutionResult {
            projection,
            projection_bytes,
            run_must_end: false,
        })
    }

    async fn accept(
        &self,
        request: &RuntimeToolRequest,
        request_task_id: &str,
        request_fingerprint: [u8; 32],
    ) -> Result<AcceptAgentToolExecutionResult, ToolExecutionError> {
        let (acknowledgement, acknowledged) = oneshot::channel();
        self.persistence
            .send(ModelEvent::AcceptAgentToolExecution(
                AcceptAgentToolExecution {
                    conversation_id: request.conversation_id.clone(),
                    run_id: request.run_id.clone(),
                    tool_call_id: request.tool_call_id.clone(),
                    request_fingerprint,
                    request_payload: ToolRequestPayload::current(request_payload(
                        request,
                        &request_task_id,
                    )),
                    request_limit: TOOL_REQUEST_LIMIT,
                    acknowledgement,
                },
            ))
            .map_err(|_| ToolExecutionError::PersistenceUnavailable)?;
        Ok(acknowledged
            .await
            .map_err(|_| ToolExecutionError::PersistenceAcknowledgementDropped)??)
    }

    async fn mark_executing(
        &self,
        request: &RuntimeToolRequest,
        request_fingerprint: [u8; 32],
    ) -> Result<AcceptAgentToolExecutionResult, ToolExecutionError> {
        let (acknowledgement, acknowledged) = oneshot::channel();
        self.persistence
            .send(ModelEvent::MarkAgentToolExecutionExecuting(
                MarkAgentToolExecutionExecuting {
                    conversation_id: request.conversation_id.clone(),
                    run_id: request.run_id.clone(),
                    tool_call_id: request.tool_call_id.clone(),
                    request_fingerprint,
                    acknowledgement,
                },
            ))
            .map_err(|_| ToolExecutionError::PersistenceUnavailable)?;
        Ok(acknowledged
            .await
            .map_err(|_| ToolExecutionError::PersistenceAcknowledgementDropped)??)
    }

    #[allow(clippy::too_many_arguments)]
    async fn complete(
        &self,
        request: &RuntimeToolRequest,
        state: &mut ToolRunState,
        tool: Option<api::message::tool_call::Tool>,
        result: Option<api::message::tool_call_result::Result>,
        projection: ToolResultProjection,
        complete_outcome: Vec<u8>,
        completion_state: CompletionState,
    ) -> Result<Vec<u8>, ToolExecutionError> {
        let mut tasks = state.tasks.clone();
        let task = tasks
            .iter_mut()
            .find(|task| task.id == state.task_id)
            .ok_or(ToolExecutionError::TaskNotFound)?;
        task.messages
            .push(tool_request_message(request, &state.task_id, tool));
        task.messages
            .push(tool_result_message(request, &state.task_id, result));
        let (expected_state, run_terminal_outcome) = match completion_state {
            CompletionState::Pending => (AgentToolExecutionState::Pending, None),
            CompletionState::Executing => (AgentToolExecutionState::Executing, None),
            CompletionState::LimitReached => (
                AgentToolExecutionState::Pending,
                Some(AgentRuntimeTerminalOutcome::LimitReached),
            ),
            CompletionState::RecoveringPending => (
                AgentToolExecutionState::Pending,
                Some(AgentRuntimeTerminalOutcome::Failed),
            ),
            CompletionState::RecoveringExecuting => (
                AgentToolExecutionState::Executing,
                Some(AgentRuntimeTerminalOutcome::Failed),
            ),
        };
        let projection_bytes = serde_json::to_vec(&projection)?;
        let sidecar_mutation = AgentRuntimeSidecarMutation::CompleteToolExecution {
            tool_call_id: request.tool_call_id.clone(),
            expected_state,
            complete_outcome: CompleteToolOutcomePayload::current(complete_outcome),
            tool_result_projection: ToolResultProjectionPayload::current(projection_bytes.clone()),
            run_terminal_outcome,
        };
        self.inject_fault(ToolExecutionFaultPoint::BeforeOutcomeCommitted)?;
        let (acknowledgement, acknowledged) = oneshot::channel();
        self.persistence
            .send(ModelEvent::CommitAgentRuntimeMutation(
                CommitAgentRuntimeMutation {
                    conversation_id: request.conversation_id.clone(),
                    run_id: request.run_id.clone(),
                    commit_id: format!("tool-result:{}:{}", request.run_id, request.tool_call_id),
                    expected_revision: state.revision,
                    updated_tasks: tasks.clone(),
                    conversation_data: state.conversation_data.clone(),
                    sidecar_mutation: Some(sidecar_mutation),
                    acknowledgement,
                },
            ))
            .map_err(|_| ToolExecutionError::PersistenceUnavailable)?;
        let revision = acknowledged
            .await
            .map_err(|_| ToolExecutionError::PersistenceAcknowledgementDropped)??;
        state.revision = revision;
        state.tasks = tasks;
        state.conversation_data.runtime_transcript_revision = Some(revision);
        self.inject_fault(ToolExecutionFaultPoint::AfterOutcomeCommitted)?;
        Ok(projection_bytes)
    }

    fn inject_fault(&self, point: ToolExecutionFaultPoint) -> Result<(), ToolExecutionError> {
        #[cfg(test)]
        if self
            .fault_injector
            .as_ref()
            .is_some_and(|injector| injector.should_fail(point))
        {
            return Err(ToolExecutionError::InjectedFault(point));
        }
        let _ = point;
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum CompletionState {
    Pending,
    Executing,
    LimitReached,
    RecoveringPending,
    RecoveringExecuting,
}

#[cfg(test)]
#[path = "tool_execution_tests.rs"]
mod tests;
