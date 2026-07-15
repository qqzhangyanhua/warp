use std::collections::HashSet;

use diesel::prelude::*;
use diesel::SqliteConnection;

use super::agent::upsert_agent_conversation;
use super::model::{
    AgentConversationData, AgentRuntimeBinding, AgentRuntimeRunState, CompleteAgentToolExecution,
    NewAgentRuntimeRunRecord,
};
use super::schema::agent_conversations::dsl as conversations_dsl;
use super::schema::agent_runtime_runs::dsl as runs_dsl;
use super::schema::agent_tool_execution_records::dsl as tools_dsl;
use super::{
    AgentRuntimeRunMutation, AgentRuntimeSidecarMutation, CommitAgentRuntimeMutation,
    CommitAgentRuntimeMutationError, PersistAgentRuntimeRun, PersistAgentRuntimeRunError,
};

#[allow(
    dead_code,
    reason = "Runtime restart reconstruction consumes this query when selection is enabled in Phase 7"
)]
pub(crate) fn read_interrupted_agent_message_ids(
    conn: &mut SqliteConnection,
    conversation_id: &str,
) -> Result<HashSet<String>, diesel::result::Error> {
    let run_ids = runs_dsl::agent_runtime_runs
        .filter(runs_dsl::conversation_id.eq(conversation_id))
        .filter(runs_dsl::state.eq("finished"))
        .filter(runs_dsl::terminal_outcome.ne("completed"))
        .select(runs_dsl::run_id)
        .load::<String>(conn)?;
    Ok(run_ids
        .into_iter()
        .map(|run_id| format!("interrupted:{run_id}"))
        .collect())
}

pub(super) fn persist_agent_runtime_run(
    conn: &mut SqliteConnection,
    command: &PersistAgentRuntimeRun,
) -> Result<(), PersistAgentRuntimeRunError> {
    match &command.mutation {
        AgentRuntimeRunMutation::Start {
            retry_of_run_id,
            starting_revision,
        } => conn.transaction::<(), PersistAgentRuntimeRunError, _>(|conn| {
            let stored_data = conversations_dsl::agent_conversations
                .filter(conversations_dsl::conversation_id.eq(&command.conversation_id))
                .select(conversations_dsl::conversation_data)
                .first::<String>(conn)
                .optional()?
                .ok_or(PersistAgentRuntimeRunError::ConversationNotFound)?;
            let stored_data: AgentConversationData = serde_json::from_str(&stored_data)
                .map_err(|_| PersistAgentRuntimeRunError::Persistence)?;
            if stored_data.effective_runtime_binding() != AgentRuntimeBinding::Pi {
                return Err(PersistAgentRuntimeRunError::RuntimeBindingMismatch);
            }
            let actual_revision = stored_data.effective_runtime_transcript_revision();
            if actual_revision != *starting_revision {
                return Err(PersistAgentRuntimeRunError::RevisionConflict {
                    expected: *starting_revision,
                    actual: actual_revision,
                });
            }
            let starting_revision = i64::try_from(*starting_revision)
                .map_err(|_| PersistAgentRuntimeRunError::RevisionOverflow)?;
            diesel::insert_into(runs_dsl::agent_runtime_runs)
                .values(NewAgentRuntimeRunRecord::starting(
                    &command.conversation_id,
                    &command.run_id,
                    retry_of_run_id.as_deref(),
                    starting_revision,
                ))
                .execute(conn)?;
            Ok(())
        }),
        AgentRuntimeRunMutation::SetState(state) => {
            if matches!(
                state,
                AgentRuntimeRunState::Starting | AgentRuntimeRunState::Finished
            ) {
                return Err(PersistAgentRuntimeRunError::InvalidTransition);
            }
            let updated = diesel::update(
                runs_dsl::agent_runtime_runs
                    .filter(runs_dsl::conversation_id.eq(&command.conversation_id))
                    .filter(runs_dsl::run_id.eq(&command.run_id))
                    .filter(runs_dsl::state.ne("finished")),
            )
            .set(runs_dsl::state.eq(state.as_database_value()))
            .execute(conn)?;
            match updated {
                1 => Ok(()),
                _ => Err(PersistAgentRuntimeRunError::RunNotFound),
            }
        }
        AgentRuntimeRunMutation::Finish(outcome) => conn
            .transaction::<(), PersistAgentRuntimeRunError, _>(|conn| {
                let current = runs_dsl::agent_runtime_runs
                    .filter(runs_dsl::conversation_id.eq(&command.conversation_id))
                    .filter(runs_dsl::run_id.eq(&command.run_id))
                    .select((runs_dsl::state, runs_dsl::terminal_outcome))
                    .first::<(String, Option<String>)>(conn)
                    .optional()?
                    .ok_or(PersistAgentRuntimeRunError::RunNotFound)?;
                let outcome_value = outcome.as_database_value();
                if current.0 == "finished" {
                    return (current.1.as_deref() == Some(outcome_value))
                        .then_some(())
                        .ok_or(PersistAgentRuntimeRunError::InvalidTransition);
                }
                diesel::update(
                    runs_dsl::agent_runtime_runs
                        .filter(runs_dsl::conversation_id.eq(&command.conversation_id))
                        .filter(runs_dsl::run_id.eq(&command.run_id)),
                )
                .set((
                    runs_dsl::state.eq("finished"),
                    runs_dsl::terminal_outcome.eq(outcome_value),
                ))
                .execute(conn)?;
                Ok(())
            }),
    }
}

pub(super) fn commit_agent_runtime_mutation(
    conn: &mut SqliteConnection,
    mutation: &CommitAgentRuntimeMutation,
) -> Result<u64, CommitAgentRuntimeMutationError> {
    let payload_fingerprint = mutation.payload_fingerprint()?;

    conn.transaction::<u64, CommitAgentRuntimeMutationError, _>(|conn| {
        let stored_data = conversations_dsl::agent_conversations
            .filter(conversations_dsl::conversation_id.eq(&mutation.conversation_id))
            .select(conversations_dsl::conversation_data)
            .first::<String>(conn)
            .optional()
            .map_err(|_| CommitAgentRuntimeMutationError::Persistence)?
            .ok_or(CommitAgentRuntimeMutationError::ConversationNotFound)?;
        let stored_data: AgentConversationData = serde_json::from_str(&stored_data)
            .map_err(|_| CommitAgentRuntimeMutationError::Persistence)?;
        if stored_data.effective_runtime_binding() != AgentRuntimeBinding::Pi {
            return Err(CommitAgentRuntimeMutationError::RuntimeBindingMismatch);
        }

        let (last_commit_id, last_committed_revision, last_commit_payload_fingerprint) =
            runs_dsl::agent_runtime_runs
                .filter(runs_dsl::conversation_id.eq(&mutation.conversation_id))
                .filter(runs_dsl::run_id.eq(&mutation.run_id))
                .select((
                    runs_dsl::last_commit_id,
                    runs_dsl::last_committed_revision,
                    runs_dsl::last_commit_payload_fingerprint,
                ))
                .first::<(Option<String>, Option<i64>, Option<Vec<u8>>)>(conn)
                .optional()
                .map_err(|_| CommitAgentRuntimeMutationError::Persistence)?
                .ok_or(CommitAgentRuntimeMutationError::RunNotFound)?;

        let actual_revision = stored_data.effective_runtime_transcript_revision();
        if last_commit_id.as_deref() == Some(mutation.commit_id.as_str()) {
            let committed_revision = last_committed_revision
                .and_then(|revision| u64::try_from(revision).ok())
                .ok_or(CommitAgentRuntimeMutationError::Persistence)?;
            let is_identical_redelivery = actual_revision == committed_revision
                && mutation.expected_revision.checked_add(1) == Some(committed_revision)
                && last_commit_payload_fingerprint.as_deref()
                    == Some(payload_fingerprint.as_slice());
            if is_identical_redelivery {
                return Ok(committed_revision);
            }
            return Err(CommitAgentRuntimeMutationError::CommitIdentityConflict);
        }

        if actual_revision != mutation.expected_revision {
            return Err(CommitAgentRuntimeMutationError::RevisionConflict {
                expected: mutation.expected_revision,
                actual: actual_revision,
            });
        }
        let committed_revision = actual_revision
            .checked_add(1)
            .ok_or(CommitAgentRuntimeMutationError::RevisionOverflow)?;
        let committed_revision_i64 = i64::try_from(committed_revision)
            .map_err(|_| CommitAgentRuntimeMutationError::RevisionOverflow)?;

        let mut conversation_data = mutation.conversation_data.clone();
        conversation_data.runtime_binding = Some(AgentRuntimeBinding::Pi);
        conversation_data.runtime_transcript_revision = Some(committed_revision);
        upsert_agent_conversation(
            conn,
            &mutation.conversation_id,
            &mutation.updated_tasks,
            conversation_data,
        )
        .map_err(|_| CommitAgentRuntimeMutationError::Persistence)?;

        if let Some(sidecar_mutation) = &mutation.sidecar_mutation {
            apply_agent_runtime_sidecar_mutation(
                conn,
                &mutation.conversation_id,
                &mutation.run_id,
                sidecar_mutation,
            )?;
        }

        let updated_runs = diesel::update(
            runs_dsl::agent_runtime_runs
                .filter(runs_dsl::conversation_id.eq(&mutation.conversation_id))
                .filter(runs_dsl::run_id.eq(&mutation.run_id)),
        )
        .set((
            runs_dsl::last_commit_id.eq(&mutation.commit_id),
            runs_dsl::last_committed_revision.eq(committed_revision_i64),
            runs_dsl::last_commit_payload_fingerprint.eq(payload_fingerprint.as_slice()),
        ))
        .execute(conn)
        .map_err(|_| CommitAgentRuntimeMutationError::Persistence)?;
        if updated_runs != 1 {
            return Err(CommitAgentRuntimeMutationError::RunNotFound);
        }

        Ok(committed_revision)
    })
}

fn apply_agent_runtime_sidecar_mutation(
    conn: &mut SqliteConnection,
    conversation_id: &str,
    run_id: &str,
    mutation: &AgentRuntimeSidecarMutation,
) -> Result<(), CommitAgentRuntimeMutationError> {
    match mutation {
        AgentRuntimeSidecarMutation::CompleteToolExecution {
            tool_call_id,
            complete_outcome,
            tool_result_projection,
        } => {
            let updated_tools = diesel::update(
                tools_dsl::agent_tool_execution_records
                    .filter(tools_dsl::conversation_id.eq(conversation_id))
                    .filter(tools_dsl::run_id.eq(run_id))
                    .filter(tools_dsl::tool_call_id.eq(tool_call_id))
                    .filter(tools_dsl::state.eq("executing")),
            )
            .set(CompleteAgentToolExecution::new(
                complete_outcome.versioned(),
                tool_result_projection.versioned(),
            ))
            .execute(conn)
            .map_err(|_| CommitAgentRuntimeMutationError::Persistence)?;
            if updated_tools != 1 {
                return Err(CommitAgentRuntimeMutationError::Persistence);
            }
        }
    }

    Ok(())
}
