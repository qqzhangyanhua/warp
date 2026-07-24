#[cfg(test)]
use std::collections::VecDeque;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use ai::skills::SkillPathOrigin;
use async_channel::{Receiver, Sender};
#[cfg(not(test))]
use uuid::Uuid;
use warpui::{Entity, EntityId, ModelContext, ModelHandle, SingletonEntity};
use warpui_core::r#async::executor::Background;

mod errors;
mod history;
mod launch;
mod lifecycle;
mod provider;
#[cfg(test)]
#[path = "service/provider_tests.rs"]
mod provider_tests;
mod run_result;
#[cfg(test)]
mod test_support;
mod ui_events;

pub(crate) use errors::{MissingProviderField, RuntimeStartError};
use history::PendingHistoryEdit;
use launch::runtime_launch_config;
use provider::selected_custom_provider;
pub(crate) use provider::{validate_provider_configuration, validate_provider_inventory};
use run_result::interrupted_message_ids;
use ui_events::{add_message_action, append_message_action, RuntimeUiEvent, StreamedMessageKey};

use super::configuration::{ReasoningEffort, RunConfiguration};
use super::resources::ResourceSnapshotBuilder;
use super::text_run::{prepare_text_run, TextRunRequest};
use super::tool_catalog::ToolCatalog;
use super::tool_execution::blocklist_adapter::BlocklistRuntimeToolActionAdapter;
use super::tool_execution::ToolExecutionAuthority;
use super::transcript::RuntimeTranscript;
use super::{AgentRuntimeHandle, AgentRuntimeSupervisor, RuntimeError};
use crate::ai::agent::api::RequestParams;
use crate::ai::agent::conversation::{AIConversation, AIConversationId, ConversationStatus};
use crate::ai::agent::EntrypointType;
use crate::ai::blocklist::{BlocklistAIActionModel, BlocklistAIHistoryModel, ResponseStreamId};
use crate::persistence::ModelEvent;
use crate::GlobalResourceHandlesProvider;

const DEFAULT_CONTEXT_LIMIT: u64 = 200_000;

#[derive(Clone, Debug)]
pub(crate) enum AgentRuntimeServiceEvent {
    RunFinished { conversation_id: AIConversationId },
}

pub(crate) struct AgentRuntimeService {
    supervisor: Option<AgentRuntimeSupervisor>,
    active_handles_by_conversation_id: HashMap<AIConversationId, Arc<AgentRuntimeHandle>>,
    active_run_ids_by_conversation_id: HashMap<AIConversationId, String>,
    active_run_cancellations_by_conversation_id: HashMap<AIConversationId, Arc<AtomicBool>>,
    cancelled_starting_run_ids: HashSet<String>,
    pending_history_edits_by_run_id: HashMap<String, PendingHistoryEdit>,
    history_commit_barrier_run_ids: HashSet<String>,
    last_run_ids_by_conversation_id: HashMap<AIConversationId, String>,
    streamed_message_ids: HashSet<StreamedMessageKey>,
    runtime_events: Sender<RuntimeUiEvent>,
    runtime_event_receiver: Option<Receiver<RuntimeUiEvent>>,
    #[cfg(test)]
    start_result_for_test: Option<Result<(), RuntimeStartError>>,
    #[cfg(test)]
    start_attempts_by_conversation_id: HashMap<AIConversationId, usize>,
    #[cfg(test)]
    cancel_attempts_by_conversation_id: HashMap<AIConversationId, usize>,
    #[cfg(test)]
    invalidate_attempts_by_conversation_id: HashMap<AIConversationId, usize>,
    #[cfg(test)]
    restore_attempts_by_conversation_id: HashMap<AIConversationId, usize>,
    #[cfg(test)]
    run_ids_for_test: VecDeque<String>,
}

impl AgentRuntimeService {
    pub(crate) fn new() -> Self {
        Self::new_with_supervisor(None)
    }

    pub(crate) fn new_for_app(executor: Arc<Background>) -> Self {
        let supervisor = runtime_launch_config().map(|launch_config| {
            let supervisor = AgentRuntimeSupervisor::new(launch_config, executor);
            supervisor.start_idle_eviction();
            supervisor
        });
        Self::new_with_supervisor(supervisor)
    }

    pub(crate) fn new_with_supervisor(supervisor: Option<AgentRuntimeSupervisor>) -> Self {
        let (runtime_events, runtime_event_receiver) = async_channel::unbounded();
        Self {
            supervisor,
            active_handles_by_conversation_id: HashMap::new(),
            active_run_ids_by_conversation_id: HashMap::new(),
            active_run_cancellations_by_conversation_id: HashMap::new(),
            cancelled_starting_run_ids: HashSet::new(),
            pending_history_edits_by_run_id: HashMap::new(),
            history_commit_barrier_run_ids: HashSet::new(),
            last_run_ids_by_conversation_id: HashMap::new(),
            streamed_message_ids: HashSet::new(),
            runtime_events,
            runtime_event_receiver: Some(runtime_event_receiver),
            #[cfg(test)]
            start_result_for_test: None,
            #[cfg(test)]
            start_attempts_by_conversation_id: HashMap::new(),
            #[cfg(test)]
            cancel_attempts_by_conversation_id: HashMap::new(),
            #[cfg(test)]
            invalidate_attempts_by_conversation_id: HashMap::new(),
            #[cfg(test)]
            restore_attempts_by_conversation_id: HashMap::new(),
            #[cfg(test)]
            run_ids_for_test: VecDeque::new(),
        }
    }

    pub(crate) fn start_text_run(
        &mut self,
        conversation: &AIConversation,
        request_params: RequestParams,
        response_stream_id: ResponseStreamId,
        output_task_id: String,
        resource_message_ids: HashSet<String>,
        bootstrap_runtime_record: bool,
        terminal_surface_id: EntityId,
        action_model: ModelHandle<BlocklistAIActionModel>,
        ctx: &mut ModelContext<Self>,
    ) -> Result<(), RuntimeStartError> {
        let conversation_id = conversation.id();
        #[cfg(test)]
        {
            *self
                .start_attempts_by_conversation_id
                .entry(conversation_id)
                .or_default() += 1;

            if let Some(result) = &self.start_result_for_test {
                return result.clone();
            }
        }

        self.ensure_runtime_event_stream(ctx);

        self.ensure_conversation_idle(conversation_id)?;

        let supervisor = self.supervisor.clone();
        let persistence = GlobalResourceHandlesProvider::as_ref(ctx)
            .get()
            .model_event_sender
            .clone()
            .ok_or(RuntimeStartError::MissingPersistence)?;
        let provider = selected_custom_provider(&request_params)
            .map_err(RuntimeStartError::MissingProvider)?;
        let tool_catalog = ToolCatalog::initial(request_params.mcp_context.as_ref())
            .map_err(|_| RuntimeStartError::InvalidRunConfiguration)?;
        let resources = ResourceSnapshotBuilder::default()
            .build(
                conversation
                    .all_linearized_messages()
                    .into_iter()
                    .filter(|message| resource_message_ids.contains(&message.id)),
            )
            .map_err(|_| RuntimeStartError::InvalidRunConfiguration)?;
        let configuration = RunConfiguration::with_tools(
            provider,
            request_params
                .session_context
                .current_working_directory()
                .clone()
                .unwrap_or_else(|| ".".to_string()),
            u64::from(request_params.context_window_limit.unwrap_or(0)).max(DEFAULT_CONTEXT_LIMIT),
            ReasoningEffort::None,
            &tool_catalog,
            resources,
        )
        .map_err(|_| RuntimeStartError::InvalidRunConfiguration)?;
        let (tasks, conversation_data) = conversation.runtime_persistence_snapshot();
        if bootstrap_runtime_record
            && persistence
                .send(ModelEvent::UpdateMultiAgentConversation {
                    conversation_id: conversation_id.to_string(),
                    updated_tasks: Vec::new(),
                    conversation_data: conversation_data.clone(),
                })
                .is_err()
        {
            return Err(RuntimeStartError::MissingPersistence);
        }
        let transcript = RuntimeTranscript::project(
            conversation,
            conversation.runtime_transcript_revision(),
            &interrupted_message_ids(conversation),
            &HashMap::new(),
        )
        .map_err(|_| RuntimeStartError::TranscriptProjectionFailed)?;
        let run_id = self.next_run_id();
        let is_explicit_retry = request_params.metadata.as_ref().is_some_and(|metadata| {
            metadata.entrypoint == EntrypointType::ResumeConversation
                && !metadata.is_auto_resume_after_error
        });
        let retry_of_run_id = is_explicit_retry
            .then(|| {
                self.last_run_ids_by_conversation_id
                    .get(&conversation_id)
                    .cloned()
            })
            .flatten();
        let runtime_events = self.runtime_events.clone();
        let action_adapter =
            BlocklistRuntimeToolActionAdapter::new(action_model, conversation_id, ctx);
        let authority = Arc::new(ToolExecutionAuthority::new(
            tool_catalog,
            Arc::new(action_adapter),
            persistence.clone(),
        ));
        self.active_run_ids_by_conversation_id
            .insert(conversation_id, run_id.clone());
        let cancellation = Arc::new(AtomicBool::new(false));
        self.active_run_cancellations_by_conversation_id
            .insert(conversation_id, cancellation.clone());

        let mut request = TextRunRequest::new(
            run_id.clone(),
            retry_of_run_id,
            transcript,
            configuration,
            tasks,
            conversation_data,
            output_task_id.clone(),
        )
        .with_tool_execution_authority(authority);
        if is_explicit_retry {
            request = request.with_retry_lineage_lookup();
        }
        if !resource_message_ids.is_empty() {
            request = request.with_initial_input_commit(format!("input:{run_id}"));
        }

        ctx.spawn(
            async move {
                let result =
                    prepare_text_run(&persistence, &conversation_id.to_string(), &mut request)
                        .await;
                (result, request, persistence)
            },
            move |service, (result, request, persistence), ctx| {
                if service.cancelled_starting_run_ids.remove(&run_id) {
                    let revision = request.revision();
                    service.finish_starting_cancellation(
                        persistence,
                        conversation_id,
                        run_id,
                        revision,
                        terminal_surface_id,
                        ctx,
                    );
                    return;
                }
                if let Err(error) = result {
                    service.finish_starting_failure(
                        persistence,
                        conversation_id,
                        run_id,
                        response_stream_id,
                        terminal_surface_id,
                        error,
                        ctx,
                    );
                    return;
                }
                service
                    .last_run_ids_by_conversation_id
                    .insert(conversation_id, run_id.clone());
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, _| {
                    if let Some(conversation) = history_model.conversation_mut(&conversation_id) {
                        conversation.set_runtime_transcript_revision(request.revision());
                    }
                });
                let Some(supervisor) = supervisor else {
                    service.finish_starting_failure(
                        persistence,
                        conversation_id,
                        run_id,
                        response_stream_id,
                        terminal_surface_id,
                        RuntimeError::BridgeUnavailable,
                        ctx,
                    );
                    return;
                };
                ctx.spawn(
                    async move {
                        let result = supervisor.attach(conversation_id.to_string()).await;
                        (result, request, persistence)
                    },
                    move |service, (result, request, persistence), ctx| {
                        if service.cancelled_starting_run_ids.remove(&run_id) {
                            let revision = request.revision();
                            service.finish_starting_cancellation(
                                persistence,
                                conversation_id,
                                run_id,
                                revision,
                                terminal_surface_id,
                                ctx,
                            );
                            return;
                        }
                        let handle = match result {
                            Ok(handle) => Arc::new(handle),
                            Err(error) => {
                                service.finish_starting_failure(
                                    persistence,
                                    conversation_id,
                                    run_id,
                                    response_stream_id,
                                    terminal_surface_id,
                                    error,
                                    ctx,
                                );
                                return;
                            }
                        };
                        service
                            .active_handles_by_conversation_id
                            .insert(conversation_id, handle.clone());
                        let response_stream_id_for_run = response_stream_id.clone();
                        let run_id_for_result = run_id.clone();
                        ctx.spawn(
                            async move {
                                handle
                                    .run_text_cancellable(
                                        &persistence,
                                        request,
                                        cancellation,
                                        |event| {
                                            let _ = runtime_events.try_send(
                                                RuntimeUiEvent::from_runtime_event(
                                                    conversation_id,
                                                    response_stream_id_for_run.clone(),
                                                    output_task_id.clone(),
                                                    terminal_surface_id,
                                                    event,
                                                ),
                                            );
                                        },
                                    )
                                    .await
                            },
                            move |service, result, ctx| {
                                service.handle_text_run_result(
                                    conversation_id,
                                    &run_id_for_result,
                                    response_stream_id,
                                    terminal_surface_id,
                                    result,
                                    ctx,
                                );
                            },
                        );
                    },
                );
            },
        );

        Ok(())
    }

    fn ensure_runtime_event_stream(&mut self, ctx: &mut ModelContext<Self>) {
        let Some(receiver) = self.runtime_event_receiver.take() else {
            return;
        };
        ctx.spawn_stream_local(
            receiver,
            |service, event, ctx| service.handle_runtime_event(event, ctx),
            |_, _| {},
        );
    }

    fn ensure_conversation_idle(
        &self,
        conversation_id: AIConversationId,
    ) -> Result<(), RuntimeStartError> {
        if self.has_active_run(conversation_id) {
            Err(RuntimeStartError::RunAlreadyActive)
        } else {
            Ok(())
        }
    }

    fn handle_runtime_event(&mut self, event: RuntimeUiEvent, ctx: &mut ModelContext<Self>) {
        match event {
            RuntimeUiEvent::RunStatus {
                conversation_id,
                terminal_surface_id,
                run_id,
            } => {
                if !self.accepts_runtime_event(conversation_id, &run_id) {
                    return;
                }
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    history_model.update_conversation_status(
                        terminal_surface_id,
                        conversation_id,
                        ConversationStatus::InProgress,
                        ctx,
                    );
                });
            }
            RuntimeUiEvent::TextDelta {
                conversation_id,
                response_stream_id,
                output_task_id,
                terminal_surface_id,
                run_id,
                event_id,
                delta,
            } => {
                if !self.accepts_runtime_event(conversation_id, &run_id) {
                    return;
                }
                let key = StreamedMessageKey {
                    conversation_id,
                    run_id: run_id.clone(),
                    event_id: event_id.clone(),
                };
                let action = if self.streamed_message_ids.insert(key) {
                    add_message_action(&output_task_id, &event_id, &run_id, delta)
                } else {
                    append_message_action(&output_task_id, &event_id, &run_id, delta)
                };
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    if let Err(error) = history_model.apply_client_actions(
                        &response_stream_id,
                        vec![action],
                        conversation_id,
                        terminal_surface_id,
                        &SkillPathOrigin::Unavailable,
                        ctx,
                    ) {
                        log::debug!("Failed to apply Agent Runtime UI event: {error:#}");
                    }
                });
            }
            RuntimeUiEvent::ConversationCommit {
                conversation_id,
                response_stream_id,
                terminal_surface_id,
                run_id,
                revision,
                tasks,
                conversation_data,
            } => {
                if !self.accepts_runtime_event(conversation_id, &run_id) {
                    return;
                }
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    history_model.commit_runtime_text_run_progress(
                        conversation_id,
                        &response_stream_id,
                        terminal_surface_id,
                        conversation_data,
                        tasks,
                        revision,
                        ctx,
                    );
                });
            }
            RuntimeUiEvent::RunFinished { .. } => {}
        }
    }

    fn accepts_runtime_event(&self, conversation_id: AIConversationId, run_id: &str) -> bool {
        self.active_run_ids_by_conversation_id
            .get(&conversation_id)
            .is_some_and(|active_run_id| active_run_id == run_id)
            && !self.pending_history_edits_by_run_id.contains_key(run_id)
            && !self.history_commit_barrier_run_ids.contains(run_id)
    }

    #[cfg(not(test))]
    fn next_run_id(&mut self) -> String {
        Uuid::new_v4().to_string()
    }
}

impl Entity for AgentRuntimeService {
    type Event = AgentRuntimeServiceEvent;
}

impl SingletonEntity for AgentRuntimeService {}
