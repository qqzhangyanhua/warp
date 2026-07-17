use warp_core::features::FeatureFlag;
use warpui::App;

use super::ImportQueue;

#[test]
#[serial_test::serial]
fn local_only_queue_initializes_without_update_manager() {
    let _flag = FeatureFlag::LocalOnlyCustomProviderMode.override_enabled(true);

    App::test((), |mut app| async move {
        app.add_model(ImportQueue::new);
    });
}
