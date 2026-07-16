use std::sync::atomic::Ordering;
use std::sync::mpsc::SyncSender;

use futures::channel::oneshot;
use uuid::Uuid;
use warpui::{ModelContext, SingletonEntity};

use super::AgentRuntimeService;
use crate::ai::agent::conversation::{AIConversation, AIConversationId};
use crate::ai::agent::runtime::RuntimeError;
use crate::ai::blocklist::BlocklistAIHistoryModel;
use crate::persistence::{CommitAgentRuntimeMutation, ModelEvent, ReadLatestAgentRuntimeRunId};
use crate::GlobalResourceHandlesProvider;

pub(super) struct PendingHistoryEdit {
    persistence: SyncSender<ModelEvent>,
    conversation_id: AIConversationId,
    run_id: String,
    commit_id: String,
    expected_revision: u64,
    release_active_guard: bool,
    updated_tasks: Vec<warp_multi_agent_api::Task>,
    conversation_data: crate::persistence::model::AgentConversationData,
}

impl PendingHistoryEdit {
    pub(super) fn expected_revision(&self) -> u64 {
        self.expected_revision
    }
}

impl AgentRuntimeService {
    pub(crate) fn commit_history_edit(
        &mut self,
        conversation: &AIConversation,
        ctx: &mut ModelContext<Self>,
    ) {
        let conversation_id = conversation.id();
        #[cfg(test)]
        {
            *self
                .invalidate_attempts_by_conversation_id
                .entry(conversation_id)
                .or_default() += 1;
        }
        let active_run_id = self
            .active_run_ids_by_conversation_id
            .get(&conversation_id)
            .cloned();
        let run_id = active_run_id
            .clone()
            .or_else(|| {
                self.last_run_ids_by_conversation_id
                    .get(&conversation_id)
                    .cloned()
            })
            .or_else(|| latest_runtime_run_id_in_conversation(conversation));
        let Some(run_id) = run_id else {
            log::warn!(
                "Cannot commit Pi runtime history edit for conversation {conversation_id:?} without an Agent Run identity"
            );
            return;
        };
        let Some(persistence) = GlobalResourceHandlesProvider::as_ref(ctx)
            .get()
            .model_event_sender
            .clone()
        else {
            log::warn!(
                "Cannot commit Pi runtime history edit for conversation {conversation_id:?}: persistence is unavailable"
            );
            return;
        };
        let expected_revision = conversation.runtime_transcript_revision();
        let (updated_tasks, conversation_data) = conversation.runtime_persistence_snapshot();
        let handle = self
            .active_handles_by_conversation_id
            .get(&conversation_id)
            .cloned();
        let supervisor = self.supervisor.clone();
        let pending = PendingHistoryEdit {
            persistence,
            conversation_id,
            run_id: run_id.clone(),
            commit_id: format!("history-edit:{}", Uuid::new_v4()),
            expected_revision,
            release_active_guard: active_run_id.as_deref() == Some(run_id.as_str()),
            updated_tasks,
            conversation_data,
        };
        if active_run_id.as_deref() != Some(run_id.as_str()) {
            self.commit_pending_history_edit(pending, expected_revision, ctx);
            return;
        }
        if let Some(cancellation) = self
            .active_run_cancellations_by_conversation_id
            .get(&conversation_id)
        {
            cancellation.store(true, Ordering::Release);
        }
        self.pending_history_edits_by_run_id
            .insert(run_id.clone(), pending);
        let Some(handle) = handle else {
            self.cancelled_starting_run_ids.insert(run_id);
            if let Some(supervisor) = supervisor {
                ctx.spawn(
                    async move {
                        supervisor
                            .invalidate_checkpoint(&conversation_id.to_string())
                            .await
                    },
                    |_, (), _| {},
                );
            }
            return;
        };
        ctx.spawn(
            async move {
                let revision = handle.cancel_run().await;
                if let Some(supervisor) = supervisor {
                    supervisor
                        .invalidate_checkpoint(&conversation_id.to_string())
                        .await;
                }
                revision
            },
            move |service, result, ctx| match result {
                Ok(revision) => {
                    if let Some(pending) = service.pending_history_edits_by_run_id.remove(&run_id) {
                        service.commit_pending_history_edit(
                            pending,
                            revision.unwrap_or(expected_revision),
                            ctx,
                        );
                    }
                }
                Err(RuntimeError::NoActiveRun) => {}
                Err(error) => {
                    log::warn!(
                        "Failed to cancel Agent Run before Pi runtime history edit for conversation {conversation_id:?}: {error}"
                    );
                }
            },
        );
    }

    pub(super) fn commit_pending_history_edit(
        &mut self,
        pending: PendingHistoryEdit,
        expected_revision: u64,
        ctx: &mut ModelContext<Self>,
    ) {
        let conversation_id = pending.conversation_id;
        let run_id = pending.run_id.clone();
        let release_active_guard = pending.release_active_guard;
        if release_active_guard {
            self.history_commit_barrier_run_ids.insert(run_id.clone());
        }
        ctx.spawn(
            async move {
                commit_history_mutation(
                    &pending.persistence,
                    pending.conversation_id,
                    pending.run_id,
                    pending.commit_id,
                    expected_revision,
                    pending.updated_tasks,
                    pending.conversation_data,
                )
                .await
            },
            move |service, result, ctx| {
                let mut can_release_active_guard = true;
                match result {
                    Ok(revision) => {
                        BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, _| {
                            if let Some(conversation) =
                                history_model.conversation_mut(&conversation_id)
                            {
                                conversation.set_runtime_transcript_revision(revision);
                            }
                        });
                    }
                    Err(RuntimeError::CommitPersistence(
                        crate::persistence::CommitAgentRuntimeMutationError::RevisionConflict {
                            ..
                        },
                    )) => {
                        can_release_active_guard = BlocklistAIHistoryModel::handle(ctx).update(
                            ctx,
                            |history_model, ctx| {
                                history_model
                                    .reload_runtime_conversation_from_db(conversation_id, ctx)
                            },
                        );
                    }
                    Err(error) => {
                        log::warn!(
                            "Failed to commit Pi runtime history edit for conversation {conversation_id:?}: {error}"
                        );
                        can_release_active_guard = BlocklistAIHistoryModel::handle(ctx).update(
                            ctx,
                            |history_model, ctx| {
                                history_model
                                    .reload_runtime_conversation_from_db(conversation_id, ctx)
                            },
                        );
                    }
                }
                if release_active_guard && can_release_active_guard {
                    service.history_commit_barrier_run_ids.remove(&run_id);
                    if service
                        .active_run_ids_by_conversation_id
                        .get(&conversation_id)
                        .is_some_and(|active| active == &run_id)
                    {
                        service
                            .active_run_ids_by_conversation_id
                            .remove(&conversation_id);
                    }
                    service
                        .active_handles_by_conversation_id
                        .remove(&conversation_id);
                    service
                        .active_run_cancellations_by_conversation_id
                        .remove(&conversation_id);
                    service.cancelled_starting_run_ids.remove(&run_id);
                    ctx.emit(super::AgentRuntimeServiceEvent::RunFinished { conversation_id });
                } else if release_active_guard {
                    log::warn!(
                        "Keeping Pi runtime history barrier active for conversation {conversation_id:?} because the authoritative Conversation Record could not be restored"
                    );
                }
            },
        );
    }

    pub(crate) fn restore_conversation(
        &mut self,
        conversation: &AIConversation,
        ctx: &mut ModelContext<Self>,
    ) {
        let conversation_id = conversation.id();
        #[cfg(test)]
        {
            *self
                .restore_attempts_by_conversation_id
                .entry(conversation_id)
                .or_default() += 1;
        }
        if let Some(run_id) = latest_runtime_run_id_in_conversation(conversation) {
            self.last_run_ids_by_conversation_id
                .insert(conversation_id, run_id);
        }

        let Some(persistence) = GlobalResourceHandlesProvider::as_ref(ctx)
            .get()
            .model_event_sender
            .clone()
        else {
            return;
        };
        let (acknowledgement, acknowledged) = oneshot::channel();
        if persistence
            .send(ModelEvent::ReadLatestAgentRuntimeRunId(
                ReadLatestAgentRuntimeRunId {
                    conversation_id: conversation_id.to_string(),
                    acknowledgement,
                },
            ))
            .is_err()
        {
            return;
        }
        ctx.spawn(
            async move { acknowledged.await },
            move |service, result, _| match result {
                Ok(Ok(Some(run_id))) => {
                    service
                        .last_run_ids_by_conversation_id
                        .insert(conversation_id, run_id);
                }
                Ok(Ok(None)) => {}
                Ok(Err(error)) => {
                    log::warn!(
                        "Failed to restore Agent Runtime run lineage for conversation {conversation_id:?}: {error}"
                    );
                }
                Err(_) => {
                    log::warn!(
                        "Agent Runtime run-lineage restore acknowledgement was dropped for conversation {conversation_id:?}"
                    );
                }
            },
        );
    }
}

#[allow(clippy::too_many_arguments)]
async fn commit_history_mutation(
    persistence: &SyncSender<ModelEvent>,
    conversation_id: AIConversationId,
    run_id: String,
    commit_id: String,
    expected_revision: u64,
    updated_tasks: Vec<warp_multi_agent_api::Task>,
    conversation_data: crate::persistence::model::AgentConversationData,
) -> Result<u64, RuntimeError> {
    let (acknowledgement, acknowledged) = oneshot::channel();
    persistence
        .send(ModelEvent::CommitAgentRuntimeMutation(
            CommitAgentRuntimeMutation {
                conversation_id: conversation_id.to_string(),
                run_id,
                commit_id,
                expected_revision,
                updated_tasks,
                conversation_data,
                sidecar_mutation: None,
                acknowledgement,
            },
        ))
        .map_err(|_| RuntimeError::PersistenceUnavailable)?;
    acknowledged
        .await
        .map_err(|_| RuntimeError::PersistenceAcknowledgementDropped)?
        .map_err(RuntimeError::from)
}

fn latest_runtime_run_id_in_conversation(conversation: &AIConversation) -> Option<String> {
    conversation
        .all_linearized_messages()
        .into_iter()
        .rev()
        .find(|message| {
            matches!(
                message.message.as_ref(),
                Some(
                    warp_multi_agent_api::message::Message::AgentOutput(_)
                        | warp_multi_agent_api::message::Message::ToolCall(_)
                        | warp_multi_agent_api::message::Message::ToolCallResult(_)
                )
            ) && !message.request_id.is_empty()
        })
        .map(|message| message.request_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::model::AgentConversationData;
    use crate::persistence::CommitAgentRuntimeMutationError;

    #[test]
    fn history_commit_does_not_overwrite_a_newer_revision_after_conflict() {
        let (sender, receiver) = std::sync::mpsc::sync_channel(2);
        let conversation_id = AIConversationId::new();
        let conversation_data: AgentConversationData =
            serde_json::from_str(r#"{"server_conversation_token":null}"#)
                .expect("minimal conversation data should deserialize");
        let commit = std::thread::spawn(move || {
            futures::executor::block_on(commit_history_mutation(
                &sender,
                conversation_id,
                "run-1".to_string(),
                "history-edit-1".to_string(),
                0,
                vec![],
                conversation_data,
            ))
        });

        let ModelEvent::CommitAgentRuntimeMutation(command) = receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("history mutation should be sent")
        else {
            panic!("expected CommitAgentRuntimeMutation");
        };
        command
            .acknowledgement
            .send(Err(CommitAgentRuntimeMutationError::RevisionConflict {
                expected: 0,
                actual: 1,
            }))
            .expect("commit acknowledgement receiver should remain active");

        assert!(matches!(
            commit.join().expect("history commit thread should finish"),
            Err(RuntimeError::CommitPersistence(
                CommitAgentRuntimeMutationError::RevisionConflict {
                    expected: 0,
                    actual: 1,
                }
            ))
        ));
        assert!(receiver.try_recv().is_err(), "history edit must not retry");
    }
}
