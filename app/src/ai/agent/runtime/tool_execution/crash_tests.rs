use super::*;

struct FailAt(ToolExecutionFaultPoint);

impl ToolExecutionFaultInjector for FailAt {
    fn should_fail(&self, point: ToolExecutionFaultPoint) -> bool {
        self.0 == point
    }
}

#[tokio::test]
async fn failure_after_pending_persistence_prevents_permission_and_effect() {
    let mut harness = Harness::new(ToolPermissionDecision::Approved);
    harness.authority.set_fault_injector(Arc::new(FailAt(
        ToolExecutionFaultPoint::AfterPendingPersisted,
    )));
    let mut state = harness.state(0);

    let error = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        ToolExecutionError::InjectedFault(ToolExecutionFaultPoint::AfterPendingPersisted)
    );
    assert_eq!(
        tool_state(&harness.database_path, "call-1"),
        Some(AgentToolExecutionState::Pending)
    );
    assert_eq!(harness.adapter.permission_count.load(Ordering::SeqCst), 0);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);
    harness.finish();
}

#[tokio::test]
async fn failure_after_denial_decision_leaves_pending_without_effect() {
    let mut harness = Harness::new(ToolPermissionDecision::DeniedByPolicy);
    harness.authority.set_fault_injector(Arc::new(FailAt(
        ToolExecutionFaultPoint::AfterPermissionDecision,
    )));
    let mut state = harness.state(0);

    let error = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        ToolExecutionError::InjectedFault(ToolExecutionFaultPoint::AfterPermissionDecision)
    );
    assert_eq!(
        tool_state(&harness.database_path, "call-1"),
        Some(AgentToolExecutionState::Pending)
    );
    assert_eq!(harness.adapter.permission_count.load(Ordering::SeqCst), 1);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);
    harness.finish();
}

#[tokio::test]
async fn failure_after_executing_persistence_prevents_effect() {
    let mut harness = Harness::new(ToolPermissionDecision::Approved);
    harness.authority.set_fault_injector(Arc::new(FailAt(
        ToolExecutionFaultPoint::AfterExecutingPersisted,
    )));
    let mut state = harness.state(0);

    let error = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        ToolExecutionError::InjectedFault(ToolExecutionFaultPoint::AfterExecutingPersisted)
    );
    assert_eq!(
        tool_state(&harness.database_path, "call-1"),
        Some(AgentToolExecutionState::Executing)
    );
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);
    harness.finish();
}

#[tokio::test]
async fn failure_after_effect_return_recovers_unknown_without_reexecution() {
    let mut harness = Harness::new(ToolPermissionDecision::Approved);
    harness.authority.set_fault_injector(Arc::new(FailAt(
        ToolExecutionFaultPoint::AfterEffectReturned,
    )));
    let mut state = harness.state(0);

    let error = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap_err();
    assert_eq!(
        error,
        ToolExecutionError::InjectedFault(ToolExecutionFaultPoint::AfterEffectReturned)
    );
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        tool_state(&harness.database_path, "call-1"),
        Some(AgentToolExecutionState::Executing)
    );

    harness.authority.clear_fault_injector();
    let mut recovered_state = harness.state(0);
    harness
        .authority
        .recover_unfinished(CONVERSATION_ID, &mut recovered_state)
        .await
        .unwrap();

    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        tool_state(&harness.database_path, "call-1"),
        Some(AgentToolExecutionState::Completed)
    );
    harness.finish();
}

#[tokio::test]
async fn failure_after_outcome_commit_redelivers_stored_bytes_without_reexecution() {
    let mut harness = Harness::new(ToolPermissionDecision::Approved);
    harness.authority.set_fault_injector(Arc::new(FailAt(
        ToolExecutionFaultPoint::AfterOutcomeCommitted,
    )));
    let mut state = harness.state(0);

    let error = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap_err();
    assert_eq!(
        error,
        ToolExecutionError::InjectedFault(ToolExecutionFaultPoint::AfterOutcomeCommitted)
    );
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 1);
    let stored_projection = harness.stored_projection("call-1");

    harness.authority.clear_fault_injector();
    let redelivered = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap();

    assert_eq!(redelivered.projection_bytes, stored_projection);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 1);
    harness.finish();
}
