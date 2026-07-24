//! Environment Variable Collections are removed from the ZYH local product.
//!
//! Stale panes, menus, Drive items, and restore paths must fail closed without
//! constructing CloudModel or UpdateManager state. Users are directed to shell
//! configuration, project environment files, system environment variables, or a
//! user-selected Secret Manager CLI. No plaintext JSON or SQLite secret store
//! replaces EVC.

/// Product flag: Environment Variable Collections are not a supported surface.
pub const EVC_PRODUCT_REMOVED: bool = true;

/// User-facing guidance after EVC surfaces are removed.
pub const EVC_REMOVED_GUIDANCE: &str = "Environment Variable Collections are no longer available. \
Use shell configuration, a project .env file, system environment variables, \
or your Secret Manager CLI instead.";

/// Outcome when session restore encounters a stale EVC pane snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaleEvcRestoreOutcome {
    /// Do not construct cloud-backed EVC UI or managers for this leaf.
    Unsupported { message: &'static str },
}

/// Evaluate restoration of a persisted Environment Variable Collection pane.
///
/// Never requires CloudModel or UpdateManager. Always fail closed while EVC is
/// removed from the product.
pub fn evaluate_stale_evc_restore(
    _env_var_collection_id_present: bool,
) -> StaleEvcRestoreOutcome {
    debug_assert!(EVC_PRODUCT_REMOVED);
    StaleEvcRestoreOutcome::Unsupported {
        message: EVC_REMOVED_GUIDANCE,
    }
}

/// Whether any product UI may open or create an EVC pane.
pub fn may_open_or_create_evc() -> bool {
    !EVC_PRODUCT_REMOVED
}

/// Whether Drive, command palette, menus, or Agent context may list EVC.
pub fn may_expose_evc_in_ui() -> bool {
    !EVC_PRODUCT_REMOVED
}

/// Error message for restore/create APIs that still take Result paths.
pub fn unsupported_evc_error_message() -> String {
    EVC_REMOVED_GUIDANCE.to_string()
}

#[cfg(test)]
#[path = "product_removal_tests.rs"]
mod tests;
