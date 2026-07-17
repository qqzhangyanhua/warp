use warp_core::features::FeatureFlag;
use warpui::App;

use super::WorkflowManager;

#[test]
#[serial_test::serial]
fn local_only_manager_initializes_without_update_manager() {
    let _flag = FeatureFlag::LocalOnlyCustomProviderMode.override_enabled(true);

    App::test((), |app| async move {
        app.add_singleton_model(WorkflowManager::new);
    });
}
