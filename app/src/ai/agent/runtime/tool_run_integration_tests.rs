use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use diesel::prelude::*;
use futures::channel::oneshot;
use futures::future::{pending, BoxFuture};
use tempfile::TempDir;
use warp_core::features::FeatureFlag;
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
use super::transcript::{
    RuntimeContentBlock, RuntimeTranscript, ToolErrorCode, ToolResultProjection,
};
use super::{AgentRuntimeSupervisor, RuntimeError};
use crate::ai::agent::conversation::{AIConversation, AIConversationId};
use crate::ai::agent::AIAgentAction;
use crate::ai::personal_memory::MemoryCapability;
use crate::persistence::model::{
    AgentRuntimeRunRecord, AgentRuntimeRunState, AgentRuntimeTerminalOutcome,
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

struct EndingAdapter {
    permissions: AtomicUsize,
    effects: AtomicUsize,
}

const MEMORY_CREATE_CONVERSATION_ID: &str = "018f8a1e-7d2c-7c45-9c3a-6f78f04b3d21";
const MEMORY_QUERY_CONVERSATION_ID: &str = "018f8a1e-7d2c-7c45-9c3a-6f78f04b3d22";

#[tokio::test]
async fn personal_memory_survives_restart_and_recalls_from_a_new_conversation() {
    let tempdir = TempDir::new().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let create_observer = TempDir::new().unwrap();
    let create_text = "帮我记住我的 GitHub 帐号是 zyh-work";
    let create_tasks = vec![memory_task(create_text)];
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(
        &mut conn,
        MEMORY_CREATE_CONVERSATION_ID,
        &create_tasks,
        runtime_data(0),
    )
    .unwrap();
    let writer = start_writer(conn, database_path.clone()).unwrap();
    let create_adapter = Arc::new(SuccessfulAdapter {
        permissions: AtomicUsize::new(0),
        effects: AtomicUsize::new(0),
    });
    let create_catalog = {
        let _flag = FeatureFlag::PersonalMemory.override_enabled(true);
        ToolCatalog::for_user_input(None, MemoryCapability::derive(create_text)).unwrap()
    };
    let create_authority = Arc::new(ToolExecutionAuthority::new(
        create_catalog.clone(),
        create_adapter.clone(),
        writer.sender.clone(),
    ));
    let create_supervisor = AgentRuntimeSupervisor::new(
        test_launch_config("text-run-personal-memory-create", &create_observer),
        Arc::new(warpui_core::r#async::executor::Background::default()),
    );
    let create_handle = create_supervisor
        .attach(MEMORY_CREATE_CONVERSATION_ID)
        .await
        .unwrap();

    let create_result = create_handle
        .run_text(
            &writer.sender,
            personal_memory_run_request(
                MEMORY_CREATE_CONVERSATION_ID,
                create_tasks,
                create_catalog,
                create_authority,
            ),
            |_| {},
        )
        .await
        .unwrap();
    assert_eq!(create_result.outcome(), &TextRunOutcome::Completed);
    assert_eq!(create_adapter.permissions.load(Ordering::SeqCst), 0);
    assert_eq!(create_adapter.effects.load(Ordering::SeqCst), 0);

    create_supervisor.shutdown_all().await;
    writer
        .sender
        .send(ModelEvent::DeleteMultiAgentConversations {
            conversation_ids: vec![MEMORY_CREATE_CONVERSATION_ID.to_string()],
        })
        .unwrap();
    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();

    let query_observer = TempDir::new().unwrap();
    let query_text = "我的 GitHub 帐号还记得么？";
    let query_tasks = vec![memory_task(query_text)];
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(
        &mut conn,
        MEMORY_QUERY_CONVERSATION_ID,
        &query_tasks,
        runtime_data(0),
    )
    .unwrap();
    let writer = start_writer(conn, database_path).unwrap();
    let query_adapter = Arc::new(SuccessfulAdapter {
        permissions: AtomicUsize::new(0),
        effects: AtomicUsize::new(0),
    });
    let query_catalog = {
        let _flag = FeatureFlag::PersonalMemory.override_enabled(true);
        ToolCatalog::for_user_input(None, MemoryCapability::derive(query_text)).unwrap()
    };
    let query_authority = Arc::new(ToolExecutionAuthority::new(
        query_catalog.clone(),
        query_adapter.clone(),
        writer.sender.clone(),
    ));
    let query_supervisor = AgentRuntimeSupervisor::new(
        test_launch_config("text-run-personal-memory-query", &query_observer),
        Arc::new(warpui_core::r#async::executor::Background::default()),
    );
    let query_handle = query_supervisor
        .attach(MEMORY_QUERY_CONVERSATION_ID)
        .await
        .unwrap();

    let query_result = query_handle
        .run_text(
            &writer.sender,
            personal_memory_run_request(
                MEMORY_QUERY_CONVERSATION_ID,
                query_tasks,
                query_catalog,
                query_authority,
            ),
            |_| {},
        )
        .await
        .unwrap();
    let fetched_memories = &query_result.tasks()[0]
        .messages
        .last()
        .unwrap()
        .fetched_memories;
    assert_eq!(query_result.outcome(), &TextRunOutcome::Completed);
    assert_eq!(fetched_memories.len(), 1);
    assert_eq!(fetched_memories[0].memory_store_id, "zyh-personal-memory");
    assert!(fetched_memories[0].content.contains("zyh-work"));
    assert_eq!(query_adapter.permissions.load(Ordering::SeqCst), 0);
    assert_eq!(query_adapter.effects.load(Ordering::SeqCst), 0);

    query_supervisor.shutdown_all().await;
    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();
}

fn memory_task(query: &str) -> warp_multi_agent_api::Task {
    warp_multi_agent_api::Task {
        id: "root-task".to_string(),
        messages: vec![warp_multi_agent_api::Message {
            id: "user-1".to_string(),
            task_id: "root-task".to_string(),
            request_id: "request-1".to_string(),
            message: Some(warp_multi_agent_api::message::Message::UserQuery(
                warp_multi_agent_api::message::UserQuery {
                    query: query.to_string(),
                    ..Default::default()
                },
            )),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn personal_memory_run_request(
    conversation_id: &str,
    tasks: Vec<warp_multi_agent_api::Task>,
    catalog: ToolCatalog,
    authority: Arc<ToolExecutionAuthority>,
) -> TextRunRequest {
    let conversation = AIConversation::new_restored(
        AIConversationId::try_from(conversation_id.to_string()).unwrap(),
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

impl RuntimeToolActionAdapter for BlockingPermissionAdapter {
    fn request_permission(
        &self,
        _run_id: String,
        _action: AIAgentAction,
    ) -> BoxFuture<'static, ToolPermissionDecision> {
        self.permissions.fetch_add(1, Ordering::SeqCst);
        if let Some(started) = self.started.lock().unwrap().take() {
            let _ = started.send(());
        }
        Box::pin(pending())
    }

    fn execute(
        &self,
        _run_id: String,
        _action: AIAgentAction,
    ) -> BoxFuture<'static, ToolEffectOutcome> {
        self.effects.fetch_add(1, Ordering::SeqCst);
        Box::pin(pending())
    }
}

impl RuntimeToolActionAdapter for SuccessfulAdapter {
    fn request_permission(
        &self,
        _run_id: String,
        _action: AIAgentAction,
    ) -> BoxFuture<'static, ToolPermissionDecision> {
        self.permissions.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { ToolPermissionDecision::Approved })
    }

    fn execute(
        &self,
        _run_id: String,
        _action: AIAgentAction,
    ) -> BoxFuture<'static, ToolEffectOutcome> {
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

impl RuntimeToolActionAdapter for EndingAdapter {
    fn request_permission(
        &self,
        _run_id: String,
        _action: AIAgentAction,
    ) -> BoxFuture<'static, ToolPermissionDecision> {
        self.permissions.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { ToolPermissionDecision::Approved })
    }

    fn execute(
        &self,
        _run_id: String,
        _action: AIAgentAction,
    ) -> BoxFuture<'static, ToolEffectOutcome> {
        self.effects.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            ToolEffectOutcome {
                complete_outcome: Vec::new(),
                result: None,
                projection: ToolResultProjection::Error {
                    error_code: ToolErrorCode::ToolOutcomeUnknown,
                    may_have_executed: true,
                    content: vec![RuntimeContentBlock::Text {
                        text: "unknown".to_string(),
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
            tool_run_request(tasks, catalog, authority, None),
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
async fn supervisor_ends_run_locally_when_tool_result_must_end_run() {
    let tempdir = TempDir::new().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let observer_dir = TempDir::new().unwrap();
    let tasks = vec![task_with_user_message()];
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(&mut conn, CONVERSATION_ID, &tasks, runtime_data(0)).unwrap();
    let writer = start_writer(conn, database_path.clone()).unwrap();
    let adapter = Arc::new(EndingAdapter {
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
            tool_run_request(tasks, catalog, authority, None),
            |_| {},
        )
        .await
        .unwrap();

    assert!(matches!(result.outcome(), TextRunOutcome::Failed { .. }));
    assert_eq!(result.revision(), 1);
    assert_eq!(result.tasks()[0].messages.len(), 3);
    assert_eq!(adapter.permissions.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.effects.load(Ordering::SeqCst), 1);
    assert!(handle.process_is_running().await.is_err());

    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();
    let mut conn = setup_database(&database_path).unwrap();
    let run = agent_runtime_runs::table
        .filter(agent_runtime_runs::run_id.eq("run-1"))
        .select(AgentRuntimeRunRecord::as_select())
        .first::<AgentRuntimeRunRecord>(&mut conn)
        .unwrap();
    assert_eq!(
        run.terminal_outcome(),
        Some(AgentRuntimeTerminalOutcome::Failed)
    );
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
                tool_run_request(tasks, catalog, authority, None),
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
async fn new_run_materializes_unfinished_tools_and_fails_their_original_runs() {
    let tempdir = TempDir::new().unwrap();
    let database_path = tempdir.path().join("warp.sqlite");
    let observer_dir = TempDir::new().unwrap();
    let tasks = vec![task_with_user_message()];
    let mut conn = setup_database(&database_path).unwrap();
    upsert_agent_conversation(&mut conn, CONVERSATION_ID, &tasks, runtime_data(0)).unwrap();
    insert_unfinished_record(&mut conn, "executing-run", "executing-call", "executing");
    insert_unfinished_record(&mut conn, "pending-run", "pending-call", "pending");
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

    let ordinary_run = handle
        .run_text(
            &writer.sender,
            tool_run_request(tasks.clone(), catalog.clone(), authority.clone(), None),
            |_| {},
        )
        .await;
    assert!(matches!(ordinary_run, Err(RuntimeError::RetryRequired)));

    let handle = supervisor.attach(CONVERSATION_ID).await.unwrap();
    let result = handle
        .run_text(
            &writer.sender,
            tool_run_request(tasks, catalog, authority, Some("executing-run")),
            |_| {},
        )
        .await
        .unwrap();

    assert_eq!(result.outcome(), &TextRunOutcome::Completed);
    assert_eq!(result.revision(), 3);
    assert_eq!(result.tasks()[0].messages.len(), 7);
    assert_eq!(adapter.effects.load(Ordering::SeqCst), 1);
    let transcript =
        fs::read_to_string(observer_dir.path().join("accepted-transcripts.jsonl")).unwrap();
    assert!(transcript.contains(r#""error_code":"tool_outcome_unknown""#));
    assert!(transcript.contains(r#""error_code":"tool_execution_failed""#));

    supervisor.shutdown_all().await;
    writer.sender.send(ModelEvent::Terminate).unwrap();
    writer.handle.join().unwrap();
    let mut conn = setup_database(&database_path).unwrap();
    let old_records = agent_tool_execution_records::table
        .filter(agent_tool_execution_records::run_id.ne("run-1"))
        .select(AgentToolExecutionRecord::as_select())
        .load::<AgentToolExecutionRecord>(&mut conn)
        .unwrap();
    assert_eq!(old_records.len(), 2);
    assert!(old_records
        .iter()
        .all(|record| record.state() == Some(AgentToolExecutionState::Completed)));
    let old_runs = agent_runtime_runs::table
        .filter(agent_runtime_runs::run_id.ne("run-1"))
        .select(AgentRuntimeRunRecord::as_select())
        .load::<AgentRuntimeRunRecord>(&mut conn)
        .unwrap();
    assert_eq!(old_runs.len(), 2);
    assert!(old_runs.iter().all(|run| {
        run.state() == Some(AgentRuntimeRunState::Finished)
            && run.terminal_outcome() == Some(AgentRuntimeTerminalOutcome::Failed)
    }));
}

fn insert_unfinished_record(
    conn: &mut SqliteConnection,
    run_id: &str,
    tool_call_id: &str,
    state: &str,
) {
    diesel::insert_into(agent_runtime_runs::table)
        .values(NewAgentRuntimeRunRecord::starting(
            CONVERSATION_ID,
            run_id,
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
            run_id,
            tool_call_id,
            &[7; 32],
            VersionedToolRequest::current(&payload),
        ))
        .execute(conn)
        .unwrap();
    diesel::update(
        agent_tool_execution_records::table.filter(agent_tool_execution_records::run_id.eq(run_id)),
    )
    .set(agent_tool_execution_records::state.eq(state))
    .execute(conn)
    .unwrap();
}

fn tool_run_request(
    tasks: Vec<warp_multi_agent_api::Task>,
    catalog: ToolCatalog,
    authority: Arc<ToolExecutionAuthority>,
    retry_of_run_id: Option<&str>,
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
        retry_of_run_id,
        transcript,
        configuration,
        tasks,
        runtime_data(0),
        "root-task",
    )
    .with_tool_execution_authority(authority)
}
