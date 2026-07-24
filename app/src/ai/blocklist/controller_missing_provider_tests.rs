use std::sync::{Arc, Mutex};
use std::time::Duration;

use ai::api_keys::ApiKeyManager;
use warpui::{App, SingletonEntity};
use warpui_core::r#async::Timer;

use super::BlocklistAIControllerEvent;
use crate::ai::agent::runtime::AgentRuntimeService;
use crate::ai::blocklist::BlocklistAIHistoryModel;
use crate::ai::execution_profiles::profiles::AIExecutionProfilesModel;
use crate::ai::llms::LLMId;
use crate::persistence::model::AgentRuntimeBinding;
use crate::settings_view::SettingsSection;
use crate::test_util::terminal::{add_window_with_terminal, initialize_app_for_terminal_view};

#[derive(Clone, Copy)]
enum MissingProviderTestCase {
    BaseUrl,
    Model,
    ApiKey,
}

fn assert_missing_provider_opens_settings_without_starting_a_run(
    test_case: MissingProviderTestCase,
    expected_field: &'static str,
) {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        app.add_singleton_model(|_| crate::workspace::ToastStack);
        match test_case {
            MissingProviderTestCase::BaseUrl => {}
            MissingProviderTestCase::Model => {
                ApiKeyManager::handle(&app).update(&mut app, |manager, ctx| {
                    manager.add_custom_endpoint(
                        "Local Provider".to_string(),
                        "http://localhost:11434/v1".to_string(),
                        "test-key".to_string(),
                        vec![(String::new(), None, Some("test-config-key".to_string()))],
                        ctx,
                    );
                });
            }
            MissingProviderTestCase::ApiKey => {
                let custom_model_id = LLMId::from("test-config-key");
                ApiKeyManager::handle(&app).update(&mut app, |manager, ctx| {
                    manager.add_custom_endpoint(
                        "Local Provider".to_string(),
                        "http://localhost:11434/v1".to_string(),
                        String::new(),
                        vec![(
                            "test-model".to_string(),
                            None,
                            Some(custom_model_id.to_string()),
                        )],
                        ctx,
                    );
                });
                AIExecutionProfilesModel::handle(&app).update(&mut app, |profiles, ctx| {
                    let default_profile_id = profiles.default_profile_id();
                    profiles.set_base_model(default_profile_id, Some(custom_model_id.clone()), ctx);
                    profiles.set_coding_model(
                        default_profile_id,
                        Some(custom_model_id.clone()),
                        ctx,
                    );
                    profiles.set_cli_agent_model(
                        default_profile_id,
                        Some(custom_model_id.clone()),
                        ctx,
                    );
                    profiles.set_computer_use_model(default_profile_id, Some(custom_model_id), ctx);
                });
            }
        }
        let terminal = add_window_with_terminal(&mut app, None);
        terminal.update(&mut app, |terminal, ctx| {
            terminal.input().update(ctx, |input, ctx| {
                input.set_input_mode_agent(false, ctx);
            });
        });
        let captured_events = Arc::new(Mutex::new(Vec::new()));
        let controller = terminal.read(&app, |terminal, _| terminal.ai_controller().clone());
        let events_for_subscription = Arc::clone(&captured_events);
        app.update(|ctx| {
            ctx.subscribe_to_model(&controller, move |_, event, _| match event {
                BlocklistAIControllerEvent::ShowError(message) => {
                    events_for_subscription
                        .lock()
                        .unwrap()
                        .push(format!("error:{message}"));
                }
                BlocklistAIControllerEvent::OpenSettings(section) => {
                    events_for_subscription
                        .lock()
                        .unwrap()
                        .push(format!("settings:{section:?}"));
                }
                _ => {}
            });
        });

        let conversation_id = terminal.update(&mut app, |terminal, ctx| {
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    let conversation_id = history_model.start_new_conversation(
                        terminal.id(),
                        false,
                        false,
                        false,
                        ctx,
                    );
                    history_model
                        .conversation_mut(&conversation_id)
                        .expect("conversation should exist")
                        .set_runtime_binding(AgentRuntimeBinding::Pi);
                    conversation_id
                });

            terminal.ai_controller().update(ctx, |controller, ctx| {
                controller.send_user_query_in_conversation(
                    "hello".to_string(),
                    conversation_id,
                    None,
                    ctx,
                );
            });
            conversation_id
        });

        for _ in 0..100 {
            if !captured_events.lock().unwrap().is_empty() {
                break;
            }
            Timer::after(Duration::from_millis(10)).await;
        }
        let events = captured_events.lock().unwrap();
        assert!(
            events.iter().any(|event| event.contains(expected_field)),
            "expected {expected_field:?} in controller events: {events:?}"
        );
        assert!(events
            .iter()
            .any(|event| event == &format!("settings:{:?}", SettingsSection::WarpAgent)));
        assert!(events.iter().all(|event| {
            let event = event.to_ascii_lowercase();
            !event.contains("quota") && !event.contains("account")
        }));
        AgentRuntimeService::handle(&app).read(&app, |service, _| {
            assert_eq!(service.start_attempts_for_test(conversation_id), 0);
        });
    });
}

#[test]
fn missing_provider_base_url_opens_local_provider_settings_without_starting_a_run() {
    assert_missing_provider_opens_settings_without_starting_a_run(
        MissingProviderTestCase::BaseUrl,
        "Base URL",
    );
}

#[test]
fn missing_provider_model_opens_local_provider_settings_without_starting_a_run() {
    assert_missing_provider_opens_settings_without_starting_a_run(
        MissingProviderTestCase::Model,
        "Model",
    );
}

#[test]
fn missing_provider_api_key_opens_local_provider_settings_without_starting_a_run() {
    assert_missing_provider_opens_settings_without_starting_a_run(
        MissingProviderTestCase::ApiKey,
        "API Key",
    );
}
