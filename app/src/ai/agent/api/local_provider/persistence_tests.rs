use std::sync::Arc;

use bytes::Bytes;
use futures::channel::oneshot;
use futures::stream;
use futures_lite::StreamExt as _;
use reqwest::header::HeaderMap;
use warp_multi_agent_api as api;
use warpui::r#async::BoxFuture;

use super::tests::params_with_custom_model;
use super::transport::{LocalProviderResponse, LocalProviderTransport, ProviderByteStream};
use super::{
    generate_local_provider_output_with_transport, ChatCompletionRequest, LocalProviderModel,
};
use crate::ai::agent::task::TaskId;
use crate::ai::agent::{
    AIAgentActionId, AIAgentActionResult, AIAgentActionResultType, AIAgentInput,
    RequestCommandOutputResult,
};
use crate::server::server_api::AIApiError;

struct StatusTransport(http::StatusCode);

impl LocalProviderTransport for StatusTransport {
    fn send(
        &self,
        _provider_model: LocalProviderModel,
        _request: ChatCompletionRequest,
    ) -> BoxFuture<'static, Result<LocalProviderResponse, AIApiError>> {
        let status = self.0;
        Box::pin(async move {
            Ok(LocalProviderResponse {
                status,
                headers: HeaderMap::new(),
                body: Box::pin(stream::iter([Ok(Bytes::from_static(b"data: [DONE]\n\n"))]))
                    as ProviderByteStream,
            })
        })
    }
}

fn persisted_input_message(params: crate::ai::agent::api::RequestParams) -> api::Message {
    let (_tx, rx) = oneshot::channel();
    let mut output = futures::executor::block_on(generate_local_provider_output_with_transport(
        params,
        rx,
        Arc::new(StatusTransport(http::StatusCode::OK)),
    ))
    .expect("stream should be created");

    futures::executor::block_on(async {
        while let Some(event) = output.next().await {
            let event = event.expect("provider event should succeed");
            let Some(api::response_event::Type::ClientActions(actions)) = event.r#type else {
                continue;
            };
            let Some(api::client_action::Action::AddMessagesToTask(add)) = actions
                .actions
                .first()
                .and_then(|action| action.action.as_ref())
            else {
                continue;
            };
            return add.messages[0].clone();
        }
        panic!("expected input persistence action");
    })
}

#[test]
fn accepted_user_query_is_persisted_before_provider_output() {
    let message = persisted_input_message(params_with_custom_model());

    let Some(api::message::Message::UserQuery(user_query)) = message.message else {
        panic!("expected the accepted user query to be persisted");
    };
    assert_eq!(user_query.query, "hello");
}

#[test]
fn accepted_tool_result_is_persisted_before_provider_output() {
    let mut params = params_with_custom_model();
    params.tasks[0].messages = vec![api::Message {
        id: "message-call".to_string(),
        task_id: "task-1".to_string(),
        request_id: "request-call".to_string(),
        message: Some(api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: "call-shell".to_string(),
            tool: Some(api::message::tool_call::Tool::RunShellCommand(
                api::message::tool_call::RunShellCommand {
                    command: "pwd".to_string(),
                    ..Default::default()
                },
            )),
        })),
        ..Default::default()
    }];
    params.input = vec![AIAgentInput::ActionResult {
        result: AIAgentActionResult {
            id: AIAgentActionId::from("call-shell".to_string()),
            task_id: TaskId::new("task-1".to_string()),
            result: AIAgentActionResultType::RequestCommandOutput(
                RequestCommandOutputResult::CancelledBeforeExecution,
            ),
        },
        context: Arc::from([]),
    }];

    let message = persisted_input_message(params);
    let Some(api::message::Message::ToolCallResult(tool_result)) = message.message else {
        panic!("expected the accepted tool result to be persisted");
    };
    assert_eq!(tool_result.tool_call_id, "call-shell");
    assert!(matches!(
        tool_result.result,
        Some(api::message::tool_call_result::Result::Cancel(()))
    ));
}

#[test]
fn rejected_request_does_not_persist_input() {
    let (_tx, rx) = oneshot::channel();
    let mut output = futures::executor::block_on(generate_local_provider_output_with_transport(
        params_with_custom_model(),
        rx,
        Arc::new(StatusTransport(http::StatusCode::BAD_REQUEST)),
    ))
    .expect("stream should be created");
    let events = futures::executor::block_on(async {
        let mut events = Vec::new();
        while let Some(event) = output.next().await {
            events.push(event);
        }
        events
    });

    assert_eq!(events.len(), 2);
    assert!(events[1].is_err());
    assert!(!events.iter().any(|event| matches!(
        event,
        Ok(api::ResponseEvent {
            r#type: Some(api::response_event::Type::ClientActions(_)),
        })
    )));
}
