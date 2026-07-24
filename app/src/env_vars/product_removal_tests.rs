use super::{
    evaluate_stale_evc_restore, may_expose_evc_in_ui, may_open_or_create_evc,
    unsupported_evc_error_message, EVC_PRODUCT_REMOVED, EVC_REMOVED_GUIDANCE, StaleEvcRestoreOutcome,
};

#[test]
fn product_flag_marks_evc_removed() {
    assert!(EVC_PRODUCT_REMOVED);
    assert!(!may_open_or_create_evc());
    assert!(!may_expose_evc_in_ui());
}

#[test]
fn stale_restore_is_unsupported_without_cloud() {
    // With or without a persisted cloud id, restore never needs CloudModel.
    assert_eq!(
        evaluate_stale_evc_restore(true),
        StaleEvcRestoreOutcome::Unsupported {
            message: EVC_REMOVED_GUIDANCE,
        }
    );
    assert_eq!(
        evaluate_stale_evc_restore(false),
        StaleEvcRestoreOutcome::Unsupported {
            message: EVC_REMOVED_GUIDANCE,
        }
    );
}

#[test]
fn guidance_points_to_shell_env_and_secret_manager_not_a_local_store() {
    let message = unsupported_evc_error_message();
    assert!(message.contains("shell"));
    assert!(message.contains(".env") || message.contains("environment"));
    assert!(message.contains("Secret Manager"));
    // Must not advertise a ZYH-owned secret store replacement.
    assert!(!message.to_ascii_lowercase().contains("sqlite"));
    assert!(!message.to_ascii_lowercase().contains("json store"));
}

#[test]
fn guidance_is_stable_for_toasts_and_restore_errors() {
    assert_eq!(unsupported_evc_error_message(), EVC_REMOVED_GUIDANCE);
}
