use std::collections::HashMap;
use std::sync::Arc;

use futures::channel::oneshot;
use warpui::{ModelContext, SingletonEntity};

use super::{
    AIAgentAction, AIAgentActionId, AIAgentActionResult, AIConversationId, BlocklistAIActionEvent,
    BlocklistAIActionModel, CancellationReason, TryExecuteResult,
};
use crate::ai::agent::runtime::ToolPermissionDecision;
use crate::ai::blocklist::BlocklistAIHistoryModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeToolExecutionError {
    ExecutorUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RuntimeToolKey {
    conversation_id: AIConversationId,
    run_id: String,
    action_id: AIAgentActionId,
}

#[derive(Default)]
pub(super) struct RuntimeToolState {
    permission_requests: HashMap<RuntimeToolKey, oneshot::Sender<ToolPermissionDecision>>,
    executions: HashMap<
        RuntimeToolKey,
        oneshot::Sender<Result<AIAgentActionResult, RuntimeToolExecutionError>>,
    >,
}

impl BlocklistAIActionModel {
    pub(crate) fn request_runtime_tool_permission(
        &mut self,
        action: AIAgentAction,
        conversation_id: AIConversationId,
        run_id: String,
        response: oneshot::Sender<ToolPermissionDecision>,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.is_view_only {
            let _ = response.send(ToolPermissionDecision::DeniedByPolicy);
            return;
        }

        let preprocess = self.preprocess_action(&action, conversation_id, ctx);
        ctx.spawn(preprocess, move |me, (), ctx| {
            if !response.is_canceled() {
                me.finish_runtime_tool_permission_request(
                    action,
                    conversation_id,
                    run_id,
                    response,
                    ctx,
                );
            }
        });
    }

    fn finish_runtime_tool_permission_request(
        &mut self,
        action: AIAgentAction,
        conversation_id: AIConversationId,
        run_id: String,
        response: oneshot::Sender<ToolPermissionDecision>,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.has_runtime_tool_conflict(conversation_id, &run_id, &action.id) {
            let _ = response.send(ToolPermissionDecision::DeniedByPolicy);
            return;
        }

        let can_autoexecute = self.executor.update(ctx, |executor, ctx| {
            executor.can_autoexecute_action(&action, conversation_id, ctx)
        });
        if can_autoexecute {
            let _ = response.send(ToolPermissionDecision::Approved);
            return;
        }

        let action_id = action.id.clone();
        let key = RuntimeToolKey {
            conversation_id,
            run_id,
            action_id: action_id.clone(),
        };
        self.runtime_tools.permission_requests.insert(key, response);
        self.pending_actions
            .entry(conversation_id)
            .or_default()
            .push_back(action);
        ctx.emit(BlocklistAIActionEvent::QueuedAction(action_id.clone()));
        ctx.emit(BlocklistAIActionEvent::ActionBlockedOnUserConfirmation(
            action_id,
        ));
    }

    pub(crate) fn execute_runtime_tool_action(
        &mut self,
        action: AIAgentAction,
        conversation_id: AIConversationId,
        run_id: String,
        response: oneshot::Sender<Result<AIAgentActionResult, RuntimeToolExecutionError>>,
        ctx: &mut ModelContext<Self>,
    ) {
        let action_id = action.id.clone();
        if self.has_runtime_tool_conflict(conversation_id, &run_id, &action_id) {
            let _ = response.send(Err(RuntimeToolExecutionError::ExecutorUnavailable));
            return;
        }
        let key = RuntimeToolKey {
            conversation_id,
            run_id,
            action_id: action_id.clone(),
        };
        let phase = self.action_phase_for_action(&action, ctx);
        self.runtime_tools.executions.insert(key.clone(), response);
        BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, _| {
            history.register_runtime_action(conversation_id, action_id.clone());
        });
        let result = self.executor.update(ctx, |executor, ctx| {
            executor.execute_approved_action(action, conversation_id, ctx)
        });
        match result {
            TryExecuteResult::ExecutedAsync => {
                self.update_conversation_in_progress_status(conversation_id, ctx);
                self.add_running_action(conversation_id, action_id, phase);
            }
            TryExecuteResult::ExecutedSync => {
                self.update_conversation_in_progress_status(conversation_id, ctx);
            }
            TryExecuteResult::NotExecuted { .. } => {
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, _| {
                    history.unregister_runtime_action(conversation_id, &action_id);
                });
                if let Some(response) = self.runtime_tools.executions.remove(&key) {
                    let _ = response.send(Err(RuntimeToolExecutionError::ExecutorUnavailable));
                }
            }
        }
    }

    pub(crate) fn cancel_runtime_tool_run(
        &mut self,
        conversation_id: &AIConversationId,
        run_id: &str,
        ctx: &mut ModelContext<Self>,
    ) {
        let permission_keys = self
            .runtime_tools
            .permission_requests
            .keys()
            .filter(|key| key.conversation_id == *conversation_id && key.run_id == run_id)
            .cloned()
            .collect::<Vec<_>>();
        let pending_action_ids = permission_keys
            .iter()
            .map(|key| key.action_id.clone())
            .collect::<Vec<_>>();
        for key in permission_keys {
            if let Some(response) = self.runtime_tools.permission_requests.remove(&key) {
                let _ = response.send(ToolPermissionDecision::DeniedByUser);
            }
        }
        if let Some(pending_actions) = self.pending_actions.get_mut(conversation_id) {
            pending_actions.retain(|action| !pending_action_ids.contains(&action.id));
        }

        let executing_action_ids = self
            .runtime_tools
            .executions
            .keys()
            .filter(|key| key.conversation_id == *conversation_id && key.run_id == run_id)
            .map(|key| key.action_id.clone())
            .collect::<Vec<_>>();
        for action_id in executing_action_ids {
            self.executor.update(ctx, |executor, ctx| {
                executor.cancel_running_async_action(
                    &action_id,
                    Some(CancellationReason::ManuallyCancelled),
                    ctx,
                );
            });
        }
    }

    fn has_runtime_tool_conflict(
        &self,
        conversation_id: AIConversationId,
        run_id: &str,
        action_id: &AIAgentActionId,
    ) -> bool {
        self.runtime_tools
            .permission_requests
            .keys()
            .chain(self.runtime_tools.executions.keys())
            .any(|key| {
                key.conversation_id == conversation_id
                    && key.action_id == *action_id
                    && key.run_id != run_id
            })
    }

    pub(super) fn finish_runtime_tool_action(
        &mut self,
        conversation_id: AIConversationId,
        result: Arc<AIAgentActionResult>,
        cancellation_reason: Option<CancellationReason>,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        let Some(key) = self.runtime_tool_key_for_execution(conversation_id, &result.id) else {
            return false;
        };
        let Some(response) = self.runtime_tools.executions.remove(&key) else {
            return false;
        };

        let should_remove_entry =
            self.running_actions
                .get_mut(&conversation_id)
                .is_some_and(|running| {
                    running.remove_action(&result.id);
                    running.is_empty()
                });
        if should_remove_entry {
            self.running_actions.remove(&conversation_id);
        }
        self.executor.update(ctx, |executor, ctx| {
            executor.discard_action_state(&result.id, ctx);
        });
        BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, _| {
            history.unregister_runtime_action(conversation_id, &result.id);
        });
        let _ = response.send(Ok((*result).clone()));
        ctx.emit(BlocklistAIActionEvent::FinishedAction {
            action_id: result.id.clone(),
            conversation_id,
            cancellation_reason,
        });
        true
    }

    fn runtime_tool_key_for_execution(
        &self,
        conversation_id: AIConversationId,
        action_id: &AIAgentActionId,
    ) -> Option<RuntimeToolKey> {
        self.runtime_tools
            .executions
            .keys()
            .find(|key| key.conversation_id == conversation_id && key.action_id == *action_id)
            .cloned()
    }

    fn runtime_tool_key_for_permission(
        &self,
        conversation_id: AIConversationId,
        action_id: &AIAgentActionId,
    ) -> Option<RuntimeToolKey> {
        self.runtime_tools
            .permission_requests
            .keys()
            .find(|key| key.conversation_id == conversation_id && key.action_id == *action_id)
            .cloned()
    }

    pub(super) fn is_runtime_tool_awaiting_permission(
        &self,
        conversation_id: AIConversationId,
        action_id: &AIAgentActionId,
    ) -> bool {
        self.runtime_tool_key_for_permission(conversation_id, action_id)
            .is_some()
    }

    pub(super) fn approve_runtime_tool_permission(
        &mut self,
        conversation_id: AIConversationId,
        action_id: &AIAgentActionId,
    ) {
        let Some(key) = self.runtime_tool_key_for_permission(conversation_id, action_id) else {
            return;
        };
        if let Some(response) = self.runtime_tools.permission_requests.remove(&key) {
            let response: oneshot::Sender<ToolPermissionDecision> = response;
            let _ = response.send(ToolPermissionDecision::Approved);
        }
    }

    pub(super) fn deny_runtime_tool_permission(
        &mut self,
        conversation_id: AIConversationId,
        action_id: &AIAgentActionId,
    ) -> bool {
        let Some(key) = self.runtime_tool_key_for_permission(conversation_id, action_id) else {
            return false;
        };
        let Some(response) = self.runtime_tools.permission_requests.remove(&key) else {
            return false;
        };
        let response: oneshot::Sender<ToolPermissionDecision> = response;
        let _ = response.send(ToolPermissionDecision::DeniedByUser);
        true
    }
}
