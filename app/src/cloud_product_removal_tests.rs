use super::*;

#[test]
fn cloud_product_surfaces_are_removed() {
    assert!(CLOUD_PRODUCT_SURFACES_REMOVED);
    assert!(!may_expose_warp_drive());
    assert!(!may_expose_account_and_cloud_settings());
    assert!(!may_expose_sharing_or_handoff());
    assert!(!may_expose_cloud_agent_surfaces());
    assert!(!may_expose_account_and_cloud_actions());
    assert!(!may_present_cloud_quota_or_sync_state());
}

#[test]
fn stale_cloud_notebook_restore_fails_closed() {
    assert_eq!(
        evaluate_stale_cloud_notebook_restore(),
        StaleCloudSurfaceRestoreOutcome::Unsupported {
            message: DRIVE_REMOVED_GUIDANCE,
        }
    );
}

#[test]
fn stale_cloud_workflow_restore_fails_closed() {
    assert_eq!(
        evaluate_stale_cloud_workflow_restore(),
        StaleCloudSurfaceRestoreOutcome::Unsupported {
            message: DRIVE_REMOVED_GUIDANCE,
        }
    );
}

#[test]
fn error_messages_point_to_local_alternatives() {
    let drive = unsupported_drive_error_message();
    assert!(drive.contains("local Markdown") || drive.contains("Workflow"));
    assert!(!drive.is_empty());

    let account = unsupported_account_surface_error_message();
    assert!(account.contains("Account") || account.contains("billing"));

    let cloud = unsupported_cloud_surface_error_message();
    assert!(cloud.contains("local") || cloud.contains("SSH"));
}

#[test]
fn retained_local_surfaces_are_not_gated_by_cloud_removal() {
    // Policy module only removes cloud shell surfaces. Local product entry
    // points are independent booleans elsewhere (MCP, local notebooks, etc.).
    // This test documents the intentional non-overlap: removal flags never
    // disable "local" wording paths.
    assert!(CLOUD_PRODUCT_SURFACES_REMOVED);
    assert!(DRIVE_REMOVED_GUIDANCE.contains("local"));
    assert!(CLOUD_SURFACE_REMOVED_GUIDANCE.contains("MCP"));
    assert!(CLOUD_SURFACE_REMOVED_GUIDANCE.contains("SSH"));
    assert!(CLOUD_SURFACE_REMOVED_GUIDANCE.contains("Conversations"));
}
