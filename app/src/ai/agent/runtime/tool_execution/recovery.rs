use futures::channel::oneshot;
use serde::Deserialize;

use super::{
    unknown_outcome_projection, CompletionState, ToolExecutionAuthority, ToolExecutionError,
    ToolRunState,
};
use crate::ai::agent::runtime::protocol::RuntimeToolRequest;
use crate::ai::agent::runtime::transcript::TranscriptItem;
use crate::persistence::{ModelEvent, ReadExecutingAgentToolExecutions};

impl ToolExecutionAuthority {
    pub(in crate::ai::agent::runtime) async fn recover_indeterminate(
        &self,
        conversation_id: &str,
        state: &mut ToolRunState,
    ) -> Result<Vec<TranscriptItem>, ToolExecutionError> {
        let (acknowledgement, acknowledged) = oneshot::channel();
        self.persistence
            .send(ModelEvent::ReadExecutingAgentToolExecutions(
                ReadExecutingAgentToolExecutions {
                    conversation_id: conversation_id.to_string(),
                    acknowledgement,
                },
            ))
            .map_err(|_| ToolExecutionError::PersistenceUnavailable)?;
        let executing = acknowledged
            .await
            .map_err(|_| ToolExecutionError::PersistenceAcknowledgementDropped)??;
        let mut transcript_items = Vec::with_capacity(executing.len() * 2);
        for record in executing {
            let stored: StoredToolRequest = serde_json::from_slice(record.request_payload.bytes())
                .map_err(|_| ToolExecutionError::InvalidStoredRequest)?;
            if stored.version != 1 {
                return Err(ToolExecutionError::InvalidStoredRequest);
            }
            let request = RuntimeToolRequest {
                conversation_id: conversation_id.to_string(),
                run_id: record.run_id,
                tool_call_id: record.tool_call_id,
                tool_id: stored.tool_id,
                tool_name: stored.tool_name,
                arguments: stored.arguments,
            };
            let projection = unknown_outcome_projection();
            let original_task_id = std::mem::replace(&mut state.task_id, stored.task_id);
            let completion = self
                .complete(
                    &request,
                    state,
                    self.resolve_tool(&request),
                    None,
                    projection.clone(),
                    serde_json::to_vec(&projection)?,
                    CompletionState::Executing,
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
}

#[derive(Deserialize)]
struct StoredToolRequest {
    version: u32,
    task_id: String,
    tool_id: String,
    tool_name: String,
    arguments: serde_json::Map<String, serde_json::Value>,
}
