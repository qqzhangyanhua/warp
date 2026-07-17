use futures::channel::oneshot;
use serde::Deserialize;

use super::{
    error_projection, unknown_outcome_projection, CompletionState, ToolExecutionAuthority,
    ToolExecutionError, ToolRunState,
};
use crate::ai::agent::runtime::protocol::RuntimeToolRequest;
use crate::ai::agent::runtime::transcript::{ToolErrorCode, TranscriptItem};
use crate::persistence::model::AgentToolExecutionState;
use crate::persistence::{
    ModelEvent, ReadUnfinishedAgentToolExecutions, UnfinishedAgentToolExecution,
};

impl ToolExecutionAuthority {
    pub(in crate::ai::agent::runtime) async fn has_unfinished(
        &self,
        conversation_id: &str,
    ) -> Result<bool, ToolExecutionError> {
        Ok(!self.read_unfinished(conversation_id).await?.is_empty())
    }

    pub(in crate::ai::agent::runtime) async fn recover_unfinished(
        &self,
        conversation_id: &str,
        state: &mut ToolRunState,
    ) -> Result<Vec<TranscriptItem>, ToolExecutionError> {
        let unfinished = self.read_unfinished(conversation_id).await?;
        let mut transcript_items = Vec::with_capacity(unfinished.len() * 2);
        for record in unfinished {
            let stored =
                StoredToolRequest::decode(record.request_payload.bytes(), state.task_id.clone())?;
            if stored.version != 1 {
                return Err(ToolExecutionError::InvalidStoredRequest);
            }
            let request = RuntimeToolRequest {
                frame_fingerprint: record.request_fingerprint,
                conversation_id: conversation_id.to_string(),
                run_id: record.run_id,
                tool_call_id: record.tool_call_id,
                tool_id: stored.tool_id,
                tool_name: stored.tool_name,
                arguments: stored.arguments,
            };
            let (projection, completion_state) = match record.state {
                AgentToolExecutionState::Pending => (
                    error_projection(
                        ToolErrorCode::ToolExecutionFailed,
                        false,
                        "The previous tool request stopped before execution and was not retried.",
                    ),
                    CompletionState::RecoveringPending,
                ),
                AgentToolExecutionState::Executing => (
                    unknown_outcome_projection(),
                    CompletionState::RecoveringExecuting,
                ),
                AgentToolExecutionState::Completed => {
                    return Err(ToolExecutionError::InvalidPersistenceState);
                }
            };
            let original_task_id = std::mem::replace(&mut state.task_id, stored.task_id);
            let completion = self
                .complete(
                    &request,
                    state,
                    self.resolve_tool(&request),
                    None,
                    projection.clone(),
                    serde_json::to_vec(&projection)?,
                    completion_state,
                )
                .await;
            state.task_id = original_task_id;
            completion?;
            transcript_items.extend([
                TranscriptItem::ToolRequest {
                    tool_call_id: request.tool_call_id.clone(),
                    tool_id: request.tool_id,
                    tool_name: request.tool_name,
                    arguments: request.arguments,
                },
                TranscriptItem::ToolResult {
                    tool_call_id: request.tool_call_id,
                    result: projection,
                },
            ]);
        }
        Ok(transcript_items)
    }

    async fn read_unfinished(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<UnfinishedAgentToolExecution>, ToolExecutionError> {
        let (acknowledgement, acknowledged) = oneshot::channel();
        self.persistence
            .send(ModelEvent::ReadUnfinishedAgentToolExecutions(
                ReadUnfinishedAgentToolExecutions {
                    conversation_id: conversation_id.to_string(),
                    acknowledgement,
                },
            ))
            .map_err(|_| ToolExecutionError::PersistenceUnavailable)?;
        acknowledged
            .await
            .map_err(|_| ToolExecutionError::PersistenceAcknowledgementDropped)?
            .map_err(Into::into)
    }
}

#[derive(Deserialize)]
struct StoredToolRequest {
    version: u32,
    task_id: String,
    tool_id: String,
    tool_name: String,
    arguments: serde_json::Map<String, serde_json::Value>,
}

impl StoredToolRequest {
    fn decode(bytes: &[u8], fallback_task_id: String) -> Result<Self, ToolExecutionError> {
        if bytes.is_empty() {
            return Ok(Self {
                version: 1,
                task_id: fallback_task_id,
                tool_id: "legacy.unavailable".to_string(),
                tool_name: "legacy_unavailable".to_string(),
                arguments: serde_json::Map::new(),
            });
        }
        serde_json::from_slice(bytes).map_err(|_| ToolExecutionError::InvalidStoredRequest)
    }
}
