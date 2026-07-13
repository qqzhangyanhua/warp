use anyhow::anyhow;

use super::{
    deferred_retry_action, is_recoverable_for_request, recovery_action, retry_limit,
    DeferredRetryAction, RecoveryAction, LOCAL_ONLY_MAX_RETRIES, MAX_RETRIES,
};
use crate::server::server_api::AIApiError;

#[test]
fn local_only_mode_retries_at_most_once() {
    assert_eq!(retry_limit(true), LOCAL_ONLY_MAX_RETRIES);
    assert_eq!(retry_limit(true), 1);
    assert_eq!(retry_limit(false), MAX_RETRIES);
    assert_eq!(retry_limit(false), 3);
}

#[test]
fn local_only_mode_retries_only_transient_provider_failures() {
    for status in [
        http::StatusCode::REQUEST_TIMEOUT,
        http::StatusCode::TOO_MANY_REQUESTS,
        http::StatusCode::INTERNAL_SERVER_ERROR,
    ] {
        let error = AIApiError::ProviderErrorStatus {
            status,
            message: "sanitized".to_string(),
            retry_after: None,
        };
        assert!(is_recoverable_for_request(&error, true));
    }

    for status in [
        http::StatusCode::TEMPORARY_REDIRECT,
        http::StatusCode::BAD_REQUEST,
        http::StatusCode::UNAUTHORIZED,
    ] {
        let error = AIApiError::ProviderErrorStatus {
            status,
            message: "sanitized".to_string(),
            retry_after: None,
        };
        assert!(!is_recoverable_for_request(&error, true));
    }

    assert!(is_recoverable_for_request(&AIApiError::UnexpectedEof, true));
    assert!(!is_recoverable_for_request(
        &AIApiError::Other(anyhow!("malformed provider protocol")),
        true
    ));
}

#[test]
fn remote_mode_preserves_existing_recovery_classification() {
    let error = AIApiError::Other(anyhow!("remote recoverable error"));
    assert_eq!(
        is_recoverable_for_request(&error, false),
        error.is_recoverable()
    );
}

#[test]
fn deferred_retry_requires_same_active_request_and_online_state() {
    let request_id = uuid::Uuid::new_v4();
    assert_eq!(
        deferred_retry_action(Some(request_id), Some(request_id), true),
        DeferredRetryAction::Retry
    );
    assert_eq!(
        deferred_retry_action(Some(request_id), Some(request_id), false),
        DeferredRetryAction::WaitForNetwork
    );
    assert_eq!(
        deferred_retry_action(Some(request_id), None, true),
        DeferredRetryAction::Drop
    );
    assert_eq!(
        deferred_retry_action(Some(request_id), Some(uuid::Uuid::new_v4()), true),
        DeferredRetryAction::Drop
    );
}

// Argument order: has_received_client_actions, is_recoverable, has_retry_budget,
// can_attempt_resume_on_error, is_online.

#[test]
fn pre_action_failures_retry() {
    assert_eq!(
        recovery_action(false, true, true, true, true),
        RecoveryAction::RetryNow
    );
    // Resume eligibility is irrelevant pre-actions.
    assert_eq!(
        recovery_action(false, true, true, false, true),
        RecoveryAction::RetryNow
    );
}

#[test]
fn pre_action_failures_wait_for_connectivity_when_offline() {
    assert_eq!(
        recovery_action(false, true, true, true, false),
        RecoveryAction::RetryWhenOnline
    );
}

#[test]
fn pre_action_budget_exhaustion_is_terminal() {
    // The request has already been retried MAX_RETRIES times; stop.
    assert_eq!(
        recovery_action(false, true, false, true, true),
        RecoveryAction::Fail
    );
    assert_eq!(
        recovery_action(false, true, false, true, false),
        RecoveryAction::Fail
    );
}

#[test]
fn non_recoverable_pre_action_failure_is_terminal() {
    assert_eq!(
        recovery_action(false, false, true, true, true),
        RecoveryAction::Fail
    );
}

#[test]
fn post_action_recoverable_failures_resume() {
    assert_eq!(
        recovery_action(true, true, true, true, true),
        RecoveryAction::Resume
    );
    // Offline doesn't change the decision; the resume spawn waits for connectivity.
    assert_eq!(
        recovery_action(true, true, true, true, false),
        RecoveryAction::Resume
    );
    // The in-request retry budget is irrelevant once actions have executed.
    assert_eq!(
        recovery_action(true, true, false, true, true),
        RecoveryAction::Resume
    );
}

#[test]
fn post_action_failures_without_resume_eligibility_are_terminal() {
    // Resume requests themselves run with can_attempt_resume_on_error=false,
    // bounding recovery to a single resume.
    assert_eq!(
        recovery_action(true, true, true, false, true),
        RecoveryAction::Fail
    );
}

#[test]
fn non_recoverable_post_action_failure_is_terminal() {
    // A non-recoverable error (e.g. a client error) ends the conversation even
    // after actions have executed.
    assert_eq!(
        recovery_action(true, false, true, true, true),
        RecoveryAction::Fail
    );
}
