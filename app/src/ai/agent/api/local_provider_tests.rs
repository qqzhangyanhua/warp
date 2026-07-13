use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use futures::channel::oneshot;
use futures::stream;
use futures_lite::StreamExt as _;
use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};
use warp_multi_agent_api as api;
use warpui::r#async::BoxFuture;

use super::connection_test::test_provider_connection_with_transport;
use super::transport::{LocalProviderResponse, LocalProviderTransport, ProviderByteStream};
use super::{
    build_chat_completion_request, content_deltas_from_sse_data, generate_local_provider_output,
    generate_local_provider_output_with_transport, provider_status_error,
    resolve_local_provider_model, retry_after_duration, stream_finished_event,
    ChatCompletionRequest, ChatRole, LocalProviderModel, SseDataParser,
};
use crate::ai::agent::api::RequestParams;
use crate::ai::agent::{AIAgentInput, UserQueryMode};
use crate::ai::llms::LLMId;
use crate::server::server_api::AIApiError;

fn custom_model_providers(config_key: &str) -> api::request::settings::CustomModelProviders {
    api::request::settings::CustomModelProviders {
        providers: vec![
            api::request::settings::custom_model_providers::CustomModelProvider {
                base_url: "http://localhost:8080/v1".to_string(),
                api_key: "provider-key".to_string(),
                models: vec![
                    api::request::settings::custom_model_providers::CustomModel {
                        slug: "provider-model".to_string(),
                        config_key: config_key.to_string(),
                    },
                ],
            },
        ],
    }
}

fn user_query(query: &str) -> AIAgentInput {
    AIAgentInput::UserQuery {
        query: query.to_string(),
        context: Arc::from([]),
        static_query_type: None,
        referenced_attachments: HashMap::new(),
        user_query_mode: UserQueryMode::Normal,
        running_command: None,
        intended_agent: None,
    }
}

fn text_message(role: ChatRole, content: &str, id: &str, task_id: &str) -> api::Message {
    let message = match role {
        ChatRole::User => api::message::Message::UserQuery(api::message::UserQuery {
            query: content.to_string(),
            ..Default::default()
        }),
        ChatRole::Assistant => api::message::Message::AgentOutput(api::message::AgentOutput {
            text: content.to_string(),
        }),
    };

    api::Message {
        id: id.to_string(),
        task_id: task_id.to_string(),
        message: Some(message),
        ..Default::default()
    }
}

fn params_with_custom_model() -> RequestParams {
    let mut params = RequestParams::new_for_test();
    let model = LLMId::from("custom-model");
    params.model = model.clone();
    params.coding_model = model.clone();
    params.cli_agent_model = model.clone();
    params.computer_use_model = model;
    params.custom_model_providers = Some(custom_model_providers("custom-model"));
    params.tasks = vec![api::Task {
        id: "task-1".to_string(),
        ..Default::default()
    }];
    params.input = vec![user_query("hello")];
    params
}

struct FakeProviderTransport {
    captured_request: Arc<Mutex<Option<(LocalProviderModel, ChatCompletionRequest)>>>,
}

struct FakeConnectionTestTransport {
    captured_request: Arc<Mutex<Option<(LocalProviderModel, ChatCompletionRequest)>>>,
}

impl LocalProviderTransport for FakeConnectionTestTransport {
    fn send(
        &self,
        provider_model: LocalProviderModel,
        request: ChatCompletionRequest,
    ) -> BoxFuture<'static, Result<LocalProviderResponse, AIApiError>> {
        *self.captured_request.lock().unwrap() = Some((provider_model, request));
        Box::pin(async {
            Ok(LocalProviderResponse {
                status: http::StatusCode::OK,
                headers: HeaderMap::new(),
                body: Box::pin(stream::iter([Ok(Bytes::from_static(
                    br#"{"choices":[{"message":{"role":"assistant","content":"ok"}}]}"#,
                ))])) as ProviderByteStream,
            })
        })
    }
}

#[test]
fn connection_test_sends_one_minimal_non_streaming_request() {
    let captured_request = Arc::new(Mutex::new(None));
    let transport = Arc::new(FakeConnectionTestTransport {
        captured_request: captured_request.clone(),
    });

    futures::executor::block_on(test_provider_connection_with_transport(
        "https://provider.example/v1".to_string(),
        "provider-key".to_string(),
        "provider-model".to_string(),
        Duration::from_secs(15),
        transport,
    ))
    .expect("connection test should succeed");

    let captured = captured_request
        .lock()
        .unwrap()
        .clone()
        .expect("request should be sent");
    assert_eq!(captured.0.base_url, "https://provider.example/v1");
    assert_eq!(captured.1.model, "provider-model");
    assert!(!captured.1.stream);
    assert_eq!(captured.1.messages.len(), 1);
    assert_eq!(captured.1.messages[0].role, ChatRole::User);
    assert_eq!(captured.1.messages[0].content, "ping");
}

impl LocalProviderTransport for FakeProviderTransport {
    fn send(
        &self,
        provider_model: LocalProviderModel,
        request: ChatCompletionRequest,
    ) -> BoxFuture<'static, Result<LocalProviderResponse, AIApiError>> {
        *self.captured_request.lock().unwrap() = Some((provider_model, request));
        Box::pin(async {
            Ok(LocalProviderResponse {
                status: http::StatusCode::OK,
                headers: HeaderMap::new(),
                body: Box::pin(stream::iter([
                    Ok(Bytes::from_static(
                        br#"data: {"choices":[{"delta":{"content":"hel"}}]}

"#,
                    )),
                    Ok(Bytes::from_static(
                        br#"data: {"choices":[{"delta":{"content":"lo"}}]}

data: [DONE]

"#,
                    )),
                ])) as ProviderByteStream,
            })
        })
    }
}

#[test]
fn agent_event_stream_sends_typed_request_and_streams_provider_text() {
    let captured_request = Arc::new(Mutex::new(None));
    let transport = Arc::new(FakeProviderTransport {
        captured_request: captured_request.clone(),
    });
    let params = params_with_custom_model();
    let (_tx, rx) = oneshot::channel();

    let mut output = futures::executor::block_on(generate_local_provider_output_with_transport(
        params, rx, transport,
    ))
    .expect("stream should be created");
    let events = futures::executor::block_on(async {
        let mut events = Vec::new();
        while let Some(event) = output.next().await {
            events.push(event.expect("provider event should succeed"));
        }
        events
    });

    let captured = captured_request
        .lock()
        .unwrap()
        .clone()
        .expect("request should be sent");
    assert_eq!(captured.0.base_url, "http://localhost:8080/v1");
    assert_eq!(captured.0.model, "provider-model");
    assert_eq!(captured.1.model, "provider-model");
    assert_eq!(captured.1.messages.len(), 1);
    assert_eq!(captured.1.messages[0].content, "hello");

    assert_eq!(events.len(), 4);
    assert!(matches!(
        events[0].r#type,
        Some(api::response_event::Type::Init(_))
    ));
    assert!(matches!(
        events[1].r#type,
        Some(api::response_event::Type::ClientActions(_))
    ));
    assert!(matches!(
        events[2].r#type,
        Some(api::response_event::Type::ClientActions(_))
    ));
    assert!(matches!(
        events[3].r#type,
        Some(api::response_event::Type::Finished(_))
    ));

    let Some(api::response_event::Type::ClientActions(first_actions)) = &events[1].r#type else {
        panic!("expected first text action");
    };
    let Some(api::client_action::Action::AddMessagesToTask(first_text)) =
        &first_actions.actions[0].action
    else {
        panic!("expected add-message action");
    };
    let Some(api::message::Message::AgentOutput(first_output)) = &first_text.messages[0].message
    else {
        panic!("expected first agent output");
    };
    assert_eq!(first_output.text, "hel");

    let Some(api::response_event::Type::ClientActions(second_actions)) = &events[2].r#type else {
        panic!("expected second text action");
    };
    let Some(api::client_action::Action::AppendToMessageContent(second_text)) =
        &second_actions.actions[0].action
    else {
        panic!("expected append-message action");
    };
    let Some(api::message::Message::AgentOutput(second_output)) = second_text
        .message
        .as_ref()
        .and_then(|message| message.message.as_ref())
    else {
        panic!("expected appended agent output");
    };
    assert_eq!(second_output.text, "lo");
}

#[test]
fn resolves_selected_custom_provider_model() {
    let params = params_with_custom_model();

    let model = resolve_local_provider_model(&params).expect("custom model should resolve");

    assert_eq!(model.base_url, "http://localhost:8080/v1");
    assert_eq!(model.api_key, "provider-key");
    assert_eq!(model.model, "provider-model");
}

#[test]
fn refuses_local_provider_model_when_auxiliary_model_is_hosted() {
    let mut params = params_with_custom_model();
    params.coding_model = LLMId::from("hosted-model");

    assert!(resolve_local_provider_model(&params).is_none());
}

#[test]
fn builds_chat_request_from_local_history_and_latest_user_input() {
    let mut params = params_with_custom_model();
    params.tasks = vec![api::Task {
        id: "task-1".to_string(),
        messages: vec![
            text_message(ChatRole::User, "previous question", "m1", "task-1"),
            text_message(ChatRole::Assistant, "previous answer", "m2", "task-1"),
        ],
        ..Default::default()
    }];
    params.input = vec![user_query("next question")];

    let request = build_chat_completion_request(&params, "provider-model".to_string())
        .expect("chat request should build");

    assert_eq!(request.model, "provider-model");
    assert!(request.stream);
    assert_eq!(request.messages.len(), 3);
    assert_eq!(request.messages[0].role, ChatRole::User);
    assert_eq!(request.messages[0].content, "previous question");
    assert_eq!(request.messages[1].role, ChatRole::Assistant);
    assert_eq!(request.messages[1].content, "previous answer");
    assert_eq!(request.messages[2].role, ChatRole::User);
    assert_eq!(request.messages[2].content, "next question");
}

#[test]
fn parses_fragmented_openai_sse_text_and_done_events() {
    let mut parser = SseDataParser::default();
    let first = br#"data: {"choices":[{"delta":{"content":"hel"#;
    let second = br#"lo"}}]}

data: [DONE]

"#;

    assert!(parser.push_bytes(first).unwrap().is_empty());
    let events = parser.push_bytes(second).unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(
        content_deltas_from_sse_data(&events[0]).unwrap(),
        vec!["hello".to_string()]
    );
    assert_eq!(events[1], "[DONE]");
}

#[test]
fn provider_rate_limit_is_not_warp_quota() {
    let error = provider_status_error(http::StatusCode::TOO_MANY_REQUESTS, &HeaderMap::new());

    assert!(
        matches!(error, AIApiError::ProviderErrorStatus { status, .. } if status == http::StatusCode::TOO_MANY_REQUESTS)
    );
    assert!(!matches!(error, AIApiError::QuotaLimit { .. }));
}

#[test]
fn provider_status_errors_are_distinct_and_actionable() {
    let cases = [
        (
            http::StatusCode::UNAUTHORIZED,
            "Provider authentication failed. Check the API Key and Provider permissions.",
        ),
        (
            http::StatusCode::NOT_FOUND,
            "Provider returned not found. Check the Base URL and configured model name.",
        ),
        (
            http::StatusCode::REQUEST_TIMEOUT,
            "Provider timed out while processing the request. Try again.",
        ),
        (
            http::StatusCode::TOO_MANY_REQUESTS,
            "Provider rate limit reached. Wait for the Provider limit to reset and try again.",
        ),
        (
            http::StatusCode::BAD_GATEWAY,
            "Provider server error. Check the Provider status and try again.",
        ),
    ];

    for (status, expected_message) in cases {
        let AIApiError::ProviderErrorStatus {
            status: actual_status,
            message: actual_message,
            retry_after,
        } = provider_status_error(status, &HeaderMap::new())
        else {
            panic!("expected status error");
        };
        assert_eq!(actual_status, status);
        assert_eq!(actual_message, expected_message);
        assert_eq!(retry_after, None);
    }
}

#[test]
fn parses_retry_after_seconds_and_http_date() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let mut headers = HeaderMap::new();
    headers.insert(RETRY_AFTER, HeaderValue::from_static("12"));
    assert_eq!(
        retry_after_duration(&headers, now),
        Some(Duration::from_secs(12))
    );

    headers.insert(
        RETRY_AFTER,
        HeaderValue::from_static("Tue, 14 Nov 2023 22:13:25 GMT"),
    );
    assert_eq!(
        retry_after_duration(&headers, now),
        Some(Duration::from_secs(5))
    );
}

#[test]
fn ignores_invalid_retry_after() {
    let mut headers = HeaderMap::new();
    headers.insert(RETRY_AFTER, HeaderValue::from_static("not-a-delay"));

    assert_eq!(retry_after_duration(&headers, SystemTime::now()), None);
}

#[test]
fn malformed_provider_stream_does_not_expose_response_content() {
    let sensitive_response = r#"{"error":"secret-token-123""#;
    let error = content_deltas_from_sse_data(sensitive_response).unwrap_err();
    let message = error.to_string();

    assert_eq!(
        message,
        "Provider returned a malformed Chat Completions stream. Check OpenAI compatibility."
    );
    assert!(!message.contains("secret-token-123"));
}

#[test]
fn local_finished_event_is_done_not_quota() {
    let event = stream_finished_event(api::response_event::stream_finished::Reason::Done(
        api::response_event::stream_finished::Done {},
    ));

    let Some(api::response_event::Type::Finished(finished)) = event.r#type else {
        panic!("expected finished event");
    };
    assert!(matches!(
        finished.reason,
        Some(api::response_event::stream_finished::Reason::Done(_))
    ));
    assert!(!matches!(
        finished.reason,
        Some(api::response_event::stream_finished::Reason::QuotaLimit(_))
    ));
}

#[test]
fn missing_custom_provider_finishes_with_invalid_key_not_quota() {
    let mut params = RequestParams::new_for_test();
    params.input = vec![user_query("hello")];
    params.tasks = vec![api::Task {
        id: "task-1".to_string(),
        ..Default::default()
    }];
    let (_tx, rx) = oneshot::channel();

    let mut stream = futures::executor::block_on(generate_local_provider_output(params, rx))
        .expect("stream should be created");
    let events: Vec<_> = futures::executor::block_on(async {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event.expect("event should be ok"));
        }
        events
    });

    assert_eq!(events.len(), 2);
    let Some(api::response_event::Type::Finished(finished)) =
        events.last().and_then(|event| event.r#type.clone())
    else {
        panic!("expected finished event");
    };
    assert!(matches!(
        finished.reason,
        Some(api::response_event::stream_finished::Reason::InvalidApiKey(
            _
        ))
    ));
    assert!(!matches!(
        finished.reason,
        Some(api::response_event::stream_finished::Reason::QuotaLimit(_))
    ));
}
