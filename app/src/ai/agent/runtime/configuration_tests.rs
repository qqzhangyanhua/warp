use serde_json::json;

use super::configuration::{ChatCompletionsProvider, ReasoningEffort, RunConfiguration};
use super::resources::ResourceSnapshot;
use super::transcript::RuntimeContentBlock;

#[test]
fn builds_immutable_text_run_configuration_from_provider_and_resources() {
    let provider =
        ChatCompletionsProvider::new("https://provider.example/v1/", "local-model", "secret-key")
            .unwrap();
    let resources = vec![ResourceSnapshot {
        initiating_message_id: "message-1".to_string(),
        resource_id: "message-1:rule:0:0".to_string(),
        name: "AGENTS.md".to_string(),
        content: vec![RuntimeContentBlock::Text {
            text: "Retained instructions".to_string(),
        }],
    }];

    let configuration = RunConfiguration::text_only(
        provider,
        "/workspace",
        32_768,
        ReasoningEffort::Medium,
        resources,
    )
    .unwrap();

    assert_eq!(
        serde_json::to_value(configuration).unwrap(),
        json!({
            "provider": {
                "protocol": "chat_completions",
                "base_url": "https://provider.example/v1/chat/completions",
                "provider_origin": "https://provider.example",
                "model": "local-model",
                "api_key": "secret-key",
                "max_provider_attempts": 2,
                "max_redirects": 3
            },
            "working_directory": "/workspace",
            "context_limit": 32768,
            "reasoning_effort": "medium",
            "tool_request_limit": 0,
            "tools": [],
            "resources": [{
                "id": "message-1:rule:0:0",
                "name": "AGENTS.md",
                "content": [{"type": "text", "text": "Retained instructions"}]
            }]
        })
    );
}

#[test]
fn preserves_an_existing_chat_completions_endpoint() {
    let provider = ChatCompletionsProvider::new(
        "http://127.0.0.1:11434/v1/chat/completions",
        "local-model",
        "",
    )
    .unwrap();

    assert_eq!(
        provider.chat_completions_url(),
        "http://127.0.0.1:11434/v1/chat/completions"
    );
    assert_eq!(provider.origin(), "http://127.0.0.1:11434");
}

#[test]
fn rejects_provider_urls_without_an_http_origin() {
    assert!(ChatCompletionsProvider::new("not a URL", "model", "key").is_err());
    assert!(ChatCompletionsProvider::new("file:///tmp/provider", "model", "key").is_err());
}
