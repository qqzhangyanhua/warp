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

#[cfg(test)]
impl ToolExecutionAuthority {
    pub(super) fn set_fault_injector(
        &mut self,
        fault_injector: Arc<dyn ToolExecutionFaultInjector>,
    ) {
        self.fault_injector = Some(fault_injector);
    }

    pub(super) fn clear_fault_injector(&mut self) {
        self.fault_injector = None;
    }
}
#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use super::ToolExecutionAuthority;
