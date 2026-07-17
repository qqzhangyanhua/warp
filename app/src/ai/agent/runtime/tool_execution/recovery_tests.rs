use super::*;

#[tokio::test]
async fn executing_redelivery_recovers_unknown_and_fails_run_without_reexecution() {
    let harness = Harness::new(ToolPermissionDecision::Approved);
    let request = valid_request("call-1");
    harness.accept_only(&request).await;
    harness.set_tool_state("call-1", AgentToolExecutionState::Executing);
    let mut state = harness.state(0);

    let recovered = harness.authority.handle(request, &mut state).await.unwrap();

    assert_error(
        &recovered.projection,
        ToolErrorCode::ToolOutcomeUnknown,
        true,
    );
    assert!(recovered.run_must_end);
    assert_eq!(harness.adapter.permission_count.load(Ordering::SeqCst), 0);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);
    assert_eq!(
        run_terminal_outcome(&harness.database_path, RUN_ID),
        Some(AgentRuntimeTerminalOutcome::Failed)
    );
    harness.finish();
}

#[tokio::test]
async fn failed_outcome_commit_leaves_executing_and_recovers_as_unknown() {
    let harness = Harness::new(ToolPermissionDecision::Approved);
    harness.install_completion_failure();
    let mut state = harness.state(0);

    let error = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap_err();
    assert_eq!(
        error,
        ToolExecutionError::Commit(CommitAgentRuntimeMutationError::Persistence)
    );
    assert_eq!(
        tool_state(&harness.database_path, "call-1"),
        Some(AgentToolExecutionState::Executing)
    );

    harness.remove_completion_failure();
    let mut recovered_state = harness.state(0);
    let recovered = harness
        .authority
        .recover_unfinished(CONVERSATION_ID, &mut recovered_state)
        .await
        .unwrap();
    assert!(matches!(
        recovered.as_slice(),
        [
            TranscriptItem::ToolRequest { tool_call_id, .. },
            TranscriptItem::ToolResult { result, .. },
        ] if tool_call_id == "call-1" && matches!(
            result,
            ToolResultProjection::Error {
                error_code: ToolErrorCode::ToolOutcomeUnknown,
                may_have_executed: true,
                ..
            }
        )
    ));
    assert_eq!(recovered_state.revision, 1);
    assert_eq!(recovered_state.tasks[0].messages.len(), 3);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        tool_state(&harness.database_path, "call-1"),
        Some(AgentToolExecutionState::Completed)
    );
    assert_eq!(
        run_terminal_outcome(&harness.database_path, RUN_ID),
        Some(AgentRuntimeTerminalOutcome::Failed)
    );
    harness.finish();
}

#[tokio::test]
async fn pending_recovery_does_not_prompt_or_execute() {
    let harness = Harness::new(ToolPermissionDecision::Approved);
    let request = valid_request("call-1");
    harness.accept_only(&request).await;
    let mut state = harness.state(0);

    let recovered = harness.authority.handle(request, &mut state).await.unwrap();

    assert_error(
        &recovered.projection,
        ToolErrorCode::ToolExecutionFailed,
        false,
    );
    assert!(recovered.run_must_end);
    assert_eq!(harness.adapter.permission_count.load(Ordering::SeqCst), 0);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);
    assert_eq!(
        run_terminal_outcome(&harness.database_path, RUN_ID),
        Some(AgentRuntimeTerminalOutcome::Failed)
    );
    harness.finish();
}

#[tokio::test]
async fn migrated_legacy_request_recovers_without_prompt_or_execution() {
    let harness = Harness::new(ToolPermissionDecision::Approved);
    let request = valid_request("legacy-call");
    harness.accept_only(&request).await;
    harness.clear_request_payload("legacy-call");
    let mut state = harness.state(0);

    let recovered = harness
        .authority
        .recover_unfinished(CONVERSATION_ID, &mut state)
        .await
        .unwrap();

    assert!(matches!(
        recovered.as_slice(),
        [
            TranscriptItem::ToolRequest { tool_call_id, .. },
            TranscriptItem::ToolResult { result, .. },
        ] if tool_call_id == "legacy-call" && matches!(
            result,
            ToolResultProjection::Error {
                error_code: ToolErrorCode::ToolExecutionFailed,
                may_have_executed: false,
                ..
            }
        )
    ));
    assert_eq!(harness.adapter.permission_count.load(Ordering::SeqCst), 0);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);
    assert_eq!(
        tool_state(&harness.database_path, "legacy-call"),
        Some(AgentToolExecutionState::Completed)
    );
    assert_eq!(
        run_terminal_outcome(&harness.database_path, RUN_ID),
        Some(AgentRuntimeTerminalOutcome::Failed)
    );
    harness.finish();
}

#[tokio::test]
async fn migrated_legacy_executing_request_recovers_as_unknown_without_reexecution() {
    let harness = Harness::new(ToolPermissionDecision::Approved);
    let request = valid_request("legacy-call");
    harness.accept_only(&request).await;
    harness.set_tool_state("legacy-call", AgentToolExecutionState::Executing);
    harness.clear_request_payload("legacy-call");
    let mut state = harness.state(0);

    let recovered = harness
        .authority
        .recover_unfinished(CONVERSATION_ID, &mut state)
        .await
        .unwrap();

    assert!(matches!(
        recovered.as_slice(),
        [
            TranscriptItem::ToolRequest { tool_call_id, .. },
            TranscriptItem::ToolResult { result, .. },
        ] if tool_call_id == "legacy-call" && matches!(
            result,
            ToolResultProjection::Error {
                error_code: ToolErrorCode::ToolOutcomeUnknown,
                may_have_executed: true,
                ..
            }
        )
    ));
    assert_eq!(harness.adapter.permission_count.load(Ordering::SeqCst), 0);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);
    assert_eq!(
        tool_state(&harness.database_path, "legacy-call"),
        Some(AgentToolExecutionState::Completed)
    );
    assert_eq!(
        run_terminal_outcome(&harness.database_path, RUN_ID),
        Some(AgentRuntimeTerminalOutcome::Failed)
    );
    harness.finish();
}
