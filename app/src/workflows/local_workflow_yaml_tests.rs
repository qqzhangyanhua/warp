use std::fs;

use warpui_extras::owner_only_file::{content_hash, ExpectedContent};

use super::{
    create_workflow, delete_workflow, list_workflows, load_workflow, path_for_workflow_name,
    rename_workflow, save_workflow, serialize_workflow_yaml, LocalWorkflowScope,
    LocalWorkflowYamlError,
};
use crate::workflows::workflow::Workflow;

fn sample_workflow(name: &str, command: &str) -> Workflow {
    Workflow::new(name, command)
}

#[test]
fn user_and_project_directories_resolve_under_zyh_paths() {
    let user = LocalWorkflowScope::User {
        home_data_dir: "/tmp/zyh-home".into(),
    };
    assert_eq!(
        user.directory(),
        std::path::PathBuf::from("/tmp/zyh-home/workflows")
    );

    let project = LocalWorkflowScope::project("/repos/app");
    assert_eq!(
        project.directory(),
        std::path::PathBuf::from("/repos/app/.zyh/workflows")
    );
}

#[test]
fn list_empty_directory_returns_empty() {
    let temp = tempfile::tempdir().unwrap();
    assert!(list_workflows(temp.path()).unwrap().is_empty());
}

#[test]
fn create_writes_user_workflow_yaml_atomically() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("workflows");
    let workflow = sample_workflow("Deploy Staging", "echo deploy");

    let entry = create_workflow(&directory, &workflow).unwrap();

    assert_eq!(entry.path, directory.join("deploy_staging.yaml"));
    assert_eq!(entry.workflow.name(), "Deploy Staging");
    assert_eq!(entry.workflow.command(), Some("echo deploy"));
    assert!(entry.path.exists());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        assert_eq!(
            fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&entry.path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn create_project_workflow_under_zyh_project_directory() {
    let temp = tempfile::tempdir().unwrap();
    let scope = LocalWorkflowScope::project(temp.path());
    let directory = scope.directory();
    let workflow = sample_workflow("Project Task", "cargo test");

    let entry = create_workflow(&directory, &workflow).unwrap();
    assert!(entry
        .path
        .starts_with(temp.path().join(".zyh").join("workflows")));
    assert_eq!(load_workflow(&entry.path).unwrap().workflow.name(), "Project Task");
}

#[test]
fn create_reports_filename_collision_without_overwriting() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path();
    let first = sample_workflow("Same Name", "echo first");
    let second = sample_workflow("Same Name", "echo second");

    create_workflow(directory, &first).unwrap();
    let error = create_workflow(directory, &second).unwrap_err();

    assert!(matches!(
        error,
        LocalWorkflowYamlError::FilenameCollision { .. }
    ));
    assert_eq!(
        load_workflow(&path_for_workflow_name(directory, "Same Name").unwrap())
            .unwrap()
            .workflow
            .command(),
        Some("echo first")
    );
}

#[test]
fn save_replaces_content_when_hash_matches() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path();
    let entry = create_workflow(directory, &sample_workflow("Edit Me", "echo a")).unwrap();

    let updated = sample_workflow("Edit Me", "echo b");
    let new_hash = save_workflow(
        &entry.path,
        &updated,
        ExpectedContent::Hash(entry.content_hash),
    )
    .unwrap();

    let loaded = load_workflow(&entry.path).unwrap();
    assert_eq!(loaded.workflow.command(), Some("echo b"));
    assert_eq!(loaded.content_hash, new_hash);
}

#[test]
fn stale_save_reports_conflict_without_overwriting() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path();
    let entry = create_workflow(directory, &sample_workflow("Conflict", "echo a")).unwrap();

    // External edit.
    fs::write(&entry.path, "name: Conflict\ncommand: echo external\n").unwrap();
    let external_hash = content_hash(&entry.path).unwrap().unwrap();

    let error = save_workflow(
        &entry.path,
        &sample_workflow("Conflict", "echo stale"),
        ExpectedContent::Hash(entry.content_hash),
    )
    .unwrap_err();

    assert!(matches!(error, LocalWorkflowYamlError::Conflict { .. }));
    assert_eq!(
        content_hash(&entry.path).unwrap().unwrap(),
        external_hash
    );
    assert!(fs::read_to_string(&entry.path)
        .unwrap()
        .contains("echo external"));
}

#[test]
fn rename_moves_file_and_updates_name() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path();
    let entry = create_workflow(directory, &sample_workflow("Old Name", "echo hi")).unwrap();

    let renamed = rename_workflow(
        &entry.path,
        "New Name",
        ExpectedContent::Hash(entry.content_hash),
    )
    .unwrap();

    assert_eq!(renamed.path, directory.join("new_name.yaml"));
    assert!(!entry.path.exists());
    assert_eq!(renamed.workflow.name(), "New Name");
    assert_eq!(load_workflow(&renamed.path).unwrap().workflow.name(), "New Name");
}

#[test]
fn rename_collision_does_not_overwrite_or_remove_source() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path();
    let a = create_workflow(directory, &sample_workflow("Alpha", "echo a")).unwrap();
    let b = create_workflow(directory, &sample_workflow("Beta", "echo b")).unwrap();

    let error = rename_workflow(&a.path, "Beta", ExpectedContent::Hash(a.content_hash)).unwrap_err();

    assert!(matches!(
        error,
        LocalWorkflowYamlError::FilenameCollision { .. }
    ));
    assert!(a.path.exists());
    assert_eq!(load_workflow(&b.path).unwrap().workflow.command(), Some("echo b"));
}

#[test]
fn delete_removes_file_when_hash_matches() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path();
    let entry = create_workflow(directory, &sample_workflow("Delete Me", "echo x")).unwrap();

    delete_workflow(&entry.path, ExpectedContent::Hash(entry.content_hash)).unwrap();
    assert!(!entry.path.exists());
    assert!(list_workflows(directory).unwrap().is_empty());
}

#[test]
fn invalid_yaml_is_reported_and_skipped_from_list() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path();
    let good = create_workflow(directory, &sample_workflow("Good", "echo good")).unwrap();
    let bad_path = directory.join("bad.yaml");
    fs::write(&bad_path, ":::: not yaml").unwrap();

    let listed = list_workflows(directory).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].path, good.path);

    let error = load_workflow(&bad_path).unwrap_err();
    assert!(matches!(error, LocalWorkflowYamlError::InvalidYaml { .. }));
}

#[test]
fn round_trip_serialization_preserves_command_workflow() {
    let workflow = sample_workflow("Round Trip", "echo {{name}}")
        .with_description("desc".into())
        .with_arguments(vec![crate::workflows::workflow::Argument {
            name: "name".into(),
            arg_type: crate::workflows::workflow::ArgumentType::Text,
            description: Some("who".into()),
            default_value: Some("world".into()),
        }]);
    let yaml = serialize_workflow_yaml(&workflow).unwrap();
    let parsed: Workflow = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(parsed, workflow);
}

#[test]
fn external_edit_is_visible_after_reload() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path();
    let entry = create_workflow(directory, &sample_workflow("External", "echo a")).unwrap();

    fs::write(&entry.path, "name: External\ncommand: echo reloaded\n").unwrap();
    let reloaded = load_workflow(&entry.path).unwrap();
    assert_eq!(reloaded.workflow.command(), Some("echo reloaded"));
    assert_ne!(reloaded.content_hash, entry.content_hash);
}

#[test]
fn restart_reload_lists_user_and_project_workflows() {
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    let user_dir = LocalWorkflowScope::User {
        home_data_dir: home.path().into(),
    }
    .directory();
    let project_dir = LocalWorkflowScope::project(repo.path()).directory();

    create_workflow(&user_dir, &sample_workflow("User WF", "echo user")).unwrap();
    create_workflow(&project_dir, &sample_workflow("Project WF", "echo project")).unwrap();

    // Simulate restart: new list from directories only.
    let user_list = list_workflows(&user_dir).unwrap();
    let project_list = list_workflows(&project_dir).unwrap();
    assert_eq!(user_list.len(), 1);
    assert_eq!(user_list[0].workflow.name(), "User WF");
    assert_eq!(project_list.len(), 1);
    assert_eq!(project_list[0].workflow.name(), "Project WF");
}

#[test]
fn execution_payload_matches_from_path_and_listed_entry() {
    // Search and editor both resolve to the same Workflow body for a path.
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path();
    let created = create_workflow(
        directory,
        &sample_workflow("Shared Run", "echo {{target}}"),
    )
    .unwrap();

    let from_list = list_workflows(directory)
        .unwrap()
        .into_iter()
        .find(|entry| entry.path == created.path)
        .expect("listed entry");
    let from_path = load_workflow(&created.path).unwrap();

    assert_eq!(from_list.workflow, from_path.workflow);
    assert_eq!(from_list.workflow.command(), Some("echo {{target}}"));
    assert_eq!(from_list.content_hash, from_path.content_hash);
}
