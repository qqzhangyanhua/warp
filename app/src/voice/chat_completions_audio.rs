use ai::api_keys::VoiceTranscriptionProvider;
use async_trait::async_trait;
use base64::Engine as _;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use url::Url;

use super::transcriber::Transcriber;
use crate::server::server_api::TranscribeError;

const MAX_QWEN_DATA_URL_BYTES: usize = 10 * 1024 * 1024;
const WAV_DATA_URL_PREFIX: &str = "data:audio/wav;base64,";

pub struct ChatCompletionsAudioTranscriber {
    provider: VoiceTranscriptionProvider,
    client: reqwest::Client,
}

impl ChatCompletionsAudioTranscriber {
    pub fn new(provider: VoiceTranscriptionProvider) -> Self {
        Self {
            provider,
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("voice transcription HTTP client configuration should be valid"),
        }
    }
}

pub fn test_wav_base64() -> String {
    const SAMPLE_RATE: u32 = 16_000;
    const SAMPLE_COUNT: u32 = 1_600;
    const DATA_LEN: u32 = SAMPLE_COUNT * 2;
    let mut wav = Vec::with_capacity((44 + DATA_LEN) as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + DATA_LEN).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&DATA_LEN.to_le_bytes());
    wav.resize((44 + DATA_LEN) as usize, 0);
    base64::engine::general_purpose::STANDARD.encode(wav)
}

#[derive(Serialize)]
struct ChatCompletionsAudioRequest {
    model: String,
    messages: Vec<AudioMessage>,
    stream: bool,
    asr_options: AsrOptions,
}

#[derive(Serialize)]
struct AudioMessage {
    role: &'static str,
    content: Vec<AudioContent>,
}

#[derive(Serialize)]
struct AudioContent {
    r#type: &'static str,
    input_audio: InputAudio,
}

#[derive(Serialize)]
struct InputAudio {
    data: String,
}

#[derive(Serialize)]
struct AsrOptions {
    enable_itn: bool,
}

#[derive(Deserialize)]
struct ChatCompletionsAudioResponse {
    choices: Vec<AudioChoice>,
}

#[derive(Deserialize)]
struct AudioChoice {
    message: AudioResponseMessage,
}

#[derive(Deserialize)]
struct AudioResponseMessage {
    content: String,
    #[serde(default)]
    annotations: Vec<AudioAnnotation>,
}

#[derive(Deserialize)]
struct AudioAnnotation {
    #[serde(default)]
    language: Option<String>,
}

fn chat_completions_url(base_url: &str) -> Result<Url, TranscribeError> {
    let mut url = Url::parse(base_url).map_err(|_| TranscribeError::ProviderRejected)?;
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

fn provider_error(status: StatusCode) -> TranscribeError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => TranscribeError::ProviderAuthentication,
        StatusCode::NOT_FOUND => TranscribeError::ProviderModelNotFound,
        StatusCode::TOO_MANY_REQUESTS => TranscribeError::ProviderRateLimit,
        status if status.is_server_error() => TranscribeError::ProviderServer,
        _ => TranscribeError::ProviderRejected,
    }
}

fn is_language_code(value: &str) -> bool {
    matches!(
        value,
        "ar"
            | "de"
            | "en"
            | "es"
            | "fr"
            | "hi"
            | "id"
            | "it"
            | "ja"
            | "ko"
            | "ms"
            | "pt"
            | "ru"
            | "th"
            | "tr"
            | "vi"
            | "yue"
            | "zh"
    )
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl Transcriber for ChatCompletionsAudioTranscriber {
    async fn transcribe(&self, wav_base64: String) -> Result<String, TranscribeError> {
        if WAV_DATA_URL_PREFIX.len().saturating_add(wav_base64.len()) > MAX_QWEN_DATA_URL_BYTES {
            return Err(TranscribeError::RecordingTooLong);
        }
        let url = chat_completions_url(&self.provider.base_url)?;
        let request = ChatCompletionsAudioRequest {
            model: self.provider.model.clone(),
            messages: vec![AudioMessage {
                role: "user",
                content: vec![AudioContent {
                    r#type: "input_audio",
                    input_audio: InputAudio {
                        data: format!("{WAV_DATA_URL_PREFIX}{wav_base64}"),
                    },
                }],
            }],
            stream: false,
            asr_options: AsrOptions { enable_itn: false },
        };
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.provider.api_key)
            .json(&request)
            .send()
            .await
            .map_err(TranscribeError::ProviderTransport)?;
        if !response.status().is_success() {
            return Err(provider_error(response.status()));
        }

        let response: ChatCompletionsAudioResponse = response
            .json()
            .await
            .map_err(|_| TranscribeError::Deserialization)?;
        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or(TranscribeError::Deserialization)?;
        if let Some(language) = choice
            .message
            .annotations
            .iter()
            .find_map(|annotation| annotation.language.as_deref())
            .filter(|language| is_language_code(language))
        {
            log::debug!("Voice transcription completed: language={language}");
        }
        Ok(choice.message.content)
    }
}

#[cfg(test)]
#[path = "chat_completions_audio_tests.rs"]
mod tests;
