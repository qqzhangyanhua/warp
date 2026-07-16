use super::*;

struct FailAt(ToolExecutionFaultPoint);

impl ToolExecutionFaultInjector for FailAt {
    fn should_fail(&self, point: ToolExecutionFaultPoint) -> bool {
        self.0 == point
    }
}

#[tokio::test]
async fn failure_before_pending_persistence_leaves_no_record_or_effect() {
    let mut harness = Harness::new(ToolPermissionDecision::Approved);
    harness.authority.set_fault_injector(Arc::new(FailAt(
        ToolExecutionFaultPoint::BeforePendingPersisted,
    )));
    let mut state = harness.state(0);

    let error = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        ToolExecutionError::InjectedFault(ToolExecutionFaultPoint::BeforePendingPersisted)
    );
    assert_eq!(tool_state(&harness.database_path, "call-1"), None);
    assert_eq!(harness.adapter.permission_count.load(Ordering::SeqCst), 0);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);
    harness.finish();
}

#[tokio::test]
async fn failure_before_permission_leaves_pending_without_a_decision() {
    let mut harness = Harness::new(ToolPermissionDecision::Approved);
    harness.authority.set_fault_injector(Arc::new(FailAt(
        ToolExecutionFaultPoint::BeforePermissionDecision,
    )));
    let mut state = harness.state(0);

    let error = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        ToolExecutionError::InjectedFault(ToolExecutionFaultPoint::BeforePermissionDecision)
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
async fn failure_before_executing_persistence_leaves_pending_without_effect() {
    let mut harness = Harness::new(ToolPermissionDecision::Approved);
    harness.authority.set_fault_injector(Arc::new(FailAt(
        ToolExecutionFaultPoint::BeforeExecutingPersisted,
    )));
    let mut state = harness.state(0);

    let error = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        ToolExecutionError::InjectedFault(ToolExecutionFaultPoint::BeforeExecutingPersisted)
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
async fn failure_before_effect_recovers_unknown_without_execution() {
    let mut harness = Harness::new(ToolPermissionDecision::Approved);
    harness
        .authority
        .set_fault_injector(Arc::new(FailAt(ToolExecutionFaultPoint::BeforeEffect)));
    let mut state = harness.state(0);

    let error = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap_err();
    assert_eq!(
        error,
        ToolExecutionError::InjectedFault(ToolExecutionFaultPoint::BeforeEffect)
    );
    assert_eq!(
        tool_state(&harness.database_path, "call-1"),
        Some(AgentToolExecutionState::Executing)
    );
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);

    harness.authority.clear_fault_injector();
    let mut recovered_state = harness.state(0);
    harness
        .authority
        .recover_unfinished(CONVERSATION_ID, &mut recovered_state)
        .await
        .unwrap();
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);
    harness.finish();
}

#[tokio::test]
async fn failure_before_outcome_commit_recovers_unknown_without_reexecution() {
    let mut harness = Harness::new(ToolPermissionDecision::Approved);
    harness.authority.set_fault_injector(Arc::new(FailAt(
        ToolExecutionFaultPoint::BeforeOutcomeCommitted,
    )));
    let mut state = harness.state(0);

    let error = harness
        .authority
        .handle(valid_request("call-1"), &mut state)
        .await
        .unwrap_err();
    assert_eq!(
        error,
        ToolExecutionError::InjectedFault(ToolExecutionFaultPoint::BeforeOutcomeCommitted)
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
    harness.finish();
}
