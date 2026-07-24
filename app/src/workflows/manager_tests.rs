use warpui::App;

use super::WorkflowManager;

#[test]
#[serial_test::serial]
fn local_only_manager_initializes_without_update_manager() {
    App::test((), |app| async move {
        app.add_singleton_model(WorkflowManager::new);
    });
}
