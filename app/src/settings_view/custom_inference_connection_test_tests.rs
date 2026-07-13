use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::anyhow;
use http::StatusCode;

use super::{ConnectionTestController, ConnectionTestFailure, ConnectionTestState};
use crate::server::server_api::AIApiError;

#[test]
fn restarting_cancels_the_older_test_and_ignores_its_result() {
    let first_cancelled = Arc::new(AtomicBool::new(false));
    let mut controller = ConnectionTestController::default();
    let first_generation = controller.begin();
    let cancelled = first_cancelled.clone();
    controller.set_cancellation(first_generation, move || {
        cancelled.store(true, Ordering::SeqCst);
    });

    let second_generation = controller.begin();
    controller.complete(first_generation, Ok(()));

    assert!(first_cancelled.load(Ordering::SeqCst));
    assert_eq!(controller.state(), &ConnectionTestState::Testing);

    controller.complete(second_generation, Ok(()));
    assert_eq!(controller.state(), &ConnectionTestState::Succeeded);
}

#[test]
fn cancelling_keeps_late_results_from_restoring_a_completed_state() {
    let cancelled = Arc::new(AtomicBool::new(false));
    let mut controller = ConnectionTestController::default();
    let generation = controller.begin();
    let was_cancelled = cancelled.clone();
    controller.set_cancellation(generation, move || {
        was_cancelled.store(true, Ordering::SeqCst);
    });

    controller.cancel();
    controller.complete(generation, Ok(()));

    assert!(cancelled.load(Ordering::SeqCst));
    assert_eq!(controller.state(), &ConnectionTestState::Idle);
}

#[test]
fn provider_failures_are_exposed_as_typed_categories() {
    let mut controller = ConnectionTestController::default();
    let generation = controller.begin();

    controller.complete(generation, Err(ConnectionTestFailure::RateLimited));

    assert_eq!(
        controller.state(),
        &ConnectionTestState::Failed(ConnectionTestFailure::RateLimited)
    );
}

#[test]
fn provider_statuses_map_to_distinct_failure_categories() {
    let cases = [
        (
            StatusCode::UNAUTHORIZED,
            ConnectionTestFailure::Authentication,
        ),
        (StatusCode::NOT_FOUND, ConnectionTestFailure::MissingModel),
        (StatusCode::REQUEST_TIMEOUT, ConnectionTestFailure::Timeout),
        (
            StatusCode::TOO_MANY_REQUESTS,
            ConnectionTestFailure::RateLimited,
        ),
        (StatusCode::BAD_GATEWAY, ConnectionTestFailure::Server),
    ];

    for (status, expected) in cases {
        let error = AIApiError::ProviderErrorStatus {
            status,
            message: "sanitized".to_string(),
            retry_after: None,
        };
        assert_eq!(ConnectionTestFailure::from_api_error(error), expected);
    }
    assert_eq!(
        ConnectionTestFailure::from_api_error(AIApiError::Other(anyhow!(
            "Provider returned a malformed Chat Completions response"
        ))),
        ConnectionTestFailure::MalformedProtocol
    );
}
