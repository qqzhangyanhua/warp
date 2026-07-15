use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;
use warpui_core::r#async::executor::Background;
use warpui_core::r#async::{FutureExt as _, Timer};

use super::bridge_process::BridgeProcessError;
use super::supervisor::RuntimeSupervisorConfig;
use super::text_run_integration_tests::{
    runtime_data, task_with_user_message, test_launch_config, text_run_request, CONVERSATION_ID,
};
use super::{AgentRuntimeSupervisor, RuntimeError};
use crate::persistence::{setup_database, start_writer, upsert_agent_conversation, ModelEvent};

#[tokio::test]
async fn cancellation_grace_period_bounds_a_hung_transcript_sync() {
    let tempdir = TempDir::new().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let observer_dir = TempDir::new().unwrap();
    let initial_tasks = vec![task_with_user_message()];
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(&mut conn, CONVERSATION_ID, &initial_tasks, runtime_data(0)).unwrap();
    let writer = start_writer(conn, database_path.clone()).unwrap();
    let supervisor = AgentRuntimeSupervisor::new_with_config(
        test_launch_config("text-run-hang-sync", &observer_dir),
        RuntimeSupervisorConfig {
            cancellation_grace_period: Duration::from_millis(50),
            ..RuntimeSupervisorConfig::default()
        },
        Arc::new(Background::default()),
    );
    let handle = supervisor.attach(CONVERSATION_ID).await.unwrap();
    let original_process_id = handle.process_id().await.unwrap();

    let result = async {
        futures::join!(
            handle.run_text(
                &writer.sender,
                text_run_request("run-1", None, initial_tasks, 0, HashSet::new()),
                |_| {},
            ),
            async {
                Timer::after(Duration::from_millis(50)).await;
                handle.cancel_run().await
            }
        )
    }
    .with_timeout(Duration::from_secs(1))
    .await
    .expect("cancellation must interrupt a hung Transcript Sync");

    assert!(matches!(
        result.0,
        Err(RuntimeError::Bridge(
            BridgeProcessError::CancellationTimedOut
        ))
    ));
    assert!(matches!(
        result.1,
        Err(RuntimeError::Bridge(
            BridgeProcessError::CancellationTimedOut
        ))
    ));
    let rebuilt = supervisor.attach(CONVERSATION_ID).await.unwrap();
    assert_ne!(rebuilt.process_id().await.unwrap(), original_process_id);

    supervisor.shutdown_all().await;
    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();
}
