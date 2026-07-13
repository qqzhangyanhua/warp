use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use futures::future::{select, Either};
use futures::StreamExt as _;
use serde::Deserialize;

use super::transport::{LocalProviderTransport, ReqwestLocalProviderTransport};
use super::{
    provider_status_error, ChatCompletionRequest, ChatMessage, ChatRole, LocalProviderModel,
};
use crate::server::server_api::AIApiError;

const TIMEOUT: Duration = Duration::from_secs(15);
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Deserialize)]
struct TestCompletionResponse {
    choices: Vec<TestCompletionChoice>,
}

#[derive(Deserialize)]
struct TestCompletionChoice {
    message: TestCompletionMessage,
}

#[derive(Deserialize)]
struct TestCompletionMessage {
    role: String,
    content: String,
}

#[expect(
    dead_code,
    reason = "used by the custom Provider settings UI increment"
)]
pub(crate) async fn test_provider_connection(
    base_url: String,
    api_key: String,
    model: String,
) -> Result<(), AIApiError> {
    test_provider_connection_with_transport(
        base_url,
        api_key,
        model,
        TIMEOUT,
        Arc::new(ReqwestLocalProviderTransport),
    )
    .await
}

pub(super) async fn test_provider_connection_with_transport(
    base_url: String,
    api_key: String,
    model: String,
    timeout: Duration,
    transport: Arc<dyn LocalProviderTransport>,
) -> Result<(), AIApiError> {
    let test = async move {
        let response = transport
            .send(
                LocalProviderModel {
                    base_url,
                    api_key,
                    model: model.clone(),
                },
                ChatCompletionRequest {
                    model,
                    messages: vec![ChatMessage {
                        role: ChatRole::User,
                        content: "ping".to_string(),
                    }],
                    stream: false,
                },
            )
            .await?;
        if !response.status.is_success() {
            return Err(provider_status_error(response.status, &response.headers));
        }

        let mut body = Vec::new();
        let mut chunks = response.body;
        while let Some(chunk) = chunks.next().await {
            let chunk = chunk?;
            if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(AIApiError::Other(anyhow!(
                    "Provider returned an oversized Chat Completions response"
                )));
            }
            body.extend_from_slice(&chunk);
        }
        let response: TestCompletionResponse = serde_json::from_slice(&body).map_err(|_| {
            AIApiError::Other(anyhow!(
                "Provider returned a malformed Chat Completions response"
            ))
        })?;
        if !response.choices.iter().any(|choice| {
            choice.message.role == "assistant" && !choice.message.content.trim().is_empty()
        }) {
            return Err(AIApiError::Other(anyhow!(
                "Provider returned a malformed Chat Completions response"
            )));
        }
        Ok(())
    };

    futures::pin_mut!(test);
    let timer = warpui::r#async::Timer::after(timeout);
    futures::pin_mut!(timer);
    match select(test, timer).await {
        Either::Left((result, _)) => result,
        Either::Right((_, _)) => Err(AIApiError::Other(anyhow!(
            "Provider connection test timed out"
        ))),
    }
}

#[cfg(test)]
#[path = "connection_test_tests.rs"]
mod tests;
