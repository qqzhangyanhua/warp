use std::collections::HashMap;
use std::sync::Arc;

use futures::channel::oneshot;
use futures_lite::StreamExt as _;
use warp_multi_agent_api as api;

use super::{
    build_chat_completion_request, content_deltas_from_sse_data, generate_local_provider_output,
    provider_status_error, resolve_local_provider_model, stream_finished_event, ChatRole,
    SseDataParser,
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
    let error = provider_status_error(http::StatusCode::TOO_MANY_REQUESTS, "slow down".to_string());

    assert!(
        matches!(error, AIApiError::ErrorStatus(status, _) if status == http::StatusCode::TOO_MANY_REQUESTS)
    );
    assert!(!matches!(error, AIApiError::QuotaLimit { .. }));
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
