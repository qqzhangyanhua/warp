#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolExecutionFaultPoint {
    BeforePendingPersisted,
    AfterPendingPersisted,
    BeforePermissionDecision,
    AfterPermissionDecision,
    BeforeExecutingPersisted,
    AfterExecutingPersisted,
    BeforeEffect,
    AfterEffectReturned,
    BeforeOutcomeCommitted,
    AfterOutcomeCommitted,
}

#[cfg(test)]
pub(super) trait ToolExecutionFaultInjector: Send + Sync {
    fn should_fail(&self, point: ToolExecutionFaultPoint) -> bool;
}
