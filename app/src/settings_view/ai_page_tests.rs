use ::ai::api_keys::{ApiKeyManager, EmbeddingProviderConfig, VoiceTranscriptionConfig};
use warpui::platform::WindowStyle;
use warpui::{App, SingletonEntity, TypedActionView};

use super::{
    derive_agent_attribution_toggle_state, AISettingsPageAction, AISettingsPageView, AISubpage,
    AgentAttributionToggleState,
};
#[cfg(all(feature = "local_fs", feature = "personal_memory"))]
use crate::settings::AISettings;
use crate::settings_view::SettingsSection;
use crate::test_util::terminal::initialize_app_for_terminal_view;
use crate::view_components::dropdown::DropdownAction;
#[cfg(all(feature = "local_fs", feature = "personal_memory"))]
use crate::workspace::ToastStack;
use crate::workspaces::workspace::AdminEnablementSetting;
#[cfg(all(feature = "local_fs", feature = "personal_memory"))]
use crate::{
    ai::personal_memory::{CreatePersonalMemoryInput, PersonalMemoryService},
    persistence::{setup_database, start_writer, ModelEvent},
    GlobalResourceHandlesProvider,
};

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

#[cfg(all(feature = "local_fs", feature = "personal_memory"))]
#[test]
fn personal_memory_section_selects_personal_memory_subpage() {
    assert_eq!(
        AISubpage::from_section(SettingsSection::PersonalMemory),
        Some(AISubpage::PersonalMemory)
    );
}

#[cfg(all(feature = "local_fs", feature = "personal_memory"))]
#[test]
fn personal_memory_toggle_updates_the_persisted_setting() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let (_, page) = app.add_window(WindowStyle::NotStealFocus, AISettingsPageView::new);

        page.update(&mut app, |page, ctx| {
            page.handle_action(&AISettingsPageAction::TogglePersonalMemory, ctx);
        });

        app.read(|ctx| {
            assert!(!*AISettings::as_ref(ctx).personal_memory_enabled);
            assert!(!AISettings::as_ref(ctx).is_personal_memory_enabled(ctx));
        });
    });
}

#[cfg(all(feature = "local_fs", feature = "personal_memory"))]
#[test]
fn personal_memory_settings_load_separate_embedding_provider_configuration() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let expected = EmbeddingProviderConfig {
            base_url: "http://localhost:11434/v1".into(),
            model: "bge-m3".into(),
            api_key: "embedding-key".into(),
        };
        ApiKeyManager::handle(&app).update(&mut app, |manager, ctx| {
            manager.set_embedding_provider(Some(expected.clone()), ctx);
        });

        let (_, page) = app.add_window(WindowStyle::NotStealFocus, AISettingsPageView::new);

        page.read(&app, |page, ctx| {
            assert_eq!(
                page.personal_memory_embedding_config_for_test(ctx),
                expected
            );
        });
    });
}

#[cfg(all(feature = "local_fs", feature = "personal_memory"))]
#[test]
fn invalid_personal_memory_embedding_test_shows_error_toast() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        app.add_singleton_model(|_| ToastStack);
        let (sender, receiver) = async_channel::unbounded();
        app.update(|ctx| {
            ctx.subscribe_to_model(&ToastStack::handle(ctx), move |_, _, _| {
                let _ = sender.try_send(());
            });
        });
        let (_, page) = app.add_window(WindowStyle::NotStealFocus, AISettingsPageView::new);

        page.update(&mut app, |page, ctx| {
            page.handle_action(
                &AISettingsPageAction::TestPersonalMemoryEmbeddingProvider,
                ctx,
            );
        });

        assert!(page.read(&app, |page, _| {
            page.personal_memory_embedding_test_failed_for_test()
        }));
        assert!(receiver.try_recv().is_ok(), "expected an error toast event");
    });
}

#[cfg(all(feature = "local_fs", feature = "personal_memory"))]
#[test]
fn personal_memory_citation_focuses_the_requested_record() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let (_, page) = app.add_window(WindowStyle::NotStealFocus, AISettingsPageView::new);

        page.update(&mut app, |page, ctx| {
            page.focus_personal_memory_record("record-2".to_string(), ctx);
        });

        page.read(&app, |page, _| {
            assert_eq!(
                page.personal_memory_focus_record_id.as_deref(),
                Some("record-2")
            );
            assert_eq!(page.active_subpage, Some(AISubpage::PersonalMemory));
        });
    });
}

#[cfg(all(feature = "local_fs", feature = "personal_memory"))]
#[test]
fn personal_memory_subpage_loads_canonical_records_through_service() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let tempdir = tempfile::tempdir().unwrap();
        let database_path = tempdir.path().join("warp.sqlite");
        let conn = setup_database(&database_path).unwrap();
        let writer = start_writer(conn, database_path).unwrap();
        app.update(|ctx| {
            GlobalResourceHandlesProvider::handle(ctx).update(ctx, |provider, _| {
                provider.set_model_event_sender_for_test(Some(writer.sender.clone()));
            });
        });
        PersonalMemoryService::new(writer.sender.clone())
            .create(CreatePersonalMemoryInput::exact(
                "My GitHub account is zyh-work".to_string(),
                "zyh-work".to_string(),
                "GitHub account".to_string(),
            ))
            .await
            .unwrap();

        let (_, page) = app.add_window(WindowStyle::NotStealFocus, AISettingsPageView::new);
        page.update(&mut app, |page, ctx| {
            page.set_active_subpage(Some(AISubpage::PersonalMemory), ctx);
        });
        let mut loaded = false;
        for _ in 0..20 {
            loaded = page.read(&app, |page, _| {
                matches!(
                    &page.personal_memory_state,
                    super::PersonalMemorySettingsState::Loaded(records)
                        if records.len() == 1 && records[0].value_text == "zyh-work"
                )
            });
            if loaded {
                break;
            }
            warpui::r#async::Timer::after(std::time::Duration::from_millis(10)).await;
        }

        writer.sender.send(ModelEvent::Terminate).unwrap();
        writer.handle.join().unwrap();
        assert!(
            loaded,
            "Personal Memory settings did not load the canonical record"
        );
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
