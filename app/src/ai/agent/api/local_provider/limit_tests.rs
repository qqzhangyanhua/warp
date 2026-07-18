use std::sync::Arc;

use futures::channel::oneshot;
use futures_lite::StreamExt as _;
use warp_multi_agent_api as api;
use warpui::r#async::BoxFuture;

use super::tests::{params_with_custom_model, text_message};
use super::transport::{LocalProviderResponse, LocalProviderTransport};
use super::{
    generate_local_provider_output_with_transport, ChatCompletionRequest, ChatRole,
    LocalProviderModel,
};
use crate::ai::agent::task::TaskId;
use crate::ai::agent::{
    AIAgentActionId, AIAgentActionResult, AIAgentActionResultType, AIAgentInput,
    RequestCommandOutputResult,
};
use crate::server::server_api::AIApiError;

struct UnexpectedProviderTransport;

impl LocalProviderTransport for UnexpectedProviderTransport {
    fn send(
        &self,
        _provider_model: LocalProviderModel,
        _request: ChatCompletionRequest,
    ) -> BoxFuture<'static, Result<LocalProviderResponse, AIApiError>> {
        panic!("provider must not be called after the local tool-call limit")
    }
}

fn shell_tool_call(index: usize) -> api::Message {
    api::Message {
        id: format!("message-{index}"),
        task_id: "task-1".to_string(),
        message: Some(api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: format!("call-{index}"),
            tool: Some(api::message::tool_call::Tool::RunShellCommand(
                api::message::tool_call::RunShellCommand {
                    command: "pwd".to_string(),
                    ..Default::default()
                },
            )),
        })),
        ..Default::default()
    }
}

#[test]
fn thirty_second_tool_result_pauses_for_confirmation_without_calling_provider() {
    let mut params = params_with_custom_model();
    params.tasks[0].messages = vec![text_message(ChatRole::User, "do work", "m1", "task-1")];
    params.tasks[0]
        .messages
        .extend((1..=32).map(shell_tool_call));
    params.input = vec![AIAgentInput::ActionResult {
        result: AIAgentActionResult {
            id: AIAgentActionId::from("call-32".to_string()),
            task_id: TaskId::new("task-1".to_string()),
            result: AIAgentActionResultType::RequestCommandOutput(
                RequestCommandOutputResult::CancelledBeforeExecution,
            ),
        },
        context: Arc::from([]),
    }];
    let (_tx, rx) = oneshot::channel();

    let mut output = futures::executor::block_on(generate_local_provider_output_with_transport(
        params,
        rx,
        Arc::new(UnexpectedProviderTransport),
    ))
    .expect("stream should be created");
    let events = futures::executor::block_on(async {
        let mut events = Vec::new();
        while let Some(event) = output.next().await {
            events.push(event.expect("limit event should succeed"));
        }
        events
    });

    assert_eq!(events.len(), 4);
    let persisted_message = match &events[1].r#type {
        Some(api::response_event::Type::ClientActions(actions)) => actions
            .actions
            .first()
            .and_then(|action| action.action.as_ref())
            .and_then(|action| match action {
                api::client_action::Action::AddMessagesToTask(add) => add.messages.first(),
                _ => None,
            })
            .expect("current tool result should be persisted before pausing"),
        _ => panic!("expected input persistence action before confirmation"),
    };
    let Some(api::message::Message::ToolCallResult(tool_result)) = &persisted_message.message
    else {
        panic!("expected the current tool result to be persisted");
    };
    assert_eq!(tool_result.tool_call_id, "call-32");

    let message = events
        .iter()
        .filter_map(|event| match &event.r#type {
            Some(api::response_event::Type::ClientActions(actions)) => actions.actions.first(),
            _ => None,
        })
        .filter_map(|action| match &action.action {
            Some(api::client_action::Action::AddMessagesToTask(add)) => add.messages.first(),
            _ => None,
        })
        .find_map(|message| match &message.message {
            Some(api::message::Message::AgentOutput(output)) => Some(output.text.as_str()),
            _ => None,
        })
        .expect("confirmation prompt should be visible");
    assert!(message.contains("tool-call limit (32)"));
    assert!(message.contains("Confirm"));
    assert!(!message.to_ascii_lowercase().contains("quota"));
}
