use std::path::PathBuf;

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use futures::channel::oneshot;
use warp_multi_agent_api as api;

use super::{setup_database, start_writer, upsert_agent_conversation};
use crate::persistence::agent::read_agent_conversation_by_id;
use crate::persistence::model::{
    AgentConversationData, AgentRuntimeBinding, AgentToolExecutionRecord, AgentToolExecutionState,
    NewAgentRuntimeRunRecord, NewAgentToolExecutionRecord, VersionedToolRequest,
    COMPLETE_TOOL_OUTCOME_ENCODING_VERSION, TOOL_RESULT_PROJECTION_ENCODING_VERSION,
};
use crate::persistence::schema::agent_runtime_runs::dsl as runs_dsl;
use crate::persistence::schema::agent_tool_execution_records::dsl as tools_dsl;
use crate::persistence::schema::{agent_runtime_runs, agent_tool_execution_records};
use crate::persistence::{
    AgentRuntimeSidecarMutation, CommitAgentRuntimeMutation, CommitAgentRuntimeMutationError,
    CompleteToolOutcomePayload, ModelEvent, ToolResultProjectionPayload, WriterHandles,
};

const CONVERSATION_ID: &str = "conv-1";
const RUN_ID: &str = "run-1";
const TOOL_CALL_ID: &str = "call-1";

struct RuntimeWriterHarness {
    _tempdir: tempfile::TempDir,
    database_path: PathBuf,
    writer: WriterHandles,
}

impl RuntimeWriterHarness {
    fn new(revision: u64, tasks: &[api::Task]) -> Self {
        Self::build(revision, tasks, false, |_| {})
    }

    fn with_executing_tool(revision: u64, tasks: &[api::Task]) -> Self {
        Self::build(revision, tasks, true, |_| {})
    }

    fn build(
        revision: u64,
        tasks: &[api::Task],
        with_executing_tool: bool,
        configure: impl FnOnce(&mut SqliteConnection),
    ) -> Self {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let database_path = tempdir.path().join("warp.sqlite");
        let mut conn = setup_database(&database_path).expect("database should initialize");
        upsert_agent_conversation(
            &mut conn,
            CONVERSATION_ID,
            tasks,
            runtime_conversation_data(revision),
        )
        .expect("Pi-bound conversation should insert");
        diesel::insert_into(agent_runtime_runs::table)
            .values(NewAgentRuntimeRunRecord::starting(
                CONVERSATION_ID,
                RUN_ID,
                None,
                i64::try_from(revision).expect("test revision should fit SQLite"),
            ))
            .execute(&mut conn)
            .expect("Agent Run Record should insert");
        if with_executing_tool {
            let request_fingerprint = [7_u8; 32];
            diesel::insert_into(agent_tool_execution_records::table)
                .values(NewAgentToolExecutionRecord::pending(
                    CONVERSATION_ID,
                    RUN_ID,
                    TOOL_CALL_ID,
                    &request_fingerprint,
                    VersionedToolRequest::current(b"request"),
                ))
                .execute(&mut conn)
                .expect("Tool Execution Record should insert");
            diesel::update(
                tools_dsl::agent_tool_execution_records
                    .filter(tools_dsl::tool_call_id.eq(TOOL_CALL_ID)),
            )
            .set(tools_dsl::state.eq("executing"))
            .execute(&mut conn)
            .expect("Tool Execution Record should enter executing state");
        }
        configure(&mut conn);
        let writer = start_writer(conn, database_path.clone()).expect("writer should start");
        Self {
            _tempdir: tempdir,
            database_path,
            writer,
        }
    }

    fn commit(
        &self,
        commit_id: &str,
        expected_revision: u64,
        updated_tasks: Vec<api::Task>,
        sidecar_mutation: Option<AgentRuntimeSidecarMutation>,
    ) -> Result<u64, CommitAgentRuntimeMutationError> {
        futures::executor::block_on(self.send_commit(
            commit_id,
            expected_revision,
            updated_tasks,
            sidecar_mutation,
        ))
        .expect("writer should acknowledge runtime mutation")
    }

    fn lose_acknowledgement(
        &self,
        commit_id: &str,
        expected_revision: u64,
        updated_tasks: Vec<api::Task>,
        sidecar_mutation: Option<AgentRuntimeSidecarMutation>,
    ) {
        drop(self.send_commit(
            commit_id,
            expected_revision,
            updated_tasks,
            sidecar_mutation,
        ));
    }

    fn send_commit(
        &self,
        commit_id: &str,
        expected_revision: u64,
        updated_tasks: Vec<api::Task>,
        sidecar_mutation: Option<AgentRuntimeSidecarMutation>,
    ) -> oneshot::Receiver<Result<u64, CommitAgentRuntimeMutationError>> {
        let (acknowledgement, acknowledged) = oneshot::channel();
        self.writer
            .sender
            .send(ModelEvent::CommitAgentRuntimeMutation(
                CommitAgentRuntimeMutation {
                    conversation_id: CONVERSATION_ID.to_string(),
                    run_id: RUN_ID.to_string(),
                    commit_id: commit_id.to_string(),
                    expected_revision,
                    updated_tasks,
                    conversation_data: runtime_conversation_data(expected_revision),
                    sidecar_mutation,
                    acknowledgement,
                },
            ))
            .expect("runtime mutation should send");
        acknowledged
    }

    fn delete_conversation(&self) {
        self.writer
            .sender
            .send(ModelEvent::DeleteMultiAgentConversations {
                conversation_ids: vec![CONVERSATION_ID.to_string()],
            })
            .expect("conversation deletion should send");
    }

    fn replace_history(&self, revision: u64, tasks: &[api::Task]) {
        let mut conn = setup_database(&self.database_path).expect("database should reopen");
        upsert_agent_conversation(
            &mut conn,
            CONVERSATION_ID,
            tasks,
            runtime_conversation_data(revision),
        )
        .expect("concurrent history edit should persist");
    }

    fn finish(self, assert_persisted: impl FnOnce(&mut SqliteConnection)) {
        self.writer
            .sender
            .send(ModelEvent::Terminate)
            .expect("terminate event should send");
        self.writer.handle.join().expect("writer should terminate");
        let mut conn = setup_database(&self.database_path).expect("database should reopen");
        assert_persisted(&mut conn);
    }
}

fn runtime_conversation_data(revision: u64) -> AgentConversationData {
    let mut data: AgentConversationData =
        serde_json::from_str(r#"{"server_conversation_token":null}"#)
            .expect("minimal conversation data should deserialize");
    data.runtime_binding = Some(AgentRuntimeBinding::Pi);
    data.runtime_transcript_revision = Some(revision);
    data
}

fn runtime_task(task_id: &str, query: &str) -> api::Task {
    api::Task {
        id: task_id.to_string(),
        messages: vec![api::Message {
            id: format!("{task_id}-query"),
            task_id: task_id.to_string(),
            message: Some(api::message::Message::UserQuery(api::message::UserQuery {
                query: query.to_string(),
                ..Default::default()
            })),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn complete_tool_execution() -> AgentRuntimeSidecarMutation {
    AgentRuntimeSidecarMutation::CompleteToolExecution {
        tool_call_id: TOOL_CALL_ID.to_string(),
        complete_outcome: CompleteToolOutcomePayload::current(vec![1, 2]),
        tool_result_projection: ToolResultProjectionPayload::current(vec![3, 4]),
    }
}

fn assert_runtime_conversation(
    conn: &mut SqliteConnection,
    expected_revision: u64,
    expected_tasks: &[api::Task],
) {
    let restored = read_agent_conversation_by_id(conn, CONVERSATION_ID)
        .expect("conversation should be readable")
        .expect("conversation should remain present");
    let data: AgentConversationData =
        serde_json::from_str(&restored.conversation.conversation_data)
            .expect("conversation data should deserialize");
    assert_eq!(
        data.effective_runtime_transcript_revision(),
        expected_revision
    );
    assert_eq!(restored.tasks, expected_tasks);
}

fn load_tool(conn: &mut SqliteConnection) -> AgentToolExecutionRecord {
    agent_tool_execution_records::table
        .select(AgentToolExecutionRecord::as_select())
        .first(conn)
        .expect("Tool Execution Record should load")
}

fn assert_completed_tool(conn: &mut SqliteConnection) {
    let tool = load_tool(conn);
    assert_eq!(tool.state(), Some(AgentToolExecutionState::Completed));
    let outcome = tool
        .complete_outcome()
        .expect("complete outcome should persist");
    assert_eq!(
        outcome.encoding_version(),
        COMPLETE_TOOL_OUTCOME_ENCODING_VERSION
    );
    assert_eq!(outcome.bytes(), [1, 2]);
    let projection = tool
        .tool_result_projection()
        .expect("fixed projection should persist");
    assert_eq!(
        projection.encoding_version(),
        TOOL_RESULT_PROJECTION_ENCODING_VERSION
    );
    assert_eq!(projection.bytes(), [3, 4]);
}

#[test]
fn sqlite_writer_commits_pi_runtime_mutation_and_acknowledges_revision() {
    let task = runtime_task("task-1", "Persist this message");
    let harness = RuntimeWriterHarness::new(0, &[]);

    assert_eq!(
        harness.commit("commit-1", 0, vec![task.clone()], None),
        Ok(1)
    );

    harness.finish(|conn| assert_runtime_conversation(conn, 1, &[task]));
}

#[test]
fn sqlite_writer_rejects_stale_pi_runtime_mutation_without_overwriting_history() {
    let existing_task = runtime_task("task-1", "Keep this message");
    let harness = RuntimeWriterHarness::new(1, std::slice::from_ref(&existing_task));

    assert_eq!(
        harness.commit(
            "stale-commit",
            0,
            vec![runtime_task("task-2", "Do not persist this message")],
            None,
        ),
        Err(CommitAgentRuntimeMutationError::RevisionConflict {
            expected: 0,
            actual: 1,
        })
    );

    harness.finish(|conn| assert_runtime_conversation(conn, 1, &[existing_task]));
}

#[test]
fn sqlite_writer_atomically_commits_tool_outcome_task_and_revision() {
    let task = runtime_task("task-1", "Retain the completed tool outcome");
    let harness = RuntimeWriterHarness::with_executing_tool(0, &[]);

    assert_eq!(
        harness.commit(
            "commit-1",
            0,
            vec![task.clone()],
            Some(complete_tool_execution()),
        ),
        Ok(1)
    );

    harness.finish(|conn| {
        assert_runtime_conversation(conn, 1, &[task]);
        assert_completed_tool(conn);
    });
}

#[test]
fn sqlite_writer_rolls_back_task_tool_outcome_and_revision_when_commit_fails() {
    let harness = RuntimeWriterHarness::build(0, &[], true, |conn| {
        conn.batch_execute(
            "CREATE TRIGGER corrupt_runtime_run_fingerprint \
             AFTER UPDATE OF last_commit_payload_fingerprint ON agent_runtime_runs BEGIN \
             UPDATE agent_runtime_runs \
             SET last_commit_payload_fingerprint = X'01' WHERE id = NEW.id; END",
        )
        .expect("constraint failure trigger should install");
    });

    assert_eq!(
        harness.commit(
            "commit-1",
            0,
            vec![runtime_task("task-1", "Do not partially persist")],
            Some(complete_tool_execution()),
        ),
        Err(CommitAgentRuntimeMutationError::Persistence)
    );

    harness.finish(|conn| {
        assert_runtime_conversation(conn, 0, &[]);
        let tool = load_tool(conn);
        assert_eq!(tool.state(), Some(AgentToolExecutionState::Executing));
        assert_eq!(tool.complete_outcome(), None);
        assert_eq!(tool.tool_result_projection(), None);
    });
}

#[test]
fn sqlite_writer_reports_deleted_conversation_without_partial_runtime_commit() {
    let harness = RuntimeWriterHarness::new(0, &[]);
    harness.delete_conversation();

    assert_eq!(
        harness.commit(
            "commit-after-delete",
            0,
            vec![runtime_task("task-1", "Do not revive this conversation")],
            None,
        ),
        Err(CommitAgentRuntimeMutationError::ConversationNotFound)
    );

    harness.finish(|conn| {
        assert!(read_agent_conversation_by_id(conn, CONVERSATION_ID)
            .expect("conversation lookup should succeed")
            .is_none());
        assert_eq!(
            runs_dsl::agent_runtime_runs
                .filter(runs_dsl::conversation_id.eq(CONVERSATION_ID))
                .count()
                .get_result::<i64>(conn)
                .expect("Agent Run Records should count"),
            0
        );
        assert_eq!(
            tools_dsl::agent_tool_execution_records
                .filter(tools_dsl::conversation_id.eq(CONVERSATION_ID))
                .count()
                .get_result::<i64>(conn)
                .expect("Tool Execution Records should count"),
            0
        );
    });
}

#[test]
fn sqlite_writer_rejects_runtime_commit_after_concurrent_history_edit() {
    let edited_task = runtime_task("task-1", "Edited history");
    let harness = RuntimeWriterHarness::new(0, &[]);
    assert_eq!(
        harness.commit("history-edit", 0, vec![edited_task.clone()], None),
        Ok(1)
    );

    assert_eq!(
        harness.commit(
            "stale-run-output",
            0,
            vec![runtime_task("task-1", "Stale runtime output")],
            None,
        ),
        Err(CommitAgentRuntimeMutationError::RevisionConflict {
            expected: 0,
            actual: 1,
        })
    );

    harness.finish(|conn| assert_runtime_conversation(conn, 1, &[edited_task]));
}

#[test]
fn sqlite_writer_keeps_rust_bound_conversation_update_path_unchanged() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let database_path = tempdir.path().join("warp.sqlite");
    let conn = setup_database(&database_path).expect("database should initialize");
    let writer = start_writer(conn, database_path.clone()).expect("writer should start");
    let task = runtime_task("task-1", "Persist through the Rust runtime path");
    let conversation_data: AgentConversationData =
        serde_json::from_str(r#"{"server_conversation_token":null}"#)
            .expect("minimal conversation data should deserialize");

    writer
        .sender
        .send(ModelEvent::UpdateMultiAgentConversation {
            conversation_id: "conv-rust".to_string(),
            updated_tasks: vec![task.clone()],
            conversation_data,
        })
        .expect("Rust-bound conversation update should send");
    writer
        .sender
        .send(ModelEvent::Terminate)
        .expect("terminate event should send");
    writer.handle.join().expect("writer should terminate");

    let mut conn = setup_database(&database_path).expect("database should reopen");
    let restored = read_agent_conversation_by_id(&mut conn, "conv-rust")
        .expect("conversation should be readable")
        .expect("conversation should be present");
    let data: AgentConversationData =
        serde_json::from_str(&restored.conversation.conversation_data)
            .expect("conversation data should deserialize");
    assert_eq!(data.effective_runtime_binding(), AgentRuntimeBinding::Rust);
    assert_eq!(data.effective_runtime_transcript_revision(), 0);
    assert_eq!(restored.tasks, [task]);
}

#[path = "sqlite_agent_runtime_idempotency_tests.rs"]
mod idempotency_tests;
