use uuid::Uuid;
use warpui::ModelContext;

use super::{AgentRuntimeService, AgentRuntimeServiceEvent, RuntimeStartError};
use crate::ai::agent::conversation::AIConversationId;
use crate::ai::agent::runtime::AgentRuntimeSupervisor;

impl AgentRuntimeService {
    pub(crate) fn set_start_result_for_test(&mut self, result: Result<(), RuntimeStartError>) {
        self.start_result_for_test = Some(result);
    }

    pub(crate) fn set_supervisor_for_test(&mut self, supervisor: Option<AgentRuntimeSupervisor>) {
        self.supervisor = supervisor;
    }

    pub(crate) fn set_run_ids_for_test(
        &mut self,
        run_ids: impl IntoIterator<Item = impl Into<String>>,
    ) {
        self.run_ids_for_test = run_ids.into_iter().map(Into::into).collect();
    }

    pub(crate) fn set_last_run_id_for_test(
        &mut self,
        conversation_id: AIConversationId,
        run_id: impl Into<String>,
    ) {
        self.last_run_ids_by_conversation_id
            .insert(conversation_id, run_id.into());
    }

    pub(crate) fn set_active_run_for_test(
        &mut self,
        conversation_id: AIConversationId,
        run_id: impl Into<String>,
    ) {
        self.active_run_ids_by_conversation_id
            .insert(conversation_id, run_id.into());
    }

    pub(crate) fn finish_active_run_for_test(
        &mut self,
        conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) {
        self.active_run_ids_by_conversation_id
            .remove(&conversation_id);
        ctx.emit(AgentRuntimeServiceEvent::RunFinished { conversation_id });
    }

    pub(crate) fn start_attempts_for_test(&self, conversation_id: AIConversationId) -> usize {
        self.start_attempts_by_conversation_id
            .get(&conversation_id)
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn cancel_attempts_for_test(&self, conversation_id: AIConversationId) -> usize {
        self.cancel_attempts_by_conversation_id
            .get(&conversation_id)
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn invalidate_attempts_for_test(&self, conversation_id: AIConversationId) -> usize {
        self.invalidate_attempts_by_conversation_id
            .get(&conversation_id)
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn restore_attempts_for_test(&self, conversation_id: AIConversationId) -> usize {
        self.restore_attempts_by_conversation_id
            .get(&conversation_id)
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn last_run_id_for_test(&self, conversation_id: AIConversationId) -> Option<&str> {
        self.last_run_ids_by_conversation_id
            .get(&conversation_id)
            .map(String::as_str)
    }

    pub(crate) fn active_run_id_for_test(&self, conversation_id: AIConversationId) -> Option<&str> {
        self.active_run_ids_by_conversation_id
            .get(&conversation_id)
            .map(String::as_str)
    }

    pub(crate) fn starting_run_cancelled_for_test(&self, run_id: &str) -> bool {
        self.cancelled_starting_run_ids.contains(run_id)
    }

    pub(super) fn next_run_id(&mut self) -> String {
        self.run_ids_for_test
            .pop_front()
            .unwrap_or_else(|| Uuid::new_v4().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_run_identity_cannot_be_replaced_by_a_concurrent_start() {
        let mut service = AgentRuntimeService::new();
        let conversation_id = AIConversationId::new();
        service.set_active_run_for_test(conversation_id, "run-existing");

        assert_eq!(
            service.ensure_conversation_idle(conversation_id),
            Err(RuntimeStartError::RunAlreadyActive)
        );
        assert_eq!(
            service.active_run_id_for_test(conversation_id),
            Some("run-existing")
        );
    }
}
