use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::anyhow;
use async_stream::stream;
use futures::channel::oneshot;
use futures::StreamExt as _;
use http::StatusCode;
use reqwest::header::{HeaderMap, RETRY_AFTER};
use serde::Serialize;
use url::Url;
use uuid::Uuid;
use warp_multi_agent_api as api;

mod connection_test;
mod messages;
mod streaming;
mod tools;
mod transport;

pub(crate) use connection_test::test_provider_connection;
use messages::{
    agent_output_message, chat_messages_from_tasks, chat_messages_from_user_inputs,
    local_messages_from_inputs, ChatMessage, ChatRole,
};
#[cfg(test)]
pub(super) use streaming::content_deltas_from_sse_data;
use streaming::{completion_deltas_from_sse_data, SseDataParser};
use tools::{ChatToolDefinition, ToolCallAssembler, ToolCatalog};
use transport::{LocalProviderTransport, ReqwestLocalProviderTransport};

use super::{ConvertToAPITypeError, RequestParams, ResponseStream, ServerConversationToken};
use crate::server::server_api::AIApiError;

const DEFAULT_LOCAL_TOOL_CALL_LIMIT: usize = 32;
const MIN_LOCAL_TOOL_CALL_LIMIT: usize = 1;
const MAX_LOCAL_TOOL_CALL_LIMIT: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalProviderModel {
    base_url: String,
    api_key: String,
    model: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ChatToolDefinition>,
}

pub(super) async fn generate_local_provider_output(
    params: RequestParams,
    cancellation_rx: oneshot::Receiver<()>,
) -> Result<ResponseStream, ConvertToAPITypeError> {
    generate_local_provider_output_with_transport(
        params,
        cancellation_rx,
        Arc::new(ReqwestLocalProviderTransport),
    )
    .await
}

async fn generate_local_provider_output_with_transport(
    params: RequestParams,
    cancellation_rx: oneshot::Receiver<()>,
    transport: Arc<dyn LocalProviderTransport>,
) -> Result<ResponseStream, ConvertToAPITypeError> {
    let stream = stream! {
        let request_id = Uuid::new_v4().to_string();
        let conversation_id = params
            .conversation_token
            .as_ref()
            .map(ServerConversationToken::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        yield Ok(stream_init_event(conversation_id, request_id.clone()));

        let Some(provider_model) = resolve_local_provider_model(&params) else {
            yield Ok(stream_finished_event(api::response_event::stream_finished::Reason::InvalidApiKey(
                api::response_event::stream_finished::InvalidApiKey {
                    provider: api::LlmProvider::Openai.into(),
                    model_name: params.model.as_str().to_string(),
                },
            )));
            return;
        };

        let task_id = params
            .tasks
            .first()
            .map(|task| task.id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let input_messages = local_messages_from_inputs(&task_id, &request_id, &params.input);
        let input_action = if params.tasks.is_empty() {
            Some(create_task_action(api::Task {
                id: task_id.clone(),
                messages: input_messages,
                ..Default::default()
            }))
        } else if input_messages.is_empty() {
            None
        } else {
            Some(add_messages_action(&task_id, input_messages))
        };

        let tool_call_limit = match local_tool_call_limit() {
            Ok(limit) => limit,
            Err(error) => {
                yield Err(Arc::new(AIApiError::Other(anyhow!(error))));
                return;
            }
        };
        if should_pause_for_tool_call_limit(&params, tool_call_limit) {
            if let Some(action) = input_action.clone() {
                yield Ok(client_actions_event(vec![action]));
            }
            let message_id = Uuid::new_v4().to_string();
            let message = format!(
                "The local tool-call limit ({tool_call_limit}) was reached. Confirm that you want to continue."
            );
            yield Ok(client_actions_event(vec![add_message_action(
                &task_id,
                &message_id,
                &request_id,
                message,
            )]));
            yield Ok(stream_finished_event(
                api::response_event::stream_finished::Reason::Done(
                    api::response_event::stream_finished::Done {},
                ),
            ));
            return;
        }

        let request = match build_chat_completion_request(&params, provider_model.model.clone()) {
            Ok(request) => request,
            Err(error) => {
                yield Err(Arc::new(AIApiError::Other(anyhow!(error))));
                return;
            }
        };
        let tool_catalog = ToolCatalog::new(params.mcp_context.as_ref());

        let response = match transport.send(provider_model, request).await {
            Ok(response) => response,
            Err(error) => {
                yield Err(Arc::new(error));
                return;
            }
        };

        if !response.status.is_success() {
            yield Err(Arc::new(provider_status_error(response.status, &response.headers)));
            return;
        }

        if let Some(action) = input_action {
            yield Ok(client_actions_event(vec![action]));
        }

        let mut byte_stream = response.body;
        let mut parser = SseDataParser::default();
        let message_id = Uuid::new_v4().to_string();
        let mut created_message = false;
        let mut tool_calls = ToolCallAssembler::default();

        while let Some(chunk) = byte_stream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => {
                    yield Err(Arc::new(error));
                    return;
                }
            };

            let data_events = match parser.push_bytes(&chunk) {
                Ok(data_events) => data_events,
                Err(error) => {
                    yield Err(Arc::new(error));
                    return;
                }
            };

            for data in data_events {
                if data.trim() == "[DONE]" {
                    let messages = match tool_calls.finish(&task_id, &request_id, &tool_catalog) {
                        Ok(messages) => messages,
                        Err(error) => {
                            yield Err(Arc::new(AIApiError::Other(anyhow!(error))));
                            return;
                        }
                    };
                    if !messages.is_empty() {
                        yield Ok(client_actions_event(vec![add_messages_action(
                            &task_id,
                            messages,
                        )]));
                    }
                    yield Ok(stream_finished_event(
                        api::response_event::stream_finished::Reason::Done(
                            api::response_event::stream_finished::Done {},
                        ),
                    ));
                    return;
                }

                let completion_deltas = match completion_deltas_from_sse_data(&data) {
                    Ok(completion_deltas) => completion_deltas,
                    Err(error) => {
                        yield Err(Arc::new(error));
                        return;
                    }
                };

                for delta in completion_deltas {
                    if let Err(error) = tool_calls.push(delta.tool_calls) {
                        yield Err(Arc::new(AIApiError::Other(anyhow!(error))));
                        return;
                    }
                    let Some(content) = delta.content else {
                        continue;
                    };
                    if content.is_empty() {
                        continue;
                    }
                    let action = if created_message {
                        append_to_message_action(&task_id, &message_id, &request_id, content)
                    } else {
                        created_message = true;
                        add_message_action(&task_id, &message_id, &request_id, content)
                    };
                    yield Ok(client_actions_event(vec![action]));
                }
            }
        }

        yield Err(Arc::new(AIApiError::UnexpectedEof));
    };

    Ok(Box::pin(stream.take_until(cancellation_rx)))
}

fn local_tool_call_limit() -> Result<usize, &'static str> {
    match std::env::var("WARP_LOCAL_AGENT_TOOL_CALL_LIMIT") {
        Ok(value) => parse_local_tool_call_limit(Some(&value)),
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_LOCAL_TOOL_CALL_LIMIT),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err("WARP_LOCAL_AGENT_TOOL_CALL_LIMIT must be a UTF-8 integer between 1 and 128.")
        }
    }
}

fn parse_local_tool_call_limit(value: Option<&str>) -> Result<usize, &'static str> {
    let Some(value) = value else {
        return Ok(DEFAULT_LOCAL_TOOL_CALL_LIMIT);
    };
    let limit = value
        .parse::<usize>()
        .map_err(|_| "WARP_LOCAL_AGENT_TOOL_CALL_LIMIT must be an integer between 1 and 128.")?;
    if !(MIN_LOCAL_TOOL_CALL_LIMIT..=MAX_LOCAL_TOOL_CALL_LIMIT).contains(&limit) {
        return Err("WARP_LOCAL_AGENT_TOOL_CALL_LIMIT must be between 1 and 128.");
    }
    Ok(limit)
}

fn should_pause_for_tool_call_limit(params: &RequestParams, limit: usize) -> bool {
    let is_tool_follow_up = params
        .input
        .iter()
        .any(|input| matches!(input, crate::ai::agent::AIAgentInput::ActionResult { .. }));
    is_tool_follow_up && consecutive_tool_call_count(params) >= limit
}

fn consecutive_tool_call_count(params: &RequestParams) -> usize {
    params
        .tasks
        .first()
        .into_iter()
        .flat_map(|task| task.messages.iter().rev())
        .take_while(|message| {
            !matches!(
                message.message.as_ref(),
                Some(api::message::Message::UserQuery(_))
            )
        })
        .filter(|message| {
            matches!(
                message.message.as_ref(),
                Some(api::message::Message::ToolCall(_))
            )
        })
        .count()
}

fn resolve_local_provider_model(params: &RequestParams) -> Option<LocalProviderModel> {
    if !params.model_config_is_backed_by_custom_providers() {
        return None;
    }

    let selected_model = params.model.as_str();
    params
        .custom_model_providers
        .as_ref()?
        .providers
        .iter()
        .find_map(|provider| {
            provider
                .models
                .iter()
                .find(|model| model.config_key == selected_model)
                .map(|model| LocalProviderModel {
                    base_url: provider.base_url.clone(),
                    api_key: provider.api_key.clone(),
                    model: model.slug.clone(),
                })
        })
}

fn build_chat_completion_request(
    params: &RequestParams,
    model: String,
) -> Result<ChatCompletionRequest, &'static str> {
    let mut messages = chat_messages_from_tasks(&params.tasks, &params.input);
    messages.extend(chat_messages_from_user_inputs(&params.input));

    if messages.is_empty() {
        return Err("Local provider request requires at least one text message");
    }

    Ok(ChatCompletionRequest {
        model,
        messages,
        stream: true,
        tools: ToolCatalog::new(params.mcp_context.as_ref()).definitions(),
    })
}

pub(super) fn chat_completions_url(base_url: &str) -> Result<Url, AIApiError> {
    let mut url = Url::parse(base_url).map_err(|error| AIApiError::Other(anyhow!(error)))?;
    if url
        .path()
        .trim_end_matches('/')
        .ends_with("/chat/completions")
    {
        return Ok(url);
    }

    let path = format!("{}/chat/completions", url.path().trim_end_matches('/'));
    url.set_path(&path);
    Ok(url)
}

fn provider_status_error(status: StatusCode, headers: &HeaderMap) -> AIApiError {
    let message = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            "Provider authentication failed. Check the API Key and Provider permissions."
        }
        StatusCode::NOT_FOUND => {
            "Provider returned not found. Check the Base URL and configured model name."
        }
        StatusCode::REQUEST_TIMEOUT => {
            "Provider timed out while processing the request. Try again."
        }
        StatusCode::TOO_MANY_REQUESTS => {
            "Provider rate limit reached. Wait for the Provider limit to reset and try again."
        }
        status if status.is_server_error() => {
            "Provider server error. Check the Provider status and try again."
        }
        _ => "Provider rejected the request. Check the Provider configuration and model settings.",
    };
    AIApiError::ProviderErrorStatus {
        status,
        message: message.to_string(),
        retry_after: retry_after_duration(headers, SystemTime::now()),
    }
}

fn retry_after_duration(headers: &HeaderMap, now: SystemTime) -> Option<Duration> {
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    let retry_at = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    let retry_at = SystemTime::from(retry_at.with_timezone(&chrono::Utc));
    Some(retry_at.duration_since(now).unwrap_or(Duration::ZERO))
}

fn stream_init_event(conversation_id: String, request_id: String) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::Init(
            api::response_event::StreamInit {
                conversation_id,
                request_id,
                run_id: String::new(),
            },
        )),
    }
}

fn stream_finished_event(
    reason: api::response_event::stream_finished::Reason,
) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::Finished(
            api::response_event::StreamFinished {
                reason: Some(reason),
                ..Default::default()
            },
        )),
    }
}

fn client_actions_event(actions: Vec<api::ClientAction>) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions { actions },
        )),
    }
}

fn create_task_action(task: api::Task) -> api::ClientAction {
    api::ClientAction {
        action: Some(api::client_action::Action::CreateTask(
            api::client_action::CreateTask { task: Some(task) },
        )),
    }
}

fn add_message_action(
    task_id: &str,
    message_id: &str,
    request_id: &str,
    content: String,
) -> api::ClientAction {
    api::ClientAction {
        action: Some(api::client_action::Action::AddMessagesToTask(
            api::client_action::AddMessagesToTask {
                task_id: task_id.to_string(),
                messages: vec![agent_output_message(
                    task_id, message_id, request_id, content,
                )],
            },
        )),
    }
}

fn add_messages_action(task_id: &str, messages: Vec<api::Message>) -> api::ClientAction {
    api::ClientAction {
        action: Some(api::client_action::Action::AddMessagesToTask(
            api::client_action::AddMessagesToTask {
                task_id: task_id.to_string(),
                messages,
            },
        )),
    }
}

fn append_to_message_action(
    task_id: &str,
    message_id: &str,
    request_id: &str,
    content: String,
) -> api::ClientAction {
    api::ClientAction {
        action: Some(api::client_action::Action::AppendToMessageContent(
            api::client_action::AppendToMessageContent {
                task_id: task_id.to_string(),
                message: Some(agent_output_message(
                    task_id, message_id, request_id, content,
                )),
                mask: Some(prost_types::FieldMask {
                    paths: vec!["agent_output.text".to_string()],
                }),
            },
        )),
    }
}

#[cfg(test)]
#[path = "local_provider_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "local_provider/limit_tests.rs"]
mod limit_tests;

#[cfg(test)]
#[path = "local_provider/history_tests.rs"]
mod history_tests;

#[cfg(test)]
#[path = "local_provider/persistence_tests.rs"]
mod persistence_tests;
