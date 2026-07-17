use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use futures::channel::oneshot;
use futures::future::BoxFuture;
use serde_json::json;
use warp_multi_agent_api as api;

use super::*;
use crate::ai::agent::runtime::transcript::TranscriptItem;
use crate::ai::agent::AIAgentAction;
use crate::persistence::model::{
    AgentConversationData, AgentRuntimeBinding, AgentRuntimeRunRecord, AgentRuntimeTerminalOutcome,
    AgentToolExecutionRecord, AgentToolExecutionState, NewAgentRuntimeRunRecord,
};
use crate::persistence::schema::{agent_runtime_runs, agent_tool_execution_records};
use crate::persistence::{
    setup_database, start_writer, CommitAgentRuntimeMutationError, WriterHandles,
};

#[path = "tool_execution/test_support.rs"]
mod support;

use support::{assert_error, success_projection};

const CONVERSATION_ID: &str = "tool-authority-conversation";
const RUN_ID: &str = "tool-authority-run";
const TASK_ID: &str = "root-task";

struct FakeAdapter {
    default_decision: ToolPermissionDecision,
    decisions: Mutex<VecDeque<ToolPermissionDecision>>,
    permission_count: AtomicUsize,
    execution_count: AtomicUsize,
    observed_executing: AtomicBool,
    database_path: PathBuf,
}

impl FakeAdapter {
    fn new(default_decision: ToolPermissionDecision, database_path: PathBuf) -> Self {
        Self {
            default_decision,
            decisions: Mutex::new(VecDeque::new()),
            permission_count: AtomicUsize::new(0),
            execution_count: AtomicUsize::new(0),
            observed_executing: AtomicBool::new(false),
            database_path,
        }
    }

    fn push_decision(&self, decision: ToolPermissionDecision) {
        self.decisions.lock().unwrap().push_back(decision);
    }
}

impl RuntimeToolActionAdapter for FakeAdapter {
    fn request_permission(
        &self,
        _run_id: String,
        _action: AIAgentAction,
    ) -> BoxFuture<'static, ToolPermissionDecision> {
        self.permission_count.fetch_add(1, Ordering::SeqCst);
        let decision = self
            .decisions
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(self.default_decision);
        Box::pin(async move { decision })
    }

    fn execute(
        &self,
        _run_id: String,
        _action: AIAgentAction,
    ) -> BoxFuture<'static, ToolEffectOutcome> {
        self.execution_count.fetch_add(1, Ordering::SeqCst);
        let database_path = self.database_path.clone();
        let observed_executing = &self.observed_executing;
        let is_executing =
            tool_state(&database_path, "call-1") == Some(AgentToolExecutionState::Executing);
        observed_executing.store(is_executing, Ordering::SeqCst);
        Box::pin(async {
            ToolEffectOutcome {
                complete_outcome: b"complete outcome".to_vec(),
                result: Some(api::message::tool_call_result::Result::RunShellCommand(
                    api::RunShellCommandResult::default(),
                )),
                projection: success_projection("effect complete"),
            }
        })
    }
}

struct Harness {
    _tempdir: tempfile::TempDir,
    database_path: PathBuf,
    writer: WriterHandles,
    adapter: Arc<FakeAdapter>,
    authority: ToolExecutionAuthority,
}

impl Harness {
    fn new(default_decision: ToolPermissionDecision) -> Self {
        let tempdir = tempfile::tempdir().unwrap();
        let database_path = tempdir.path().join("warp.sqlite");
        let mut conn = setup_database(&database_path).unwrap();
        let data = runtime_data(0);
        crate::persistence::upsert_agent_conversation(&mut conn, CONVERSATION_ID, &[task()], data)
            .unwrap();
        diesel::insert_into(agent_runtime_runs::table)
            .values(NewAgentRuntimeRunRecord::starting(
                CONVERSATION_ID,
                RUN_ID,
                None,
                0,
            ))
            .execute(&mut conn)
            .unwrap();
        let writer = start_writer(conn, database_path.clone()).unwrap();
        let adapter = Arc::new(FakeAdapter::new(default_decision, database_path.clone()));
        let catalog = ToolCatalog::initial(None).unwrap();
        let authority =
            ToolExecutionAuthority::new(catalog, adapter.clone(), writer.sender.clone());
        Self {
            _tempdir: tempdir,
            database_path,
            writer,
            adapter,
            authority,
        }
    }

    fn state(&self, revision: u64) -> ToolRunState {
        ToolRunState {
            revision,
            tasks: vec![task()],
            conversation_data: runtime_data(revision),
            task_id: TASK_ID.to_string(),
        }
    }

    fn stored_projection(&self, tool_call_id: &str) -> Vec<u8> {
        let mut conn = SqliteConnection::establish(self.database_path.to_str().unwrap()).unwrap();
        agent_tool_execution_records::table
            .filter(agent_tool_execution_records::tool_call_id.eq(tool_call_id))
            .select(AgentToolExecutionRecord::as_select())
            .first::<AgentToolExecutionRecord>(&mut conn)
            .unwrap()
            .tool_result_projection()
            .unwrap()
            .bytes()
            .to_vec()
    }

    fn install_completion_failure(&self) {
        let mut conn = SqliteConnection::establish(self.database_path.to_str().unwrap()).unwrap();
        conn.batch_execute(
            "CREATE TRIGGER fail_tool_completion \
             BEFORE UPDATE OF state ON agent_tool_execution_records \
             WHEN OLD.state = 'executing' AND NEW.state = 'completed' \
             BEGIN SELECT RAISE(FAIL, 'injected completion failure'); END;",
        )
        .unwrap();
    }

    fn remove_completion_failure(&self) {
        let mut conn = SqliteConnection::establish(self.database_path.to_str().unwrap()).unwrap();
        conn.batch_execute("DROP TRIGGER fail_tool_completion;")
            .unwrap();
    }

    async fn accept_only(&self, request: &RuntimeToolRequest) {
        let (acknowledgement, acknowledged) = oneshot::channel();
        self.writer
            .sender
            .send(ModelEvent::AcceptAgentToolExecution(
                AcceptAgentToolExecution {
                    conversation_id: request.conversation_id.clone(),
                    run_id: request.run_id.clone(),
                    tool_call_id: request.tool_call_id.clone(),
                    request_fingerprint: request.frame_fingerprint,
                    request_payload: ToolRequestPayload::current(request_payload(request, TASK_ID)),
                    request_limit: TOOL_REQUEST_LIMIT,
                    acknowledgement,
                },
            ))
            .unwrap();
        assert_eq!(
            acknowledged.await.unwrap().unwrap(),
            AcceptAgentToolExecutionResult::Pending {
                newly_inserted: true
            }
        );
    }

    fn set_tool_state(&self, tool_call_id: &str, state: AgentToolExecutionState) {
        let mut conn = SqliteConnection::establish(self.database_path.to_str().unwrap()).unwrap();
        diesel::update(
            agent_tool_execution_records::table
                .filter(agent_tool_execution_records::tool_call_id.eq(tool_call_id)),
        )
        .set(agent_tool_execution_records::state.eq(state.as_database_value()))
        .execute(&mut conn)
        .unwrap();
    }

    fn clear_request_payload(&self, tool_call_id: &str) {
        let mut conn = SqliteConnection::establish(self.database_path.to_str().unwrap()).unwrap();
        diesel::update(
            agent_tool_execution_records::table
                .filter(agent_tool_execution_records::tool_call_id.eq(tool_call_id)),
        )
        .set(agent_tool_execution_records::request_payload.eq(Vec::<u8>::new()))
        .execute(&mut conn)
        .unwrap();
    }

    fn finish(self) {
        self.writer.sender.send(ModelEvent::Terminate).unwrap();
        self.writer.handle.join().unwrap();
    }
}

#[tokio::test]
async fn invalid_requests_complete_without_permission_or_effect() {
    let harness = Harness::new(ToolPermissionDecision::Approved);
    let mut state = harness.state(0);

    let result = harness
        .authority
        .handle(
            request("call-1", "wrong_name", valid_arguments()),
            &mut state,
        )
        .await
        .unwrap();
    let invalid_arguments = harness
        .authority
        .handle(
            request(
                "call-2",
                "run_shell_command",
                json!({ "wait_until_complete": true })
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
            &mut state,
        )
        .await
        .unwrap();

    assert_error(&result.projection, ToolErrorCode::InvalidToolRequest, false);
    assert_error(
        &invalid_arguments.projection,
        ToolErrorCode::InvalidToolRequest,
        false,
    );
    assert!(!result.run_must_end);
    assert_eq!(harness.adapter.permission_count.load(Ordering::SeqCst), 0);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);
    assert_eq!(state.revision, 2);
    assert_eq!(
        tool_state(&harness.database_path, "call-1"),
        Some(AgentToolExecutionState::Completed)
    );
    assert_eq!(
        tool_state(&harness.database_path, "call-2"),
        Some(AgentToolExecutionState::Completed)
    );
    harness.finish();
}

#[tokio::test]
async fn policy_and_user_denials_are_completed_without_effect() {
    let harness = Harness::new(ToolPermissionDecision::DeniedByPolicy);
    harness
        .adapter
        .push_decision(ToolPermissionDecision::DeniedByPolicy);
    harness
        .adapter
        .push_decision(ToolPermissionDecision::DeniedByUser);
    let mut state = harness.state(0);

    let policy = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap();
    let user = harness
        .authority
        .handle(valid_request("call-2"), &mut state)
        .await
        .unwrap();

    assert!(matches!(
        policy.projection,
        ToolResultProjection::Denied {
            denied_by: ToolDenialSource::Policy,
            ..
        }
    ));
    assert!(matches!(
        user.projection,
        ToolResultProjection::Denied {
            denied_by: ToolDenialSource::User,
            ..
        }
    ));
    assert_eq!(harness.adapter.permission_count.load(Ordering::SeqCst), 2);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);
    assert_eq!(state.revision, 2);
    harness.finish();
}

#[tokio::test]
async fn approved_effect_starts_after_executing_and_completed_redelivery_is_fixed() {
    let harness = Harness::new(ToolPermissionDecision::Approved);
    let mut state = harness.state(0);

    let first = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap();
    let redelivered = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap();

    assert_eq!(first.projection, redelivered.projection);
    assert_eq!(
        serde_json::to_vec(&redelivered.projection).unwrap(),
        harness.stored_projection("call-1")
    );
    assert!(harness.adapter.observed_executing.load(Ordering::SeqCst));
    assert_eq!(harness.adapter.permission_count.load(Ordering::SeqCst), 1);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 1);
    assert_eq!(state.revision, 1);
    assert_eq!(state.tasks[0].messages.len(), 3);
    assert_eq!(
        tool_state(&harness.database_path, "call-1"),
        Some(AgentToolExecutionState::Completed)
    );
    harness.finish();
}

#[tokio::test]
async fn thirty_third_request_is_durably_completed_at_the_limit() {
    let harness = Harness::new(ToolPermissionDecision::DeniedByPolicy);
    let mut state = harness.state(0);
    for index in 1..=32 {
        let result = harness
            .authority
            .handle(valid_request(&format!("call-{index}")), &mut state)
            .await
            .unwrap();
        assert!(!result.run_must_end);
    }

    let limited = harness
        .authority
        .handle(valid_request("call-33"), &mut state)
        .await
        .unwrap();

    assert_error(
        &limited.projection,
        ToolErrorCode::ToolRequestLimitExceeded,
        false,
    );
    assert!(limited.run_must_end);
    assert_eq!(harness.adapter.permission_count.load(Ordering::SeqCst), 32);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);
    assert_eq!(state.revision, 33);
    assert_eq!(
        tool_state(&harness.database_path, "call-33"),
        Some(AgentToolExecutionState::Completed)
    );
    assert_eq!(
        run_terminal_outcome(&harness.database_path, RUN_ID),
        Some(AgentRuntimeTerminalOutcome::LimitReached)
    );
    harness.finish();
}

fn task() -> api::Task {
    api::Task {
        id: TASK_ID.to_string(),
        messages: vec![api::Message {
            id: "user-1".to_string(),
            task_id: TASK_ID.to_string(),
            request_id: "user-run".to_string(),
            message: Some(api::message::Message::UserQuery(api::message::UserQuery {
                query: "Inspect the workspace".to_string(),
                ..Default::default()
            })),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn runtime_data(revision: u64) -> AgentConversationData {
    let mut data: AgentConversationData =
        serde_json::from_str(r#"{"server_conversation_token":null}"#).unwrap();
    data.runtime_binding = Some(AgentRuntimeBinding::Pi);
    data.runtime_transcript_revision = Some(revision);
    data
}

fn valid_request(tool_call_id: &str) -> RuntimeToolRequest {
    request(tool_call_id, "run_shell_command", valid_arguments())
}

fn request(
    tool_call_id: &str,
    tool_name: &str,
    arguments: serde_json::Map<String, serde_json::Value>,
) -> RuntimeToolRequest {
    RuntimeToolRequest {
        frame_fingerprint: [1; 32],
        conversation_id: CONVERSATION_ID.to_string(),
        run_id: RUN_ID.to_string(),
        tool_call_id: tool_call_id.to_string(),
        tool_id: "builtin.run_shell_command".to_string(),
        tool_name: tool_name.to_string(),
        arguments,
    }
}

fn valid_arguments() -> serde_json::Map<String, serde_json::Value> {
    json!({ "command": "pwd", "wait_until_complete": true })
        .as_object()
        .unwrap()
        .clone()
}

fn tool_state(database_path: &Path, tool_call_id: &str) -> Option<AgentToolExecutionState> {
    let mut conn = SqliteConnection::establish(database_path.to_str().unwrap()).unwrap();
    agent_tool_execution_records::table
        .filter(agent_tool_execution_records::tool_call_id.eq(tool_call_id))
        .select(AgentToolExecutionRecord::as_select())
        .first::<AgentToolExecutionRecord>(&mut conn)
        .optional()
        .unwrap()
        .and_then(|record| record.state())
}

fn run_terminal_outcome(database_path: &Path, run_id: &str) -> Option<AgentRuntimeTerminalOutcome> {
    let mut conn = SqliteConnection::establish(database_path.to_str().unwrap()).unwrap();
    agent_runtime_runs::table
        .filter(agent_runtime_runs::run_id.eq(run_id))
        .select(AgentRuntimeRunRecord::as_select())
        .first::<AgentRuntimeRunRecord>(&mut conn)
        .optional()
        .unwrap()
        .and_then(|record| record.terminal_outcome())
}

#[path = "tool_execution/crash_boundary_tests.rs"]
mod crash_boundary_tests;
#[path = "tool_execution/crash_tests.rs"]
mod crash_tests;
#[path = "tool_execution/recovery_tests.rs"]
mod recovery_tests;
