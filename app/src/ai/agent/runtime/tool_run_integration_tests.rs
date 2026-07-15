use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use diesel::prelude::*;
use futures::channel::oneshot;
use futures::future::{pending, BoxFuture};
use tempfile::TempDir;
use warpui_core::r#async::FutureExt as _;

use super::configuration::{ChatCompletionsProvider, ReasoningEffort, RunConfiguration};
use super::supervisor::TextRunRequest;
use super::text_run::TextRunOutcome;
use super::text_run_integration_tests::{
    runtime_data, task_with_user_message, test_launch_config, CONVERSATION_ID,
};
use super::tool_catalog::ToolCatalog;
use super::tool_execution::{
    RuntimeToolActionAdapter, ToolEffectOutcome, ToolExecutionAuthority, ToolPermissionDecision,
};
use super::transcript::{RuntimeContentBlock, RuntimeTranscript, ToolResultProjection};
use super::AgentRuntimeSupervisor;
use crate::ai::agent::conversation::{AIConversation, AIConversationId};
use crate::ai::agent::AIAgentAction;
use crate::persistence::model::{
    AgentToolExecutionRecord, AgentToolExecutionState, NewAgentRuntimeRunRecord,
    NewAgentToolExecutionRecord, VersionedToolRequest,
};
use crate::persistence::schema::{agent_runtime_runs, agent_tool_execution_records};
use crate::persistence::{setup_database, start_writer, upsert_agent_conversation, ModelEvent};

struct SuccessfulAdapter {
    permissions: AtomicUsize,
    effects: AtomicUsize,
}

struct BlockingPermissionAdapter {
    started: Mutex<Option<oneshot::Sender<()>>>,
    permissions: AtomicUsize,
    effects: AtomicUsize,
}

impl RuntimeToolActionAdapter for BlockingPermissionAdapter {
    fn request_permission(
        &self,
        _action: AIAgentAction,
    ) -> BoxFuture<'static, ToolPermissionDecision> {
        self.permissions.fetch_add(1, Ordering::SeqCst);
        if let Some(started) = self.started.lock().unwrap().take() {
            let _ = started.send(());
        }
        Box::pin(pending())
    }

    fn execute(&self, _action: AIAgentAction) -> BoxFuture<'static, ToolEffectOutcome> {
        self.effects.fetch_add(1, Ordering::SeqCst);
        Box::pin(pending())
    }
}

impl RuntimeToolActionAdapter for SuccessfulAdapter {
    fn request_permission(
        &self,
        _action: AIAgentAction,
    ) -> BoxFuture<'static, ToolPermissionDecision> {
        self.permissions.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { ToolPermissionDecision::Approved })
    }

    fn execute(&self, _action: AIAgentAction) -> BoxFuture<'static, ToolEffectOutcome> {
        self.effects.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            ToolEffectOutcome {
                complete_outcome: b"pwd complete".to_vec(),
                result: Some(
                    warp_multi_agent_api::message::tool_call_result::Result::RunShellCommand(
                        warp_multi_agent_api::RunShellCommandResult::default(),
                    ),
                ),
                projection: ToolResultProjection::Success {
                    content: vec![RuntimeContentBlock::Text {
                        text: "/workspace".to_string(),
                    }],
                    truncated: false,
                },
            }
        })
    }
}

#[tokio::test]
async fn supervisor_commits_tool_outcome_before_acknowledging_bridge() {
    let tempdir = TempDir::new().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let observer_dir = TempDir::new().unwrap();
    let tasks = vec![task_with_user_message()];
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(&mut conn, CONVERSATION_ID, &tasks, runtime_data(0)).unwrap();
    let writer = start_writer(conn, database_path.clone()).unwrap();
    let adapter = Arc::new(SuccessfulAdapter {
        permissions: AtomicUsize::new(0),
        effects: AtomicUsize::new(0),
    });
    let catalog = ToolCatalog::initial(None).unwrap();
    let authority = Arc::new(ToolExecutionAuthority::new(
        catalog.clone(),
        adapter.clone(),
        writer.sender.clone(),
    ));
    let supervisor = AgentRuntimeSupervisor::new(
        test_launch_config("text-run-tool", &observer_dir),
        Arc::new(warpui_core::r#async::executor::Background::default()),
    );
    let handle = supervisor.attach(CONVERSATION_ID).await.unwrap();

    let result = handle
        .run_text(
            &writer.sender,
            tool_run_request(tasks, catalog, authority),
            |_| {},
        )
        .await
        .unwrap();

    assert_eq!(result.outcome(), &TextRunOutcome::Completed);
    assert_eq!(result.revision(), 1);
    assert_eq!(result.tasks()[0].messages.len(), 3);
    assert_eq!(adapter.permissions.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.effects.load(Ordering::SeqCst), 1);
    let result_frame = fs::read_to_string(observer_dir.path().join("tool-results.jsonl")).unwrap();
    assert!(result_frame.contains(r#""status":"success""#));

    supervisor.shutdown_all().await;
    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();
    let mut conn = setup_database(&database_path).unwrap();
    let record = agent_tool_execution_records::table
        .select(AgentToolExecutionRecord::as_select())
        .first::<AgentToolExecutionRecord>(&mut conn)
        .unwrap();
    assert_eq!(record.state(), Some(AgentToolExecutionState::Completed));
}

#[tokio::test]
async fn tool_permission_wait_does_not_block_run_cancellation() {
    let tempdir = TempDir::new().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let observer_dir = TempDir::new().unwrap();
    let tasks = vec![task_with_user_message()];
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(&mut conn, CONVERSATION_ID, &tasks, runtime_data(0)).unwrap();
    let writer = start_writer(conn, database_path).unwrap();
    let (started, permission_started) = oneshot::channel();
    let adapter = Arc::new(BlockingPermissionAdapter {
        started: Mutex::new(Some(started)),
        permissions: AtomicUsize::new(0),
        effects: AtomicUsize::new(0),
    });
    let catalog = ToolCatalog::initial(None).unwrap();
    let authority = Arc::new(ToolExecutionAuthority::new(
        catalog.clone(),
        adapter.clone(),
        writer.sender.clone(),
    ));
    let supervisor = AgentRuntimeSupervisor::new(
        test_launch_config("text-run-tool", &observer_dir),
        Arc::new(warpui_core::r#async::executor::Background::default()),
    );
    let handle = supervisor.attach(CONVERSATION_ID).await.unwrap();

    let (run, cancellation) = async {
        futures::join!(
            handle.run_text(
                &writer.sender,
                tool_run_request(tasks, catalog, authority),
                |_| {},
            ),
            async {
                permission_started.await.unwrap();
                handle.cancel_run().await
            }
        )
    }
    .with_timeout(Duration::from_secs(1))
    .await
    .expect("Tool permission wait must not block cancellation");

    assert_eq!(run.unwrap().outcome(), &TextRunOutcome::Cancelled);
    cancellation.unwrap();
    assert_eq!(adapter.permissions.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.effects.load(Ordering::SeqCst), 0);

    supervisor.shutdown_all().await;
    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();
}

#[tokio::test]
async fn new_run_materializes_executing_record_into_retry_transcript() {
    let tempdir = TempDir::new().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let observer_dir = TempDir::new().unwrap();
    let tasks = vec![task_with_user_message()];
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(&mut conn, CONVERSATION_ID, &tasks, runtime_data(0)).unwrap();
    insert_executing_record(&mut conn);
    let writer = start_writer(conn, database_path.clone()).unwrap();
    let adapter = Arc::new(SuccessfulAdapter {
        permissions: AtomicUsize::new(0),
        effects: AtomicUsize::new(0),
    });
    let catalog = ToolCatalog::initial(None).unwrap();
    let authority = Arc::new(ToolExecutionAuthority::new(
        catalog.clone(),
        adapter.clone(),
        writer.sender.clone(),
    ));
    let supervisor = AgentRuntimeSupervisor::new(
        test_launch_config("text-run-tool", &observer_dir),
        Arc::new(warpui_core::r#async::executor::Background::default()),
    );
    let handle = supervisor.attach(CONVERSATION_ID).await.unwrap();

    let result = handle
        .run_text(
            &writer.sender,
            tool_run_request(tasks, catalog, authority),
            |_| {},
        )
        .await
        .unwrap();

    assert_eq!(result.outcome(), &TextRunOutcome::Completed);
    assert_eq!(result.revision(), 2);
    assert_eq!(result.tasks()[0].messages.len(), 5);
    assert_eq!(adapter.effects.load(Ordering::SeqCst), 1);
    let transcript =
        fs::read_to_string(observer_dir.path().join("accepted-transcripts.jsonl")).unwrap();
    assert!(transcript.contains(r#""error_code":"tool_outcome_unknown""#));

    supervisor.shutdown_all().await;
    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();
    let mut conn = setup_database(&database_path).unwrap();
    let old_record = agent_tool_execution_records::table
        .filter(agent_tool_execution_records::run_id.eq("old-run"))
        .select(AgentToolExecutionRecord::as_select())
        .first::<AgentToolExecutionRecord>(&mut conn)
        .unwrap();
    assert_eq!(old_record.state(), Some(AgentToolExecutionState::Completed));
}

fn insert_executing_record(conn: &mut SqliteConnection) {
    diesel::insert_into(agent_runtime_runs::table)
        .values(NewAgentRuntimeRunRecord::starting(
            CONVERSATION_ID,
            "old-run",
            None,
            0,
        ))
        .execute(conn)
        .unwrap();
    let payload = serde_json::to_vec(&serde_json::json!({
        "version": 1,
        "task_id": "root-task",
        "tool_id": "builtin.run_shell_command",
        "tool_name": "run_shell_command",
        "arguments": { "command": "touch changed", "wait_until_complete": true },
    }))
    .unwrap();
    diesel::insert_into(agent_tool_execution_records::table)
        .values(NewAgentToolExecutionRecord::pending(
            CONVERSATION_ID,
            "old-run",
            "old-call",
            &[7; 32],
            VersionedToolRequest::current(&payload),
        ))
        .execute(conn)
        .unwrap();
    diesel::update(
        agent_tool_execution_records::table
            .filter(agent_tool_execution_records::run_id.eq("old-run")),
    )
    .set(agent_tool_execution_records::state.eq("executing"))
    .execute(conn)
    .unwrap();
}

fn tool_run_request(
    tasks: Vec<warp_multi_agent_api::Task>,
    catalog: ToolCatalog,
    authority: Arc<ToolExecutionAuthority>,
) -> TextRunRequest {
    let conversation = AIConversation::new_restored(
        AIConversationId::try_from(CONVERSATION_ID.to_string()).unwrap(),
        tasks.clone(),
        Some(runtime_data(0)),
    )
    .unwrap();
    let transcript =
        RuntimeTranscript::project(&conversation, 0, &HashSet::new(), &HashMap::new()).unwrap();
    let provider =
        ChatCompletionsProvider::new("https://provider.example/v1", "local-model", "secret-key")
            .unwrap();
    let configuration = RunConfiguration::with_tools(
        provider,
        "/workspace",
        32_768,
        ReasoningEffort::Medium,
        &catalog,
        Vec::new(),
    )
    .unwrap();
    TextRunRequest::new(
        "run-1",
        None::<String>,
        transcript,
        configuration,
        tasks,
        runtime_data(0),
        "root-task",
    )
    .with_tool_execution_authority(authority)
}
