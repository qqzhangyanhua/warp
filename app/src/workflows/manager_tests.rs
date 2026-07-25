use std::path::PathBuf;

use warpui::{App, SingletonEntity};

use super::{WorkflowManager, WorkflowOpenSource};
use crate::server::ids::{ClientId, SyncId};
use crate::workflows::workflow::Workflow;
use crate::workflows::{WorkflowSource, WorkflowType};

#[test]
#[serial_test::serial]
fn local_only_manager_initializes_without_update_manager() {
    App::test((), |app| async move {
        app.add_singleton_model(WorkflowManager::new);
    });
}

#[test]
fn workflow_type_has_no_cloud_variant_and_no_cloud_ids() {
    let wf = WorkflowType::Local(Workflow::new("demo", "echo hi"));
    assert!(wf.sync_id().is_none());
    assert!(wf.server_id().is_none());
    assert!(wf.object_id().is_none());
    assert_eq!(wf.as_workflow().name(), "demo");

    let notebook = WorkflowType::Notebook(Workflow::new("nb", "ls"));
    assert!(notebook.sync_id().is_none());
}

#[test]
#[serial_test::serial]
fn existing_cloud_workflow_open_does_not_require_cloud_model() {
    // WorkflowOpenSource::Existing must not touch CloudModel (not registered
    // in ZYH production). find_pane/create path must not require cloud singletons.
    App::test((), |mut app| async move {
        app.add_singleton_model(WorkflowManager::new);
        app.update(|ctx| {
            let source =
                WorkflowOpenSource::Existing(SyncId::ClientId(ClientId::new()));
            // Never registered — None without needing CloudModel.
            assert!(WorkflowManager::as_ref(ctx).find_pane(&source).is_none());
        });
    });
}

#[test]
#[serial_test::serial]
fn local_file_open_source_is_still_findable() {
    App::test((), |mut app| async move {
        app.add_singleton_model(WorkflowManager::new);
        app.update(|ctx| {
            let manager = WorkflowManager::as_ref(ctx);
            let source = WorkflowOpenSource::LocalFile {
                path: PathBuf::from("/tmp/demo.yaml"),
                source: WorkflowSource::Local,
            };
            // Not registered yet — find returns None without panicking.
            assert!(manager.find_pane(&source).is_none());
        });
    });
}
