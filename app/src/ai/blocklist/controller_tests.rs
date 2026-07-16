use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ai::api_keys::ApiKeyManager;
use chrono::Local;
use diesel::prelude::*;
use tempfile::TempDir;
use uuid::Uuid;
use warp_core::features::FeatureFlag;
use warp_multi_agent_api as api;
use warp_multi_agent_api::response_event;
use warpui::{App, SingletonEntity};
use warpui_core::r#async::executor::Background;
use warpui_core::r#async::Timer;

use crate::ai::agent::conversation::{AIConversationId, ConversationStatus};
use crate::ai::agent::runtime::{
    AgentRuntimeLaunchConfig, AgentRuntimeService, AgentRuntimeSupervisor, RuntimeStartError,
    RuntimeSupervisorConfig,
};
use crate::ai::agent::task::TaskId;
use crate::ai::agent::{
    AIAgentAttachment, AIAgentContext, AIAgentInput, AIAgentOutputStatus, CancellationReason,
    FinishedAIAgentOutput, ImageContext, PassiveSuggestionTrigger, RenderableAIError,
    UserQueryMode,
};
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::ai::blocklist::{
    BlocklistAIHistoryEvent, BlocklistAIHistoryModel, PendingAttachment, PendingFile, RequestInput,
    ResponseStream, ResponseStreamId,
};
use crate::ai::execution_profiles::profiles::AIExecutionProfilesModel;
use crate::ai::llms::LLMId;
use crate::persistence::model::{
    AgentRuntimeBinding, AgentRuntimeRunRecord, AgentRuntimeRunState, AgentRuntimeTerminalOutcome,
};
use crate::persistence::schema::agent_runtime_runs::dsl as runtime_runs_dsl;
use crate::persistence::{setup_database, start_writer, ModelEvent};
use crate::test_util::terminal::{add_window_with_terminal, initialize_app_for_terminal_view};
use crate::GlobalResourceHandlesProvider;

fn new_ambient_agent_task_id() -> AmbientAgentTaskId {
    Uuid::new_v4().to_string().parse().unwrap()
}

fn image_attachment(file_name: &str) -> PendingAttachment {
    PendingAttachment::Image(ImageContext {
        data: String::new(),
        mime_type: "image/png".to_owned(),
        file_name: file_name.to_owned(),
        is_figma: false,
    })
}

fn file_attachment(file_name: &str) -> PendingAttachment {
    PendingAttachment::File(PendingFile {
        file_name: file_name.to_owned(),
        file_path: file_name.into(),
        mime_type: "text/plain".to_owned(),
    })
}

fn install_custom_runtime_provider(app: &mut App) {
    let custom_model_id = LLMId::from("test-config-key");
    ApiKeyManager::handle(app).update(app, |manager, ctx| {
        manager.add_custom_endpoint(
            "Local Provider".to_string(),
            "http://localhost:11434/v1".to_string(),
            "test-key".to_string(),
            vec![(
                "test-model".to_string(),
                None,
                Some(custom_model_id.to_string()),
            )],
            ctx,
        );
    });
    AIExecutionProfilesModel::handle(app).update(app, |profiles, ctx| {
        let default_profile_id = profiles.default_profile_id();
        profiles.set_base_model(default_profile_id, Some(custom_model_id.clone()), ctx);
        profiles.set_coding_model(default_profile_id, Some(custom_model_id.clone()), ctx);
        profiles.set_cli_agent_model(default_profile_id, Some(custom_model_id.clone()), ctx);
        profiles.set_computer_use_model(default_profile_id, Some(custom_model_id), ctx);
    });
}

async fn wait_for_conversation_status(
    app: &mut App,
    conversation_id: AIConversationId,
    expected: ConversationStatus,
) {
    for _ in 0..500 {
        let status = BlocklistAIHistoryModel::handle(app).read(app, |history, _| {
            history
                .conversation(&conversation_id)
                .map(|conversation| conversation.status().clone())
        });
        if status.as_ref() == Some(&expected) {
            return;
        }
        Timer::after(Duration::from_millis(10)).await;
    }
    let status = BlocklistAIHistoryModel::handle(app).read(app, |history, _| {
        history
            .conversation(&conversation_id)
            .map(|conversation| conversation.status().clone())
    });
    panic!("conversation did not reach {expected:?}; last status: {status:?}");
}

async fn wait_for_agent_output_count(
    app: &mut App,
    conversation_id: AIConversationId,
    expected_count: usize,
) {
    for _ in 0..500 {
        let output_count = BlocklistAIHistoryModel::handle(app).read(app, |history, _| {
            history
                .conversation(&conversation_id)
                .into_iter()
                .flat_map(|conversation| conversation.all_tasks())
                .flat_map(|task| task.messages())
                .filter(|message| {
                    matches!(
                        message.message.as_ref(),
                        Some(api::message::Message::AgentOutput(_))
                    )
                })
                .count()
        });
        if output_count == expected_count {
            return;
        }
        Timer::after(Duration::from_millis(10)).await;
    }
    panic!("conversation did not reach {expected_count} agent outputs");
}

fn assert_agent_outputs(
    app: &App,
    conversation_id: AIConversationId,
    expected_outputs: &[(&str, &str)],
) {
    BlocklistAIHistoryModel::handle(app).read(app, |history, _| {
        let conversation = history
            .conversation(&conversation_id)
            .expect("conversation should exist");
        let agent_outputs = conversation
            .all_tasks()
            .flat_map(|task| task.messages())
            .filter_map(|message| match message.message.as_ref() {
                Some(api::message::Message::AgentOutput(output)) => {
                    Some((message.request_id.as_str(), output.text.as_str()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(agent_outputs, expected_outputs);
    });
}

fn assert_retry_transcript_excludes_interrupted_output(observer_dir: &TempDir) {
    let transcripts = fs::read_to_string(observer_dir.path().join("accepted-transcripts.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(transcripts.len(), 2);
    assert_eq!(transcripts[1]["revision"], 2);
    assert_eq!(transcripts[1]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        transcripts[1]["items"][0]["content"][0]["text"],
        "Inspect the workspace"
    );
}

fn test_launch_config(mode: &str, observer_dir: &TempDir) -> AgentRuntimeLaunchConfig {
    AgentRuntimeLaunchConfig::new(
        node_executable(),
        [
            OsString::from(fake_bridge_path()),
            OsString::from(mode),
            observer_dir.path().as_os_str().to_owned(),
        ],
    )
}

fn fake_bridge_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tools/warp-bridge/test/supervisor-fake-bridge.mjs")
}

fn node_executable() -> PathBuf {
    let executable = if cfg!(windows) { "node.exe" } else { "node" };
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(executable))
        .find(|candidate| is_executable_file(candidate))
        .expect("Node.js must be available for the fake Bridge tests")
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    true
}

#[test]
fn passive_suggestions_request_params_omit_ambient_agent_task_id() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        terminal.update(&mut app, |terminal, ctx| {
            let task_id = new_ambient_agent_task_id();
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    history_model.start_new_conversation(terminal.id(), false, false, false, ctx)
                });

            terminal.ai_controller().update(ctx, |controller, ctx| {
                controller.set_ambient_agent_task_id(Some(task_id), ctx);

                assert_eq!(controller.get_ambient_agent_task_id(), Some(task_id));
                assert_eq!(
                    controller
                        .build_passive_suggestions_request_params(
                            Some(conversation_id),
                            PassiveSuggestionTrigger::FilesChanged,
                            vec![],
                            ctx,
                        )
                        .expect("existing conversation should build passive suggestion params")
                        .1
                        .ambient_agent_task_id,
                    None
                );
                assert_eq!(
                    controller
                        .build_passive_suggestions_request_params(
                            None,
                            PassiveSuggestionTrigger::FilesChanged,
                            vec![],
                            ctx,
                        )
                        .expect("new conversation should build passive suggestion params")
                        .1
                        .ambient_agent_task_id,
                    None
                );
            });
        });
    });
}

#[test]
#[serial_test::serial]
fn passive_suggestion_conversation_stays_rust_bound_when_pi_rollout_is_enabled() {
    let _pi_flag = FeatureFlag::PiAgentRuntime.override_enabled(true);
    let _local_flag = FeatureFlag::LocalOnlyCustomProviderMode.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        install_custom_runtime_provider(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        let conversation_id = terminal.update(&mut app, |terminal, ctx| {
            terminal
                .ai_controller()
                .update(ctx, |controller, ctx| {
                    controller.send_unit_test_suggestions_request(
                        "test output".to_string(),
                        PassiveSuggestionTrigger::FilesChanged,
                        ctx,
                    )
                })
                .expect("passive suggestion request should start")
                .0
        });

        BlocklistAIHistoryModel::handle(&app).read(&app, |history, _| {
            assert_eq!(
                history
                    .conversation(&conversation_id)
                    .expect("passive conversation should exist")
                    .runtime_binding(),
                AgentRuntimeBinding::Rust,
            );
        });
    });
}

#[test]
fn input_for_query_converts_prompt_attachments_and_ignores_live_staging() {
    // `input_for_query` builds its image/file context purely from the explicitly-provided
    // attachment set (resolved by `send_query` from either the queued row or live staging),
    // never from the context model's pending attachments.
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        terminal.update(&mut app, |terminal, ctx| {
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    history_model.start_new_conversation(terminal.id(), false, false, false, ctx)
                });

            let controller = terminal.ai_controller();
            let context_model = controller.as_ref(ctx).context_model.clone();
            let active_session = controller.as_ref(ctx).active_session.clone();

            // Stage *live* attachments that must NOT leak into a query built from a different,
            // explicitly-provided attachment set.
            context_model.update(ctx, |m, ctx| {
                m.append_pending_attachments(
                    vec![image_attachment("live.png"), file_attachment("live.txt")],
                    ctx,
                );
            });

            let task_id = TaskId::new("test-task".to_owned());
            // Two files sharing a basename to exercise duplicate-basename suffixing.
            let prompt_attachments = vec![
                image_attachment("queued.png"),
                file_attachment("notes.txt"),
                file_attachment("notes.txt"),
            ];

            let input = super::input_for_query(
                "build a query".to_owned(),
                &task_id,
                conversation_id,
                None,
                UserQueryMode::Normal,
                None,
                HashMap::new(),
                prompt_attachments,
                context_model.as_ref(ctx),
                active_session.as_ref(ctx),
                ctx,
            );

            let AIAgentInput::UserQuery {
                context,
                referenced_attachments,
                ..
            } = input
            else {
                panic!("expected UserQuery");
            };

            // The provided image is attached as image context; the live-staged image is not.
            let image_names: Vec<&str> = context
                .iter()
                .filter_map(|c| match c {
                    AIAgentContext::Image(img) => Some(img.file_name.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(image_names, vec!["queued.png"]);

            // The provided files are attached as FilePathReference with duplicate-basename
            // suffixing; the live-staged file is not.
            let mut file_names: Vec<String> = referenced_attachments
                .values()
                .filter_map(|a| match a {
                    AIAgentAttachment::FilePathReference { file_name, .. } => {
                        Some(file_name.clone())
                    }
                    _ => None,
                })
                .collect();
            file_names.sort();
            assert_eq!(
                file_names,
                vec!["notes.txt".to_owned(), "notes.txt".to_owned()]
            );
            assert!(referenced_attachments.contains_key("notes.txt"));
            assert!(referenced_attachments.contains_key("notes.txt (1)"));
            assert!(!referenced_attachments.contains_key("live.txt"));
        });
    });
}

#[test]
fn pi_bound_conversation_does_not_fall_back_to_response_stream() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        let conversation_id = terminal.update(&mut app, |terminal, ctx| {
            AgentRuntimeService::handle(ctx).update(ctx, |service, _| {
                service.set_start_result_for_test(Err(RuntimeStartError::BridgeStartupFailed));
            });
            let terminal_surface_id = terminal.id();
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    let conversation_id = history_model.start_new_conversation(
                        terminal_surface_id,
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

                assert!(
                    !controller.has_active_stream_for_conversation(conversation_id, ctx),
                    "Pi-bound requests must not fall back to the Rust response stream"
                );
            });
            AgentRuntimeService::handle(ctx).read(ctx, |service, _| {
                assert_eq!(
                    service.start_attempts_for_test(conversation_id),
                    1,
                    "Pi-bound requests must route through the app-wide Runtime Supervisor"
                );
            });

            conversation_id
        });

        BlocklistAIHistoryModel::handle(&app).read(&app, |history, _| {
            let conversation = history
                .conversation(&conversation_id)
                .expect("conversation should remain viewable");
            assert_eq!(conversation.status(), &ConversationStatus::Error);
            assert_eq!(conversation.runtime_binding(), AgentRuntimeBinding::Pi);
            assert!(matches!(
                conversation
                    .latest_exchange()
                    .map(|exchange| &exchange.output_status),
                Some(AIAgentOutputStatus::Finished {
                    finished_output: FinishedAIAgentOutput::Error {
                        error: RenderableAIError::AgentRuntimeUnavailable { .. },
                        ..
                    },
                })
            ));
        });
    });
}

#[test]
fn concurrent_pi_start_cannot_overwrite_the_active_run_identity() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        let conversation_id = terminal.update(&mut app, |terminal, ctx| {
            let terminal_surface_id = terminal.id();
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    let conversation_id = history_model.start_new_conversation(
                        terminal_surface_id,
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
            AgentRuntimeService::handle(ctx).update(ctx, |service, _| {
                service.set_active_run_for_test(conversation_id, "run-existing");
            });

            terminal.ai_controller().update(ctx, |controller, ctx| {
                controller.send_user_query_in_conversation(
                    "Do not replace the active run".to_string(),
                    conversation_id,
                    None,
                    ctx,
                );
            });
            conversation_id
        });

        AgentRuntimeService::handle(&app).read(&app, |service, _| {
            assert_eq!(
                service.active_run_id_for_test(conversation_id),
                Some("run-existing")
            );
            assert_eq!(
                service.start_attempts_for_test(conversation_id),
                0,
                "the second controller must queue behind the app-wide active Run"
            );
            assert_eq!(service.cancel_attempts_for_test(conversation_id), 1);
        });
    });
}

#[test]
fn cancelling_conversation_aborts_pending_auto_resume() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        // An ID with no backing conversation: if the scheduled wait ever
        // completes, the resume is a harmless no-op.
        let conversation_id = AIConversationId::new();

        terminal.update(&mut app, |terminal, ctx| {
            terminal.ai_controller().update(ctx, |controller, ctx| {
                controller.schedule_auto_resume_after_error(conversation_id, ctx);
                assert!(controller
                    .pending_auto_resume_handles
                    .contains_key(&conversation_id));

                controller.cancel_conversation_progress(
                    conversation_id,
                    CancellationReason::ManuallyCancelled,
                    ctx,
                );
                assert!(!controller
                    .pending_auto_resume_handles
                    .contains_key(&conversation_id));
            });
        });
    });
}

#[test]
fn cancelling_pi_bound_conversation_routes_to_runtime_supervisor() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        let conversation_id = terminal.update(&mut app, |terminal, ctx| {
            let terminal_surface_id = terminal.id();
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    let conversation_id = history_model.start_new_conversation(
                        terminal_surface_id,
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
                controller.cancel_conversation_progress(
                    conversation_id,
                    CancellationReason::ManuallyCancelled,
                    ctx,
                );
            });

            AgentRuntimeService::handle(ctx).read(ctx, |service, _| {
                assert_eq!(
                    service.cancel_attempts_for_test(conversation_id),
                    1,
                    "Pi-bound cancellation must route through the app-wide Runtime Supervisor"
                );
            });

            conversation_id
        });

        BlocklistAIHistoryModel::handle(&app).read(&app, |history, _| {
            assert!(
                history.conversation(&conversation_id).is_some(),
                "cancelling must not remove the conversation"
            );
        });
    });
}

#[test]
fn resuming_pi_bound_conversation_routes_to_runtime_supervisor() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        terminal.update(&mut app, |terminal, ctx| {
            AgentRuntimeService::handle(ctx).update(ctx, |service, _| {
                service.set_start_result_for_test(Err(RuntimeStartError::BridgeStartupFailed));
            });
            let terminal_surface_id = terminal.id();
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    let conversation_id = history_model.start_new_conversation(
                        terminal_surface_id,
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
                controller.resume_conversation(
                    conversation_id,
                    /*can_attempt_resume_on_error*/ true,
                    /*is_auto_resume_after_error*/ false,
                    vec![],
                    ctx,
                );

                assert!(
                    !controller.has_active_stream_for_conversation(conversation_id, ctx),
                    "Pi-bound resume must not fall back to the Rust response stream"
                );
            });
            AgentRuntimeService::handle(ctx).read(ctx, |service, _| {
                assert_eq!(
                    service.start_attempts_for_test(conversation_id),
                    1,
                    "Pi-bound resume must route through the app-wide Runtime Supervisor"
                );
            });
        });
    });
}

#[test]
#[serial_test::serial]
fn pi_runtime_start_delta_commit_finish_and_retry_run_through_controller() {
    let _anonymous_only = FeatureFlag::AnonymousOnlyMode.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        install_custom_runtime_provider(&mut app);

        let database_dir = TempDir::new().unwrap();
        let database_path = database_dir.path().join("warp.sqlite");
        let conn = setup_database(&database_path).unwrap();
        let writer = start_writer(conn, database_path.clone()).unwrap();
        GlobalResourceHandlesProvider::handle(&app).update(&mut app, |provider, _| {
            provider.set_model_event_sender_for_test(Some(writer.sender.clone()));
        });

        let observer_dir = TempDir::new().unwrap();
        let supervisor = AgentRuntimeSupervisor::new(
            test_launch_config("text-runs", &observer_dir),
            Arc::new(Background::default()),
        );
        AgentRuntimeService::handle(&app).update(&mut app, |service, _| {
            service.set_supervisor_for_test(Some(supervisor.clone()));
            service.set_run_ids_for_test(["run-1", "run-2"]);
        });

        let terminal = add_window_with_terminal(&mut app, None);
        let conversation_id = terminal.update(&mut app, |terminal, ctx| {
            let terminal_surface_id = terminal.id();
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    let conversation_id = history_model.start_new_conversation(
                        terminal_surface_id,
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
                    "Inspect the workspace".to_string(),
                    conversation_id,
                    None,
                    ctx,
                );
                assert!(
                    !controller.has_active_stream_for_conversation(conversation_id, ctx),
                    "Pi-bound start must not fall back to the Rust response stream"
                );
            });

            conversation_id
        });

        wait_for_conversation_status(&mut app, conversation_id, ConversationStatus::Error).await;
        assert_agent_outputs(&app, conversation_id, &[("run-1", "Partial output")]);

        terminal.update(&mut app, |terminal, ctx| {
            terminal.ai_controller().update(ctx, |controller, ctx| {
                controller.resume_conversation(
                    conversation_id,
                    /*can_attempt_resume_on_error*/ true,
                    /*is_auto_resume_after_error*/ false,
                    vec![],
                    ctx,
                );
                assert!(
                    !controller.has_active_stream_for_conversation(conversation_id, ctx),
                    "Pi-bound retry must not fall back to the Rust response stream"
                );
            });
        });

        wait_for_conversation_status(&mut app, conversation_id, ConversationStatus::Success).await;
        assert_agent_outputs(
            &app,
            conversation_id,
            &[("run-1", "Partial output"), ("run-2", "Completed output")],
        );
        AgentRuntimeService::handle(&app).read(&app, |service, _| {
            assert_eq!(service.start_attempts_for_test(conversation_id), 2);
        });
        assert_retry_transcript_excludes_interrupted_output(&observer_dir);

        supervisor.shutdown_all().await;
        GlobalResourceHandlesProvider::handle(&app).update(&mut app, |provider, _| {
            provider.set_model_event_sender_for_test(None);
        });
        writer.sender.send(ModelEvent::Terminate).unwrap();
        writer.handle.join().unwrap();
    });
}

#[test]
#[serial_test::serial]
fn cancelling_active_pi_runtime_run_reaches_the_bridge() {
    let _anonymous_only = FeatureFlag::AnonymousOnlyMode.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        install_custom_runtime_provider(&mut app);

        let database_dir = TempDir::new().unwrap();
        let database_path = database_dir.path().join("warp.sqlite");
        let conn = setup_database(&database_path).unwrap();
        let writer = start_writer(conn, database_path.clone()).unwrap();
        GlobalResourceHandlesProvider::handle(&app).update(&mut app, |provider, _| {
            provider.set_model_event_sender_for_test(Some(writer.sender.clone()));
        });

        let observer_dir = TempDir::new().unwrap();
        let supervisor = AgentRuntimeSupervisor::new(
            test_launch_config("text-run-cancel", &observer_dir),
            Arc::new(Background::default()),
        );
        AgentRuntimeService::handle(&app).update(&mut app, |service, _| {
            service.set_supervisor_for_test(Some(supervisor.clone()));
            service.set_run_ids_for_test(["run-cancel"]);
        });

        let terminal = add_window_with_terminal(&mut app, None);
        let conversation_id = terminal.update(&mut app, |terminal, ctx| {
            let terminal_surface_id = terminal.id();
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    let conversation_id = history_model.start_new_conversation(
                        terminal_surface_id,
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
                    "Cancel this run".to_string(),
                    conversation_id,
                    None,
                    ctx,
                );
            });
            conversation_id
        });

        wait_for_agent_output_count(&mut app, conversation_id, 1).await;
        terminal.update(&mut app, |terminal, ctx| {
            terminal.ai_controller().update(ctx, |controller, ctx| {
                controller.cancel_conversation_progress(
                    conversation_id,
                    CancellationReason::ManuallyCancelled,
                    ctx,
                );
            });
        });

        wait_for_conversation_status(&mut app, conversation_id, ConversationStatus::Cancelled)
            .await;
        assert_agent_outputs(&app, conversation_id, &[("run-cancel", "Partial output")]);

        supervisor.shutdown_all().await;
        GlobalResourceHandlesProvider::handle(&app).update(&mut app, |provider, _| {
            provider.set_model_event_sender_for_test(None);
        });
        writer.sender.send(ModelEvent::Terminate).unwrap();
        writer.handle.join().unwrap();
    });
}

#[test]
#[serial_test::serial]
fn cancelling_pi_run_during_bridge_startup_preserves_input_and_stops_start() {
    let _anonymous_only = FeatureFlag::AnonymousOnlyMode.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        install_custom_runtime_provider(&mut app);

        let database_dir = TempDir::new().unwrap();
        let database_path = database_dir.path().join("warp.sqlite");
        let conn = setup_database(&database_path).unwrap();
        let writer = start_writer(conn, database_path.clone()).unwrap();
        GlobalResourceHandlesProvider::handle(&app).update(&mut app, |provider, _| {
            provider.set_model_event_sender_for_test(Some(writer.sender.clone()));
        });

        let observer_dir = TempDir::new().unwrap();
        let supervisor = AgentRuntimeSupervisor::new_with_config(
            test_launch_config("hang-handshake", &observer_dir),
            RuntimeSupervisorConfig {
                handshake_timeout: Duration::from_millis(200),
                ..RuntimeSupervisorConfig::default()
            },
            Arc::new(Background::default()),
        );
        AgentRuntimeService::handle(&app).update(&mut app, |service, _| {
            service.set_supervisor_for_test(Some(supervisor.clone()));
            service.set_run_ids_for_test(["run-starting"]);
        });

        let terminal = add_window_with_terminal(&mut app, None);
        let conversation_id = terminal.update(&mut app, |terminal, ctx| {
            let terminal_surface_id = terminal.id();
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    let conversation_id = history_model.start_new_conversation(
                        terminal_surface_id,
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
                    "Persist before startup".to_string(),
                    conversation_id,
                    None,
                    ctx,
                );
            });
            conversation_id
        });

        for _ in 0..500 {
            let prepared = AgentRuntimeService::handle(&app).read(&app, |service, _| {
                service.last_run_id_for_test(conversation_id) == Some("run-starting")
            });
            if prepared {
                break;
            }
            Timer::after(Duration::from_millis(10)).await;
        }
        BlocklistAIHistoryModel::handle(&app).read(&app, |history, _| {
            let conversation = history
                .conversation(&conversation_id)
                .expect("conversation should remain viewable during startup");
            assert_eq!(conversation.runtime_transcript_revision(), 1);
            assert!(conversation
                .all_linearized_messages()
                .iter()
                .any(|message| matches!(
                    message.message.as_ref(),
                    Some(api::message::Message::UserQuery(query))
                        if query.query == "Persist before startup"
                )));
        });

        terminal.update(&mut app, |terminal, ctx| {
            terminal.ai_controller().update(ctx, |controller, ctx| {
                controller.cancel_conversation_progress(
                    conversation_id,
                    CancellationReason::ManuallyCancelled,
                    ctx,
                );
            });
        });
        wait_for_conversation_status(&mut app, conversation_id, ConversationStatus::Cancelled)
            .await;
        AgentRuntimeService::handle(&app).read(&app, |service, _| {
            assert!(!service.has_active_run(conversation_id));
        });

        supervisor.shutdown_all().await;
        GlobalResourceHandlesProvider::handle(&app).update(&mut app, |provider, _| {
            provider.set_model_event_sender_for_test(None);
        });
        writer.sender.send(ModelEvent::Terminate).unwrap();
        writer.handle.join().unwrap();
    });
}

#[test]
#[serial_test::serial]
fn missing_bridge_supervisor_persists_a_failed_pi_run_and_input() {
    let _anonymous_only = FeatureFlag::AnonymousOnlyMode.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        install_custom_runtime_provider(&mut app);

        let database_dir = TempDir::new().unwrap();
        let database_path = database_dir.path().join("warp.sqlite");
        let conn = setup_database(&database_path).unwrap();
        let writer = start_writer(conn, database_path.clone()).unwrap();
        GlobalResourceHandlesProvider::handle(&app).update(&mut app, |provider, _| {
            provider.set_model_event_sender_for_test(Some(writer.sender.clone()));
        });
        AgentRuntimeService::handle(&app).update(&mut app, |service, _| {
            service.set_supervisor_for_test(None);
            service.set_run_ids_for_test(["run-no-supervisor"]);
        });

        let terminal = add_window_with_terminal(&mut app, None);
        let conversation_id = terminal.update(&mut app, |terminal, ctx| {
            let terminal_surface_id = terminal.id();
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    let conversation_id = history_model.start_new_conversation(
                        terminal_surface_id,
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
                    "Persist without Bridge".to_string(),
                    conversation_id,
                    None,
                    ctx,
                );
            });
            conversation_id
        });

        wait_for_conversation_status(&mut app, conversation_id, ConversationStatus::Error).await;
        BlocklistAIHistoryModel::handle(&app).read(&app, |history, _| {
            let conversation = history
                .conversation(&conversation_id)
                .expect("failed conversation should remain viewable");
            assert_eq!(conversation.runtime_transcript_revision(), 1);
            assert!(conversation
                .all_linearized_messages()
                .iter()
                .any(|message| matches!(
                    message.message.as_ref(),
                    Some(api::message::Message::UserQuery(query))
                        if query.query == "Persist without Bridge"
                )));
        });

        GlobalResourceHandlesProvider::handle(&app).update(&mut app, |provider, _| {
            provider.set_model_event_sender_for_test(None);
        });
        writer.sender.send(ModelEvent::Terminate).unwrap();
        writer.handle.join().unwrap();

        let mut conn = setup_database(&database_path).unwrap();
        let run = runtime_runs_dsl::agent_runtime_runs
            .filter(runtime_runs_dsl::run_id.eq("run-no-supervisor"))
            .select(AgentRuntimeRunRecord::as_select())
            .first::<AgentRuntimeRunRecord>(&mut conn)
            .unwrap();
        assert_eq!(run.state(), Some(AgentRuntimeRunState::Finished));
        assert_eq!(
            run.terminal_outcome(),
            Some(AgentRuntimeTerminalOutcome::Failed)
        );
    });
}

#[test]
fn direct_pi_follow_up_waits_for_the_active_run_to_finish() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        let conversation_id = terminal.update(&mut app, |terminal, ctx| {
            let terminal_surface_id = terminal.id();
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history_model, ctx| {
                    let conversation_id = history_model.start_new_conversation(
                        terminal_surface_id,
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
            AgentRuntimeService::handle(ctx).update(ctx, |service, _| {
                service.set_active_run_for_test(conversation_id, "run-active");
                service.set_start_result_for_test(Err(RuntimeStartError::BridgeStartupFailed));
            });

            terminal.ai_controller().update(ctx, |controller, ctx| {
                controller.send_user_query_in_conversation(
                    "follow up".to_string(),
                    conversation_id,
                    None,
                    ctx,
                );
                assert_eq!(
                    controller
                        .pending_pi_follow_ups
                        .get(&conversation_id)
                        .map(|pending| pending.len()),
                    Some(1),
                );
            });
            AgentRuntimeService::handle(ctx).read(ctx, |service, _| {
                assert_eq!(service.cancel_attempts_for_test(conversation_id), 1);
                assert_eq!(service.start_attempts_for_test(conversation_id), 0);
            });
            conversation_id
        });

        AgentRuntimeService::handle(&app).update(&mut app, |service, ctx| {
            service.finish_active_run_for_test(conversation_id, ctx);
        });

        terminal.read(&app, |terminal, ctx| {
            terminal.ai_controller().read(ctx, |controller, _| {
                assert!(!controller
                    .pending_pi_follow_ups
                    .contains_key(&conversation_id));
            });
        });
        AgentRuntimeService::handle(&app).read(&app, |service, _| {
            assert_eq!(service.start_attempts_for_test(conversation_id), 1);
        });
    });
}

#[test]
fn mock_response_stream_updates_history_through_controller() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);
        let captured_events = Arc::new(Mutex::new(Vec::new()));
        let events_for_subscription = Arc::clone(&captured_events);
        app.update(|ctx| {
            ctx.subscribe_to_model(&BlocklistAIHistoryModel::handle(ctx), move |_, event, _| {
                events_for_subscription.lock().unwrap().push(event.clone())
            });
        });

        let (conversation_id, stream) = terminal.update(&mut app, |view, ctx| {
            let terminal_surface_id = view.id();
            let stream_id = ResponseStreamId::new_for_test();
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, ctx| {
                    let conversation_id = history.start_new_conversation(
                        terminal_surface_id,
                        false,
                        false,
                        false,
                        ctx,
                    );
                    let task_id = history
                        .conversation(&conversation_id)
                        .unwrap()
                        .get_root_task_id()
                        .clone();
                    history
                        .update_conversation_for_new_request_input(
                            RequestInput {
                                conversation_id,
                                input_messages: HashMap::from([(task_id, vec![])]),
                                working_directory: None,
                                model_id: LLMId::from("test-model"),
                                coding_model_id: LLMId::from("test-coding-model"),
                                cli_agent_model_id: LLMId::from("test-cli-agent-model"),
                                computer_use_model_id: LLMId::from("test-computer-use-model"),
                                shared_session_response_initiator: None,
                                request_start_ts: Local::now(),
                                supported_tools_override: None,
                            },
                            stream_id.clone(),
                            terminal_surface_id,
                            ctx,
                        )
                        .unwrap();
                    conversation_id
                });
            let stream = ctx.add_model(|_| ResponseStream::new_for_test(stream_id.clone()));
            view.ai_controller().update(ctx, |controller, ctx| {
                controller.register_mock_stream_for_test(
                    stream_id,
                    conversation_id,
                    stream.clone(),
                    ctx,
                );
            });
            (conversation_id, stream)
        });

        stream.update(&mut app, |stream, ctx| {
            stream.emit_response_event_for_test(
                warp_multi_agent_api::ResponseEvent {
                    r#type: Some(response_event::Type::Init(response_event::StreamInit {
                        request_id: "test-request".to_string(),
                        conversation_id: "test-server-conversation".to_string(),
                        run_id: String::new(),
                    })),
                },
                ctx,
            );
            stream.emit_response_event_for_test(
                warp_multi_agent_api::ResponseEvent {
                    r#type: Some(response_event::Type::Finished(
                        response_event::StreamFinished {
                            reason: Some(response_event::stream_finished::Reason::Done(
                                response_event::stream_finished::Done {},
                            )),
                            conversation_usage_metadata: None,
                            token_usage: vec![],
                            should_refresh_model_config: false,
                            request_cost: None,
                        },
                    )),
                },
                ctx,
            );
        });

        BlocklistAIHistoryModel::handle(&app).read(&app, |history, _| {
            assert_eq!(
                history.conversation(&conversation_id).map(|c| c.status()),
                Some(&crate::ai::agent::conversation::ConversationStatus::Success)
            );
        });
        let events = captured_events.lock().unwrap();
        assert!(events.iter().any(|event| matches!(
            event,
            BlocklistAIHistoryEvent::ConversationServerTokenAssigned {
                conversation_id: id,
                ..
            } if *id == conversation_id
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            BlocklistAIHistoryEvent::UpdatedStreamingExchange {
                conversation_id: id,
                ..
            } if *id == conversation_id
        )));
    });
}

/// When an agent command exits the shell, the conversation must be finalized as
/// `Error` (not `Cancelled`), and a subsequent `ManuallyCancelled` (as fired by
/// the pane-close path) must not overwrite that failure.
#[test]
fn fail_conversation_due_to_shell_exit_reports_error_and_survives_manual_cancel() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        let conversation_id = terminal.update(&mut app, |view, ctx| {
            let terminal_surface_id = view.id();
            let stream_id = ResponseStreamId::new_for_test();
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, ctx| {
                    let conversation_id = history.start_new_conversation(
                        terminal_surface_id,
                        false,
                        false,
                        false,
                        ctx,
                    );
                    let task_id = history
                        .conversation(&conversation_id)
                        .unwrap()
                        .get_root_task_id()
                        .clone();
                    history
                        .update_conversation_for_new_request_input(
                            RequestInput {
                                conversation_id,
                                input_messages: HashMap::from([(task_id, vec![])]),
                                working_directory: None,
                                model_id: LLMId::from("test-model"),
                                coding_model_id: LLMId::from("test-coding-model"),
                                cli_agent_model_id: LLMId::from("test-cli-agent-model"),
                                computer_use_model_id: LLMId::from("test-computer-use-model"),
                                shared_session_response_initiator: None,
                                request_start_ts: Local::now(),
                                supported_tools_override: None,
                            },
                            stream_id.clone(),
                            terminal_surface_id,
                            ctx,
                        )
                        .unwrap();
                    conversation_id
                });
            let stream = ctx.add_model(|_| ResponseStream::new_for_test(stream_id.clone()));
            view.ai_controller().update(ctx, |controller, ctx| {
                controller.register_mock_stream_for_test(stream_id, conversation_id, stream, ctx);
                controller.fail_conversation_due_to_shell_exit(conversation_id, ctx);
            });
            conversation_id
        });

        // The in-flight request is finalized as Error (with the shell-exit error
        // on its exchange), not Cancelled.
        BlocklistAIHistoryModel::handle(&app).read(&app, |history, _| {
            assert_eq!(
                history.conversation(&conversation_id).map(|c| c.status()),
                Some(&crate::ai::agent::conversation::ConversationStatus::Error)
            );
        });

        // The pane-close cancellation path must be a no-op now that the
        // conversation is terminal.
        terminal.update(&mut app, |view, ctx| {
            view.ai_controller().update(ctx, |controller, ctx| {
                controller.cancel_conversation_progress(
                    conversation_id,
                    CancellationReason::ManuallyCancelled,
                    ctx,
                );
            });
        });
        BlocklistAIHistoryModel::handle(&app).read(&app, |history, _| {
            assert_eq!(
                history.conversation(&conversation_id).map(|c| c.status()),
                Some(&crate::ai::agent::conversation::ConversationStatus::Error)
            );
        });
    });
}

/// An optimistic long-running-command completion that cancels an in-flight
/// stream must finalize the conversation as `Success`, not `Cancelled`. This is
/// a regression test for the reason -> status mapping living in a single place
/// (`CancellationReason::conversation_outcome`).
#[test]
fn optimistic_cli_subagent_completion_with_in_flight_stream_reports_success() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        let conversation_id = terminal.update(&mut app, |view, ctx| {
            let terminal_surface_id = view.id();
            let stream_id = ResponseStreamId::new_for_test();
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, ctx| {
                    let conversation_id = history.start_new_conversation(
                        terminal_surface_id,
                        false,
                        false,
                        false,
                        ctx,
                    );
                    let task_id = history
                        .conversation(&conversation_id)
                        .unwrap()
                        .get_root_task_id()
                        .clone();
                    history
                        .update_conversation_for_new_request_input(
                            RequestInput {
                                conversation_id,
                                input_messages: HashMap::from([(task_id, vec![])]),
                                working_directory: None,
                                model_id: LLMId::from("test-model"),
                                coding_model_id: LLMId::from("test-coding-model"),
                                cli_agent_model_id: LLMId::from("test-cli-agent-model"),
                                computer_use_model_id: LLMId::from("test-computer-use-model"),
                                shared_session_response_initiator: None,
                                request_start_ts: Local::now(),
                                supported_tools_override: None,
                            },
                            stream_id.clone(),
                            terminal_surface_id,
                            ctx,
                        )
                        .unwrap();
                    conversation_id
                });
            let stream = ctx.add_model(|_| ResponseStream::new_for_test(stream_id.clone()));
            view.ai_controller().update(ctx, |controller, ctx| {
                controller.register_mock_stream_for_test(stream_id, conversation_id, stream, ctx);
                // The long-running command finished while the agent was still
                // streaming, cancelling the in-flight stream optimistically.
                controller.cancel_conversation_progress(
                    conversation_id,
                    CancellationReason::CommandFinishedDuringInlineAgentView,
                    ctx,
                );
            });
            conversation_id
        });

        BlocklistAIHistoryModel::handle(&app).read(&app, |history, _| {
            assert_eq!(
                history.conversation(&conversation_id).map(|c| c.status()),
                Some(&crate::ai::agent::conversation::ConversationStatus::Success)
            );
        });
    });
}
