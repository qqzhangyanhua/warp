use diesel::migration::{Migration, MigrationSource};
use diesel::sql_types::{Binary, Nullable, Text};
use diesel::sqlite::Sqlite;
use diesel_migrations::MigrationHarness;

use super::*;

const RUNTIME_MIGRATION_VERSION: &str = "20260713000000";
const COMMIT_FINGERPRINT_MIGRATION_VERSION: &str = "20260714000000";

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

#[derive(QueryableByName)]
struct SqliteTable {
    #[diesel(sql_type = Text)]
    name: String,
}

#[derive(QueryableByName)]
struct RuntimeCommitFingerprint {
    #[diesel(sql_type = Nullable<Binary>)]
    fingerprint: Option<Vec<u8>>,
}

fn runtime_sidecar_table_names(conn: &mut SqliteConnection) -> Vec<String> {
    diesel::sql_query(
        "SELECT name FROM sqlite_master \
         WHERE type = 'table' \
         AND name IN ('agent_runtime_runs', 'agent_tool_execution_records') \
         ORDER BY name",
    )
    .load::<SqliteTable>(conn)
    .expect("sqlite schema should be readable")
    .into_iter()
    .map(|table| table.name)
    .collect()
}

fn legacy_run_commit_fingerprint(conn: &mut SqliteConnection) -> Option<Vec<u8>> {
    diesel::sql_query(
        "SELECT last_commit_payload_fingerprint AS fingerprint \
         FROM agent_runtime_runs WHERE run_id = 'legacy-run'",
    )
    .get_result::<RuntimeCommitFingerprint>(conn)
    .expect("legacy Agent Run Record should remain readable")
    .fingerprint
}

#[test]
fn runtime_migrations_preserve_legacy_records_and_support_redo() {
    let mut conn =
        SqliteConnection::establish(":memory:").expect("in-memory sqlite connection should open");
    let migrations: Vec<Box<dyn Migration<Sqlite>>> =
        MigrationSource::migrations(&::persistence::MIGRATIONS)
            .expect("embedded migrations should load");
    let runtime_migration_index = migrations
        .iter()
        .position(|migration| migration.name().version().to_string() == RUNTIME_MIGRATION_VERSION)
        .expect("the runtime migration should exist");
    let fingerprint_migration_index = migrations
        .iter()
        .position(|migration| {
            migration.name().version().to_string() == COMMIT_FINGERPRINT_MIGRATION_VERSION
        })
        .expect("the commit fingerprint migration should exist");
    let earlier_migrations = &migrations[..runtime_migration_index];
    let runtime_migration = migrations[runtime_migration_index].as_ref();
    let fingerprint_migration = migrations[fingerprint_migration_index].as_ref();

    conn.applied_migrations()
        .expect("diesel migration metadata should initialize");
    conn.run_migrations(earlier_migrations)
        .expect("pre-runtime migrations should run");
    diesel::sql_query(
        "INSERT INTO agent_conversations (conversation_id, conversation_data) \
         VALUES ('legacy-conversation', '{\"server_conversation_token\":null}')",
    )
    .execute(&mut conn)
    .expect("legacy conversation fixture should insert");
    let legacy_task = task_with_user_query("legacy-task", "Legacy query", "Legacy title");
    diesel::sql_query("INSERT INTO agent_tasks (conversation_id, task_id, task) VALUES (?, ?, ?)")
        .bind::<Text, _>("legacy-conversation")
        .bind::<Text, _>(&legacy_task.id)
        .bind::<Binary, _>(legacy_task.encode_to_vec())
        .execute(&mut conn)
        .expect("legacy task fixture should insert");

    conn.run_migration(runtime_migration)
        .expect("runtime sidecar migration should run");

    let restored = read_agent_conversation_by_id(&mut conn, "legacy-conversation")
        .expect("legacy conversation should remain readable")
        .expect("legacy conversation should still exist");
    let data: AgentConversationData =
        serde_json::from_str(&restored.conversation.conversation_data)
            .expect("legacy conversation data should deserialize");
    assert_eq!(
        data.effective_runtime_binding(),
        ::persistence::model::AgentRuntimeBinding::Rust
    );
    assert_eq!(data.effective_runtime_transcript_revision(), 0);
    assert_eq!(restored.tasks, [legacy_task]);
    assert_eq!(
        runtime_sidecar_table_names(&mut conn),
        ["agent_runtime_runs", "agent_tool_execution_records"]
    );
    let pending_versions = conn
        .pending_migrations(::persistence::MIGRATIONS)
        .expect("pending migrations should load")
        .into_iter()
        .map(|migration| migration.name().version().to_string())
        .collect::<Vec<_>>();
    assert!(
        !pending_versions
            .iter()
            .any(|version| version == RUNTIME_MIGRATION_VERSION),
        "a second startup must not rerun the runtime migration"
    );

    diesel::sql_query(
        "INSERT INTO agent_runtime_runs \
         (conversation_id, run_id, starting_revision, state) \
         VALUES ('legacy-conversation', 'legacy-run', 0, 'running')",
    )
    .execute(&mut conn)
    .expect("legacy Agent Run Record fixture should insert");
    conn.run_migration(fingerprint_migration)
        .expect("commit fingerprint migration should run");
    assert_eq!(legacy_run_commit_fingerprint(&mut conn), None);
    let pending_versions = conn
        .pending_migrations(::persistence::MIGRATIONS)
        .expect("pending migrations should load after the fingerprint migration")
        .into_iter()
        .map(|migration| migration.name().version().to_string())
        .collect::<Vec<_>>();
    assert!(
        !pending_versions.iter().any(|version| {
            version == RUNTIME_MIGRATION_VERSION || version == COMMIT_FINGERPRINT_MIGRATION_VERSION
        }),
        "a second startup must not rerun applied runtime migrations"
    );

    conn.revert_migration(fingerprint_migration)
        .expect("commit fingerprint migration should revert on a disposable database");
    conn.run_migration(fingerprint_migration)
        .expect("commit fingerprint migration should apply again after revert");
    assert_eq!(legacy_run_commit_fingerprint(&mut conn), None);

    conn.revert_migration(fingerprint_migration)
        .expect("commit fingerprint migration should revert before the sidecar migration");

    conn.revert_migration(runtime_migration)
        .expect("runtime migration should revert on a disposable database");
    assert!(runtime_sidecar_table_names(&mut conn).is_empty());
    conn.run_migration(runtime_migration)
        .expect("runtime migration should apply again after revert");
    conn.run_migration(fingerprint_migration)
        .expect("commit fingerprint migration should apply after sidecar migration redo");
    assert_eq!(
        runtime_sidecar_table_names(&mut conn),
        ["agent_runtime_runs", "agent_tool_execution_records"]
    );
}
