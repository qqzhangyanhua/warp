use std::sync::atomic::{AtomicUsize, Ordering};

use bytes::Bytes;
use futures::{future, stream};
use http::StatusCode;
use reqwest::header::HeaderMap;
use warpui::r#async::BoxFuture;

use super::*;
use crate::ai::agent::api::local_provider::transport::{LocalProviderResponse, ProviderByteStream};

struct ResponseTransport {
    sends: Arc<AtomicUsize>,
    status: StatusCode,
    body: Vec<u8>,
}

impl LocalProviderTransport for ResponseTransport {
    fn send(
        &self,
        _: LocalProviderModel,
        _: ChatCompletionRequest,
    ) -> BoxFuture<'static, Result<LocalProviderResponse, AIApiError>> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        let status = self.status;
        let body = self.body.clone();
        Box::pin(async move {
            Ok(LocalProviderResponse {
                status,
                headers: HeaderMap::new(),
                body: Box::pin(stream::iter([Ok(Bytes::from(body))])) as ProviderByteStream,
            })
        })
    }
}

struct PendingTransport {
    sends: Arc<AtomicUsize>,
}

impl LocalProviderTransport for PendingTransport {
    fn send(
        &self,
        _: LocalProviderModel,
        _: ChatCompletionRequest,
    ) -> BoxFuture<'static, Result<LocalProviderResponse, AIApiError>> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        Box::pin(future::pending())
    }
}

fn run_test(
    timeout: Duration,
    transport: Arc<dyn LocalProviderTransport>,
) -> Result<(), AIApiError> {
    futures::executor::block_on(test_provider_connection_with_transport(
        "https://provider.example/v1".to_string(),
        "provider-key".to_string(),
        "provider-model".to_string(),
        timeout,
        transport,
    ))
}

#[test]
fn provider_rate_limit_is_not_retried_or_reported_as_warp_quota() {
    let sends = Arc::new(AtomicUsize::new(0));
    let result = run_test(
        TIMEOUT,
        Arc::new(ResponseTransport {
            sends: sends.clone(),
            status: StatusCode::TOO_MANY_REQUESTS,
            body: b"secret response body".to_vec(),
        }),
    );

    assert_eq!(sends.load(Ordering::SeqCst), 1);
    assert!(matches!(
        result,
        Err(AIApiError::ProviderErrorStatus {
            status: StatusCode::TOO_MANY_REQUESTS,
            ..
        })
    ));
}

#[test]
fn malformed_protocol_uses_a_sanitized_error() {
    let result = run_test(
        TIMEOUT,
        Arc::new(ResponseTransport {
            sends: Arc::new(AtomicUsize::new(0)),
            status: StatusCode::OK,
            body: b"sensitive malformed response".to_vec(),
        }),
    );

    let message = result.unwrap_err().to_string();
    assert_eq!(
        message,
        "Provider returned a malformed Chat Completions response"
    );
    assert!(!message.contains("sensitive"));
}

#[test]
fn pending_connection_test_times_out_after_one_send() {
    let sends = Arc::new(AtomicUsize::new(0));
    let result = run_test(
        Duration::from_millis(1),
        Arc::new(PendingTransport {
            sends: sends.clone(),
        }),
    );

    assert_eq!(sends.load(Ordering::SeqCst), 1);
    assert_eq!(
        result.unwrap_err().to_string(),
        "Provider connection test timed out"
    );
}

#[test]
fn empty_choices_are_rejected_as_malformed_protocol() {
    let result = run_test(
        TIMEOUT,
        Arc::new(ResponseTransport {
            sends: Arc::new(AtomicUsize::new(0)),
            status: StatusCode::OK,
            body: br#"{"choices":[]}"#.to_vec(),
        }),
    );

    assert_eq!(
        result.unwrap_err().to_string(),
        "Provider returned a malformed Chat Completions response"
    );
}

#[test]
fn oversized_response_is_rejected_without_exposing_content() {
    let result = run_test(
        TIMEOUT,
        Arc::new(ResponseTransport {
            sends: Arc::new(AtomicUsize::new(0)),
            status: StatusCode::OK,
            body: vec![b'x'; MAX_RESPONSE_BYTES + 1],
        }),
    );

    assert_eq!(
        result.unwrap_err().to_string(),
        "Provider returned an oversized Chat Completions response"
    );
}
