use std::sync::Arc;

use anyhow::anyhow;
use async_stream::stream;
use futures::channel::oneshot;
use futures::StreamExt as _;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;
use warp_multi_agent_api as api;

use super::{ConvertToAPITypeError, RequestParams, ResponseStream, ServerConversationToken};
use crate::ai::agent::AIAgentInput;
use crate::server::server_api::AIApiError;

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
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ChatMessage {
    role: ChatRole,
    content: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ChatRole {
    User,
    Assistant,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    #[serde(default)]
    delta: ChatDelta,
}

#[derive(Debug, Default, Deserialize)]
struct ChatDelta {
    content: Option<String>,
}

#[derive(Debug, Default)]
struct SseDataParser {
    pending_line: Vec<u8>,
    pending_data: Vec<String>,
}

pub(super) async fn generate_local_provider_output(
    params: RequestParams,
    cancellation_rx: oneshot::Receiver<()>,
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

        let Some(task_id) = params.tasks.first().map(|task| task.id.clone()) else {
            yield Err(Arc::new(AIApiError::Other(anyhow!(
                "Local provider request requires an existing task"
            ))));
            return;
        };

        let request = match build_chat_completion_request(&params, provider_model.model.clone()) {
            Ok(request) => request,
            Err(error) => {
                yield Err(Arc::new(AIApiError::Other(anyhow!(error))));
                return;
            }
        };

        let response = match send_chat_completion_request(provider_model, request).await {
            Ok(response) => response,
            Err(error) => {
                yield Err(Arc::new(error));
                return;
            }
        };

        let mut byte_stream = response.bytes_stream();
        let mut parser = SseDataParser::default();
        let message_id = Uuid::new_v4().to_string();
        let mut created_message = false;

        while let Some(chunk) = byte_stream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => {
                    yield Err(Arc::new(AIApiError::from(error)));
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
                    yield Ok(stream_finished_event(
                        api::response_event::stream_finished::Reason::Done(
                            api::response_event::stream_finished::Done {},
                        ),
                    ));
                    return;
                }

                let content_deltas = match content_deltas_from_sse_data(&data) {
                    Ok(content_deltas) => content_deltas,
                    Err(error) => {
                        yield Err(Arc::new(error));
                        return;
                    }
                };

                for content in content_deltas {
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
    let mut messages = chat_messages_from_tasks(&params.tasks);
    messages.extend(chat_messages_from_inputs(&params.input));

    if messages.is_empty() {
        return Err("Local provider request requires at least one text message");
    }

    Ok(ChatCompletionRequest {
        model,
        messages,
        stream: true,
    })
}

fn chat_messages_from_tasks(tasks: &[api::Task]) -> Vec<ChatMessage> {
    tasks
        .iter()
        .flat_map(|task| task.messages.iter())
        .filter_map(chat_message_from_api_message)
        .collect()
}

fn chat_message_from_api_message(message: &api::Message) -> Option<ChatMessage> {
    match message.message.as_ref()? {
        api::message::Message::UserQuery(user_query) if !user_query.query.trim().is_empty() => {
            Some(ChatMessage {
                role: ChatRole::User,
                content: user_query.query.clone(),
            })
        }
        api::message::Message::AgentOutput(output) if !output.text.trim().is_empty() => {
            Some(ChatMessage {
                role: ChatRole::Assistant,
                content: output.text.clone(),
            })
        }
        _ => None,
    }
}

fn chat_messages_from_inputs(inputs: &[AIAgentInput]) -> Vec<ChatMessage> {
    inputs
        .iter()
        .filter_map(|input| match input {
            AIAgentInput::UserQuery { query, .. } if !query.trim().is_empty() => {
                Some(ChatMessage {
                    role: ChatRole::User,
                    content: query.clone(),
                })
            }
            _ => None,
        })
        .collect()
}

async fn send_chat_completion_request(
    provider_model: LocalProviderModel,
    request: ChatCompletionRequest,
) -> Result<reqwest::Response, AIApiError> {
    let url = chat_completions_url(&provider_model.base_url)?;
    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(provider_model.api_key)
        .json(&request)
        .send()
        .await
        .map_err(AIApiError::from)?;

    let status = response.status();
    if !status.is_success() {
        return Err(provider_status_error(status));
    }

    Ok(response)
}

fn chat_completions_url(base_url: &str) -> Result<Url, AIApiError> {
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

fn provider_status_error(status: StatusCode) -> AIApiError {
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
    AIApiError::ErrorStatus(status, message.to_string())
}

impl SseDataParser {
    fn push_bytes(&mut self, bytes: &[u8]) -> Result<Vec<String>, AIApiError> {
        let mut data_events = Vec::new();
        for byte in bytes {
            if *byte == b'\n' {
                if let Some(data) = self.finish_line()? {
                    data_events.push(data);
                }
            } else {
                self.pending_line.push(*byte);
            }
        }
        Ok(data_events)
    }

    fn finish_line(&mut self) -> Result<Option<String>, AIApiError> {
        if self.pending_line.last() == Some(&b'\r') {
            self.pending_line.pop();
        }
        let line_bytes = std::mem::take(&mut self.pending_line);
        let line =
            String::from_utf8(line_bytes).map_err(|error| AIApiError::Other(anyhow!(error)))?;

        if line.is_empty() {
            if self.pending_data.is_empty() {
                return Ok(None);
            }
            return Ok(Some(std::mem::take(&mut self.pending_data).join("\n")));
        }

        if let Some(data) = line.strip_prefix("data:") {
            self.pending_data.push(data.trim_start().to_string());
        }
        Ok(None)
    }
}

fn content_deltas_from_sse_data(data: &str) -> Result<Vec<String>, AIApiError> {
    let chunk: ChatCompletionChunk = serde_json::from_str(data).map_err(|_| {
        AIApiError::Other(anyhow!(
            "Provider returned a malformed Chat Completions stream. Check OpenAI compatibility."
        ))
    })?;
    Ok(chunk
        .choices
        .into_iter()
        .filter_map(|choice| choice.delta.content)
        .collect())
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
                    paths: vec!["message.agent_output.text".to_string()],
                }),
            },
        )),
    }
}

fn agent_output_message(
    task_id: &str,
    message_id: &str,
    request_id: &str,
    content: String,
) -> api::Message {
    api::Message {
        id: message_id.to_string(),
        task_id: task_id.to_string(),
        request_id: request_id.to_string(),
        message: Some(api::message::Message::AgentOutput(
            api::message::AgentOutput { text: content },
        )),
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "local_provider_tests.rs"]
mod tests;
