use std::sync::mpsc::SyncSender;
use std::sync::Arc;

use futures::channel::oneshot;
use serde_json::json;
use sha2::{Digest as _, Sha256};
use warp_multi_agent_api as api;

mod recovery;
mod types;

pub(crate) use types::ToolExecutionError;
pub(super) use types::{
    RuntimeToolActionAdapter, ToolEffectOutcome, ToolExecutionResult, ToolPermissionDecision,
    ToolRunState,
};

use super::protocol::RuntimeToolRequest;
use super::tool_catalog::{ToolCatalog, TOOL_REQUEST_LIMIT};
use super::transcript::{
    RuntimeContentBlock, ToolDenialSource, ToolErrorCode, ToolResultProjection,
};
use crate::ai::agent::task::TaskId;
use crate::ai::agent::{AIAgentAction, AIAgentActionId};
use crate::persistence::{
    AcceptAgentToolExecution, AcceptAgentToolExecutionResult, AgentRuntimeSidecarMutation,
    CommitAgentRuntimeMutation, CompleteToolOutcomePayload, MarkAgentToolExecutionExecuting,
    ModelEvent, ToolRequestPayload, ToolResultProjectionPayload,
};

pub(super) struct ToolExecutionAuthority {
    catalog: ToolCatalog,
    adapter: Arc<dyn RuntimeToolActionAdapter>,
    persistence: SyncSender<ModelEvent>,
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
        }
    }

    pub(super) async fn handle(
        &self,
        request: RuntimeToolRequest,
        state: &mut ToolRunState,
    ) -> Result<ToolExecutionResult, ToolExecutionError> {
        let fingerprint = request_fingerprint(&request);
        match self.accept(&request, &state.task_id, fingerprint).await? {
            AcceptAgentToolExecutionResult::Completed {
                tool_result_projection,
                ..
            } => {
                let projection = serde_json::from_slice(tool_result_projection.bytes())
                    .map_err(|_| ToolExecutionError::InvalidStoredProjection)?;
                return Ok(ToolExecutionResult {
                    run_must_end: projection_ends_run(&projection),
                    projection,
                });
            }
            AcceptAgentToolExecutionResult::LimitReached { .. } => {
                let projection = error_projection(
                    ToolErrorCode::ToolRequestLimitExceeded,
                    false,
                    "Tool Request Limit reached. No tool was executed.",
                );
                self.complete(
                    &request,
                    state,
                    self.resolve_tool(&request),
                    None,
                    projection.clone(),
                    serde_json::to_vec(&projection)?,
                    CompletionState::Pending,
                )
                .await?;
                return Ok(ToolExecutionResult {
                    projection,
                    run_must_end: true,
                });
            }
            AcceptAgentToolExecutionResult::Executing => {
                let projection = unknown_outcome_projection();
                self.complete(
                    &request,
                    state,
                    self.resolve_tool(&request),
                    None,
                    projection.clone(),
                    serde_json::to_vec(&projection)?,
                    CompletionState::Executing,
                )
                .await?;
                return Ok(ToolExecutionResult {
                    projection,
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
                self.complete(
                    &request,
                    state,
                    self.resolve_tool(&request),
                    None,
                    projection.clone(),
                    serde_json::to_vec(&projection)?,
                    CompletionState::Pending,
                )
                .await?;
                return Ok(ToolExecutionResult {
                    projection,
                    run_must_end: true,
                });
            }
            AcceptAgentToolExecutionResult::Pending {
                newly_inserted: true,
            } => {}
        }

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
        match self.adapter.request_permission(action.clone()).await {
            ToolPermissionDecision::DeniedByPolicy => {
                self.complete_denial(request, state, tool, ToolDenialSource::Policy)
                    .await
            }
            ToolPermissionDecision::DeniedByUser => {
                self.complete_denial(request, state, tool, ToolDenialSource::User)
                    .await
            }
            ToolPermissionDecision::Approved => {
                match self.mark_executing(&request, fingerprint).await? {
                    AcceptAgentToolExecutionResult::Executing => {}
                    AcceptAgentToolExecutionResult::Completed {
                        tool_result_projection,
                        ..
                    } => {
                        let projection = serde_json::from_slice(tool_result_projection.bytes())
                            .map_err(|_| ToolExecutionError::InvalidStoredProjection)?;
                        return Ok(ToolExecutionResult {
                            run_must_end: projection_ends_run(&projection),
                            projection,
                        });
                    }
                    AcceptAgentToolExecutionResult::Pending { .. }
                    | AcceptAgentToolExecutionResult::LimitReached { .. } => {
                        return Err(ToolExecutionError::InvalidPersistenceState);
                    }
                }
                let outcome = self.adapter.execute(action).await;
                self.complete(
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
        self.complete(
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
        self.complete(
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
    ) -> Result<(), ToolExecutionError> {
        let mut tasks = state.tasks.clone();
        let task = tasks
            .iter_mut()
            .find(|task| task.id == state.task_id)
            .ok_or(ToolExecutionError::TaskNotFound)?;
        task.messages
            .push(tool_request_message(request, &state.task_id, tool));
        task.messages
            .push(tool_result_message(request, &state.task_id, result));
        let sidecar_mutation = match completion_state {
            CompletionState::Pending => AgentRuntimeSidecarMutation::CompletePendingToolExecution {
                tool_call_id: request.tool_call_id.clone(),
                complete_outcome: CompleteToolOutcomePayload::current(complete_outcome),
                tool_result_projection: ToolResultProjectionPayload::current(serde_json::to_vec(
                    &projection,
                )?),
            },
            CompletionState::Executing => AgentRuntimeSidecarMutation::CompleteToolExecution {
                tool_call_id: request.tool_call_id.clone(),
                complete_outcome: CompleteToolOutcomePayload::current(complete_outcome),
                tool_result_projection: ToolResultProjectionPayload::current(serde_json::to_vec(
                    &projection,
                )?),
            },
        };
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
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum CompletionState {
    Pending,
    Executing,
}

fn typed_action(
    request: &RuntimeToolRequest,
    task_id: &str,
    tool: api::message::tool_call::Tool,
) -> Result<AIAgentAction, ToolExecutionError> {
    let action = match tool {
        api::message::tool_call::Tool::RunShellCommand(tool) => tool.into(),
        api::message::tool_call::Tool::ReadFiles(tool) => tool.into(),
        api::message::tool_call::Tool::ApplyFileDiffs(tool) => tool.into(),
        api::message::tool_call::Tool::CallMcpTool(tool) => tool
            .try_into()
            .map_err(|_| ToolExecutionError::InvalidTypedAction)?,
        _ => return Err(ToolExecutionError::InvalidTypedAction),
    };
    Ok(AIAgentAction {
        id: AIAgentActionId::from(request.tool_call_id.clone()),
        task_id: TaskId::new(task_id.to_string()),
        action,
        requires_result: true,
    })
}

fn tool_request_message(
    request: &RuntimeToolRequest,
    task_id: &str,
    tool: Option<api::message::tool_call::Tool>,
) -> api::Message {
    runtime_message(
        format!("tool-request:{}:{}", request.run_id, request.tool_call_id),
        request,
        task_id,
        api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: request.tool_call_id.clone(),
            tool,
        }),
    )
}

fn tool_result_message(
    request: &RuntimeToolRequest,
    task_id: &str,
    result: Option<api::message::tool_call_result::Result>,
) -> api::Message {
    runtime_message(
        format!("tool-result:{}:{}", request.run_id, request.tool_call_id),
        request,
        task_id,
        api::message::Message::ToolCallResult(api::message::ToolCallResult {
            tool_call_id: request.tool_call_id.clone(),
            context: None,
            result,
        }),
    )
}

fn runtime_message(
    id: String,
    request: &RuntimeToolRequest,
    task_id: &str,
    message: api::message::Message,
) -> api::Message {
    api::Message {
        id,
        task_id: task_id.to_string(),
        request_id: request.run_id.clone(),
        message: Some(message),
        ..Default::default()
    }
}

fn request_fingerprint(request: &RuntimeToolRequest) -> [u8; 32] {
    let payload = serde_json::to_vec(&json!({
        "version": 1,
        "tool_id": request.tool_id,
        "tool_name": request.tool_name,
        "arguments": request.arguments,
    }))
    .expect("Tool Request fingerprint input must serialize");
    Sha256::digest(payload).into()
}

fn request_payload(request: &RuntimeToolRequest, task_id: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "version": 1,
        "task_id": task_id,
        "tool_id": request.tool_id,
        "tool_name": request.tool_name,
        "arguments": request.arguments,
    }))
    .expect("Tool Request payload must serialize")
}

fn unknown_outcome_projection() -> ToolResultProjection {
    error_projection(
        ToolErrorCode::ToolOutcomeUnknown,
        true,
        "Warp cannot determine whether the previous tool effect completed.",
    )
}

fn error_projection(
    error_code: ToolErrorCode,
    may_have_executed: bool,
    text: &str,
) -> ToolResultProjection {
    ToolResultProjection::Error {
        error_code,
        may_have_executed,
        content: vec![RuntimeContentBlock::Text {
            text: text.to_string(),
        }],
        truncated: false,
    }
}

fn projection_ends_run(projection: &ToolResultProjection) -> bool {
    matches!(
        projection,
        ToolResultProjection::Error {
            error_code: ToolErrorCode::ToolRequestLimitExceeded | ToolErrorCode::ToolOutcomeUnknown,
            ..
        }
    )
}

#[cfg(test)]
#[path = "tool_execution_tests.rs"]
mod tests;
