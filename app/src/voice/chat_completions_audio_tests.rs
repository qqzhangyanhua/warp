use ai::api_keys::VoiceTranscriptionProvider;
use mockito::{Matcher, Server};

use super::*;
use crate::server::server_api::TranscribeError;
use crate::voice::transcriber::Transcriber;

fn transcriber(base_url: String) -> ChatCompletionsAudioTranscriber {
    ChatCompletionsAudioTranscriber::new(VoiceTranscriptionProvider {
        base_url,
        api_key: "dashscope-key".into(),
        model: "qwen3-asr-flash".into(),
    })
}

fn run_transcription(
    transcriber: ChatCompletionsAudioTranscriber,
) -> Result<String, TranscribeError> {
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(transcriber.transcribe("V0FW".into()))
}

#[test]
fn sends_qwen_chat_completions_audio_request_and_returns_text() {
    let mut server = Server::new();
    let request = server
        .mock("POST", "/compatible-mode/v1/chat/completions")
        .match_header("authorization", "Bearer dashscope-key")
        .match_header("content-type", Matcher::Regex("application/json".into()))
        .match_body(Matcher::JsonString(
            serde_json::json!({
                "model": "qwen3-asr-flash",
                "messages": [{
                    "role": "user",
                    "content": [{
                        "type": "input_audio",
                        "input_audio": {
                            "data": "data:audio/wav;base64,V0FW"
                        }
                    }]
                }],
                "stream": false,
                "asr_options": {
                    "enable_itn": false
                }
            })
            .to_string(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"choices":[{"message":{"content":"你好，Warp。","annotations":[{"type":"audio_info","language":"zh"}]}}]}"#,
        )
        .create();

    let result =
        run_transcription(transcriber(format!("{}/compatible-mode/v1", server.url()))).unwrap();

    request.assert();
    assert_eq!(result, "你好，Warp。");
}

#[test]
fn keeps_an_existing_chat_completions_path() {
    let mut server = Server::new();
    let request = server
        .mock("POST", "/compatible-mode/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"choices":[{"message":{"content":"ok"}}]}"#)
        .create();

    let result = run_transcription(transcriber(format!(
        "{}/compatible-mode/v1/chat/completions",
        server.url()
    )));

    request.assert();
    assert_eq!(result.unwrap(), "ok");
}

#[test]
fn maps_provider_rate_limits_without_using_warp_quota() {
    let mut server = Server::new();
    let request = server
        .mock("POST", "/compatible-mode/v1/chat/completions")
        .with_status(429)
        .with_body("sensitive provider response")
        .create();

    let result = run_transcription(transcriber(format!("{}/compatible-mode/v1", server.url())));

    request.assert();
    assert!(matches!(result, Err(TranscribeError::ProviderRateLimit)));
}

#[test]
fn rejects_redirects_without_forwarding_the_api_key() {
    let mut destination = Server::new();
    let redirected_request = destination
        .mock("POST", "/redirected")
        .match_header("authorization", "Bearer dashscope-key")
        .with_status(200)
        .expect(0)
        .create();
    let mut source = Server::new();
    let redirect = source
        .mock("POST", "/chat/completions")
        .with_status(307)
        .with_header("location", &format!("{}/redirected", destination.url()))
        .create();

    let result = run_transcription(transcriber(source.url()));

    redirect.assert();
    redirected_request.assert();
    assert!(matches!(result, Err(TranscribeError::ProviderRejected)));
}

#[test]
fn rejects_audio_that_exceeds_qwen_data_url_limit_before_sending() {
    let result = tokio::runtime::Runtime::new().unwrap().block_on(
        transcriber("https://provider.invalid/compatible-mode/v1".into())
            .transcribe("A".repeat(10 * 1024 * 1024)),
    );

    assert!(matches!(result, Err(TranscribeError::RecordingTooLong)));
}

#[test]
fn only_accepts_bounded_ascii_language_codes_for_debug_logging() {
    assert!(is_language_code("zh"));
    assert!(is_language_code("en"));
    assert!(is_language_code("yue"));
    assert!(!is_language_code(""));
    assert!(!is_language_code("hello"));
    assert!(!is_language_code("secret123"));
    assert!(!is_language_code("用户转写内容"));
}
