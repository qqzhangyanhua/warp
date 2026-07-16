use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use diesel::prelude::*;
use futures::channel::oneshot;
use tempfile::TempDir;
use warp_multi_agent_api as api;
use warpui_core::r#async::executor::Background;
use warpui_core::r#async::FutureExt as _;

use super::bridge_process::BridgeProcessError;
use super::configuration::{ChatCompletionsProvider, ReasoningEffort, RunConfiguration};
use super::supervisor::{RuntimeEvent, RuntimeSupervisorConfig, TextRunRequest};
use super::text_run::{prepare_text_run, TextRunOutcome};
use super::transcript::RuntimeTranscript;
use super::{AgentRuntimeLaunchConfig, AgentRuntimeSupervisor, RuntimeError};
use crate::ai::agent::conversation::{AIConversation, AIConversationId};
use crate::persistence::agent::read_agent_conversation_by_id;
use crate::persistence::model::{
    AgentConversationData, AgentRuntimeBinding, AgentRuntimeRunRecord, AgentRuntimeRunState,
    AgentRuntimeTerminalOutcome,
};
use crate::persistence::schema::agent_runtime_runs::dsl as runs_dsl;
use crate::persistence::{
    read_interrupted_agent_message_ids, setup_database, start_writer, upsert_agent_conversation,
    AgentRuntimeRunMutation, ModelEvent, PersistAgentRuntimeRun,
};

pub(super) const CONVERSATION_ID: &str = "018f8a1e-7d2c-7c45-9c3a-6f78f04b3d20";

#[tokio::test]
async fn explicit_retry_resolves_latest_persisted_lineage_before_start() {
    let tempdir = TempDir::new().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let initial_tasks = vec![task_with_user_message()];
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(&mut conn, CONVERSATION_ID, &initial_tasks, runtime_data(0)).unwrap();
    let writer = start_writer(conn, database_path.clone()).unwrap();

    persist_run_for_test(
        &writer.sender,
        "failed-run",
        AgentRuntimeRunMutation::Start {
            retry_of_run_id: None,
            starting_revision: 0,
        },
    )
    .await;
    persist_run_for_test(
        &writer.sender,
        "failed-run",
        AgentRuntimeRunMutation::Finish(AgentRuntimeTerminalOutcome::Failed),
    )
    .await;

    let mut retry = text_run_request(
        "retry-run",
        Some("stale-cached-run"),
        initial_tasks,
        0,
        HashSet::new(),
    )
    .with_retry_lineage_lookup();
    prepare_text_run(&writer.sender, CONVERSATION_ID, &mut retry)
        .await
        .unwrap();

    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();
    let mut conn = setup_database(&database_path).unwrap();
    let retry = runs_dsl::agent_runtime_runs
        .filter(runs_dsl::run_id.eq("retry-run"))
        .select(AgentRuntimeRunRecord::as_select())
        .first::<AgentRuntimeRunRecord>(&mut conn)
        .unwrap();
    assert_eq!(retry.retry_of_run_id.as_deref(), Some("failed-run"));
}

#[tokio::test]
async fn cancellation_before_run_registration_never_starts_bridge_text_run() {
    let tempdir = TempDir::new().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let observer_dir = TempDir::new().unwrap();
    let initial_tasks = vec![task_with_user_message()];
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(&mut conn, CONVERSATION_ID, &initial_tasks, runtime_data(0)).unwrap();
    let writer = start_writer(conn, database_path.clone()).unwrap();
    let supervisor = AgentRuntimeSupervisor::new(
        test_launch_config("text-runs", &observer_dir),
        Arc::new(Background::default()),
    );
    let handle = supervisor.attach(CONVERSATION_ID).await.unwrap();
    let mut request = text_run_request(
        "cancel-before-start",
        None,
        initial_tasks,
        0,
        HashSet::new(),
    );
    prepare_text_run(&writer.sender, CONVERSATION_ID, &mut request)
        .await
        .unwrap();
    let cancellation = Arc::new(AtomicBool::new(false));
    cancellation.store(true, Ordering::Release);

    let result = handle
        .run_text_cancellable(&writer.sender, request, cancellation, |_| {})
        .await
        .unwrap();
    assert_eq!(result.outcome(), &TextRunOutcome::Cancelled);
    assert!(handle.process_is_running().await.unwrap());

    supervisor.shutdown_all().await;
    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();
    let mut conn = setup_database(&database_path).unwrap();
    let run = runs_dsl::agent_runtime_runs
        .filter(runs_dsl::run_id.eq("cancel-before-start"))
        .select(AgentRuntimeRunRecord::as_select())
        .first::<AgentRuntimeRunRecord>(&mut conn)
        .unwrap();
    assert_eq!(
        run.terminal_outcome(),
        Some(AgentRuntimeTerminalOutcome::Cancelled)
    );
}

async fn persist_run_for_test(
    persistence: &std::sync::mpsc::SyncSender<ModelEvent>,
    run_id: &str,
    mutation: AgentRuntimeRunMutation,
) {
    let (acknowledgement, acknowledged) = oneshot::channel();
    persistence
        .send(ModelEvent::PersistAgentRuntimeRun(PersistAgentRuntimeRun {
            conversation_id: CONVERSATION_ID.to_string(),
            run_id: run_id.to_string(),
            mutation,
            acknowledgement,
        }))
        .unwrap();
    acknowledged.await.unwrap().unwrap();
}

#[tokio::test]
async fn active_text_run_can_be_cancelled_without_waiting_for_the_process_lock() {
    let tempdir = TempDir::new().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let observer_dir = TempDir::new().unwrap();
    let initial_tasks = vec![task_with_user_message()];
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(&mut conn, CONVERSATION_ID, &initial_tasks, runtime_data(0)).unwrap();
    let writer = start_writer(conn, database_path.clone()).unwrap();
    let supervisor = AgentRuntimeSupervisor::new(
        test_launch_config("text-run-cancel", &observer_dir),
        Arc::new(Background::default()),
    );
    let handle = supervisor.attach(CONVERSATION_ID).await.unwrap();
    let (running, observed_running) = oneshot::channel();
    let mut running = Some(running);

    let result = async {
        futures::join!(
            handle.run_text(
                &writer.sender,
                text_run_request("run-1", None, initial_tasks, 0, HashSet::new()),
                move |event| {
                    if matches!(event, RuntimeEvent::TextDelta { .. }) {
                        if let Some(running) = running.take() {
                            let _ = running.send(());
                        }
                    }
                },
            ),
            async {
                observed_running.await.unwrap();
                handle.cancel_run().await
            }
        )
    }
    .with_timeout(Duration::from_millis(500))
    .await
    .expect("active text run cancellation must not contend on the process lock");

    let run = result.0.unwrap();
    assert_eq!(run.outcome(), &TextRunOutcome::Cancelled);
    assert_eq!(run.revision(), 1);
    assert_eq!(run.tasks()[0].messages[1].id, "interrupted:run-1");
    result.1.unwrap();
    assert!(handle.process_is_running().await.unwrap());

    supervisor.shutdown_all().await;
    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();
    let mut conn = setup_database(&database_path).unwrap();
    let run = runs_dsl::agent_runtime_runs
        .select(AgentRuntimeRunRecord::as_select())
        .first::<AgentRuntimeRunRecord>(&mut conn)
        .unwrap();
    assert_eq!(
        run.terminal_outcome(),
        Some(AgentRuntimeTerminalOutcome::Cancelled)
    );
}

#[tokio::test]
async fn unresponsive_text_run_is_terminated_after_the_cancellation_grace_period() {
    let tempdir = TempDir::new().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let observer_dir = TempDir::new().unwrap();
    let initial_tasks = vec![task_with_user_message()];
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(&mut conn, CONVERSATION_ID, &initial_tasks, runtime_data(0)).unwrap();
    let writer = start_writer(conn, database_path.clone()).unwrap();
    let supervisor = AgentRuntimeSupervisor::new_with_config(
        test_launch_config("text-run-hang-cancel", &observer_dir),
        RuntimeSupervisorConfig {
            cancellation_grace_period: Duration::from_millis(50),
            ..RuntimeSupervisorConfig::default()
        },
        Arc::new(Background::default()),
    );
    let handle = supervisor.attach(CONVERSATION_ID).await.unwrap();
    let original_process_id = handle.process_id().await.unwrap();
    let (running, observed_running) = oneshot::channel();
    let mut running = Some(running);

    let result = async {
        futures::join!(
            handle.run_text(
                &writer.sender,
                text_run_request("run-1", None, initial_tasks, 0, HashSet::new()),
                move |event| {
                    if matches!(event, RuntimeEvent::TextDelta { .. }) {
                        if let Some(running) = running.take() {
                            let _ = running.send(());
                        }
                    }
                },
            ),
            async {
                observed_running.await.unwrap();
                handle.cancel_run().await
            }
        )
    }
    .with_timeout(Duration::from_secs(1))
    .await
    .expect("cancellation grace period must bound an unresponsive Bridge");

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

#[tokio::test]
async fn text_run_persists_interrupted_output_and_retries_without_duplicate_user_input() {
    let tempdir = TempDir::new().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let observer_dir = TempDir::new().unwrap();
    let initial_tasks = vec![task_with_user_message()];
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(&mut conn, CONVERSATION_ID, &initial_tasks, runtime_data(0)).unwrap();
    let writer = start_writer(conn, database_path.clone()).unwrap();
    let supervisor = AgentRuntimeSupervisor::new(
        test_launch_config("text-runs", &observer_dir),
        Arc::new(Background::default()),
    );
    let handle = supervisor.attach(CONVERSATION_ID).await.unwrap();
    let deltas = Arc::new(Mutex::new(Vec::new()));

    let first = handle
        .run_text(
            &writer.sender,
            text_run_request("run-1", None, initial_tasks, 0, HashSet::new()),
            collect_deltas(deltas.clone()),
        )
        .await
        .unwrap();
    assert!(matches!(first.outcome(), TextRunOutcome::Failed { .. }));
    assert_eq!(first.revision(), 1);

    supervisor.invalidate_checkpoint(CONVERSATION_ID).await;
    let handle = supervisor.attach(CONVERSATION_ID).await.unwrap();
    let interrupted_message_ids = read_interrupted_message_ids(&database_path);

    let second = handle
        .run_text(
            &writer.sender,
            text_run_request(
                "run-2",
                Some("run-1"),
                first.tasks().to_vec(),
                first.revision(),
                interrupted_message_ids,
            ),
            collect_deltas(deltas.clone()),
        )
        .await
        .unwrap();
    assert_eq!(second.outcome(), &TextRunOutcome::Completed);
    assert_eq!(second.revision(), 2);
    assert_eq!(
        *deltas.lock().unwrap(),
        ["Partial output", "Completed output"]
    );

    supervisor.shutdown_all().await;
    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();
    assert_persisted_run_history(&database_path);
    assert_retry_transcript_excludes_interrupted_output(&observer_dir);
}

fn read_interrupted_message_ids(database_path: &Path) -> HashSet<String> {
    let mut conn = setup_database(database_path).unwrap();
    let message_ids = read_interrupted_agent_message_ids(&mut conn, CONVERSATION_ID).unwrap();
    assert_eq!(
        message_ids,
        HashSet::from(["interrupted:run-1".to_string()])
    );
    message_ids
}

#[tokio::test]
async fn unexpected_bridge_exit_after_a_delta_persists_interrupted_output() {
    let tempdir = TempDir::new().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let observer_dir = TempDir::new().unwrap();
    let initial_tasks = vec![task_with_user_message()];
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(&mut conn, CONVERSATION_ID, &initial_tasks, runtime_data(0)).unwrap();
    let writer = start_writer(conn, database_path.clone()).unwrap();
    let supervisor = AgentRuntimeSupervisor::new(
        test_launch_config("text-run-exit", &observer_dir),
        Arc::new(Background::default()),
    );
    let handle = supervisor.attach(CONVERSATION_ID).await.unwrap();

    let result = handle
        .run_text(
            &writer.sender,
            text_run_request("run-1", None, initial_tasks, 0, HashSet::new()),
            |_| {},
        )
        .await;
    assert!(matches!(
        result,
        Err(RuntimeError::Bridge(BridgeProcessError::UnexpectedExit))
    ));

    supervisor.shutdown_all().await;
    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();
    let mut conn = setup_database(&database_path).unwrap();
    let restored = read_agent_conversation_by_id(&mut conn, CONVERSATION_ID)
        .unwrap()
        .unwrap();
    let output = restored.tasks[0].messages[1].message.as_ref().unwrap();
    assert!(matches!(
        output,
        api::message::Message::AgentOutput(output) if output.text == "Partial output"
    ));
    let data: AgentConversationData =
        serde_json::from_str(&restored.conversation.conversation_data).unwrap();
    assert_eq!(data.effective_runtime_transcript_revision(), 1);
    let run = runs_dsl::agent_runtime_runs
        .select(AgentRuntimeRunRecord::as_select())
        .first::<AgentRuntimeRunRecord>(&mut conn)
        .unwrap();
    assert_eq!(
        run.terminal_outcome(),
        Some(AgentRuntimeTerminalOutcome::Failed)
    );
}

pub(super) fn text_run_request(
    run_id: &str,
    retry_of_run_id: Option<&str>,
    tasks: Vec<api::Task>,
    revision: u64,
    interrupted_message_ids: HashSet<String>,
) -> TextRunRequest {
    let conversation = AIConversation::new_restored(
        AIConversationId::try_from(CONVERSATION_ID.to_string()).unwrap(),
        tasks.clone(),
        Some(runtime_data(revision)),
    )
    .unwrap();
    let transcript = RuntimeTranscript::project(
        &conversation,
        revision,
        &interrupted_message_ids,
        &HashMap::new(),
    )
    .unwrap();
    let provider =
        ChatCompletionsProvider::new("https://provider.example/v1", "local-model", "secret-key")
            .unwrap();
    let configuration = RunConfiguration::text_only(
        provider,
        "/workspace",
        32_768,
        ReasoningEffort::Medium,
        Vec::new(),
    )
    .unwrap();
    TextRunRequest::new(
        run_id,
        retry_of_run_id,
        transcript,
        configuration,
        tasks,
        runtime_data(revision),
        "root-task",
    )
}

fn collect_deltas(deltas: Arc<Mutex<Vec<String>>>) -> impl FnMut(RuntimeEvent) + Send + 'static {
    move |event| {
        if let RuntimeEvent::TextDelta { delta, .. } = event {
            deltas.lock().unwrap().push(delta);
        }
    }
}

fn assert_persisted_run_history(database_path: &Path) {
    let mut conn = setup_database(database_path).unwrap();
    let restored = read_agent_conversation_by_id(&mut conn, CONVERSATION_ID)
        .unwrap()
        .unwrap();
    let messages = &restored.tasks[0].messages;
    let user_messages = messages
        .iter()
        .filter(|message| matches!(message.message, Some(api::message::Message::UserQuery(_))))
        .count();
    let assistant_messages = messages
        .iter()
        .filter_map(|message| match message.message.as_ref() {
            Some(api::message::Message::AgentOutput(output)) => {
                Some((message.request_id.as_str(), output.text.as_str()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(user_messages, 1);
    assert_eq!(
        assistant_messages,
        [("run-1", "Partial output"), ("run-2", "Completed output")]
    );

    let data: AgentConversationData =
        serde_json::from_str(&restored.conversation.conversation_data).unwrap();
    assert_eq!(data.effective_runtime_transcript_revision(), 2);
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

fn assert_retry_transcript_excludes_interrupted_output(observer_dir: &TempDir) {
    let transcripts = fs::read_to_string(observer_dir.path().join("accepted-transcripts.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(transcripts.len(), 2);
    assert_eq!(transcripts[1]["revision"], 1);
    assert_eq!(transcripts[1]["items"].as_array().unwrap().len(), 1);
    assert_eq!(transcripts[1]["items"][0]["message_id"], "user-1");
}

pub(super) fn task_with_user_message() -> api::Task {
    api::Task {
        id: "root-task".to_string(),
        messages: vec![api::Message {
            id: "user-1".to_string(),
            task_id: "root-task".to_string(),
            request_id: "request-1".to_string(),
            message: Some(api::message::Message::UserQuery(api::message::UserQuery {
                query: "Inspect the workspace".to_string(),
                ..Default::default()
            })),
            ..Default::default()
        }],
        ..Default::default()
    }
}

pub(super) fn runtime_data(revision: u64) -> AgentConversationData {
    let mut data: AgentConversationData =
        serde_json::from_str(r#"{"server_conversation_token":null}"#).unwrap();
    data.runtime_binding = Some(AgentRuntimeBinding::Pi);
    data.runtime_transcript_revision = Some(revision);
    data
}

pub(super) fn test_launch_config(mode: &str, observer_dir: &TempDir) -> AgentRuntimeLaunchConfig {
    AgentRuntimeLaunchConfig::new(
        node_executable(),
        [
            OsString::from(fake_bridge_path()),
            OsString::from(mode),
            observer_dir.path().as_os_str().to_owned(),
        ],
    )
}

fn fake_bridge_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tools/warp-bridge/test/supervisor-fake-bridge.mjs")
}

fn node_executable() -> PathBuf {
    let executable = if cfg!(windows) { "node.exe" } else { "node" };
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(executable))
        .find(|candidate| is_executable_file(candidate))
        .expect("Node.js must be available for the fake Bridge tests")
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    true
}
