use std::sync::mpsc::SyncSender;

use futures::channel::oneshot;

use super::{RuntimeEvent, TextRunOutcome, TextRunRequest, TextRunResult};
use crate::ai::agent::runtime::bridge_process::BridgeProcessError;
use crate::ai::agent::runtime::supervisor::RuntimeError;
use crate::persistence::model::AgentRuntimeTerminalOutcome;
use crate::persistence::{AgentRuntimeRunMutation, ModelEvent};

pub(super) async fn finish<F>(
    cancellation: Result<(), BridgeProcessError>,
    acknowledgement: oneshot::Sender<Result<u64, BridgeProcessError>>,
    persistence: &SyncSender<ModelEvent>,
    conversation_id: &str,
    request: TextRunRequest,
    revision: u64,
    on_event: &mut F,
) -> Result<TextRunResult, RuntimeError>
where
    F: FnMut(RuntimeEvent),
{
    if let Err(error) = cancellation {
        let _ = acknowledgement.send(Err(error));
        return Err(error.into());
    }
    let outcome = TextRunOutcome::Cancelled;
    super::persist_run(
        persistence,
        conversation_id,
        &request.run_id,
        AgentRuntimeRunMutation::Finish(AgentRuntimeTerminalOutcome::Cancelled),
    )
    .await?;
    on_event(RuntimeEvent::RunFinished {
        run_id: request.run_id.clone(),
        outcome: outcome.clone(),
    });
    let _ = acknowledgement.send(Ok(revision));
    Ok(TextRunResult {
        outcome,
        revision,
        tasks: request.tasks,
        conversation_data: request.conversation_data,
    })
}
