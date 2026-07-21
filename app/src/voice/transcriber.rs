use std::sync::Arc;

use ai::api_keys::{ApiKeyManager, ApiKeys, VoiceTranscriptionConfigError};
use async_trait::async_trait;
use warpui::{AppContext, Entity, SingletonEntity};

#[cfg(feature = "voice_input")]
use crate::i18n::{tr_cached, Message};
use crate::server::server_api::TranscribeError;
use crate::voice::chat_completions_audio::ChatCompletionsAudioTranscriber;

/// Interface for transcribing voice input.
#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait Transcriber: Send + Sync {
    /// Transcribe the given base64 encoded wav file into text.
    /// This is expected to be async and called off the main thread.
    async fn transcribe(&self, wav_base64: String) -> Result<String, TranscribeError>;
}

/// A voice transcriber that is enabled or disabled.
///
/// This is a singleton model that the app can decide to enable or disable.
/// The editor does expect that it will exist as a singleton fetchable from app context
/// either way though, and depending on whether the optional transcriber is set,
/// the editor considers transcriber to be enabled or disabled.
///
/// We set it up this way to avoid the editor having a direct dependency on any server api.
pub struct VoiceTranscriber {
    /// The transcriber to use. If `None`, the transcriber is disabled.
    #[cfg_attr(not(feature = "voice_input"), allow(dead_code))]
    transcriber: Option<Arc<dyn Transcriber>>,
}

pub struct ResolvedVoiceTranscriber {
    transcriber: Arc<dyn Transcriber>,
    uses_warp_voice: bool,
}

impl ResolvedVoiceTranscriber {
    pub fn transcriber(&self) -> &Arc<dyn Transcriber> {
        &self.transcriber
    }

    pub fn uses_warp_voice(&self) -> bool {
        self.uses_warp_voice
    }
}

impl VoiceTranscriber {
    pub fn new(transcriber: Arc<dyn Transcriber>) -> Self {
        Self {
            transcriber: Some(transcriber),
        }
    }

    /// Returns the transcriber if one is set.
    pub fn transcriber(&self) -> Option<&Arc<dyn Transcriber>> {
        self.transcriber.as_ref()
    }

    pub fn resolve(
        &self,
        keys: &ApiKeys,
        local_only: bool,
    ) -> Result<ResolvedVoiceTranscriber, VoiceTranscriptionConfigError> {
        match keys.voice_transcription_provider()? {
            Some(provider) => Ok(ResolvedVoiceTranscriber {
                transcriber: Arc::new(ChatCompletionsAudioTranscriber::new(provider)),
                uses_warp_voice: false,
            }),
            None if local_only => Err(VoiceTranscriptionConfigError::NotConfigured),
            None => self
                .transcriber
                .clone()
                .map(|transcriber| ResolvedVoiceTranscriber {
                    transcriber,
                    uses_warp_voice: true,
                })
                .ok_or(VoiceTranscriptionConfigError::NotConfigured),
        }
    }

    pub fn resolve_for_app(
        &self,
        app: &AppContext,
    ) -> Result<ResolvedVoiceTranscriber, VoiceTranscriptionConfigError> {
        self.resolve(
            ApiKeyManager::as_ref(app).keys(),
            crate::local_mode::is_local_only_custom_provider_mode(),
        )
    }
}

impl Entity for VoiceTranscriber {
    type Event = ();
}

impl SingletonEntity for VoiceTranscriber {}

#[cfg(feature = "voice_input")]
pub fn provider_error_message(error: &TranscribeError) -> Option<&'static str> {
    match error {
        TranscribeError::ProviderAuthentication => {
            Some(tr_cached(Message::VoiceProviderAuthFailed))
        }
        TranscribeError::ProviderModelNotFound => Some(tr_cached(Message::VoiceModelNotFound)),
        TranscribeError::ProviderRateLimit => Some(tr_cached(Message::VoiceProviderRateLimit)),
        TranscribeError::ProviderServer => Some(tr_cached(Message::VoiceProviderUnavailable)),
        TranscribeError::ProviderRejected => Some(tr_cached(Message::VoiceProviderRejected)),
        TranscribeError::ProviderTransport(_) => Some(tr_cached(Message::VoiceProviderUnreachable)),
        TranscribeError::RecordingTooLong => Some(tr_cached(Message::VoiceRecordingTooLong)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "transcriber_tests.rs"]
mod tests;
