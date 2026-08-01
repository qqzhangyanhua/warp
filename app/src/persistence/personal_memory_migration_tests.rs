use diesel::migration::{Migration, MigrationSource};
use diesel::sql_types::Text;
use diesel::sqlite::Sqlite;
use diesel::{Connection, QueryableByName, RunQueryDsl, SqliteConnection};
use diesel_migrations::MigrationHarness;

const PERSONAL_MEMORY_MIGRATION_VERSION: &str = "20260730000000";

#[derive(QueryableByName)]
struct StoredText {
    #[diesel(sql_type = Text)]
    value: String,
}

fn stored_text(conn: &mut SqliteConnection, query: &'static str) -> String {
    diesel::sql_query(query)
        .get_result::<StoredText>(conn)
        .expect("legacy data should remain readable")
        .value
}

#[test]
fn personal_memory_migration_preserves_conversation_and_terminal_data_and_supports_redo() {
    let mut conn =
        SqliteConnection::establish(":memory:").expect("in-memory sqlite connection should open");
    let migrations: Vec<Box<dyn Migration<Sqlite>>> =
        MigrationSource::migrations(&::persistence::MIGRATIONS)
            .expect("embedded migrations should load");
    let migration_index = migrations
        .iter()
        .position(|migration| {
            migration.name().version().to_string() == PERSONAL_MEMORY_MIGRATION_VERSION
        })
        .expect("the Personal Memory migration should exist");
    let earlier_migrations = &migrations[..migration_index];
    let personal_memory_migration = migrations[migration_index].as_ref();

    conn.applied_migrations()
        .expect("diesel migration metadata should initialize");
    conn.run_migrations(earlier_migrations)
        .expect("pre-Personal Memory migrations should run");
    diesel::sql_query(
        "INSERT INTO agent_conversations (conversation_id, conversation_data) \
         VALUES ('legacy-conversation', '{\"runtime\":\"legacy\"}')",
    )
    .execute(&mut conn)
    .expect("legacy Conversation Record should insert");
    diesel::sql_query("INSERT INTO commands (id, command) VALUES (42, 'echo legacy-terminal')")
        .execute(&mut conn)
        .expect("legacy terminal command should insert");

    conn.run_migration(personal_memory_migration)
        .expect("Personal Memory migration should run");

    assert_eq!(
        stored_text(
            &mut conn,
            "SELECT conversation_data AS value FROM agent_conversations \
             WHERE conversation_id = 'legacy-conversation'",
        ),
        "{\"runtime\":\"legacy\"}"
    );
    assert_eq!(
        stored_text(
            &mut conn,
            "SELECT command AS value FROM commands WHERE id = 42",
        ),
        "echo legacy-terminal"
    );
    assert!(
        conn.pending_migrations(::persistence::MIGRATIONS)
            .expect("pending migrations should load")
            .iter()
            .all(|migration| {
                migration.name().version().to_string() != PERSONAL_MEMORY_MIGRATION_VERSION
            }),
        "a second startup must not rerun the Personal Memory migration"
    );

    conn.revert_migration(personal_memory_migration)
        .expect("Personal Memory migration should revert on a disposable database");
    assert_eq!(
        stored_text(
            &mut conn,
            "SELECT command AS value FROM commands WHERE id = 42",
        ),
        "echo legacy-terminal"
    );
    conn.run_migration(personal_memory_migration)
        .expect("Personal Memory migration should apply again after revert");
    assert_eq!(
        stored_text(
            &mut conn,
            "SELECT conversation_data AS value FROM agent_conversations \
             WHERE conversation_id = 'legacy-conversation'",
        ),
        "{\"runtime\":\"legacy\"}"
    );
}
