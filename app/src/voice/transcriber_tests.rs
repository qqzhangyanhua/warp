use ai::api_keys::{
    ApiKeys, CustomEndpoint, VoiceTranscriptionConfig, VoiceTranscriptionConfigError,
};
use async_trait::async_trait;

use super::*;

struct FakeWarpTranscriber;

#[async_trait]
impl Transcriber for FakeWarpTranscriber {
    async fn transcribe(&self, _: String) -> Result<String, TranscribeError> {
        Ok("warp".into())
    }
}

fn router() -> VoiceTranscriber {
    VoiceTranscriber::new(Arc::new(FakeWarpTranscriber))
}

fn configured_keys() -> ApiKeys {
    ApiKeys {
        custom_endpoints: vec![CustomEndpoint {
            id: "voice-endpoint".into(),
            name: "Voice".into(),
            url: "https://provider.example/v1".into(),
            api_key: "key".into(),
            models: vec![],
        }],
        voice_transcription: Some(VoiceTranscriptionConfig {
            endpoint_id: "voice-endpoint".into(),
            model: "qwen3-asr-flash".into(),
        }),
        ..Default::default()
    }
}

#[test]
fn configured_provider_never_uses_warp_voice() {
    let resolved = router().resolve(&configured_keys(), false).unwrap();

    assert!(!resolved.uses_warp_voice());
}

#[test]
fn stale_provider_selection_does_not_fall_back_to_warp_voice() {
    let keys = ApiKeys {
        voice_transcription: Some(VoiceTranscriptionConfig {
            endpoint_id: "missing".into(),
            model: "qwen3-asr-flash".into(),
        }),
        ..Default::default()
    };

    assert!(matches!(
        router().resolve(&keys, false),
        Err(VoiceTranscriptionConfigError::EndpointNotFound)
    ));
}

#[test]
fn regular_mode_without_configuration_uses_warp_voice() {
    let resolved = router().resolve(&ApiKeys::default(), false).unwrap();

    assert!(resolved.uses_warp_voice());
}

#[test]
fn local_only_mode_without_configuration_requires_setup() {
    assert!(matches!(
        router().resolve(&ApiKeys::default(), true),
        Err(VoiceTranscriptionConfigError::NotConfigured)
    ));
}
