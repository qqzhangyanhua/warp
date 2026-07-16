use std::sync::Arc;
use std::time::Duration;

use futures::poll;
use warpui::{App, ReadModel, SingletonEntity};
use warpui_core::r#async::Timer;

use super::blocklist_adapter::BlocklistRuntimeToolActionAdapter;
use super::{RuntimeToolActionAdapter, ToolPermissionDecision};
use crate::ai::agent::task::TaskId;
use crate::ai::agent::{
    AIAgentAction, AIAgentActionId, AIAgentActionType, FileLocations, ReadFilesRequest,
};
use crate::ai::blocklist::BlocklistAIHistoryModel;
use crate::ai::execution_profiles::ActionPermission;
use crate::ai::execution_profiles::profiles::AIExecutionProfilesModel;
use crate::test_util::terminal::{add_window_with_terminal, initialize_app_for_terminal_view};

#[test]
fn cancelling_runtime_run_removes_its_pending_confirmation() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);
        let (conversation_id, terminal_surface_id) = terminal.update(&mut app, |_view, ctx| {
            let terminal_surface_id = ctx.view_id();
            let conversation_id =
                BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, ctx| {
                    history.start_new_conversation(terminal_surface_id, false, false, false, ctx)
                });
            (conversation_id, terminal_surface_id)
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
        let action_id = AIAgentActionId::from("runtime-cancel-read".to_string());
        let action = read_action(action_id.clone());
        let mut permission = adapter.request_permission("run-1".to_string(), action.clone());

        assert!(poll!(&mut permission).is_pending());
        wait_for_pending_action(&app, &action_model, &action_id).await;

        let second_permission = adapter
            .request_permission("run-2".to_string(), action)
            .await;
        assert_eq!(second_permission, ToolPermissionDecision::DeniedByPolicy);

        adapter.cancel_run("run-1".to_string()).await;

        assert_eq!(permission.await, ToolPermissionDecision::DeniedByUser);
        app.read_model(&action_model, |action_model, _| {
            assert!(action_model.get_pending_action_by_id(&action_id).is_none());
        });

        let history = BlocklistAIHistoryModel::handle(&app);
        history.update(&mut app, |history, ctx| {
            history.register_runtime_action(conversation_id, action_id.clone());
            assert_eq!(
                history.conversation_id_for_action(&action_id, terminal_surface_id),
                Some(conversation_id)
            );
            history.remove_conversation(conversation_id, terminal_surface_id, ctx);
            assert_eq!(
                history.conversation_id_for_action(&action_id, terminal_surface_id),
                None
            );
        });
    });
}

fn read_action(action_id: AIAgentActionId) -> AIAgentAction {
    AIAgentAction {
        id: action_id,
        task_id: TaskId::new("task".to_string()),
        action: AIAgentActionType::ReadFiles(ReadFilesRequest {
            locations: vec![FileLocations {
                name: "/tmp/runtime-cancel-read.txt".to_string(),
                lines: Vec::new(),
            }],
        }),
        requires_result: true,
    }
}

async fn wait_for_pending_action(
    app: &App,
    action_model: &warpui::ModelHandle<crate::ai::blocklist::BlocklistAIActionModel>,
    action_id: &AIAgentActionId,
) {
    for _ in 0..50 {
        if app.read_model(action_model, |action_model, _| {
            action_model.get_pending_action_by_id(action_id).is_some()
        }) {
            return;
        }
        Timer::after(Duration::from_millis(10)).await;
    }
    panic!("runtime permission was not queued");
}
