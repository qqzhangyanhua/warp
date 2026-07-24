use remote_server::codebase_index_proto::{RemoteCodebaseIndexState, RemoteCodebaseIndexStatus};
use warpui::platform::WindowStyle;
use warpui::App;

use super::{remote_codebase_index_limit_reached, CodeSettingsPageView, SettingsPageMeta};
use crate::appearance::Appearance;

fn empty_code_settings_page() -> CodeSettingsPageView {
    CodeSettingsPageView {
        page: super::PageType::new_uncategorized(Vec::new(), None),
        active_subpage: None,
        codebase_manual_resync_mouse_states: Vec::new(),
        codebase_delete_mouse_states: Vec::new(),
        remote_codebase_manual_resync_mouse_states: Vec::new(),
        remote_codebase_delete_mouse_states: Vec::new(),
        lsp_row_mouse_states: Vec::new(),
        open_project_rules_mouse_states: Vec::new(),
        suggested_server_statuses: Default::default(),
        #[cfg(feature = "local_fs")]
        external_editor_view: None,
    }
}

fn remote_status_with_failure(failure_message: Option<&str>) -> RemoteCodebaseIndexStatus {
    RemoteCodebaseIndexStatus {
        repo_path: "/workspaces/repo".to_string(),
        state: RemoteCodebaseIndexState::Unavailable,
        last_updated_epoch_millis: Some(1),
        progress_completed: None,
        progress_total: None,
        failure_message: failure_message.map(ToOwned::to_owned),
        root_hash: None,
    }
}

#[test]
fn remote_index_limit_failure_is_detected_from_status_message() {
    let status = remote_status_with_failure(Some(
        "Cannot index remote codebase because the maximum number of codebase indexes has been reached.",
    ));

    assert!(remote_codebase_index_limit_reached(&status));
}

#[test]
fn other_unavailable_failures_are_not_index_limit_failures() {
    let status = remote_status_with_failure(Some(
        "Cannot index remote codebase because indexing did not start.",
    ));

    assert!(!remote_codebase_index_limit_reached(&status));
}

#[test]
#[serial_test::serial]
fn selecting_code_settings_in_local_only_mode_does_not_require_team_update_manager() {
    App::test((), |mut app| async move {
        app.add_singleton_model(|_| Appearance::mock());
        let (_, code_settings) =
            app.add_window(WindowStyle::NotStealFocus, |_| empty_code_settings_page());

        app.update(|ctx| {
            code_settings.update(ctx, |view, ctx| view.on_page_selected(false, ctx));
        });
    });
}
