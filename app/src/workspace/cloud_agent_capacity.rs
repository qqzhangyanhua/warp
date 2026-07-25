//! Stub types for removed cloud-agent capacity UI.
//!
//! The modal itself is deleted; event variants may still be emitted by legacy
//! ambient-agent paths and are ignored by Workspace.

/// Former capacity-modal presentation variant (no UI in ZYH).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudAgentCapacityModalVariant {
    OutOfCredits,
    ConcurrentLimit,
}
