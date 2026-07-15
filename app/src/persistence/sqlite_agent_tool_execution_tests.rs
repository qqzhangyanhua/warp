use std::path::PathBuf;

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use futures::channel::oneshot;

use super::{setup_database, start_writer};
use crate::persistence::model::{
    AgentConversationData, AgentRuntimeBinding, AgentToolExecutionRecord, AgentToolExecutionState,
    NewAgentRuntimeRunRecord,
};
use crate::persistence::schema::{agent_runtime_runs, agent_tool_execution_records};
use crate::persistence::{
    AcceptAgentToolExecution, AcceptAgentToolExecutionError, AcceptAgentToolExecutionResult,
    MarkAgentToolExecutionExecuting, MarkAgentToolExecutionExecutingError, ModelEvent,
    ToolRequestPayload,
};

const CONVERSATION_ID: &str = "conv-tool";
const RUN_ID: &str = "run-tool";

#[derive(QueryableByName)]
struct StoredRequest {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    request_encoding_version: i32,
    #[diesel(sql_type = diesel::sql_types::Binary)]
    request_payload: Vec<u8>,
}

struct Harness {
    _tempdir: tempfile::TempDir,
    database_path: PathBuf,
    writer: super::WriterHandles,
}

impl Harness {
    fn new() -> Self {
        let tempdir = tempfile::tempdir().unwrap();
        let database_path = tempdir.path().join("warp.sqlite");
        let mut conn = setup_database(&database_path).unwrap();
        let mut conversation_data: AgentConversationData =
            serde_json::from_str(r#"{"server_conversation_token":null}"#).unwrap();
        conversation_data.runtime_binding = Some(AgentRuntimeBinding::Pi);
        conversation_data.runtime_transcript_revision = Some(0);
        super::upsert_agent_conversation(&mut conn, CONVERSATION_ID, &[], conversation_data)
            .unwrap();
        diesel::insert_into(agent_runtime_runs::table)
            .values(NewAgentRuntimeRunRecord::starting(
                CONVERSATION_ID,
                RUN_ID,
                None,
                0,
            ))
            .execute(&mut conn)
            .unwrap();
        let writer = start_writer(conn, database_path.clone()).unwrap();
        Self {
            _tempdir: tempdir,
            database_path,
            writer,
        }
    }

    fn accept(
        &self,
        tool_call_id: &str,
        request_fingerprint: [u8; 32],
        request_limit: u32,
    ) -> Result<AcceptAgentToolExecutionResult, AcceptAgentToolExecutionError> {
        let (acknowledgement, acknowledged) = oneshot::channel();
        self.writer
            .sender
            .send(ModelEvent::AcceptAgentToolExecution(
                AcceptAgentToolExecution {
                    conversation_id: CONVERSATION_ID.to_string(),
                    run_id: RUN_ID.to_string(),
                    tool_call_id: tool_call_id.to_string(),
                    request_fingerprint,
                    request_payload: ToolRequestPayload::current(
                        format!("request:{tool_call_id}").into_bytes(),
                    ),
                    request_limit,
                    acknowledgement,
                },
            ))
            .unwrap();
        futures::executor::block_on(acknowledged).unwrap()
    }

    fn tool_count(&self) -> i64 {
        let mut conn = SqliteConnection::establish(self.database_path.to_str().unwrap()).unwrap();
        agent_tool_execution_records::table
            .count()
            .get_result(&mut conn)
            .unwrap()
    }

    fn stored_request(&self, tool_call_id: &str) -> StoredRequest {
        let mut conn = SqliteConnection::establish(self.database_path.to_str().unwrap()).unwrap();
        diesel::sql_query(
            "SELECT request_encoding_version, request_payload \
             FROM agent_tool_execution_records WHERE tool_call_id = ?",
        )
        .bind::<diesel::sql_types::Text, _>(tool_call_id)
        .get_result(&mut conn)
        .unwrap()
    }

    fn mark_executing(
        &self,
        tool_call_id: &str,
        request_fingerprint: [u8; 32],
    ) -> Result<AcceptAgentToolExecutionResult, MarkAgentToolExecutionExecutingError> {
        let (acknowledgement, acknowledged) = oneshot::channel();
        self.writer
            .sender
            .send(ModelEvent::MarkAgentToolExecutionExecuting(
                MarkAgentToolExecutionExecuting {
                    conversation_id: CONVERSATION_ID.to_string(),
                    run_id: RUN_ID.to_string(),
                    tool_call_id: tool_call_id.to_string(),
                    request_fingerprint,
                    acknowledgement,
                },
            ))
            .unwrap();
        futures::executor::block_on(acknowledged).unwrap()
    }

    fn tool_state(&self, tool_call_id: &str) -> AgentToolExecutionState {
        let mut conn = SqliteConnection::establish(self.database_path.to_str().unwrap()).unwrap();
        agent_tool_execution_records::table
            .filter(agent_tool_execution_records::tool_call_id.eq(tool_call_id))
            .select(AgentToolExecutionRecord::as_select())
            .first::<AgentToolExecutionRecord>(&mut conn)
            .unwrap()
            .state()
            .unwrap()
    }
}

#[test]
fn accepting_a_tool_request_is_durable_idempotent_and_identity_safe() {
    let harness = Harness::new();

    assert_eq!(
        harness.accept("call-1", [1; 32], 32),
        Ok(AcceptAgentToolExecutionResult::Pending {
            newly_inserted: true
        })
    );
    assert_eq!(
        harness.accept("call-1", [1; 32], 32),
        Ok(AcceptAgentToolExecutionResult::Pending {
            newly_inserted: false
        })
    );
    assert_eq!(
        harness.accept("call-1", [2; 32], 32),
        Err(AcceptAgentToolExecutionError::IdentityConflict)
    );
    assert_eq!(harness.tool_count(), 1);
    let stored = harness.stored_request("call-1");
    assert_eq!(stored.request_encoding_version, 1);
    assert_eq!(stored.request_payload, b"request:call-1");
}

#[test]
fn accepting_a_tool_request_enforces_the_durable_run_limit() {
    let harness = Harness::new();
    assert_eq!(
        harness.accept("call-1", [1; 32], 1),
        Ok(AcceptAgentToolExecutionResult::Pending {
            newly_inserted: true
        })
    );

    assert_eq!(
        harness.accept("call-2", [2; 32], 1),
        Ok(AcceptAgentToolExecutionResult::LimitReached {
            newly_inserted: true,
        })
    );
    assert_eq!(harness.tool_count(), 2);
    assert_eq!(
        harness.tool_state("call-2"),
        AgentToolExecutionState::Pending
    );
}

#[test]
fn marking_a_tool_executing_is_a_durable_idempotent_barrier() {
    let harness = Harness::new();
    assert_eq!(
        harness.accept("call-1", [1; 32], 32),
        Ok(AcceptAgentToolExecutionResult::Pending {
            newly_inserted: true
        })
    );

    assert_eq!(
        harness.mark_executing("call-1", [1; 32]),
        Ok(AcceptAgentToolExecutionResult::Executing)
    );
    assert_eq!(
        harness.mark_executing("call-1", [1; 32]),
        Ok(AcceptAgentToolExecutionResult::Executing)
    );
    assert_eq!(
        harness.mark_executing("call-1", [2; 32]),
        Err(MarkAgentToolExecutionExecutingError::IdentityConflict)
    );
    assert_eq!(
        harness.tool_state("call-1"),
        AgentToolExecutionState::Executing
    );
}

#[test]
fn tool_request_payload_migration_upgrades_existing_records() {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    conn.batch_execute(
        "CREATE TABLE agent_tool_execution_records (\
            id INTEGER PRIMARY KEY NOT NULL,\
            conversation_id TEXT NOT NULL,\
            run_id TEXT NOT NULL,\
            tool_call_id TEXT NOT NULL,\
            request_fingerprint BLOB NOT NULL,\
            state TEXT NOT NULL,\
            complete_outcome_encoding_version INTEGER,\
            complete_outcome BLOB,\
            tool_result_projection_encoding_version INTEGER,\
            tool_result_projection BLOB,\
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,\
            last_modified_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP\
        );\
        INSERT INTO agent_tool_execution_records (\
            conversation_id, run_id, tool_call_id, request_fingerprint, state\
        ) VALUES ('conversation', 'run', 'legacy-call', zeroblob(32), 'executing');",
    )
    .unwrap();

    conn.batch_execute(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../crates/persistence/migrations/2026-07-15-000000_add_agent_tool_request_payload/up.sql"
    )))
    .unwrap();

    let stored = diesel::sql_query(
        "SELECT request_encoding_version, request_payload \
         FROM agent_tool_execution_records WHERE tool_call_id = 'legacy-call'",
    )
    .get_result::<StoredRequest>(&mut conn)
    .unwrap();
    assert_eq!(stored.request_encoding_version, 1);
    assert!(stored.request_payload.is_empty());
}
