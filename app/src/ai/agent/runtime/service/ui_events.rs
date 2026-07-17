use warp_multi_agent_api as api;
use warpui::EntityId;

use crate::ai::agent::conversation::AIConversationId;
use crate::ai::agent::runtime::text_run::RuntimeEvent;
use crate::ai::agent::runtime::RUNTIME_DELTA_MESSAGE_PREFIX;
use crate::ai::blocklist::ResponseStreamId;
use crate::persistence::model::AgentConversationData;

#[derive(Hash, PartialEq, Eq)]
pub(super) struct StreamedMessageKey {
    pub(super) conversation_id: AIConversationId,
    pub(super) run_id: String,
    pub(super) event_id: String,
}

pub(super) enum RuntimeUiEvent {
    RunStatus {
        conversation_id: AIConversationId,
        terminal_surface_id: EntityId,
        run_id: String,
    },
    TextDelta {
        conversation_id: AIConversationId,
        response_stream_id: ResponseStreamId,
        output_task_id: String,
        terminal_surface_id: EntityId,
        run_id: String,
        event_id: String,
        delta: String,
    },
    ConversationCommit {
        conversation_id: AIConversationId,
        response_stream_id: ResponseStreamId,
        terminal_surface_id: EntityId,
        run_id: String,
        revision: u64,
        tasks: Vec<api::Task>,
        conversation_data: AgentConversationData,
    },
    RunFinished {
        conversation_id: AIConversationId,
        terminal_surface_id: EntityId,
        run_id: String,
    },
}

impl RuntimeUiEvent {
    pub(super) fn from_runtime_event(
        conversation_id: AIConversationId,
        response_stream_id: ResponseStreamId,
        output_task_id: String,
        terminal_surface_id: EntityId,
        event: RuntimeEvent,
    ) -> Self {
        match event {
            RuntimeEvent::RunStatus { run_id, .. } => Self::RunStatus {
                conversation_id,
                terminal_surface_id,
                run_id,
            },
            RuntimeEvent::TextDelta {
                run_id,
                event_id,
                delta,
            } => Self::TextDelta {
                conversation_id,
                response_stream_id,
                output_task_id,
                terminal_surface_id,
                run_id,
                event_id,
                delta,
            },
            RuntimeEvent::ConversationCommit {
                run_id,
                revision,
                tasks,
                conversation_data,
                ..
            } => Self::ConversationCommit {
                conversation_id,
                response_stream_id,
                terminal_surface_id,
                run_id,
                revision,
                tasks,
                conversation_data,
            },
            RuntimeEvent::RunFinished { run_id, .. } => Self::RunFinished {
                conversation_id,
                terminal_surface_id,
                run_id,
            },
        }
    }
}

pub(super) fn add_message_action(
    task_id: &str,
    message_id: &str,
    request_id: &str,
    text: String,
) -> api::ClientAction {
    api::ClientAction {
        action: Some(api::client_action::Action::AddMessagesToTask(
            api::client_action::AddMessagesToTask {
                task_id: task_id.to_string(),
                messages: vec![agent_output_message(task_id, message_id, request_id, text)],
            },
        )),
    }
}

pub(super) fn append_message_action(
    task_id: &str,
    message_id: &str,
    request_id: &str,
    text: String,
) -> api::ClientAction {
    api::ClientAction {
        action: Some(api::client_action::Action::AppendToMessageContent(
            api::client_action::AppendToMessageContent {
                task_id: task_id.to_string(),
                message: Some(agent_output_message(task_id, message_id, request_id, text)),
                mask: Some(prost_types::FieldMask {
                    paths: vec!["agent_output.text".to_string()],
                }),
            },
        )),
    }
}

fn agent_output_message(
    task_id: &str,
    message_id: &str,
    request_id: &str,
    text: String,
) -> api::Message {
    api::Message {
        id: format!("{RUNTIME_DELTA_MESSAGE_PREFIX}{request_id}:{message_id}"),
        task_id: task_id.to_string(),
        request_id: request_id.to_string(),
        message: Some(api::message::Message::AgentOutput(
            api::message::AgentOutput { text },
        )),
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "ui_events_tests.rs"]
mod tests;
