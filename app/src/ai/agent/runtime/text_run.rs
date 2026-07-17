use std::sync::mpsc::SyncSender;
use std::sync::Arc;

use futures::channel::{mpsc, oneshot};
use futures::future::{select, Either};
use futures::{pin_mut, StreamExt as _};

mod cancel;
mod commit;
mod outcome;
mod recovery;
mod types;

use commit::{
    assistant_text, commit_initial_input, commit_interrupted_output, commit_output,
    OutputCommitRequest,
};
use outcome::{
    persistence_state, terminal_outcome, terminal_outcome_for_tool_result, text_run_outcome,
};
pub(super) use types::{RuntimeEvent, TextRunOutcome, TextRunRequest, TextRunResult};

use super::bridge_process::{BridgeProcessError, BridgeRunEvent};
use super::supervisor::{RuntimeEntry, RuntimeError, TextRunCommand};
use super::tool_execution::{ToolExecutionResult, ToolRunState};
use crate::persistence::model::AgentRuntimeTerminalOutcome;
use crate::persistence::{
    AgentRuntimeRunMutation, ModelEvent, PersistAgentRuntimeRun, ReadLatestAgentRuntimeRunId,
};

pub(super) async fn execute<F>(
    entry: Arc<RuntimeEntry>,
    persistence: &SyncSender<ModelEvent>,
    mut request: TextRunRequest,
    commands: mpsc::UnboundedReceiver<TextRunCommand>,
    on_event: F,
) -> Result<TextRunResult, RuntimeError>
where
    F: FnMut(RuntimeEvent),
{
    if !request.prepared {
        prepare_text_run(persistence, &entry.conversation_id, &mut request).await?;
    }

    let run_id = request.run_id.clone();
    let conversation_id = entry.conversation_id.clone();
    let result = execute_started(entry, persistence, request, commands, on_event).await;
    if result.is_err() {
        let _ = persist_run(
            persistence,
            &conversation_id,
            &run_id,
            AgentRuntimeRunMutation::Finish(AgentRuntimeTerminalOutcome::Failed),
        )
        .await;
    }
    result
}

pub(super) async fn prepare_text_run(
    persistence: &SyncSender<ModelEvent>,
    conversation_id: &str,
    request: &mut TextRunRequest,
) -> Result<(), RuntimeError> {
    if request.resolve_retry_lineage {
        request.retry_of_run_id = read_latest_run_id(persistence, conversation_id).await?;
    }
    recovery::materialize_before_start(conversation_id, request).await?;
    persist_run(
        persistence,
        conversation_id,
        &request.run_id,
        AgentRuntimeRunMutation::Start {
            retry_of_run_id: request.retry_of_run_id.clone(),
            starting_revision: request.transcript.revision(),
        },
    )
    .await?;
    if let Some(commit_id) = request.initial_input_commit_id.take() {
        let revision = commit_initial_input(
            persistence,
            conversation_id,
            &request.run_id,
            &commit_id,
            request.transcript.revision(),
            &request.tasks,
            &request.conversation_data,
        )
        .await?;
        request.transcript.set_revision(revision);
        request.conversation_data.runtime_transcript_revision = Some(revision);
    }
    request.prepared = true;
    Ok(())
}

async fn read_latest_run_id(
    persistence: &SyncSender<ModelEvent>,
    conversation_id: &str,
) -> Result<Option<String>, RuntimeError> {
    let (acknowledgement, acknowledged) = oneshot::channel();
    persistence
        .send(ModelEvent::ReadLatestAgentRuntimeRunId(
            ReadLatestAgentRuntimeRunId {
                conversation_id: conversation_id.to_string(),
                acknowledgement,
            },
        ))
        .map_err(|_| RuntimeError::PersistenceUnavailable)?;
    acknowledged
        .await
        .map_err(|_| RuntimeError::PersistenceAcknowledgementDropped)?
        .map_err(RuntimeError::from)
}

pub(super) async fn finish_prepared_text_run(
    persistence: &SyncSender<ModelEvent>,
    conversation_id: &str,
    run_id: &str,
    outcome: AgentRuntimeTerminalOutcome,
) -> Result<(), RuntimeError> {
    persist_run(
        persistence,
        conversation_id,
        run_id,
        AgentRuntimeRunMutation::Finish(outcome),
    )
    .await
}

pub(super) async fn cancel_prepared_text_run(
    persistence: &SyncSender<ModelEvent>,
    request: TextRunRequest,
) -> Result<TextRunResult, RuntimeError> {
    finish_prepared_text_run(
        persistence,
        request.transcript.conversation_id(),
        &request.run_id,
        AgentRuntimeTerminalOutcome::Cancelled,
    )
    .await?;
    Ok(TextRunResult {
        outcome: TextRunOutcome::Cancelled,
        revision: request.transcript.revision(),
        tasks: request.tasks,
        conversation_data: request.conversation_data,
    })
}

async fn execute_started<F>(
    entry: Arc<RuntimeEntry>,
    persistence: &SyncSender<ModelEvent>,
    mut request: TextRunRequest,
    mut commands: mpsc::UnboundedReceiver<TextRunCommand>,
    mut on_event: F,
) -> Result<TextRunResult, RuntimeError>
where
    F: FnMut(RuntimeEvent),
{
    if request.transcript.conversation_id() != entry.conversation_id {
        return Err(BridgeProcessError::ProtocolViolation.into());
    }
    let mut process = entry.process.lock().await;
    let process = process.as_mut().ok_or(BridgeProcessError::UnexpectedExit)?;
    let start_input = {
        let start =
            process.start_text_run(&request.transcript, &request.run_id, &request.configuration);
        let command = commands.next();
        pin_mut!(start, command);
        match select(command, start).await {
            Either::Left((command, _)) => StartRunInput::Command(command),
            Either::Right((started, _)) => StartRunInput::Started(started),
        }
    };
    match start_input {
        StartRunInput::Started(started) => started?,
        StartRunInput::Command(Some(TextRunCommand::Cancel {
            grace_period,
            acknowledgement,
        })) => {
            let cancellation = process
                .cancel_run(&entry.conversation_id, &request.run_id, grace_period)
                .await;
            let revision = request.transcript.revision();
            return cancel::finish(
                cancellation,
                acknowledgement,
                persistence,
                &entry.conversation_id,
                request,
                revision,
                &mut on_event,
            )
            .await;
        }
        StartRunInput::Command(None) => {
            return Err(BridgeProcessError::UnexpectedExit.into());
        }
    }

    let mut revision = request.transcript.revision();
    let mut partial_text = String::new();
    let mut partial_event_id: Option<String> = None;
    loop {
        let next = {
            let event = process.read_run_event(&entry.conversation_id, &request.run_id);
            let command = commands.next();
            pin_mut!(event, command);
            match select(command, event).await {
                Either::Left((command, _)) => NextRunInput::Command(command),
                Either::Right((event, _)) => NextRunInput::Event(event),
            }
        };
        let event = match next {
            NextRunInput::Event(Ok(event)) => event,
            NextRunInput::Event(Err(error)) => {
                commit_interrupted_output(
                    persistence,
                    &entry.conversation_id,
                    &mut request,
                    &mut revision,
                    &mut partial_text,
                    &mut partial_event_id,
                )
                .await?;
                return Err(error.into());
            }
            NextRunInput::Command(Some(TextRunCommand::Cancel {
                grace_period,
                acknowledgement,
            })) => {
                let cancellation = process
                    .cancel_run(&entry.conversation_id, &request.run_id, grace_period)
                    .await;
                commit_interrupted_output(
                    persistence,
                    &entry.conversation_id,
                    &mut request,
                    &mut revision,
                    &mut partial_text,
                    &mut partial_event_id,
                )
                .await?;
                return cancel::finish(
                    cancellation,
                    acknowledgement,
                    persistence,
                    &entry.conversation_id,
                    request,
                    revision,
                    &mut on_event,
                )
                .await;
            }
            NextRunInput::Command(None) => {
                return Err(BridgeProcessError::UnexpectedExit.into());
            }
        };
        match event {
            BridgeRunEvent::Status(status) => {
                let state = persistence_state(status);
                persist_run(
                    persistence,
                    &entry.conversation_id,
                    &request.run_id,
                    AgentRuntimeRunMutation::SetState(state),
                )
                .await?;
                on_event(RuntimeEvent::RunStatus {
                    run_id: request.run_id.clone(),
                    state,
                });
            }
            BridgeRunEvent::TextDelta(delta) => {
                if let Some(event_id) = &partial_event_id {
                    if event_id != &delta.event_id {
                        return Err(BridgeProcessError::ProtocolViolation.into());
                    }
                } else {
                    partial_event_id = Some(delta.event_id.clone());
                }
                partial_text.push_str(&delta.delta);
                on_event(RuntimeEvent::TextDelta {
                    run_id: request.run_id.clone(),
                    event_id: delta.event_id,
                    delta: delta.delta,
                });
            }
            BridgeRunEvent::AssistantMessageCommit(commit) => {
                if commit.expected_revision != revision
                    || partial_event_id
                        .as_deref()
                        .is_some_and(|event_id| event_id != commit.event_id)
                {
                    return Err(BridgeProcessError::ProtocolViolation.into());
                }
                let text = assistant_text(&commit)?;
                let committed = commit_output(
                    persistence,
                    OutputCommitRequest {
                        conversation_id: &entry.conversation_id,
                        run_id: &request.run_id,
                        output_task_id: &request.output_task_id,
                        commit_id: &commit.commit_id,
                        message_id: &commit.message_id,
                        expected_revision: revision,
                        text,
                        tasks: &request.tasks,
                        conversation_data: &request.conversation_data,
                    },
                )
                .await?;
                request.tasks = committed.tasks;
                request.conversation_data = committed.conversation_data;
                revision = committed.revision;
                on_event(RuntimeEvent::ConversationCommit {
                    run_id: request.run_id.clone(),
                    revision,
                    tasks: request.tasks.clone(),
                    conversation_data: request.conversation_data.clone(),
                });
                partial_text.clear();
                partial_event_id = None;
                process
                    .acknowledge_commit(
                        &entry.conversation_id,
                        &request.run_id,
                        &commit.commit_id,
                        revision,
                    )
                    .await?;
            }
            BridgeRunEvent::ToolRequest(tool_request) => {
                if !partial_text.is_empty() || partial_event_id.is_some() {
                    return Err(BridgeProcessError::ProtocolViolation.into());
                }
                let authority = request
                    .tool_execution_authority
                    .as_ref()
                    .ok_or(BridgeProcessError::ProtocolViolation)?
                    .clone();
                let mut state = ToolRunState {
                    revision,
                    tasks: request.tasks.clone(),
                    conversation_data: request.conversation_data.clone(),
                    task_id: request.output_task_id.clone(),
                };
                let tool_call_id = tool_request.tool_call_id.clone();
                let result = {
                    let execution = authority.handle(tool_request, &mut state);
                    let command = commands.next();
                    pin_mut!(execution, command);
                    match select(command, execution).await {
                        Either::Left((
                            Some(TextRunCommand::Cancel {
                                grace_period,
                                acknowledgement,
                            }),
                            execution,
                        )) => {
                            drop(execution);
                            authority.cancel_run(request.run_id.clone()).await;
                            let cancellation = process
                                .cancel_run(&entry.conversation_id, &request.run_id, grace_period)
                                .await;
                            return cancel::finish(
                                cancellation,
                                acknowledgement,
                                persistence,
                                &entry.conversation_id,
                                request,
                                revision,
                                &mut on_event,
                            )
                            .await;
                        }
                        Either::Left((None, execution)) => {
                            drop(execution);
                            return Err(BridgeProcessError::UnexpectedExit.into());
                        }
                        Either::Right((result, _)) => result?,
                    }
                };
                revision = state.revision;
                request.tasks = state.tasks;
                request.conversation_data = state.conversation_data;
                on_event(RuntimeEvent::ConversationCommit {
                    run_id: request.run_id.clone(),
                    revision,
                    tasks: request.tasks.clone(),
                    conversation_data: request.conversation_data.clone(),
                });
                process
                    .acknowledge_tool_result(
                        &entry.conversation_id,
                        &request.run_id,
                        &tool_call_id,
                        &result.projection_bytes,
                    )
                    .await?;
                if result.run_must_end {
                    let outcome = terminal_outcome_for_tool_result(&result);
                    persist_run(
                        persistence,
                        &entry.conversation_id,
                        &request.run_id,
                        AgentRuntimeRunMutation::Finish(terminal_outcome(&outcome)),
                    )
                    .await?;
                    on_event(RuntimeEvent::RunFinished {
                        run_id: request.run_id.clone(),
                        outcome: outcome.clone(),
                    });
                    return Ok(TextRunResult {
                        outcome,
                        revision,
                        tasks: request.tasks,
                        conversation_data: request.conversation_data,
                    });
                }
            }
            BridgeRunEvent::Finished(finished) => {
                let outcome = text_run_outcome(finished.outcome);
                if outcome != TextRunOutcome::Completed {
                    commit_interrupted_output(
                        persistence,
                        &entry.conversation_id,
                        &mut request,
                        &mut revision,
                        &mut partial_text,
                        &mut partial_event_id,
                    )
                    .await?;
                }
                persist_run(
                    persistence,
                    &entry.conversation_id,
                    &request.run_id,
                    AgentRuntimeRunMutation::Finish(terminal_outcome(&outcome)),
                )
                .await?;
                on_event(RuntimeEvent::RunFinished {
                    run_id: request.run_id.clone(),
                    outcome: outcome.clone(),
                });
                return Ok(TextRunResult {
                    outcome,
                    revision,
                    tasks: request.tasks,
                    conversation_data: request.conversation_data,
                });
            }
        }
    }
}

enum NextRunInput {
    Event(Result<BridgeRunEvent, BridgeProcessError>),
    Command(Option<TextRunCommand>),
}

enum StartRunInput {
    Started(Result<(), BridgeProcessError>),
    Command(Option<TextRunCommand>),
}

async fn persist_run(
    persistence: &SyncSender<ModelEvent>,
    conversation_id: &str,
    run_id: &str,
    mutation: AgentRuntimeRunMutation,
) -> Result<(), RuntimeError> {
    let (acknowledgement, acknowledged) = oneshot::channel();
    persistence
        .send(ModelEvent::PersistAgentRuntimeRun(PersistAgentRuntimeRun {
            conversation_id: conversation_id.to_string(),
            run_id: run_id.to_string(),
            mutation,
            acknowledgement,
        }))
        .map_err(|_| RuntimeError::PersistenceUnavailable)?;
    acknowledged
        .await
        .map_err(|_| RuntimeError::PersistenceAcknowledgementDropped)??;
    Ok(())
}

#[cfg(test)]
#[path = "text_run_tests.rs"]
mod tests;
