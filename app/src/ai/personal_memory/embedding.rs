use std::sync::Arc;
use std::time::Duration;

use ai::api_keys::EmbeddingProviderConfig;
use futures::future::BoxFuture;
use futures::StreamExt as _;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

const EMBEDDING_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_EMBEDDING_INPUTS: usize = 5;
const MAX_EMBEDDING_INPUT_BYTES: usize = 16 * 1024;
const MAX_EMBEDDING_REQUEST_BYTES: usize = 32 * 1024;
const MAX_EMBEDDING_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_EMBEDDING_DIMENSIONS: usize = 8_192;
const MAX_REDIRECTS: usize = 3;
#[cfg(any(test, feature = "personal_memory"))]
const BILINGUAL_COMPATIBILITY_THRESHOLD: f32 = 0.25;

#[derive(Clone)]
pub(crate) struct EmbeddingProvider {
    endpoint: Url,
    origin: String,
    model: String,
    api_key: String,
    index_identity: String,
    client: reqwest::Client,
}

impl EmbeddingProvider {
    pub(crate) fn new(config: EmbeddingProviderConfig) -> Result<Self, EmbeddingProviderError> {
        let endpoint = embeddings_url(&config.base_url)?;
        if config.model.trim().is_empty() {
            return Err(EmbeddingProviderError::MissingModel);
        }
        if config.api_key.trim().is_empty() {
            return Err(EmbeddingProviderError::Authentication);
        }
        let origin = endpoint.origin().ascii_serialization();
        let allowed_origin = origin.clone();
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= MAX_REDIRECTS
                    || attempt.url().origin().ascii_serialization() != allowed_origin
                {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .map_err(|_| EmbeddingProviderError::Transport)?;
        let index_identity = index_identity(&endpoint, config.model.trim());
        Ok(Self {
            endpoint,
            origin,
            model: config.model.trim().to_string(),
            api_key: config.api_key,
            index_identity,
            client,
        })
    }

    #[cfg(any(test, feature = "personal_memory"))]
    pub(crate) async fn test_connection(
        &self,
    ) -> Result<EmbeddingConnectionTestResult, EmbeddingProviderError> {
        let vectors = self
            .embed_inputs(vec![
                "我的 GitHub 帐号".to_string(),
                "my GitHub account".to_string(),
            ])
            .await?;
        let similarity = cosine_similarity(&vectors[0], &vectors[1])
            .ok_or(EmbeddingProviderError::MalformedProtocol)?;
        Ok(if similarity >= BILINGUAL_COMPATIBILITY_THRESHOLD {
            EmbeddingConnectionTestResult::Compatible
        } else {
            EmbeddingConnectionTestResult::CompatibilityWarning
        })
    }

    async fn embed_inputs(
        &self,
        inputs: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, EmbeddingProviderError> {
        validate_inputs(&inputs)?;
        let expected_count = inputs.len();
        let response = self
            .client
            .post(self.endpoint.clone())
            .bearer_auth(&self.api_key)
            .timeout(EMBEDDING_TIMEOUT)
            .json(&EmbeddingRequest {
                model: &self.model,
                input: &inputs,
            })
            .send()
            .await
            .map_err(classify_transport_error)?;
        if response.url().origin().ascii_serialization() != self.origin {
            return Err(EmbeddingProviderError::Transport);
        }
        if !response.status().is_success() {
            return Err(classify_status(response.status()));
        }

        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(classify_transport_error)?;
            if body.len().saturating_add(chunk.len()) > MAX_EMBEDDING_RESPONSE_BYTES {
                return Err(EmbeddingProviderError::MalformedProtocol);
            }
            body.extend_from_slice(&chunk);
        }
        parse_response(&body, expected_count)
    }

    #[cfg(test)]
    pub(crate) fn endpoint(&self) -> &Url {
        &self.endpoint
    }

    #[cfg(test)]
    pub(crate) fn origin(&self) -> &str {
        &self.origin
    }
}

pub(crate) trait EmbeddingClient: Send + Sync {
    fn index_identity(&self) -> &str;
    fn embed(&self, input: String) -> BoxFuture<'static, Result<Vec<f32>, EmbeddingProviderError>>;
}

impl EmbeddingClient for EmbeddingProvider {
    fn index_identity(&self) -> &str {
        &self.index_identity
    }

    fn embed(&self, input: String) -> BoxFuture<'static, Result<Vec<f32>, EmbeddingProviderError>> {
        let provider = self.clone();
        Box::pin(async move {
            provider
                .embed_inputs(vec![input])
                .await?
                .pop()
                .ok_or(EmbeddingProviderError::MalformedProtocol)
        })
    }
}

pub(crate) type SharedEmbeddingClient = Arc<dyn EmbeddingClient>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(test, feature = "personal_memory"))]
pub(crate) enum EmbeddingConnectionTestResult {
    Compatible,
    CompatibilityWarning,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum EmbeddingProviderError {
    #[error("Embedding Provider authentication failed")]
    Authentication,
    #[error("Embedding Provider model is missing")]
    MissingModel,
    #[error("Embedding Provider returned a malformed protocol response")]
    MalformedProtocol,
    #[error("Embedding Provider request timed out")]
    Timeout,
    #[error("Embedding Provider rate limited the request")]
    RateLimited,
    #[error("Embedding Provider returned a server error")]
    Server,
    #[error("Embedding Provider transport failed")]
    Transport,
}

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingItem>,
}

#[derive(Deserialize)]
struct EmbeddingItem {
    index: usize,
    embedding: Vec<f32>,
}

fn embeddings_url(base_url: &str) -> Result<Url, EmbeddingProviderError> {
    let mut url = Url::parse(base_url).map_err(|_| EmbeddingProviderError::Transport)?;
    if !matches!(url.scheme(), "http" | "https") || !url.has_host() {
        return Err(EmbeddingProviderError::Transport);
    }
    url.set_query(None);
    url.set_fragment(None);
    let path = url.path().trim_end_matches('/');
    let path = if path.ends_with("/v1/embeddings") {
        path.to_string()
    } else if path.ends_with("/v1") {
        format!("{path}/embeddings")
    } else {
        format!("{path}/v1/embeddings")
    };
    url.set_path(&path);
    Ok(url)
}

fn index_identity(endpoint: &Url, model: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"personal-memory-embedding-v1\0");
    digest.update(endpoint.as_str().as_bytes());
    digest.update(b"\0");
    digest.update(model.as_bytes());
    hex::encode(digest.finalize())
}

fn validate_inputs(inputs: &[String]) -> Result<(), EmbeddingProviderError> {
    let total = inputs.iter().map(String::len).sum::<usize>();
    let valid = !inputs.is_empty()
        && inputs.len() <= MAX_EMBEDDING_INPUTS
        && total <= MAX_EMBEDDING_REQUEST_BYTES
        && inputs
            .iter()
            .all(|input| !input.trim().is_empty() && input.len() <= MAX_EMBEDDING_INPUT_BYTES);
    valid
        .then_some(())
        .ok_or(EmbeddingProviderError::MalformedProtocol)
}

fn parse_response(
    body: &[u8],
    expected_count: usize,
) -> Result<Vec<Vec<f32>>, EmbeddingProviderError> {
    let mut response: EmbeddingResponse =
        serde_json::from_slice(body).map_err(|_| EmbeddingProviderError::MalformedProtocol)?;
    if response.data.len() != expected_count {
        return Err(EmbeddingProviderError::MalformedProtocol);
    }
    response.data.sort_by_key(|item| item.index);
    let dimensions = response
        .data
        .first()
        .map(|item| item.embedding.len())
        .unwrap_or_default();
    if dimensions == 0
        || dimensions > MAX_EMBEDDING_DIMENSIONS
        || response.data.iter().enumerate().any(|(index, item)| {
            item.index != index
                || item.embedding.len() != dimensions
                || item.embedding.iter().any(|value| !value.is_finite())
        })
    {
        return Err(EmbeddingProviderError::MalformedProtocol);
    }
    Ok(response
        .data
        .into_iter()
        .map(|item| item.embedding)
        .collect())
}

fn classify_status(status: StatusCode) -> EmbeddingProviderError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => EmbeddingProviderError::Authentication,
        StatusCode::NOT_FOUND => EmbeddingProviderError::MissingModel,
        StatusCode::TOO_MANY_REQUESTS => EmbeddingProviderError::RateLimited,
        status if status.is_server_error() => EmbeddingProviderError::Server,
        _ => EmbeddingProviderError::MalformedProtocol,
    }
}

fn classify_transport_error(error: reqwest::Error) -> EmbeddingProviderError {
    if error.is_timeout() {
        EmbeddingProviderError::Timeout
    } else {
        EmbeddingProviderError::Transport
    }
}

pub(crate) fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.is_empty() || left.len() != right.len() {
        return None;
    }
    let dot = left
        .iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    let denominator = left_norm * right_norm;
    (denominator > 0.0 && denominator.is_finite())
        .then(|| dot / denominator)
        .filter(|similarity| similarity.is_finite())
}

#[cfg(test)]
#[path = "embedding_tests.rs"]
mod tests;
