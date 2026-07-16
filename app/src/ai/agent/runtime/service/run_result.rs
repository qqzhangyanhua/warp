use std::collections::HashSet;

use warpui::{EntityId, ModelContext, SingletonEntity};

use super::{AgentRuntimeService, AgentRuntimeServiceEvent};
use crate::ai::agent::conversation::{AIConversation, AIConversationId};
use crate::ai::agent::runtime::text_run::{TextRunOutcome, TextRunResult};
use crate::ai::agent::runtime::RuntimeError;
use crate::ai::agent::RenderableAIError;
use crate::ai::blocklist::{BlocklistAIHistoryModel, ResponseStreamId, RuntimeTextRunFinish};

impl AgentRuntimeService {
    pub(super) fn handle_text_run_result(
        &mut self,
        conversation_id: AIConversationId,
        run_id: &str,
        response_stream_id: ResponseStreamId,
        terminal_surface_id: EntityId,
        result: Result<TextRunResult, RuntimeError>,
        ctx: &mut ModelContext<Self>,
    ) {
        if self
            .active_run_ids_by_conversation_id
            .get(&conversation_id)
            .is_none_or(|active_run_id| active_run_id != run_id)
        {
            return;
        }
        if self.history_commit_barrier_run_ids.contains(run_id) {
            return;
        }
        if let Some(pending) = self.pending_history_edits_by_run_id.remove(run_id) {
            let revision = result
                .as_ref()
                .ok()
                .map(TextRunResult::revision)
                .unwrap_or_else(|| pending.expected_revision());
            self.commit_pending_history_edit(pending, revision, ctx);
            return;
        }
        self.active_run_ids_by_conversation_id
            .remove(&conversation_id);
        self.active_run_cancellations_by_conversation_id
            .remove(&conversation_id);
        self.cancelled_starting_run_ids.remove(run_id);
        match result {
            Ok(result) => {
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    history_model.finish_runtime_text_run(
                        conversation_id,
                        terminal_surface_id,
                        result.conversation_data().clone(),
                        result.tasks().to_vec(),
                        result.revision(),
                        text_run_finish(result.outcome()),
                        ctx,
                    );
                });
            }
            Err(error) => {
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    history_model.mark_response_stream_completed_with_error(
                        RenderableAIError::agent_runtime_unavailable(error.to_string()),
                        false,
                        &response_stream_id,
                        conversation_id,
                        terminal_surface_id,
                        ctx,
                    );
                });
            }
        }
        ctx.emit(AgentRuntimeServiceEvent::RunFinished { conversation_id });
    }
}

pub(super) fn interrupted_message_ids(conversation: &AIConversation) -> HashSet<String> {
    conversation
        .all_tasks()
        .flat_map(|task| task.messages())
        .filter(|message| message.id.starts_with("interrupted:"))
        .map(|message| message.id.clone())
        .collect()
}

fn text_run_finish(outcome: &TextRunOutcome) -> RuntimeTextRunFinish {
    match outcome {
        TextRunOutcome::Completed => RuntimeTextRunFinish::Success,
        TextRunOutcome::Cancelled => RuntimeTextRunFinish::Cancelled,
        TextRunOutcome::Failed { diagnostic_id, .. } => RuntimeTextRunFinish::Error(
            RenderableAIError::other(format!("Pi Agent Runtime failed: {diagnostic_id}"), false),
        ),
        TextRunOutcome::LimitReached { tool_request_limit } => {
            RuntimeTextRunFinish::Error(RenderableAIError::other(
                format!("Pi Agent Runtime reached the tool request limit ({tool_request_limit})."),
                false,
            ))
        }
    }
}
