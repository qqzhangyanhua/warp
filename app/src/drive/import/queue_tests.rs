use warpui::App;

use super::ImportQueue;

#[test]
#[serial_test::serial]
fn local_only_queue_initializes_without_update_manager() {
    App::test((), |mut app| async move {
        app.add_model(ImportQueue::new);
    });
}
