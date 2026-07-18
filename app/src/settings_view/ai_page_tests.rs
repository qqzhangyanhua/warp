use ::ai::api_keys::{ApiKeyManager, VoiceTranscriptionConfig};
use warpui::platform::WindowStyle;
use warpui::{App, SingletonEntity, TypedActionView};

use super::{
    derive_agent_attribution_toggle_state, AISettingsPageAction, AISettingsPageView,
    AgentAttributionToggleState,
};
use crate::test_util::terminal::initialize_app_for_terminal_view;
use crate::view_components::dropdown::DropdownAction;
use crate::workspaces::workspace::AdminEnablementSetting;

#[test]
fn clearing_voice_provider_does_not_circularly_update_ai_settings_view() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        app.update(|ctx| {
            ApiKeyManager::handle(ctx).update(ctx, |manager, ctx| {
                manager.set_voice_transcription_config(
                    Some(VoiceTranscriptionConfig {
                        endpoint_id: "voice-endpoint".to_string(),
                        model: "qwen3-asr-flash".to_string(),
                    }),
                    ctx,
                );
            });
        });
        let (_, page) = app.add_window(WindowStyle::NotStealFocus, AISettingsPageView::new);
        let dropdown = page.read(&app, |page, _| page.voice_provider_dropdown.clone());

        dropdown.update(&mut app, |dropdown, ctx| {
            dropdown.handle_action(
                &DropdownAction::select_action_and_close(AISettingsPageAction::SetVoiceProvider(
                    None,
                )),
                ctx,
            );
        });

        app.update(|ctx| {
            assert!(ApiKeyManager::as_ref(ctx)
                .keys()
                .voice_transcription
                .is_none());
        });
    });
}

#[test]
fn respect_user_setting_returns_user_pref_unlocked() {
    let state = derive_agent_attribution_toggle_state(
        &AdminEnablementSetting::RespectUserSetting,
        true,
        true,
    );
    assert_eq!(
        state,
        AgentAttributionToggleState {
            is_enabled: true,
            is_forced_by_org: false,
            is_disabled: false,
        }
    );
}

#[test]
fn respect_user_setting_with_user_off_returns_unchecked_unlocked() {
    let state = derive_agent_attribution_toggle_state(
        &AdminEnablementSetting::RespectUserSetting,
        false,
        true,
    );
    assert_eq!(
        state,
        AgentAttributionToggleState {
            is_enabled: false,
            is_forced_by_org: false,
            is_disabled: false,
        }
    );
}

#[test]
fn team_enable_locks_toggle_on_regardless_of_user_pref() {
    let state = derive_agent_attribution_toggle_state(&AdminEnablementSetting::Enable, false, true);
    assert_eq!(
        state,
        AgentAttributionToggleState {
            is_enabled: true,
            is_forced_by_org: true,
            is_disabled: true,
        }
    );
}

#[test]
fn team_disable_locks_toggle_off_regardless_of_user_pref() {
    let state = derive_agent_attribution_toggle_state(&AdminEnablementSetting::Disable, true, true);
    assert_eq!(
        state,
        AgentAttributionToggleState {
            is_enabled: false,
            is_forced_by_org: true,
            is_disabled: true,
        }
    );
}

#[test]
fn ai_globally_disabled_marks_toggle_disabled_but_not_forced() {
    let state = derive_agent_attribution_toggle_state(
        &AdminEnablementSetting::RespectUserSetting,
        true,
        false,
    );
    assert_eq!(
        state,
        AgentAttributionToggleState {
            is_enabled: true,
            is_forced_by_org: false,
            is_disabled: true,
        }
    );
}

#[test]
fn team_force_takes_precedence_over_global_ai_disabled() {
    let state =
        derive_agent_attribution_toggle_state(&AdminEnablementSetting::Enable, false, false);
    assert_eq!(
        state,
        AgentAttributionToggleState {
            is_enabled: true,
            is_forced_by_org: true,
            is_disabled: true,
        }
    );
}
