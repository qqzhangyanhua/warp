use std::pin::Pin;

use futures::StreamExt as _;
use http::StatusCode;
use reqwest::header::HeaderMap;
use warpui::r#async::BoxFuture;

use super::{chat_completions_url, ChatCompletionRequest, LocalProviderModel};
use crate::server::server_api::AIApiError;

#[cfg(not(target_family = "wasm"))]
pub(super) type ProviderByteStream =
    Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, AIApiError>> + Send + 'static>>;

#[cfg(target_family = "wasm")]
pub(super) type ProviderByteStream =
    Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, AIApiError>> + 'static>>;

pub(super) struct LocalProviderResponse {
    pub(super) status: StatusCode,
    pub(super) headers: HeaderMap,
    pub(super) body: ProviderByteStream,
}

pub(super) trait LocalProviderTransport: Send + Sync {
    fn send(
        &self,
        provider_model: LocalProviderModel,
        request: ChatCompletionRequest,
    ) -> BoxFuture<'static, Result<LocalProviderResponse, AIApiError>>;
}

pub(super) struct ReqwestLocalProviderTransport;

impl LocalProviderTransport for ReqwestLocalProviderTransport {
    fn send(
        &self,
        provider_model: LocalProviderModel,
        request: ChatCompletionRequest,
    ) -> BoxFuture<'static, Result<LocalProviderResponse, AIApiError>> {
        Box::pin(async move {
            let url = chat_completions_url(&provider_model.base_url)?;
            let response = reqwest::Client::new()
                .post(url)
                .bearer_auth(provider_model.api_key)
                .json(&request)
                .send()
                .await
                .map_err(AIApiError::from)?;
            let status = response.status();
            let headers = response.headers().clone();
            let body = Box::pin(
                response
                    .bytes_stream()
                    .map(|chunk| chunk.map_err(AIApiError::from)),
            ) as ProviderByteStream;
            Ok(LocalProviderResponse {
                status,
                headers,
                body,
            })
        })
    }
}
