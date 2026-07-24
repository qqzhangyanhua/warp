use warpui::App;

use super::EnvVarCollectionManager;
use crate::env_vars::{
    evaluate_stale_evc_restore, may_expose_evc_in_ui, may_open_or_create_evc,
    StaleEvcRestoreOutcome, EVC_REMOVED_GUIDANCE,
};

#[test]
#[serial_test::serial]
fn local_only_manager_initializes_without_update_manager() {
    App::test((), |app| async move {
        app.add_singleton_model(EnvVarCollectionManager::new);
    });
}

#[test]
fn manager_reports_evc_unsupported() {
    assert!(!EnvVarCollectionManager::is_supported());
    assert!(!may_open_or_create_evc());
    assert!(!may_expose_evc_in_ui());
}

#[test]
fn stale_restore_outcome_is_unsupported_without_cloud_singletons() {
    // Pure policy: no App / CloudModel / UpdateManager required.
    match evaluate_stale_evc_restore(true) {
        StaleEvcRestoreOutcome::Unsupported { message } => {
            assert_eq!(message, EVC_REMOVED_GUIDANCE);
        }
    }
}

#[test]
fn pane_restore_fails_closed_without_cloud_model() {
    // EnvVarCollectionPane::restore must not need ViewContext CloudModel wiring.
    // We only assert the policy error message that restore returns.
    let err = crate::env_vars::unsupported_evc_error_message();
    assert!(err.contains("Environment Variable Collections are no longer available"));
    assert!(!EnvVarCollectionManager::is_supported());
}
