use std::sync::mpsc::SyncSender;
use std::sync::Arc;

use futures::channel::{mpsc, oneshot};
use futures::future::{select, Either};
use futures::{pin_mut, StreamExt as _};
use warp_multi_agent_api as api;

mod cancel;
mod commit;
mod recovery;

use commit::{assistant_text, commit_interrupted_output, commit_output, OutputCommitRequest};

use super::bridge_process::{BridgeProcessError, BridgeRunEvent};
use super::configuration::RunConfiguration;
use super::protocol::{RuntimeFailureCode, RuntimeRunOutcome, RuntimeRunStatus};
use super::supervisor::{RuntimeEntry, RuntimeError, TextRunCommand};
use super::tool_execution::{ToolExecutionAuthority, ToolRunState};
use super::transcript::RuntimeTranscript;
use crate::persistence::model::{
    AgentConversationData, AgentRuntimeRunState, AgentRuntimeTerminalOutcome,
};
use crate::persistence::{AgentRuntimeRunMutation, ModelEvent, PersistAgentRuntimeRun};
pub(super) struct TextRunRequest {
    run_id: String,
    retry_of_run_id: Option<String>,
    transcript: RuntimeTranscript,
    configuration: RunConfiguration,
    tasks: Vec<api::Task>,
    conversation_data: AgentConversationData,
    output_task_id: String,
    tool_execution_authority: Option<Arc<ToolExecutionAuthority>>,
}

impl TextRunRequest {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
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
            tool_execution_authority: None,
        }
    }

    pub(super) fn with_tool_execution_authority(
        mut self,
        authority: Arc<ToolExecutionAuthority>,
    ) -> Self {
        self.tool_execution_authority = Some(authority);
        self
    }

    pub(super) fn run_id(&self) -> &str {
        &self.run_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum TextRunOutcome {
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
pub(super) enum RuntimeEvent {
    RunStatus {
        run_id: String,
        state: AgentRuntimeRunState,
    },
    TextDelta {
        run_id: String,
        event_id: String,
        delta: String,
    },
    RunFinished {
        run_id: String,
        outcome: TextRunOutcome,
    },
}

pub(super) struct TextRunResult {
    outcome: TextRunOutcome,
    revision: u64,
    tasks: Vec<api::Task>,
}

impl TextRunResult {
    pub(super) fn outcome(&self) -> &TextRunOutcome {
        &self.outcome
    }

    pub(super) fn revision(&self) -> u64 {
        self.revision
    }

    pub(super) fn tasks(&self) -> &[api::Task] {
        &self.tasks
    }
}

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
    recovery::materialize_before_start(&entry, &mut request).await?;
    persist_run(
        persistence,
        &entry.conversation_id,
        &request.run_id,
        AgentRuntimeRunMutation::Start {
            retry_of_run_id: request.retry_of_run_id.clone(),
            starting_revision: request.transcript.revision(),
        },
    )
    .await?;

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
                process
                    .acknowledge_tool_result(
                        &entry.conversation_id,
                        &request.run_id,
                        &tool_call_id,
                        &result.projection,
                    )
                    .await?;
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

fn persistence_state(status: RuntimeRunStatus) -> AgentRuntimeRunState {
    match status {
        RuntimeRunStatus::Running => AgentRuntimeRunState::Running,
        RuntimeRunStatus::WaitingForCommit => AgentRuntimeRunState::WaitingForCommit,
        RuntimeRunStatus::WaitingForToolResult => AgentRuntimeRunState::WaitingForToolResult,
    }
}

fn text_run_outcome(outcome: RuntimeRunOutcome) -> TextRunOutcome {
    match outcome {
        RuntimeRunOutcome::Completed => TextRunOutcome::Completed,
        RuntimeRunOutcome::Cancelled => TextRunOutcome::Cancelled,
        RuntimeRunOutcome::Failed {
            error_code,
            diagnostic_id,
        } => TextRunOutcome::Failed {
            error_code,
            diagnostic_id,
        },
        RuntimeRunOutcome::LimitReached { tool_request_limit } => {
            TextRunOutcome::LimitReached { tool_request_limit }
        }
    }
}

fn terminal_outcome(outcome: &TextRunOutcome) -> AgentRuntimeTerminalOutcome {
    match outcome {
        TextRunOutcome::Completed => AgentRuntimeTerminalOutcome::Completed,
        TextRunOutcome::Cancelled => AgentRuntimeTerminalOutcome::Cancelled,
        TextRunOutcome::Failed { .. } => AgentRuntimeTerminalOutcome::Failed,
        TextRunOutcome::LimitReached { .. } => AgentRuntimeTerminalOutcome::LimitReached,
    }
}
