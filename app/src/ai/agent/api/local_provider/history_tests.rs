use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures::channel::oneshot;
use futures::stream;
use futures_lite::StreamExt as _;
use reqwest::header::HeaderMap;
use warp_multi_agent_api as api;
use warpui::r#async::BoxFuture;

use super::tests::{params_with_custom_model, user_query};
use super::transport::{LocalProviderResponse, LocalProviderTransport, ProviderByteStream};
use super::{
    generate_local_provider_output_with_transport, ChatCompletionRequest, LocalProviderModel,
};
use crate::server::server_api::AIApiError;

struct ValidatingHistoryTransport {
    captured_request: Arc<Mutex<Option<ChatCompletionRequest>>>,
}

impl LocalProviderTransport for ValidatingHistoryTransport {
    fn send(
        &self,
        _provider_model: LocalProviderModel,
        request: ChatCompletionRequest,
    ) -> BoxFuture<'static, Result<LocalProviderResponse, AIApiError>> {
        let status = if request_has_invalid_tool_sequence(&request) {
            http::StatusCode::BAD_REQUEST
        } else {
            http::StatusCode::OK
        };
        *self.captured_request.lock().unwrap() = Some(request);
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

fn request_has_invalid_tool_sequence(request: &ChatCompletionRequest) -> bool {
    let request = serde_json::to_value(request).expect("request should serialize");
    let mut pending_tool_call_ids = Vec::new();

    for message in request["messages"].as_array().unwrap() {
        match message["role"].as_str().unwrap() {
            "assistant" => {
                if !pending_tool_call_ids.is_empty() {
                    return true;
                }
                pending_tool_call_ids.extend(
                    message["tool_calls"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|call| call["id"].as_str().map(str::to_owned)),
                );
            }
            "tool" => {
                let Some(tool_call_id) = message["tool_call_id"].as_str() else {
                    return true;
                };
                if pending_tool_call_ids.first().map(String::as_str) != Some(tool_call_id) {
                    return true;
                }
                pending_tool_call_ids.remove(0);
            }
            "user" if !pending_tool_call_ids.is_empty() => return true,
            "user" => {}
            role => panic!("unexpected chat role: {role}"),
        }
    }

    !pending_tool_call_ids.is_empty()
}

fn projected_messages(messages: Vec<api::Message>) -> Vec<serde_json::Value> {
    let mut params = params_with_custom_model();
    params.tasks[0].messages = messages;
    params.input = vec![user_query("Who are you?")];
    let (_tx, rx) = oneshot::channel();
    let captured_request = Arc::new(Mutex::new(None));
    let mut output = futures::executor::block_on(generate_local_provider_output_with_transport(
        params,
        rx,
        Arc::new(ValidatingHistoryTransport {
            captured_request: captured_request.clone(),
        }),
    ))
    .expect("stream should be created");

    let accepted = futures::executor::block_on(async {
        while let Some(event) = output.next().await {
            if event.is_err() {
                return false;
            }
        }
        true
    });
    assert!(accepted, "projected history should be provider-valid");

    let request = captured_request
        .lock()
        .unwrap()
        .take()
        .expect("request should be captured");
    serde_json::to_value(request).unwrap()["messages"]
        .as_array()
        .unwrap()
        .clone()
}

fn tool_call(id: &str, request_id: &str, tool: api::message::tool_call::Tool) -> api::Message {
    api::Message {
        id: format!("message-{id}"),
        task_id: "task-1".to_string(),
        request_id: request_id.to_string(),
        message: Some(api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: id.to_string(),
            tool: Some(tool),
        })),
        ..Default::default()
    }
}

fn shell_call(id: &str, request_id: &str) -> api::Message {
    tool_call(
        id,
        request_id,
        api::message::tool_call::Tool::RunShellCommand(api::message::tool_call::RunShellCommand {
            command: "pwd".to_string(),
            ..Default::default()
        }),
    )
}

fn read_call(id: &str, request_id: &str) -> api::Message {
    tool_call(
        id,
        request_id,
        api::message::tool_call::Tool::ReadFiles(api::message::tool_call::ReadFiles::default()),
    )
}

fn cancelled_result(id: &str) -> api::Message {
    api::Message {
        id: format!("message-result-{id}"),
        task_id: "task-1".to_string(),
        request_id: "request-result".to_string(),
        message: Some(api::message::Message::ToolCallResult(
            api::message::ToolCallResult {
                tool_call_id: id.to_string(),
                result: Some(api::message::tool_call_result::Result::Cancel(())),
                context: None,
            },
        )),
        ..Default::default()
    }
}

#[test]
fn later_user_query_omits_unresolved_legacy_tool_call() {
    let messages = vec![
        shell_call("call-shell", "request-call"),
        api::Message {
            id: "message-output".to_string(),
            task_id: "task-1".to_string(),
            request_id: "request-output".to_string(),
            message: Some(api::message::Message::AgentOutput(
                api::message::AgentOutput {
                    text: "The command completed.".to_string(),
                },
            )),
            ..Default::default()
        },
    ];

    let messages = projected_messages(messages);

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "assistant");
    assert_eq!(messages[0]["content"], "The command completed.");
    assert!(messages
        .iter()
        .all(|message| message["tool_calls"].is_null()));
}

#[test]
fn later_user_query_replays_persisted_tool_result() {
    let messages = vec![
        shell_call("call-shell", "request-call"),
        cancelled_result("call-shell"),
    ];

    let messages = projected_messages(messages);

    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0]["tool_calls"][0]["id"], "call-shell");
    assert_eq!(messages[1]["role"], "tool");
    assert_eq!(messages[1]["tool_call_id"], "call-shell");
}

#[test]
fn parallel_tool_results_are_grouped_and_reordered_to_match_calls() {
    let messages = vec![
        shell_call("call-shell", "request-call"),
        read_call("call-read", "request-call"),
        cancelled_result("call-read"),
        cancelled_result("call-shell"),
    ];

    let messages = projected_messages(messages);

    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0]["tool_calls"][0]["id"], "call-shell");
    assert_eq!(messages[0]["tool_calls"][1]["id"], "call-read");
    assert_eq!(messages[1]["tool_call_id"], "call-shell");
    assert_eq!(messages[2]["tool_call_id"], "call-read");
}

#[test]
fn partially_resolved_parallel_legacy_calls_are_omitted_together() {
    let messages = vec![
        shell_call("call-shell", "request-call"),
        read_call("call-read", "request-call"),
        cancelled_result("call-shell"),
    ];

    let messages = projected_messages(messages);

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
}

#[test]
fn reused_tool_call_id_does_not_resolve_an_earlier_run() {
    let messages = vec![
        shell_call("call-reused", "request-old"),
        api::Message {
            id: "message-output".to_string(),
            task_id: "task-1".to_string(),
            request_id: "request-output".to_string(),
            message: Some(api::message::Message::AgentOutput(
                api::message::AgentOutput {
                    text: "Keep this text.".to_string(),
                },
            )),
            ..Default::default()
        },
        shell_call("call-reused", "request-new"),
        cancelled_result("call-reused"),
    ];

    let messages = projected_messages(messages);

    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0]["content"], "Keep this text.");
    assert_eq!(messages[1]["tool_calls"][0]["id"], "call-reused");
    assert_eq!(messages[2]["tool_call_id"], "call-reused");
    assert_eq!(messages[3]["role"], "user");
}
