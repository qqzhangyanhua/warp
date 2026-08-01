use diesel::Connection;
use diesel_migrations::MigrationHarness;

use super::*;

fn connection() -> diesel::SqliteConnection {
    let mut conn =
        diesel::SqliteConnection::establish(":memory:").expect("in-memory SQLite should open");
    conn.run_pending_migrations(::persistence::MIGRATIONS)
        .expect("migrations should run");
    conn
}

fn record(record_id: &str) -> NewPersonalMemoryRecord {
    NewPersonalMemoryRecord {
        record_id: record_id.to_string(),
        fact_text: "My GitHub account is zyh-work".to_string(),
        value_text: "zyh-work".to_string(),
        topic: "GitHub account".to_string(),
        normalized_topic: "github account".to_string(),
        labels: Vec::new(),
        is_default: false,
        index_state: PersonalMemoryIndexState::Unconfigured,
    }
}

fn command(record: NewPersonalMemoryRecord) -> CreatePersonalMemory {
    let (acknowledgement, _) = futures::channel::oneshot::channel();
    CreatePersonalMemory {
        record,
        acknowledgement,
    }
}

#[test]
fn canonical_record_round_trips_exact_text() {
    let mut conn = connection();
    let created = create_personal_memory(&mut conn, &command(record("memory-1")))
        .expect("record should be created");
    let CreatePersonalMemoryResult::Created(created) = created else {
        panic!("first insert should create a record");
    };

    assert_eq!(created.fact_text, "My GitHub account is zyh-work");
    assert_eq!(created.value_text, "zyh-work");
    assert_eq!(created.record_id, "memory-1");
    assert_eq!(list_personal_memories(&mut conn).unwrap(), vec![created]);
}

#[test]
fn identical_fact_is_idempotent() {
    let mut conn = connection();
    create_personal_memory(&mut conn, &command(record("memory-1"))).unwrap();
    let duplicate = create_personal_memory(&mut conn, &command(record("memory-2"))).unwrap();
    let CreatePersonalMemoryResult::AlreadyRemembered(existing) = duplicate else {
        panic!("identical fact should return the existing record");
    };

    assert_eq!(existing.record_id, "memory-1");
    assert_eq!(list_personal_memories(&mut conn).unwrap().len(), 1);
}

#[test]
fn conversation_rows_are_independent_from_personal_memory() {
    let mut conn = connection();
    create_personal_memory(&mut conn, &command(record("memory-1"))).unwrap();
    diesel::sql_query("DELETE FROM agent_conversations")
        .execute(&mut conn)
        .expect("conversation deletion should succeed");

    assert_eq!(list_personal_memories(&mut conn).unwrap().len(), 1);
}

#[test]
fn disposable_vector_round_trips_only_for_matching_index_identity() {
    let mut conn = connection();
    create_personal_memory(&mut conn, &command(record("memory-1"))).unwrap();
    let identity = "a".repeat(64);
    let (acknowledgement, _) = futures::channel::oneshot::channel();
    upsert_personal_memory_vector(
        &mut conn,
        &UpsertPersonalMemoryVector {
            record_id: "memory-1".into(),
            index_identity: identity.clone(),
            vector: vec![1.0, 0.5],
            acknowledgement,
        },
    )
    .unwrap();
    let (acknowledgement, _) = futures::channel::oneshot::channel();
    let vectors = list_personal_memory_vectors(
        &mut conn,
        &ListPersonalMemoryVectors {
            index_identity: identity,
            acknowledgement,
        },
    )
    .unwrap();

    assert_eq!(vectors.len(), 1);
    assert_eq!(vectors[0].record_id, "memory-1");
    assert_eq!(vectors[0].values, vec![1.0, 0.5]);
    assert_eq!(
        list_personal_memories(&mut conn).unwrap()[0].index_state,
        PersonalMemoryIndexState::Ready
    );
    let (acknowledgement, _) = futures::channel::oneshot::channel();
    assert!(list_personal_memory_vectors(
        &mut conn,
        &ListPersonalMemoryVectors {
            index_identity: "b".repeat(64),
            acknowledgement,
        },
    )
    .unwrap()
    .is_empty());
}
