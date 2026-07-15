use diesel::sql_types::Text;
use diesel_migrations::MigrationHarness;

use super::*;

fn test_connection() -> SqliteConnection {
    let mut conn =
        SqliteConnection::establish(":memory:").expect("in-memory sqlite connection should open");
    conn.run_pending_migrations(::persistence::MIGRATIONS)
        .expect("migrations should run");
    conn
}

fn task_with_user_query(task_id: &str, query: &str, description: &str) -> api::Task {
    api::Task {
        id: task_id.to_string(),
        description: description.to_string(),
        dependencies: None,
        messages: vec![api::Message {
            id: format!("{task_id}-user-query"),
            task_id: task_id.to_string(),
            message: Some(api::message::Message::UserQuery(api::message::UserQuery {
                query: query.to_string(),
                ..Default::default()
            })),
            ..Default::default()
        }],
        summary: String::new(),
        server_data: String::new(),
    }
}

fn empty_conversation_data() -> AgentConversationData {
    serde_json::from_str(r#"{"server_conversation_token":null}"#)
        .expect("minimal conversation data should deserialize")
}

#[test]
fn legacy_shaped_upsert_preserves_existing_runtime_metadata() {
    let mut conn = test_connection();
    let mut pi_data = empty_conversation_data();
    pi_data.runtime_binding = Some(::persistence::model::AgentRuntimeBinding::Pi);
    pi_data.runtime_transcript_revision = Some(7);
    upsert_agent_conversation(&mut conn, "conv-1", [], pi_data)
        .expect("Pi-bound conversation should insert");

    upsert_agent_conversation(&mut conn, "conv-1", [], empty_conversation_data())
        .expect("legacy-shaped update should succeed");

    let restored = read_agent_conversation_by_id(&mut conn, "conv-1")
        .expect("conversation should be readable")
        .expect("conversation should exist");
    let data: AgentConversationData =
        serde_json::from_str(&restored.conversation.conversation_data)
            .expect("conversation data should deserialize");
    assert_eq!(
        data.effective_runtime_binding(),
        ::persistence::model::AgentRuntimeBinding::Pi
    );
    assert_eq!(data.effective_runtime_transcript_revision(), 7);
}

#[test]
fn upsert_rejects_runtime_binding_change() {
    let mut conn = test_connection();
    let mut pi_data = empty_conversation_data();
    pi_data.runtime_binding = Some(::persistence::model::AgentRuntimeBinding::Pi);
    pi_data.runtime_transcript_revision = Some(7);
    upsert_agent_conversation(&mut conn, "conv-1", [], pi_data)
        .expect("Pi-bound conversation should insert");

    let mut rust_data = empty_conversation_data();
    rust_data.runtime_binding = Some(::persistence::model::AgentRuntimeBinding::Rust);
    rust_data.runtime_transcript_revision = Some(7);
    assert!(matches!(
        upsert_agent_conversation(&mut conn, "conv-1", [], rust_data),
        Err(UpsertConversationError::RuntimeBindingConflict)
    ));

    let restored = read_agent_conversation_by_id(&mut conn, "conv-1")
        .expect("conversation should be readable")
        .expect("conversation should exist");
    let data: AgentConversationData =
        serde_json::from_str(&restored.conversation.conversation_data)
            .expect("conversation data should deserialize");
    assert_eq!(
        data.effective_runtime_binding(),
        ::persistence::model::AgentRuntimeBinding::Pi
    );
}

#[test]
fn upsert_rejects_legacy_or_explicit_rust_binding_change_to_pi() {
    for existing_binding in [None, Some(::persistence::model::AgentRuntimeBinding::Rust)] {
        let mut conn = test_connection();
        let mut existing_data = empty_conversation_data();
        existing_data.runtime_binding = existing_binding;
        upsert_agent_conversation(&mut conn, "conv-1", [], existing_data)
            .expect("Rust-bound conversation should insert");

        let mut pi_data = empty_conversation_data();
        pi_data.runtime_binding = Some(::persistence::model::AgentRuntimeBinding::Pi);
        assert!(matches!(
            upsert_agent_conversation(&mut conn, "conv-1", [], pi_data),
            Err(UpsertConversationError::RuntimeBindingConflict)
        ));

        let restored = read_agent_conversation_by_id(&mut conn, "conv-1")
            .expect("conversation should be readable")
            .expect("conversation should exist");
        let data: AgentConversationData =
            serde_json::from_str(&restored.conversation.conversation_data)
                .expect("conversation data should deserialize");
        assert_eq!(
            data.effective_runtime_binding(),
            ::persistence::model::AgentRuntimeBinding::Rust
        );
    }
}

#[test]
fn runtime_sidecar_models_load_typed_states_and_versioned_payloads() {
    use ::persistence::model::{
        AgentRuntimeRunRecord, AgentRuntimeRunState, AgentToolExecutionRecord,
        AgentToolExecutionState, CompleteAgentToolExecution, NewAgentRuntimeRunRecord,
        NewAgentToolExecutionRecord, VersionedCompleteToolOutcome, VersionedToolRequest,
        VersionedToolResultProjection, COMPLETE_TOOL_OUTCOME_ENCODING_VERSION,
        TOOL_RESULT_PROJECTION_ENCODING_VERSION,
    };
    use schema::agent_runtime_runs::dsl as runs_dsl;
    use schema::agent_tool_execution_records::dsl as tools_dsl;

    let mut conn = test_connection();
    upsert_agent_conversation(&mut conn, "conv-1", [], empty_conversation_data())
        .expect("conversation should insert");
    diesel::insert_into(agent_runtime_runs::table)
        .values(NewAgentRuntimeRunRecord::starting(
            "conv-1", "run-1", None, 7,
        ))
        .execute(&mut conn)
        .expect("runtime run should insert");
    let request_fingerprint = [0_u8; 32];
    diesel::insert_into(agent_tool_execution_records::table)
        .values(NewAgentToolExecutionRecord::pending(
            "conv-1",
            "run-1",
            "call-1",
            &request_fingerprint,
            VersionedToolRequest::current(b"request"),
        ))
        .execute(&mut conn)
        .expect("tool execution record should insert");
    diesel::update(
        tools_dsl::agent_tool_execution_records.filter(tools_dsl::tool_call_id.eq("call-1")),
    )
    .set(CompleteAgentToolExecution::new(
        VersionedCompleteToolOutcome::current(&[1, 2]),
        VersionedToolResultProjection::current(&[3, 4]),
    ))
    .execute(&mut conn)
    .expect("tool execution record should complete");

    let run = runs_dsl::agent_runtime_runs
        .select(AgentRuntimeRunRecord::as_select())
        .first::<AgentRuntimeRunRecord>(&mut conn)
        .expect("runtime run should load");
    assert_eq!(run.state(), Some(AgentRuntimeRunState::Starting));
    assert_eq!(run.starting_revision, 7);

    let tool = tools_dsl::agent_tool_execution_records
        .select(AgentToolExecutionRecord::as_select())
        .first::<AgentToolExecutionRecord>(&mut conn)
        .expect("tool execution record should load");
    assert_eq!(tool.state(), Some(AgentToolExecutionState::Completed));
    assert_eq!(tool.request_fingerprint.len(), 32);
    assert_eq!(tool.tool_request().unwrap().bytes(), b"request");
    let complete_outcome = tool
        .complete_outcome()
        .expect("completed records have a complete outcome");
    assert_eq!(
        complete_outcome.encoding_version(),
        COMPLETE_TOOL_OUTCOME_ENCODING_VERSION
    );
    assert_eq!(complete_outcome.bytes(), [1, 2]);
    let projection = tool
        .tool_result_projection()
        .expect("completed records have a fixed projection");
    assert_eq!(
        projection.encoding_version(),
        TOOL_RESULT_PROJECTION_ENCODING_VERSION
    );
    assert_eq!(projection.bytes(), [3, 4]);
}

#[test]
fn runtime_payload_types_reject_unknown_encoding_versions() {
    use ::persistence::model::{
        VersionedCompleteToolOutcome, VersionedToolRequest, VersionedToolResultProjection,
        COMPLETE_TOOL_OUTCOME_ENCODING_VERSION, TOOL_REQUEST_ENCODING_VERSION,
        TOOL_RESULT_PROJECTION_ENCODING_VERSION,
    };

    assert_eq!(
        VersionedCompleteToolOutcome::from_parts(COMPLETE_TOOL_OUTCOME_ENCODING_VERSION + 1, &[],),
        None
    );
    assert_eq!(
        VersionedToolResultProjection::from_parts(TOOL_RESULT_PROJECTION_ENCODING_VERSION + 1, &[],),
        None
    );
    assert_eq!(
        VersionedToolRequest::from_parts(TOOL_REQUEST_ENCODING_VERSION + 1, &[]),
        None
    );
}

#[test]
fn runtime_sidecar_foreign_keys_reject_orphans() {
    let mut conn = test_connection();
    enable_foreign_keys(&mut conn);

    assert!(diesel::sql_query(
        "INSERT INTO agent_runtime_runs \
         (conversation_id, run_id, starting_revision, state) \
         VALUES ('missing-conversation', 'run-1', 0, 'starting')",
    )
    .execute(&mut conn)
    .is_err());

    upsert_agent_conversation(&mut conn, "conv-1", [], empty_conversation_data())
        .expect("conversation should insert");
    assert!(diesel::sql_query(
        "INSERT INTO agent_runtime_runs \
         (conversation_id, run_id, retry_of_run_id, starting_revision, state) \
         VALUES ('conv-1', 'run-2', 'missing-run', 0, 'starting')",
    )
    .execute(&mut conn)
    .is_err());
    assert!(diesel::sql_query(
        "INSERT INTO agent_tool_execution_records \
         (conversation_id, run_id, tool_call_id, request_fingerprint, state) \
         VALUES ('conv-1', 'missing-run', 'call-1', zeroblob(32), 'pending')",
    )
    .execute(&mut conn)
    .is_err());
}

#[test]
fn runtime_sidecar_unique_identities_are_enforced() {
    let mut conn = test_connection();
    upsert_agent_conversation(&mut conn, "conv-1", [], empty_conversation_data())
        .expect("conversation should insert");
    diesel::sql_query(
        "INSERT INTO agent_runtime_runs \
         (conversation_id, run_id, starting_revision, state) \
         VALUES ('conv-1', 'run-1', 0, 'starting')",
    )
    .execute(&mut conn)
    .expect("runtime run should insert");
    assert!(diesel::sql_query(
        "INSERT INTO agent_runtime_runs \
         (conversation_id, run_id, starting_revision, state) \
         VALUES ('conv-1', 'run-1', 0, 'starting')",
    )
    .execute(&mut conn)
    .is_err());

    diesel::sql_query(
        "INSERT INTO agent_tool_execution_records \
         (conversation_id, run_id, tool_call_id, request_fingerprint, state) \
         VALUES ('conv-1', 'run-1', 'call-1', zeroblob(32), 'pending')",
    )
    .execute(&mut conn)
    .expect("tool execution record should insert");
    assert!(diesel::sql_query(
        "INSERT INTO agent_tool_execution_records \
         (conversation_id, run_id, tool_call_id, request_fingerprint, state) \
         VALUES ('conv-1', 'run-1', 'call-1', zeroblob(32), 'pending')",
    )
    .execute(&mut conn)
    .is_err());
}

#[test]
fn runtime_sidecar_state_constraints_reject_incomplete_or_mixed_outcomes() {
    let mut conn = test_connection();
    upsert_agent_conversation(&mut conn, "conv-1", [], empty_conversation_data())
        .expect("conversation should insert");

    for invalid_run in [
        "INSERT INTO agent_runtime_runs \
         (conversation_id, run_id, starting_revision, state) \
         VALUES ('conv-1', 'negative-revision', -1, 'starting')",
        "INSERT INTO agent_runtime_runs \
         (conversation_id, run_id, starting_revision, state) \
         VALUES ('conv-1', 'unfinished', 0, 'finished')",
        "INSERT INTO agent_runtime_runs \
         (conversation_id, run_id, starting_revision, state, terminal_outcome) \
         VALUES ('conv-1', 'active', 0, 'running', 'failed')",
    ] {
        assert!(diesel::sql_query(invalid_run).execute(&mut conn).is_err());
    }

    diesel::sql_query(
        "INSERT INTO agent_runtime_runs \
         (conversation_id, run_id, starting_revision, state) \
         VALUES ('conv-1', 'run-1', 0, 'running')",
    )
    .execute(&mut conn)
    .expect("runtime run should insert");
    for invalid_tool in [
        "INSERT INTO agent_tool_execution_records \
         (conversation_id, run_id, tool_call_id, request_fingerprint, state) \
         VALUES ('conv-1', 'run-1', 'short-fingerprint', zeroblob(31), 'pending')",
        "INSERT INTO agent_tool_execution_records \
         (conversation_id, run_id, tool_call_id, request_fingerprint, state) \
         VALUES ('conv-1', 'run-1', 'incomplete', zeroblob(32), 'completed')",
        "INSERT INTO agent_tool_execution_records \
         (conversation_id, run_id, tool_call_id, request_fingerprint, state, \
          complete_outcome_encoding_version, complete_outcome, \
          tool_result_projection_encoding_version, tool_result_projection) \
         VALUES ('conv-1', 'run-1', 'premature', zeroblob(32), 'pending', \
                 1, x'01', 1, x'02')",
        "INSERT INTO agent_tool_execution_records \
         (conversation_id, run_id, tool_call_id, request_fingerprint, state, \
          complete_outcome_encoding_version, complete_outcome, \
          tool_result_projection_encoding_version, tool_result_projection) \
         VALUES ('conv-1', 'run-1', 'invalid-version', zeroblob(32), 'completed', \
                 0, x'01', 1, x'02')",
    ] {
        assert!(diesel::sql_query(invalid_tool).execute(&mut conn).is_err());
    }
}

fn enable_foreign_keys(conn: &mut SqliteConnection) {
    diesel::sql_query("PRAGMA foreign_keys = ON")
        .execute(conn)
        .expect("foreign keys should enable");
}

fn insert_runtime_sidecar_chain(conn: &mut SqliteConnection, conversation: &str) {
    diesel::sql_query(
        "INSERT INTO agent_runtime_runs \
         (conversation_id, run_id, starting_revision, state) \
         VALUES (?, 'run-1', 0, 'running')",
    )
    .bind::<Text, _>(conversation)
    .execute(conn)
    .expect("runtime run should insert");
    diesel::sql_query(
        "INSERT INTO agent_tool_execution_records \
         (conversation_id, run_id, tool_call_id, request_fingerprint, state) \
         VALUES (?, 'run-1', 'call-1', zeroblob(32), 'pending')",
    )
    .bind::<Text, _>(conversation)
    .execute(conn)
    .expect("tool execution record should insert");
}

fn runtime_sidecar_counts(conn: &mut SqliteConnection, conversation: &str) -> (i64, i64) {
    use schema::agent_runtime_runs::dsl as runs_dsl;
    use schema::agent_tool_execution_records::dsl as tools_dsl;

    let runs = runs_dsl::agent_runtime_runs
        .filter(runs_dsl::conversation_id.eq(conversation))
        .count()
        .get_result(conn)
        .expect("runtime runs should count");
    let tools = tools_dsl::agent_tool_execution_records
        .filter(tools_dsl::conversation_id.eq(conversation))
        .count()
        .get_result(conn)
        .expect("tool records should count");
    (runs, tools)
}

#[test]
fn deleting_conversation_removes_runtime_sidecars_before_tasks() {
    use schema::agent_conversations::dsl as conversations_dsl;
    use schema::agent_tasks::dsl as tasks_dsl;

    let mut conn = test_connection();
    enable_foreign_keys(&mut conn);
    let task = task_with_user_query("task-1", "Question", "Title");
    upsert_agent_conversation(&mut conn, "conv-1", [&task], empty_conversation_data())
        .expect("conversation should insert");
    insert_runtime_sidecar_chain(&mut conn, "conv-1");

    delete_agent_conversations(&mut conn, vec!["conv-1".to_string()])
        .expect("conversation and sidecars should delete");

    assert_eq!(runtime_sidecar_counts(&mut conn, "conv-1"), (0, 0));
    assert_eq!(
        tasks_dsl::agent_tasks
            .filter(tasks_dsl::conversation_id.eq("conv-1"))
            .count()
            .get_result::<i64>(&mut conn)
            .expect("tasks should count"),
        0
    );
    assert_eq!(
        conversations_dsl::agent_conversations
            .filter(conversations_dsl::conversation_id.eq("conv-1"))
            .count()
            .get_result::<i64>(&mut conn)
            .expect("conversations should count"),
        0
    );
}

#[test]
fn failed_conversation_delete_rolls_back_sidecars_and_tasks() {
    use schema::agent_conversations::dsl as conversations_dsl;
    use schema::agent_tasks::dsl as tasks_dsl;

    let mut conn = test_connection();
    enable_foreign_keys(&mut conn);
    let task = task_with_user_query("task-1", "Question", "Title");
    upsert_agent_conversation(&mut conn, "conv-1", [&task], empty_conversation_data())
        .expect("conversation should insert");
    insert_runtime_sidecar_chain(&mut conn, "conv-1");
    diesel::sql_query(
        "CREATE TRIGGER reject_test_conversation_delete \
         BEFORE DELETE ON agent_conversations BEGIN \
         SELECT RAISE(ABORT, 'injected delete failure'); END",
    )
    .execute(&mut conn)
    .expect("failure trigger should install");

    assert!(delete_agent_conversations(&mut conn, vec!["conv-1".to_string()]).is_err());

    assert_eq!(runtime_sidecar_counts(&mut conn, "conv-1"), (1, 1));
    assert_eq!(
        conversations_dsl::agent_conversations
            .filter(conversations_dsl::conversation_id.eq("conv-1"))
            .count()
            .get_result::<i64>(&mut conn)
            .expect("conversations should count"),
        1
    );
    assert_eq!(
        tasks_dsl::agent_tasks
            .filter(tasks_dsl::conversation_id.eq("conv-1"))
            .count()
            .get_result::<i64>(&mut conn)
            .expect("tasks should count"),
        1
    );
}

#[test]
fn retention_eviction_removes_runtime_sidecars() {
    let mut conn = test_connection();
    enable_foreign_keys(&mut conn);
    diesel::sql_query(
        "INSERT INTO agent_conversations \
         (conversation_id, conversation_data, last_modified_at) \
         VALUES ('conv-sidecar', '{\"server_conversation_token\":null}', \
                 '2000-01-01 00:00:00')",
    )
    .execute(&mut conn)
    .expect("old conversation should insert");
    insert_runtime_sidecar_chain(&mut conn, "conv-sidecar");
    for index in 0..(MAX_PERSISTED_CONVERSATION_COUNT - 1) {
        diesel::sql_query(
            "INSERT INTO agent_conversations (conversation_id, conversation_data) VALUES (?, ?)",
        )
        .bind::<Text, _>(format!("conv-{index:03}"))
        .bind::<Text, _>(r#"{"server_conversation_token":null}"#)
        .execute(&mut conn)
        .expect("retained conversation should insert");
    }

    upsert_agent_conversation(&mut conn, "conv-new", [], empty_conversation_data())
        .expect("new conversation should insert and evict the oldest row");

    assert_eq!(runtime_sidecar_counts(&mut conn, "conv-sidecar"), (0, 0));
    assert!(read_agent_conversation_by_id(&mut conn, "conv-sidecar")
        .expect("evicted conversation lookup should succeed")
        .is_none());
}
