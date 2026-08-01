use std::time::Duration;

use ai::agent::PERSONAL_MEMORY_STORE_ID;
use ai::api_keys::ApiKeyManager;
use warpui::integration::{AssertionCallback, AssertionOutcome, TestStep};
use warpui::{SingletonEntity, ViewHandle};

use super::{
    assert_latest_task_succeeds_or_blocked, ConversationTarget, AGENT_MODE_RUNNING_STEP_GROUP_NAME,
};
use crate::ai::agent::AIAgentCitation;
use crate::ai::blocklist::agent_view::AgentViewEntryOrigin;
use crate::ai::blocklist::block::{AIBlock, AIBlockAction};
use crate::ai::blocklist::BlocklistAIHistoryModel;
use crate::ai::execution_profiles::profiles::AIExecutionProfilesModel;
use crate::ai::llms::LLMId;
use crate::integration_testing::step::{
    new_step_with_default_assertions, new_step_with_default_assertions_for_pane,
};
use crate::integration_testing::terminal::assert_input_is_focused;
use crate::integration_testing::view_getters::terminal_view;
use crate::settings_view::{SettingsSection, SettingsView};

const TEST_MODEL_CONFIG_KEY: &str = "personal-memory-integration-model";

pub fn configure_personal_memory_test_provider() -> TestStep {
    new_step_with_default_assertions("Configure Personal Memory test Provider").with_action(
        |app, _, _| {
            let model_id = LLMId::from(TEST_MODEL_CONFIG_KEY);
            ApiKeyManager::handle(app).update(app, |manager, ctx| {
                assert!(
                    manager.keys().embedding_provider.is_none(),
                    "Personal Memory GUI test must run without an Embedding Provider"
                );
                manager.add_custom_endpoint(
                    "Personal Memory Test Provider".to_string(),
                    "http://127.0.0.1:11434/v1".to_string(),
                    "test-key".to_string(),
                    vec![("test-model".to_string(), None, Some(model_id.to_string()))],
                    ctx,
                );
            });
            AIExecutionProfilesModel::handle(app).update(app, |profiles, ctx| {
                let profile_id = profiles.default_profile_id();
                profiles.set_base_model(profile_id, Some(model_id.clone()), ctx);
                profiles.set_coding_model(profile_id, Some(model_id.clone()), ctx);
                profiles.set_cli_agent_model(profile_id, Some(model_id.clone()), ctx);
                profiles.set_computer_use_model(profile_id, Some(model_id), ctx);
            });
        },
    )
}

pub fn submit_personal_memory_query_and_wait_until_done(
    query: &str,
    timeout: Duration,
) -> TestStep {
    new_step_with_default_assertions_for_pane(&format!("Enter local AI query: {query}"), 0, 0)
        .set_timeout(timeout)
        .set_step_group_name(AGENT_MODE_RUNNING_STEP_GROUP_NAME)
        .with_typed_characters(&[query])
        .with_keystrokes(&["enter"])
        .add_named_assertion(
            "The local Agent task is complete",
            assert_latest_task_succeeds_or_blocked(ConversationTarget::Active, None),
        )
        .add_named_assertion("Input focus returns to the user", assert_input_is_focused())
}

pub fn assert_latest_personal_memory_response_text(
    assertion: impl Fn(&str) -> bool + 'static,
) -> AssertionCallback {
    Box::new(move |app, window_id| {
        let terminal = terminal_view(app, window_id, 0, 0);
        let Some(block) = terminal.read(app, |view, _| view.last_ai_block()) else {
            return AssertionOutcome::failure("No Agent response block".to_string());
        };
        let block_exchange_id = block.read(app, |block, _| block.exchange_id_for_test());
        let exchange_states = BlocklistAIHistoryModel::handle(app).read(app, |history, _| {
            history
                .active_conversation(terminal.id())
                .map(|conversation| {
                    conversation
                        .root_task_exchanges()
                        .map(|exchange| format!("{}:{:?}", exchange.id, exchange.output_status))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });
        block.read(app, |block, ctx| {
            if !block.is_ai_output_complete(ctx) {
                return AssertionOutcome::failure(format!(
                    "The latest Agent response is not complete: block_exchange={block_exchange_id}, output={:?}, finish={:?}, exchanges={exchange_states:?}",
                    block.output_status(ctx),
                    block.finish_reason()
                ));
            }
            let text = block.get_output_text(ctx);
            if assertion(&text) {
                AssertionOutcome::Success
            } else {
                AssertionOutcome::immediate_failure(format!(
                    "The latest Agent response did not contain the expected text: {text}"
                ))
            }
        })
    })
}

pub fn start_new_personal_memory_conversation() -> TestStep {
    new_step_with_default_assertions("Start a new Personal Memory conversation")
        .with_action(|app, window_id, _| {
            let terminal = terminal_view(app, window_id, 0, 0);
            let agent_view_controller =
                terminal.read(app, |terminal, _| terminal.agent_view_controller().clone());
            agent_view_controller.update(app, |controller, ctx| {
                controller
                    .try_enter_agent_view(
                        None,
                        AgentViewEntryOrigin::Input {
                            was_prompt_autodetected: false,
                        },
                        ctx,
                    )
                    .expect("Personal Memory test should start a fresh Agent conversation");
            });
        })
        .add_named_assertion("A fresh conversation is active", |app, window_id| {
            let terminal = terminal_view(app, window_id, 0, 0);
            BlocklistAIHistoryModel::handle(app).read(app, |history, _| {
                let Some(conversation) = history.active_conversation(terminal.id()) else {
                    return AssertionOutcome::failure("No active conversation".to_string());
                };
                if conversation.all_exchanges().is_empty() {
                    AssertionOutcome::Success
                } else {
                    AssertionOutcome::failure(
                        "The active conversation still contains the remember exchange".to_string(),
                    )
                }
            })
        })
}

pub fn assert_latest_personal_memory_source(expected_value: &'static str) -> AssertionCallback {
    Box::new(move |app, window_id| {
        let terminal = terminal_view(app, window_id, 0, 0);
        let citation = BlocklistAIHistoryModel::handle(app).read(app, |history, _| {
            history
                .active_conversation(terminal.id())
                .and_then(|conversation| {
                    conversation
                        .fetched_memories()
                        .into_iter()
                        .find(|memory| memory.memory_store_id == PERSONAL_MEMORY_STORE_ID)
                })
                .map(|memory| {
                    AIAgentCitation::from_fetched_memory(
                        memory.memory_store_id,
                        memory.memory_id,
                        memory.content,
                    )
                })
        });
        let Some(citation) = citation else {
            return AssertionOutcome::failure("No committed Personal Memory source".to_string());
        };
        let AIAgentCitation::PersonalMemory { content, .. } = &citation else {
            return AssertionOutcome::immediate_failure(
                "Personal Memory source mapped to the wrong citation type".to_string(),
            );
        };
        if !content.contains(expected_value) {
            return AssertionOutcome::immediate_failure(format!(
                "Personal Memory source does not contain exact value {expected_value}"
            ));
        }

        let Some(block) = terminal.read(app, |view, _| view.last_ai_block()) else {
            return AssertionOutcome::failure("No Agent response block".to_string());
        };
        block.read(app, |block, _| {
            if block.has_footer_citation_for_test(&citation) {
                AssertionOutcome::Success
            } else {
                AssertionOutcome::failure(
                    "Personal Memory source affordance has not rendered".to_string(),
                )
            }
        })
    })
}

pub fn open_latest_personal_memory_source() -> TestStep {
    new_step_with_default_assertions("Open the Personal Memory source affordance").with_action(
        |app, window_id, _| {
            let terminal = terminal_view(app, window_id, 0, 0);
            let citation = BlocklistAIHistoryModel::handle(app).read(app, |history, _| {
                history
                    .active_conversation(terminal.id())
                    .and_then(|conversation| {
                        conversation
                            .fetched_memories()
                            .into_iter()
                            .find(|memory| memory.memory_store_id == PERSONAL_MEMORY_STORE_ID)
                    })
                    .map(|memory| {
                        AIAgentCitation::from_fetched_memory(
                            memory.memory_store_id,
                            memory.memory_id,
                            memory.content,
                        )
                    })
            });
            let block: Option<ViewHandle<AIBlock>> =
                terminal.read(app, |view, _| view.last_ai_block());
            if let (Some(citation), Some(block)) = (citation, block) {
                app.dispatch_typed_action(
                    window_id,
                    &[block.id()],
                    &AIBlockAction::OpenCitation(citation),
                );
            }
        },
    )
}

pub fn assert_personal_memory_management_surface(
    expected_value: &'static str,
) -> AssertionCallback {
    Box::new(move |app, window_id| {
        let settings_views: Vec<ViewHandle<SettingsView>> =
            app.views_of_type(window_id).unwrap_or_default();
        let Some(settings) = settings_views.first() else {
            return AssertionOutcome::failure("Personal Memory Settings did not open".to_string());
        };
        settings.read(app, |settings, ctx| {
            if settings.current_settings_section() != SettingsSection::PersonalMemory {
                return AssertionOutcome::failure(
                    "Settings is not showing the Personal Memory surface".to_string(),
                );
            }
            if settings.personal_memory_focused_record_has_value_for_test(expected_value, ctx) {
                AssertionOutcome::Success
            } else {
                AssertionOutcome::failure(format!(
                    "Personal Memory management surface has not loaded {expected_value}"
                ))
            }
        })
    })
}
