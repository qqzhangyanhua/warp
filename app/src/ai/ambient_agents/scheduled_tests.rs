use warpui::App;

use super::ScheduledAgentManager;

#[test]
#[serial_test::serial]
fn local_only_manager_initializes_without_update_manager() {
    App::test((), |app| async move {
        app.add_singleton_model(ScheduledAgentManager::new);
    });
}
