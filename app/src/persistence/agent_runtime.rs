use diesel::prelude::*;
use diesel::SqliteConnection;

use super::agent::upsert_agent_conversation;
use super::model::{AgentConversationData, AgentRuntimeBinding, CompleteAgentToolExecution};
use super::schema::agent_conversations::dsl as conversations_dsl;
use super::schema::agent_runtime_runs::dsl as runs_dsl;
use super::schema::agent_tool_execution_records::dsl as tools_dsl;
use super::{
    AgentRuntimeSidecarMutation, CommitAgentRuntimeMutation, CommitAgentRuntimeMutationError,
};

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
