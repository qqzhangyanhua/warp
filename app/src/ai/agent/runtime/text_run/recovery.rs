use super::{TextRunRequest, ToolRunState};
use crate::ai::agent::runtime::supervisor::{RuntimeEntry, RuntimeError};

pub(super) async fn materialize_before_start(
    entry: &RuntimeEntry,
    request: &mut TextRunRequest,
) -> Result<(), RuntimeError> {
    let Some(authority) = request.tool_execution_authority.clone() else {
        return Ok(());
    };
    let mut state = ToolRunState {
        revision: request.transcript.revision(),
        tasks: request.tasks.clone(),
        conversation_data: request.conversation_data.clone(),
        task_id: request.output_task_id.clone(),
    };
    let recovered = authority
        .recover_indeterminate(&entry.conversation_id, &mut state)
        .await?;
    request.tasks = state.tasks;
    request.conversation_data = state.conversation_data;
    request
        .transcript
        .append_recovered_tool_activity(state.revision, recovered);
    Ok(())
}
