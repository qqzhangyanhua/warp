use super::{TextRunRequest, ToolRunState};
use crate::ai::agent::runtime::supervisor::RuntimeError;

pub(super) async fn materialize_before_start(
    conversation_id: &str,
    request: &mut TextRunRequest,
) -> Result<(), RuntimeError> {
    let Some(authority) = request.tool_execution_authority.clone() else {
        return Ok(());
    };
    let has_unfinished = authority.has_unfinished(conversation_id).await?;
    if !has_unfinished {
        return Ok(());
    }
    if request.retry_of_run_id.is_none() {
        return Err(RuntimeError::RetryRequired);
    }
    let mut state = ToolRunState {
        revision: request.transcript.revision(),
        tasks: request.tasks.clone(),
        conversation_data: request.conversation_data.clone(),
        task_id: request.output_task_id.clone(),
    };
    let recovered = authority
        .recover_unfinished(conversation_id, &mut state)
        .await?;
    request.tasks = state.tasks;
    request.conversation_data = state.conversation_data;
    request
        .transcript
        .append_recovered_tool_activity(state.revision, recovered);
    Ok(())
}
