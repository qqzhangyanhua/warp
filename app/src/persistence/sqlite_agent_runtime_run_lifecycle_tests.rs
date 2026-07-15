use diesel::prelude::*;
use futures::channel::oneshot;

use super::{setup_database, start_writer, upsert_agent_conversation};
use crate::persistence::model::{
    AgentConversationData, AgentRuntimeBinding, AgentRuntimeRunRecord, AgentRuntimeRunState,
    AgentRuntimeTerminalOutcome,
};
use crate::persistence::schema::agent_runtime_runs::dsl as runs_dsl;
use crate::persistence::{
    AgentRuntimeRunMutation, ModelEvent, PersistAgentRuntimeRun, PersistAgentRuntimeRunError,
};

#[test]
fn sqlite_writer_persists_run_lifecycle_and_retry_lineage() {
    let tempdir = tempfile::tempdir().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(&mut conn, "conversation-1", [], runtime_data(7)).unwrap();
    let writer = start_writer(conn, database_path.clone()).unwrap();

    assert_eq!(
        persist(
            &writer.sender,
            "run-1",
            AgentRuntimeRunMutation::Start {
                retry_of_run_id: None,
                starting_revision: 7,
            },
        ),
        Ok(())
    );
    assert_eq!(
        persist(
            &writer.sender,
            "run-1",
            AgentRuntimeRunMutation::SetState(AgentRuntimeRunState::Running),
        ),
        Ok(())
    );
    assert_eq!(
        persist(
            &writer.sender,
            "run-1",
            AgentRuntimeRunMutation::Finish(AgentRuntimeTerminalOutcome::Failed),
        ),
        Ok(())
    );
    assert_eq!(
        persist(
            &writer.sender,
            "run-2",
            AgentRuntimeRunMutation::Start {
                retry_of_run_id: Some("run-1".to_string()),
                starting_revision: 7,
            },
        ),
        Ok(())
    );
    assert_eq!(
        persist(
            &writer.sender,
            "run-2",
            AgentRuntimeRunMutation::Finish(AgentRuntimeTerminalOutcome::Completed),
        ),
        Ok(())
    );

    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();
    let mut conn = setup_database(&database_path).unwrap();
    let runs = runs_dsl::agent_runtime_runs
        .order(runs_dsl::id)
        .select(AgentRuntimeRunRecord::as_select())
        .load::<AgentRuntimeRunRecord>(&mut conn)
        .unwrap();

    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].state(), Some(AgentRuntimeRunState::Finished));
    assert_eq!(
        runs[0].terminal_outcome(),
        Some(AgentRuntimeTerminalOutcome::Failed)
    );
    assert_eq!(runs[1].retry_of_run_id.as_deref(), Some("run-1"));
    assert_eq!(
        runs[1].terminal_outcome(),
        Some(AgentRuntimeTerminalOutcome::Completed)
    );
}

#[test]
fn sqlite_writer_rejects_a_run_created_from_a_stale_revision() {
    let tempdir = tempfile::tempdir().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(&mut conn, "conversation-1", [], runtime_data(7)).unwrap();
    let writer = start_writer(conn, database_path.clone()).unwrap();

    assert_eq!(
        persist(
            &writer.sender,
            "stale-run",
            AgentRuntimeRunMutation::Start {
                retry_of_run_id: None,
                starting_revision: 6,
            },
        ),
        Err(PersistAgentRuntimeRunError::RevisionConflict {
            expected: 6,
            actual: 7,
        })
    );

    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();
    let mut conn = setup_database(&database_path).unwrap();
    assert_eq!(
        runs_dsl::agent_runtime_runs
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap(),
        0
    );
}

fn persist(
    sender: &std::sync::mpsc::SyncSender<ModelEvent>,
    run_id: &str,
    mutation: AgentRuntimeRunMutation,
) -> Result<(), PersistAgentRuntimeRunError> {
    let (acknowledgement, acknowledged) = oneshot::channel();
    sender
        .send(ModelEvent::PersistAgentRuntimeRun(PersistAgentRuntimeRun {
            conversation_id: "conversation-1".to_string(),
            run_id: run_id.to_string(),
            mutation,
            acknowledgement,
        }))
        .unwrap();
    futures::executor::block_on(acknowledged).unwrap()
}

fn runtime_data(revision: u64) -> AgentConversationData {
    let mut data: AgentConversationData =
        serde_json::from_str(r#"{"server_conversation_token":null}"#).unwrap();
    data.runtime_binding = Some(AgentRuntimeBinding::Pi);
    data.runtime_transcript_revision = Some(revision);
    data
}
