use std::sync::Arc;

use mockito::Server;
use serde_json::json;

use super::*;

fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}

fn config(base_url: String) -> EmbeddingProviderConfig {
    EmbeddingProviderConfig {
        base_url,
        model: "bge-m3".into(),
        api_key: "embedding-secret".into(),
    }
}

#[test]
fn normalizes_only_to_v1_embeddings_and_keeps_origin() {
    let provider = EmbeddingProvider::new(config(
        "https://provider.example/compatible?ignored=true".into(),
    ))
    .unwrap();

    assert_eq!(
        provider.endpoint().as_str(),
        "https://provider.example/compatible/v1/embeddings"
    );
    assert_eq!(provider.origin(), "https://provider.example");
}

#[test]
fn validates_bounded_finite_vectors() {
    let vectors = parse_response(
        serde_json::to_vec(&json!({
            "data": [
                {"index": 1, "embedding": [0.0, 1.0]},
                {"index": 0, "embedding": [1.0, 0.0]}
            ]
        }))
        .unwrap()
        .as_slice(),
        2,
    )
    .unwrap();

    assert_eq!(vectors, vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
    assert_eq!(
        parse_response(br#"{"data":[{"index":0,"embedding":[]}] }"#, 1),
        Err(EmbeddingProviderError::MalformedProtocol)
    );
}

#[test]
fn maps_provider_status_without_reading_provider_body() {
    assert_eq!(
        classify_status(StatusCode::UNAUTHORIZED),
        EmbeddingProviderError::Authentication
    );
    assert_eq!(
        classify_status(StatusCode::NOT_FOUND),
        EmbeddingProviderError::MissingModel
    );
    assert_eq!(
        classify_status(StatusCode::TOO_MANY_REQUESTS),
        EmbeddingProviderError::RateLimited
    );
    assert_eq!(
        classify_status(StatusCode::BAD_GATEWAY),
        EmbeddingProviderError::Server
    );
}

#[test]
fn sends_bounded_openai_compatible_request() {
    let mut server = Server::new();
    let request = server
        .mock("POST", "/v1/embeddings")
        .match_header("authorization", "Bearer embedding-secret")
        .match_body(mockito::Matcher::Json(json!({
            "model": "bge-m3",
            "input": ["remembered fact"]
        })))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"data":[{"index":0,"embedding":[1.0,0.5]}]}"#)
        .create();
    let provider = Arc::new(EmbeddingProvider::new(config(server.url())).unwrap());

    let vector = run(provider.embed("remembered fact".into())).unwrap();

    request.assert();
    assert_eq!(vector, vec![1.0, 0.5]);
}

#[test]
fn cross_origin_redirect_does_not_forward_api_key() {
    let mut destination = Server::new();
    let redirected_request = destination
        .mock("POST", "/v1/embeddings")
        .match_header("authorization", "Bearer embedding-secret")
        .expect(0)
        .create();
    let mut source = Server::new();
    let redirect = source
        .mock("POST", "/v1/embeddings")
        .with_status(307)
        .with_header("location", &format!("{}/v1/embeddings", destination.url()))
        .create();
    let provider = EmbeddingProvider::new(config(source.url())).unwrap();

    let result = run(provider.embed("fact".into()));

    redirect.assert();
    redirected_request.assert();
    assert_eq!(result, Err(EmbeddingProviderError::MalformedProtocol));
}

#[test]
fn connection_test_reports_mixed_language_warning() {
    let mut server = Server::new();
    let request = server
        .mock("POST", "/v1/embeddings")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"data":[{"index":0,"embedding":[1.0,0.0]},{"index":1,"embedding":[0.0,1.0]}]}"#,
        )
        .create();
    let provider = EmbeddingProvider::new(config(server.url())).unwrap();

    let result = run(provider.test_connection()).unwrap();

    request.assert();
    assert_eq!(result, EmbeddingConnectionTestResult::CompatibilityWarning);
}
