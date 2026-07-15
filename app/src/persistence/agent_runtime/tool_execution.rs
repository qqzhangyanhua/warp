use diesel::prelude::*;
use diesel::SqliteConnection;

use super::super::model::{AgentToolExecutionState, NewAgentToolExecutionRecord};
use super::super::schema::agent_runtime_runs::dsl as runs_dsl;
use super::super::schema::agent_tool_execution_records::dsl as tools_dsl;
use super::super::{
    AcceptAgentToolExecution, AcceptAgentToolExecutionError, AcceptAgentToolExecutionResult,
    CompleteToolOutcomePayload, ExecutingAgentToolExecution, MarkAgentToolExecutionExecuting,
    MarkAgentToolExecutionExecutingError, ReadExecutingAgentToolExecutions,
    ReadExecutingAgentToolExecutionsError, ToolRequestPayload, ToolResultProjectionPayload,
};

pub(in crate::persistence) fn read_executing_agent_tool_executions(
    conn: &mut SqliteConnection,
    command: &ReadExecutingAgentToolExecutions,
) -> Result<Vec<ExecutingAgentToolExecution>, ReadExecutingAgentToolExecutionsError> {
    tools_dsl::agent_tool_execution_records
        .filter(tools_dsl::conversation_id.eq(&command.conversation_id))
        .filter(tools_dsl::state.eq(AgentToolExecutionState::Executing.as_database_value()))
        .order(tools_dsl::id)
        .select((
            tools_dsl::run_id,
            tools_dsl::tool_call_id,
            tools_dsl::request_encoding_version,
            tools_dsl::request_payload,
        ))
        .load::<(String, String, i32, Vec<u8>)>(conn)?
        .into_iter()
        .map(|(run_id, tool_call_id, version, payload)| {
            Ok(ExecutingAgentToolExecution {
                run_id,
                tool_call_id,
                request_payload: ToolRequestPayload::from_parts(version, payload)
                    .ok_or(ReadExecutingAgentToolExecutionsError::Persistence)?,
            })
        })
        .collect()
}

pub(in crate::persistence) fn accept_agent_tool_execution(
    conn: &mut SqliteConnection,
    command: &AcceptAgentToolExecution,
) -> Result<AcceptAgentToolExecutionResult, AcceptAgentToolExecutionError> {
    if !(1..=32).contains(&command.request_limit) {
        return Err(AcceptAgentToolExecutionError::InvalidRequestLimit);
    }
    conn.transaction::<AcceptAgentToolExecutionResult, AcceptAgentToolExecutionError, _>(|conn| {
        let run_exists = runs_dsl::agent_runtime_runs
            .filter(runs_dsl::conversation_id.eq(&command.conversation_id))
            .filter(runs_dsl::run_id.eq(&command.run_id))
            .select(runs_dsl::id)
            .first::<i32>(conn)
            .optional()?
            .is_some();
        if !run_exists {
            return Err(AcceptAgentToolExecutionError::RunNotFound);
        }
        let existing = tools_dsl::agent_tool_execution_records
            .filter(tools_dsl::conversation_id.eq(&command.conversation_id))
            .filter(tools_dsl::run_id.eq(&command.run_id))
            .filter(tools_dsl::tool_call_id.eq(&command.tool_call_id))
            .select((
                tools_dsl::id,
                tools_dsl::request_fingerprint,
                tools_dsl::state,
                tools_dsl::complete_outcome_encoding_version,
                tools_dsl::complete_outcome,
                tools_dsl::tool_result_projection_encoding_version,
                tools_dsl::tool_result_projection,
            ))
            .first::<(
                i32,
                Vec<u8>,
                String,
                Option<i32>,
                Option<Vec<u8>>,
                Option<i32>,
                Option<Vec<u8>>,
            )>(conn)
            .optional()?;
        if let Some((
            record_id,
            fingerprint,
            state,
            outcome_version,
            outcome,
            projection_version,
            projection,
        )) = existing
        {
            if fingerprint != command.request_fingerprint {
                return Err(AcceptAgentToolExecutionError::IdentityConflict);
            }
            return match AgentToolExecutionState::from_database_value(&state) {
                Some(AgentToolExecutionState::Pending) => {
                    let ordinal = tools_dsl::agent_tool_execution_records
                        .filter(tools_dsl::conversation_id.eq(&command.conversation_id))
                        .filter(tools_dsl::run_id.eq(&command.run_id))
                        .filter(tools_dsl::id.le(record_id))
                        .count()
                        .get_result::<i64>(conn)?;
                    if ordinal > i64::from(command.request_limit) {
                        Ok(AcceptAgentToolExecutionResult::LimitReached {
                            newly_inserted: false,
                        })
                    } else {
                        Ok(AcceptAgentToolExecutionResult::Pending {
                            newly_inserted: false,
                        })
                    }
                }
                Some(AgentToolExecutionState::Executing) => {
                    Ok(AcceptAgentToolExecutionResult::Executing)
                }
                Some(AgentToolExecutionState::Completed) => {
                    Ok(AcceptAgentToolExecutionResult::Completed {
                        complete_outcome: CompleteToolOutcomePayload::from_parts(
                            outcome_version.ok_or(AcceptAgentToolExecutionError::Persistence)?,
                            outcome.ok_or(AcceptAgentToolExecutionError::Persistence)?,
                        )
                        .ok_or(AcceptAgentToolExecutionError::Persistence)?,
                        tool_result_projection: ToolResultProjectionPayload::from_parts(
                            projection_version.ok_or(AcceptAgentToolExecutionError::Persistence)?,
                            projection.ok_or(AcceptAgentToolExecutionError::Persistence)?,
                        )
                        .ok_or(AcceptAgentToolExecutionError::Persistence)?,
                    })
                }
                None => Err(AcceptAgentToolExecutionError::Persistence),
            };
        }
        let accepted_count = tools_dsl::agent_tool_execution_records
            .filter(tools_dsl::conversation_id.eq(&command.conversation_id))
            .filter(tools_dsl::run_id.eq(&command.run_id))
            .count()
            .get_result::<i64>(conn)?;
        diesel::insert_into(tools_dsl::agent_tool_execution_records)
            .values(NewAgentToolExecutionRecord::pending(
                &command.conversation_id,
                &command.run_id,
                &command.tool_call_id,
                &command.request_fingerprint,
                command.request_payload.versioned(),
            ))
            .execute(conn)?;
        if accepted_count >= i64::from(command.request_limit) {
            return Ok(AcceptAgentToolExecutionResult::LimitReached {
                newly_inserted: true,
            });
        }
        Ok(AcceptAgentToolExecutionResult::Pending {
            newly_inserted: true,
        })
    })
}

pub(in crate::persistence) fn mark_agent_tool_execution_executing(
    conn: &mut SqliteConnection,
    command: &MarkAgentToolExecutionExecuting,
) -> Result<AcceptAgentToolExecutionResult, MarkAgentToolExecutionExecutingError> {
    conn.transaction::<AcceptAgentToolExecutionResult, MarkAgentToolExecutionExecutingError, _>(
        |conn| {
            let stored = tools_dsl::agent_tool_execution_records
                .filter(tools_dsl::conversation_id.eq(&command.conversation_id))
                .filter(tools_dsl::run_id.eq(&command.run_id))
                .filter(tools_dsl::tool_call_id.eq(&command.tool_call_id))
                .select((tools_dsl::request_fingerprint, tools_dsl::state))
                .first::<(Vec<u8>, String)>(conn)
                .optional()?
                .ok_or(MarkAgentToolExecutionExecutingError::NotFound)?;
            if stored.0 != command.request_fingerprint {
                return Err(MarkAgentToolExecutionExecutingError::IdentityConflict);
            }
            match AgentToolExecutionState::from_database_value(&stored.1) {
                Some(AgentToolExecutionState::Pending) => {
                    let updated = diesel::update(
                        tools_dsl::agent_tool_execution_records
                            .filter(tools_dsl::conversation_id.eq(&command.conversation_id))
                            .filter(tools_dsl::run_id.eq(&command.run_id))
                            .filter(tools_dsl::tool_call_id.eq(&command.tool_call_id))
                            .filter(
                                tools_dsl::state
                                    .eq(AgentToolExecutionState::Pending.as_database_value()),
                            ),
                    )
                    .set(
                        tools_dsl::state.eq(AgentToolExecutionState::Executing.as_database_value()),
                    )
                    .execute(conn)?;
                    if updated != 1 {
                        return Err(MarkAgentToolExecutionExecutingError::Persistence);
                    }
                    Ok(AcceptAgentToolExecutionResult::Executing)
                }
                Some(AgentToolExecutionState::Executing) => {
                    Ok(AcceptAgentToolExecutionResult::Executing)
                }
                Some(AgentToolExecutionState::Completed) => {
                    let completed = tools_dsl::agent_tool_execution_records
                        .filter(tools_dsl::conversation_id.eq(&command.conversation_id))
                        .filter(tools_dsl::run_id.eq(&command.run_id))
                        .filter(tools_dsl::tool_call_id.eq(&command.tool_call_id))
                        .select((
                            tools_dsl::complete_outcome_encoding_version,
                            tools_dsl::complete_outcome,
                            tools_dsl::tool_result_projection_encoding_version,
                            tools_dsl::tool_result_projection,
                        ))
                        .first::<(Option<i32>, Option<Vec<u8>>, Option<i32>, Option<Vec<u8>>)>(
                            conn,
                        )?;
                    Ok(AcceptAgentToolExecutionResult::Completed {
                        complete_outcome: CompleteToolOutcomePayload::from_parts(
                            completed
                                .0
                                .ok_or(MarkAgentToolExecutionExecutingError::Persistence)?,
                            completed
                                .1
                                .ok_or(MarkAgentToolExecutionExecutingError::Persistence)?,
                        )
                        .ok_or(MarkAgentToolExecutionExecutingError::Persistence)?,
                        tool_result_projection: ToolResultProjectionPayload::from_parts(
                            completed
                                .2
                                .ok_or(MarkAgentToolExecutionExecutingError::Persistence)?,
                            completed
                                .3
                                .ok_or(MarkAgentToolExecutionExecutingError::Persistence)?,
                        )
                        .ok_or(MarkAgentToolExecutionExecutingError::Persistence)?,
                    })
                }
                None => Err(MarkAgentToolExecutionExecutingError::Persistence),
            }
        },
    )
}
