//! Cloud product shell surfaces are removed from the permanent ZYH local product.
//!
//! Warp Drive, Account, teams, billing, referrals, sharing, handoff, cloud
//! environments, managed secrets, and cloud Agent management must not appear in
//! settings, menus, command surfaces, panes, or restoration. Restored or stale
//! actions fail closed without constructing cloud singletons (CloudModel,
//! UpdateManager, personal drive owners).
//!
//! Retained local entry points remain: Rules, Workflows (YAML), MCP, Notebooks
//! (Markdown), terminal, local Conversations, and SSH.

/// Product flag: cloud collaboration and Drive shell surfaces are removed.
pub const CLOUD_PRODUCT_SURFACES_REMOVED: bool = true;

/// User-facing guidance when a removed cloud surface is requested.
pub const CLOUD_SURFACE_REMOVED_GUIDANCE: &str = "Cloud product surfaces are no longer available. \
Use local Notebooks (Markdown), local Workflows (YAML), MCP, terminal, local Conversations, \
and SSH instead.";

/// Guidance when a cloud Warp Drive object cannot be opened.
pub const DRIVE_REMOVED_GUIDANCE: &str = "ZYH Drive is no longer available. \
Open a local Markdown Notebook or local Workflow YAML file instead.";

/// Guidance when cloud Account / team / billing settings are requested.
pub const ACCOUNT_SURFACE_REMOVED_GUIDANCE: &str = "Account, team, billing, and cloud environment \
settings are no longer available in ZYH.";

/// Outcome when session restore encounters a stale cloud Drive-backed pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaleCloudSurfaceRestoreOutcome {
    /// Do not construct cloud-backed UI or managers for this leaf.
    Unsupported { message: &'static str },
}

/// Whether any UI may open or construct Warp Drive navigation, objects, or panels.
pub fn may_expose_warp_drive() -> bool {
    !CLOUD_PRODUCT_SURFACES_REMOVED
}

/// Whether Account, team, billing, referral, or cloud-environment settings may open.
pub fn may_expose_account_and_cloud_settings() -> bool {
    !CLOUD_PRODUCT_SURFACES_REMOVED
}

/// Whether sharing, ownership, permissions, or handoff dialogs may open.
pub fn may_expose_sharing_or_handoff() -> bool {
    !CLOUD_PRODUCT_SURFACES_REMOVED
}

/// Whether cloud Agent / Ambient / Scheduled cloud management surfaces may open.
pub fn may_expose_cloud_agent_surfaces() -> bool {
    !CLOUD_PRODUCT_SURFACES_REMOVED
}

/// Whether menus, command palette, and keybindings may offer Account/Drive/cloud actions.
pub fn may_expose_account_and_cloud_actions() -> bool {
    !CLOUD_PRODUCT_SURFACES_REMOVED
}

/// Whether UI may present cloud sync, offline-cloud, quota, upgrade, plan, owner,
/// or server revision product state as a live surface.
pub fn may_present_cloud_quota_or_sync_state() -> bool {
    !CLOUD_PRODUCT_SURFACES_REMOVED
}

/// Evaluate restoration of a persisted cloud Notebook pane.
///
/// Always fail closed while cloud Drive notebooks are removed. Local Markdown
/// notebook leaves are handled on a separate path.
pub fn evaluate_stale_cloud_notebook_restore() -> StaleCloudSurfaceRestoreOutcome {
    debug_assert!(CLOUD_PRODUCT_SURFACES_REMOVED);
    StaleCloudSurfaceRestoreOutcome::Unsupported {
        message: DRIVE_REMOVED_GUIDANCE,
    }
}

/// Evaluate restoration of a persisted cloud Workflow pane.
pub fn evaluate_stale_cloud_workflow_restore() -> StaleCloudSurfaceRestoreOutcome {
    debug_assert!(CLOUD_PRODUCT_SURFACES_REMOVED);
    StaleCloudSurfaceRestoreOutcome::Unsupported {
        message: DRIVE_REMOVED_GUIDANCE,
    }
}

/// Error message for APIs that still take `Result` paths when Drive is requested.
pub fn unsupported_drive_error_message() -> String {
    DRIVE_REMOVED_GUIDANCE.to_string()
}

/// Error message for Account/team/billing command surfaces.
pub fn unsupported_account_surface_error_message() -> String {
    ACCOUNT_SURFACE_REMOVED_GUIDANCE.to_string()
}

/// Generic removed-surface message for cloud shell actions.
pub fn unsupported_cloud_surface_error_message() -> String {
    CLOUD_SURFACE_REMOVED_GUIDANCE.to_string()
}

#[cfg(test)]
#[path = "cloud_product_removal_tests.rs"]
mod tests;
