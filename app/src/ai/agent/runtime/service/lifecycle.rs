use std::sync::atomic::Ordering;
use std::sync::mpsc::SyncSender;

use warpui::{EntityId, ModelContext, SingletonEntity};

use super::{AgentRuntimeService, AgentRuntimeServiceEvent};
use crate::ai::agent::conversation::{AIConversationId, ConversationStatus};
use crate::ai::agent::runtime::text_run::finish_prepared_text_run;
use crate::ai::agent::runtime::RuntimeError;
use crate::ai::blocklist::{BlocklistAIHistoryModel, ResponseStreamId};
use crate::persistence::model::AgentRuntimeTerminalOutcome;
use crate::persistence::ModelEvent;

impl AgentRuntimeService {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn finish_starting_failure(
        &mut self,
        persistence: SyncSender<ModelEvent>,
        conversation_id: AIConversationId,
        run_id: String,
        response_stream_id: ResponseStreamId,
        terminal_surface_id: EntityId,
        error: RuntimeError,
        ctx: &mut ModelContext<Self>,
    ) {
        let run_id_for_result = run_id.clone();
        ctx.spawn(
            async move {
                let finish = finish_prepared_text_run(
                    &persistence,
                    &conversation_id.to_string(),
                    &run_id,
                    AgentRuntimeTerminalOutcome::Failed,
                )
                .await;
                (finish, error)
            },
            move |service, (finish, error), ctx| {
                if let Err(finish_error) = finish {
                    log::debug!("Failed to finish Agent Run after startup failure: {finish_error}");
                }
                service.handle_text_run_result(
                    conversation_id,
                    &run_id_for_result,
                    response_stream_id,
                    terminal_surface_id,
                    Err(error),
                    ctx,
                );
            },
        );
    }

    pub(super) fn finish_starting_cancellation(
        &mut self,
        persistence: SyncSender<ModelEvent>,
        conversation_id: AIConversationId,
        run_id: String,
        revision: u64,
        terminal_surface_id: EntityId,
        ctx: &mut ModelContext<Self>,
    ) {
        let run_id_for_completion = run_id.clone();
        ctx.spawn(
            async move {
                finish_prepared_text_run(
                    &persistence,
                    &conversation_id.to_string(),
                    &run_id,
                    AgentRuntimeTerminalOutcome::Cancelled,
                )
                .await
            },
            move |service, result, ctx| {
                if let Err(error) = result {
                    log::debug!("Failed to finish Agent Run cancelled during startup: {error}");
                }
                let pending = service
                    .pending_history_edits_by_run_id
                    .remove(&run_id_for_completion);
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    history_model.update_conversation_status(
                        terminal_surface_id,
                        conversation_id,
                        ConversationStatus::Cancelled,
                        ctx,
                    );
                });
                if let Some(pending) = pending {
                    service.commit_pending_history_edit(pending, revision, ctx);
                    return;
                }
                service
                    .active_run_ids_by_conversation_id
                    .remove(&conversation_id);
                service
                    .active_run_cancellations_by_conversation_id
                    .remove(&conversation_id);
                ctx.emit(AgentRuntimeServiceEvent::RunFinished { conversation_id });
            },
        );
    }

    pub(crate) fn cancel_run(
        &mut self,
        conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) {
        #[cfg(test)]
        {
            *self
                .cancel_attempts_by_conversation_id
                .entry(conversation_id)
                .or_default() += 1;
        }
        let Some(run_id) = self
            .active_run_ids_by_conversation_id
            .get(&conversation_id)
            .cloned()
        else {
            return;
        };
        if let Some(cancellation) = self
            .active_run_cancellations_by_conversation_id
            .get(&conversation_id)
        {
            cancellation.store(true, Ordering::Release);
        }
        if let Some(handle) = self
            .active_handles_by_conversation_id
            .get(&conversation_id)
            .cloned()
        {
            ctx.spawn(
                async move { handle.cancel_run().await },
                move |_, result, _| {
                    if let Err(error) = result {
                        log::warn!(
                            "Failed to cancel Agent Runtime run for conversation {conversation_id:?}: {error:#}"
                        );
                    }
                },
            );
            return;
        }
        if !self.cancelled_starting_run_ids.insert(run_id) {
            return;
        }
        let Some(supervisor) = self.supervisor.clone() else {
            return;
        };
        ctx.spawn(
            async move {
                supervisor
                    .invalidate_checkpoint(&conversation_id.to_string())
                    .await
            },
            |_, (), _| {},
        );
    }

    pub(crate) fn has_active_run(&self, conversation_id: AIConversationId) -> bool {
        self.active_run_ids_by_conversation_id
            .contains_key(&conversation_id)
    }

    pub(crate) fn invalidate_checkpoint(
        &mut self,
        conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) {
        #[cfg(test)]
        {
            *self
                .invalidate_attempts_by_conversation_id
                .entry(conversation_id)
                .or_default() += 1;
        }
        let handle = self
            .active_handles_by_conversation_id
            .remove(&conversation_id);
        let had_handle = handle.is_some();
        if let Some(run_id) = self
            .active_run_ids_by_conversation_id
            .remove(&conversation_id)
        {
            if had_handle {
                self.cancelled_starting_run_ids.remove(&run_id);
            } else {
                self.cancelled_starting_run_ids.insert(run_id);
            }
        }
        if let Some(cancellation) = self
            .active_run_cancellations_by_conversation_id
            .remove(&conversation_id)
        {
            cancellation.store(true, Ordering::Release);
        }
        let supervisor = self.supervisor.clone();
        if handle.is_none() && supervisor.is_none() {
            return;
        }
        ctx.spawn(
            async move {
                if let Some(handle) = handle {
                    if let Err(error) = handle.cancel_run().await {
                        if error != RuntimeError::NoActiveRun {
                            log::warn!(
                                "Failed to cancel Agent Run before invalidating conversation {conversation_id:?}: {error}"
                            );
                        }
                    }
                }
                if let Some(supervisor) = supervisor {
                    supervisor
                        .invalidate_checkpoint(&conversation_id.to_string())
                        .await;
                }
            },
            |_, (), _| {},
        );
    }
}
