use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::Arc;
use std::time::Duration;

use diesel::prelude::*;
use prost::Message as _;
use tempfile::TempDir;
use warp_multi_agent_api as api;
use warpui::{App, ReadModel, SingletonEntity};
use warpui_core::r#async::{FutureExt as _, Timer};

use super::blocklist_adapter::{effect_outcome, BlocklistRuntimeToolActionAdapter};
use super::{RuntimeToolActionAdapter, ToolExecutionAuthority, ToolPermissionDecision};
use crate::ai::agent::conversation::AIConversation;
use crate::ai::agent::runtime::configuration::{
    ChatCompletionsProvider, ReasoningEffort, RunConfiguration,
};
use crate::ai::agent::runtime::text_run::{TextRunOutcome, TextRunRequest};
use crate::ai::agent::runtime::text_run_integration_tests::{
    runtime_data, task_with_user_message, test_launch_config,
};
use crate::ai::agent::runtime::tool_catalog::ToolCatalog;
use crate::ai::agent::runtime::transcript::{
    RuntimeContentBlock, RuntimeTranscript, ToolResultProjection,
};
use crate::ai::agent::runtime::AgentRuntimeSupervisor;
use crate::ai::agent::task::TaskId;
use crate::ai::agent::{
    AIAgentAction, AIAgentActionId, AIAgentActionResult, AIAgentActionResultType,
    AIAgentActionType, AnyFileContent, CancellationReason, FileContext, FileLocations,
    ReadFilesRequest, ReadFilesResult,
};
use crate::ai::blocklist::BlocklistAIHistoryModel;
use crate::ai::execution_profiles::profiles::AIExecutionProfilesModel;
use crate::ai::execution_profiles::ActionPermission;
use crate::persistence::model::{AgentToolExecutionRecord, AgentToolExecutionState};
use crate::persistence::schema::agent_tool_execution_records;
use crate::persistence::{setup_database, start_writer, upsert_agent_conversation, ModelEvent};
use crate::test_util::terminal::{add_window_with_terminal, initialize_app_for_terminal_view};

#[test]
fn permission_request_uses_live_blocklist_permissions_without_executing() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);
        let conversation_id = terminal.update(&mut app, |_view, ctx| {
            let terminal_view_id = ctx.view_id();
            BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, ctx| {
                history.start_new_conversation(terminal_view_id, false, false, false, ctx)
            })
        });
        AIExecutionProfilesModel::handle(&app).update(&mut app, |profiles, ctx| {
            let profile_id = *profiles.default_profile(ctx).id();
            profiles.set_read_files(profile_id, &ActionPermission::AlwaysAllow, ctx);
        });
        let controller = terminal.read(&app, |view, _| view.ai_controller().clone());
        let action_model = controller.read(&app, |controller, _| controller.action_model().clone());
        let adapter = Arc::new(BlocklistRuntimeToolActionAdapter::new_for_test(
            action_model.clone(),
            conversation_id,
            &mut app,
        ));

        let decision = adapter
            .request_permission(
                "run-1".to_string(),
                AIAgentAction {
                    id: AIAgentActionId::from("runtime-read".to_string()),
                    task_id: TaskId::new("task".to_string()),
                    action: AIAgentActionType::ReadFiles(ReadFilesRequest {
                        locations: vec![FileLocations {
                            name: "/tmp/runtime-read.txt".to_string(),
                            lines: Vec::new(),
                        }],
                    }),
                    requires_result: true,
                },
            )
            .await;

        assert_eq!(decision, ToolPermissionDecision::Approved);
        app.read_model(&action_model, |action_model, _| {
            assert!(action_model
                .get_pending_actions_for_conversation(&conversation_id)
                .next()
                .is_none());
        });
    });
}

#[test]
fn user_confirmation_approves_permission_without_executing_effect() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);
        let conversation_id = terminal.update(&mut app, |_view, ctx| {
            let terminal_view_id = ctx.view_id();
            BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, ctx| {
                history.start_new_conversation(terminal_view_id, false, false, false, ctx)
            })
        });
        AIExecutionProfilesModel::handle(&app).update(&mut app, |profiles, ctx| {
            let profile_id = *profiles.default_profile(ctx).id();
            profiles.set_read_files(profile_id, &ActionPermission::AlwaysAsk, ctx);
        });
        let controller = terminal.read(&app, |view, _| view.ai_controller().clone());
        let action_model = controller.read(&app, |controller, _| controller.action_model().clone());
        let adapter = Arc::new(BlocklistRuntimeToolActionAdapter::new_for_test(
            action_model.clone(),
            conversation_id,
            &mut app,
        ));
        let action_id = AIAgentActionId::from("runtime-confirm-read".to_string());
        let mut permission = adapter.request_permission(
            "run-1".to_string(),
            AIAgentAction {
                id: action_id.clone(),
                task_id: TaskId::new("task".to_string()),
                action: AIAgentActionType::ReadFiles(ReadFilesRequest {
                    locations: vec![FileLocations {
                        name: "/tmp/runtime-confirm-read.txt".to_string(),
                        lines: Vec::new(),
                    }],
                }),
                requires_result: true,
            },
        );

        assert!(futures::poll!(&mut permission).is_pending());
        for _ in 0..50 {
            if app.read_model(&action_model, |action_model, _| {
                action_model.get_pending_action_by_id(&action_id).is_some()
            }) {
                break;
            }
            Timer::after(Duration::from_millis(10)).await;
        }
        app.read_model(&action_model, |action_model, _| {
            assert!(action_model.get_pending_action_by_id(&action_id).is_some());
            assert!(action_model
                .get_finished_action_results(conversation_id)
                .is_none());
        });

        action_model.update(&mut app, |action_model, ctx| {
            action_model.execute_action(&action_id, conversation_id, ctx);
        });

        assert_eq!(permission.await, ToolPermissionDecision::Approved);
        app.read_model(&action_model, |action_model, _| {
            assert!(action_model.get_pending_action_by_id(&action_id).is_none());
            assert!(!action_model.has_unfinished_actions_for_conversation(conversation_id));
            assert!(action_model
                .get_finished_action_results(conversation_id)
                .is_none());
        });
    });
}

#[test]
fn user_cancellation_denies_permission_without_legacy_action_result() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);
        let conversation_id = terminal.update(&mut app, |_view, ctx| {
            let terminal_view_id = ctx.view_id();
            BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, ctx| {
                history.start_new_conversation(terminal_view_id, false, false, false, ctx)
            })
        });
        AIExecutionProfilesModel::handle(&app).update(&mut app, |profiles, ctx| {
            let profile_id = *profiles.default_profile(ctx).id();
            profiles.set_read_files(profile_id, &ActionPermission::AlwaysAsk, ctx);
        });
        let controller = terminal.read(&app, |view, _| view.ai_controller().clone());
        let action_model = controller.read(&app, |controller, _| controller.action_model().clone());
        let adapter = Arc::new(BlocklistRuntimeToolActionAdapter::new_for_test(
            action_model.clone(),
            conversation_id,
            &mut app,
        ));
        let action_id = AIAgentActionId::from("runtime-denied-read".to_string());
        let mut permission = adapter.request_permission(
            "run-1".to_string(),
            AIAgentAction {
                id: action_id.clone(),
                task_id: TaskId::new("task".to_string()),
                action: AIAgentActionType::ReadFiles(ReadFilesRequest {
                    locations: vec![FileLocations {
                        name: "/tmp/runtime-denied-read.txt".to_string(),
                        lines: Vec::new(),
                    }],
                }),
                requires_result: true,
            },
        );

        assert!(futures::poll!(&mut permission).is_pending());
        for _ in 0..50 {
            if app.read_model(&action_model, |action_model, _| {
                action_model.get_pending_action_by_id(&action_id).is_some()
            }) {
                break;
            }
            Timer::after(Duration::from_millis(10)).await;
        }
        app.read_model(&action_model, |action_model, _| {
            assert!(action_model.get_pending_action_by_id(&action_id).is_some());
        });

        controller.update(&mut app, |controller, ctx| {
            controller.cancel_conversation_progress(
                conversation_id,
                CancellationReason::ManuallyCancelled,
                ctx,
            );
        });

        assert_eq!(permission.await, ToolPermissionDecision::DeniedByUser);
        app.read_model(&action_model, |action_model, _| {
            assert!(action_model.get_pending_action_by_id(&action_id).is_none());
            assert!(action_model
                .get_finished_action_results(conversation_id)
                .is_none());
        });
    });
}

#[test]
fn dropped_permission_during_preprocessing_does_not_queue_stale_confirmation() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);
        let conversation_id = terminal.update(&mut app, |_view, ctx| {
            let terminal_view_id = ctx.view_id();
            BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, ctx| {
                history.start_new_conversation(terminal_view_id, false, false, false, ctx)
            })
        });
        AIExecutionProfilesModel::handle(&app).update(&mut app, |profiles, ctx| {
            let profile_id = *profiles.default_profile(ctx).id();
            profiles.set_read_files(profile_id, &ActionPermission::AlwaysAsk, ctx);
        });
        let controller = terminal.read(&app, |view, _| view.ai_controller().clone());
        let action_model = controller.read(&app, |controller, _| controller.action_model().clone());
        let adapter = Arc::new(BlocklistRuntimeToolActionAdapter::new_for_test(
            action_model.clone(),
            conversation_id,
            &mut app,
        ));
        let action_id = AIAgentActionId::from("runtime-cancelled-preprocess".to_string());
        let mut permission = adapter.request_permission(
            "run-1".to_string(),
            AIAgentAction {
                id: action_id.clone(),
                task_id: TaskId::new("task".to_string()),
                action: AIAgentActionType::ReadFiles(ReadFilesRequest {
                    locations: vec![FileLocations {
                        name: "/tmp/runtime-cancelled-preprocess.txt".to_string(),
                        lines: Vec::new(),
                    }],
                }),
                requires_result: true,
            },
        );

        assert!(futures::poll!(&mut permission).is_pending());
        drop(permission);
        Timer::after(Duration::from_millis(100)).await;

        app.read_model(&action_model, |action_model, _| {
            assert!(action_model.get_pending_action_by_id(&action_id).is_none());
        });
    });
}

#[test]
fn approved_read_files_executes_through_existing_typed_executor() {
    let file = tempfile::NamedTempFile::new().expect("temporary file should be created");
    std::fs::write(file.path(), "runtime typed executor content")
        .expect("temporary file should be writable");
    let file_path = file.path().to_string_lossy().into_owned();

    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);
        let conversation_id = terminal.update(&mut app, |_view, ctx| {
            let terminal_view_id = ctx.view_id();
            BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, ctx| {
                history.start_new_conversation(terminal_view_id, false, false, false, ctx)
            })
        });
        AIExecutionProfilesModel::handle(&app).update(&mut app, |profiles, ctx| {
            let profile_id = *profiles.default_profile(ctx).id();
            profiles.set_read_files(profile_id, &ActionPermission::AlwaysAllow, ctx);
        });
        let controller = terminal.read(&app, |view, _| view.ai_controller().clone());
        let action_model = controller.read(&app, |controller, _| controller.action_model().clone());
        let adapter = Arc::new(BlocklistRuntimeToolActionAdapter::new_for_test(
            action_model,
            conversation_id,
            &mut app,
        ));
        let action = AIAgentAction {
            id: AIAgentActionId::from("runtime-execute-read".to_string()),
            task_id: TaskId::new("task".to_string()),
            action: AIAgentActionType::ReadFiles(ReadFilesRequest {
                locations: vec![FileLocations {
                    name: file_path,
                    lines: Vec::new(),
                }],
            }),
            requires_result: true,
        };

        assert_eq!(
            adapter
                .request_permission("run-1".to_string(), action.clone())
                .await,
            ToolPermissionDecision::Approved
        );
        let outcome = adapter.execute("run-1".to_string(), action).await;

        match outcome.projection {
            ToolResultProjection::Success { content, truncated } => {
                assert!(!truncated);
                let RuntimeContentBlock::Text { text } = &content[0] else {
                    panic!("read-files projection should be text");
                };
                assert!(text.contains("runtime typed executor content"));
            }
            projection => panic!("expected successful projection, got {projection:?}"),
        }
    });
}

#[test]
fn supervisor_persists_real_blocklist_effect_before_bridge_result() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);
        let conversation_id = terminal.update(&mut app, |_view, ctx| {
            let terminal_view_id = ctx.view_id();
            BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, ctx| {
                history.start_new_conversation(terminal_view_id, false, false, false, ctx)
            })
        });
        AIExecutionProfilesModel::handle(&app).update(&mut app, |profiles, ctx| {
            let profile_id = *profiles.default_profile(ctx).id();
            profiles.set_read_files(profile_id, &ActionPermission::AlwaysAllow, ctx);
        });
        let controller = terminal.read(&app, |view, _| view.ai_controller().clone());
        let action_model = controller.read(&app, |controller, _| controller.action_model().clone());
        let adapter = Arc::new(BlocklistRuntimeToolActionAdapter::new_for_test(
            action_model,
            conversation_id,
            &mut app,
        ));

        let database_dir = TempDir::new().unwrap();
        let database_path = database_dir.path().join("warp.sqlite");
        let observer_dir = TempDir::new().unwrap();
        let tasks = vec![task_with_user_message()];
        let conversation_key = conversation_id.to_string();
        let mut conn = setup_database(&database_path).unwrap();
        upsert_agent_conversation(&mut conn, &conversation_key, &tasks, runtime_data(0)).unwrap();
        let writer = start_writer(conn, database_path.clone()).unwrap();
        let catalog = ToolCatalog::initial(None).unwrap();
        let authority = Arc::new(ToolExecutionAuthority::new(
            catalog.clone(),
            adapter,
            writer.sender.clone(),
        ));
        let supervisor = AgentRuntimeSupervisor::new(
            test_launch_config("text-run-read-tool", &observer_dir),
            Arc::new(warpui_core::r#async::executor::Background::default()),
        );
        let handle = supervisor.attach(conversation_key.clone()).await.unwrap();

        let result = handle
            .run_text(
                &writer.sender,
                tool_run_request(conversation_id, tasks, catalog, authority),
                |_| {},
            )
            .with_timeout(Duration::from_secs(5))
            .await
            .expect("real Blocklist tool execution should complete")
            .unwrap();

        assert_eq!(result.outcome(), &TextRunOutcome::Completed);
        let bridge_results =
            fs::read_to_string(observer_dir.path().join("tool-results.jsonl")).unwrap();
        assert!(bridge_results.contains(r#""status":"success""#));
        assert!(bridge_results.contains("runtime bridge file content"));

        supervisor.shutdown_all().await;
        writer.sender.send(ModelEvent::Terminate).unwrap();
        writer.handle.join().unwrap();
        let mut conn = setup_database(&database_path).unwrap();
        let record = agent_tool_execution_records::table
            .select(AgentToolExecutionRecord::as_select())
            .first::<AgentToolExecutionRecord>(&mut conn)
            .unwrap();
        assert_eq!(record.state(), Some(AgentToolExecutionState::Completed));
    });
}

fn tool_run_request(
    conversation_id: crate::ai::agent::conversation::AIConversationId,
    tasks: Vec<api::Task>,
    catalog: ToolCatalog,
    authority: Arc<ToolExecutionAuthority>,
) -> TextRunRequest {
    let conversation =
        AIConversation::new_restored(conversation_id, tasks.clone(), Some(runtime_data(0)))
            .unwrap();
    let transcript =
        RuntimeTranscript::project(&conversation, 0, &HashSet::new(), &HashMap::new()).unwrap();
    let provider =
        ChatCompletionsProvider::new("https://provider.example/v1", "local-model", "secret-key")
            .unwrap();
    let configuration = RunConfiguration::with_tools(
        provider,
        "/workspace",
        32_768,
        ReasoningEffort::Medium,
        &catalog,
        Vec::new(),
    )
    .unwrap();
    TextRunRequest::new(
        "run-1",
        None::<String>,
        transcript,
        configuration,
        tasks,
        runtime_data(0),
        "root-task",
    )
    .with_tool_execution_authority(authority)
}

#[test]
fn read_files_effect_projection_contains_content_and_preserves_complete_outcome() {
    let file_content = "x".repeat(64 * 1024 + 128);
    let outcome = effect_outcome(AIAgentActionResult {
        id: AIAgentActionId::from("runtime-read".to_string()),
        task_id: TaskId::new("task".to_string()),
        result: AIAgentActionResultType::ReadFiles(ReadFilesResult::Success {
            files: vec![FileContext::new(
                "/tmp/runtime-read.txt".to_string(),
                AnyFileContent::StringContent(file_content),
                None,
                None,
            )],
        }),
    });

    let complete = api::message::ToolCallResult::decode(outcome.complete_outcome.as_slice())
        .expect("complete typed outcome should decode");
    assert_eq!(complete.tool_call_id, "runtime-read");
    assert!(complete.result.is_some());
    match outcome.projection {
        ToolResultProjection::Success { content, truncated } => {
            assert!(truncated);
            assert_eq!(content.len(), 1);
            let RuntimeContentBlock::Text { text } = &content[0] else {
                panic!("read-files projection should be text");
            };
            assert!(text.contains("/tmp/runtime-read.txt"));
            assert!(text.contains("xxxx"));
            assert_eq!(text.len(), 64 * 1024);
        }
        projection => panic!("expected successful projection, got {projection:?}"),
    }
}
