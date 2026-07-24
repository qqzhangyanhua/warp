use std::fs;

use warpui_extras::owner_only_file::{content_hash, ExpectedContent};

use super::{
    GlobalAgentRulesDocument, GlobalAgentRulesError, GlobalAgentRulesState, AGENTS_FILE_NAME,
    AGENTS_SUBDIR,
};

fn document_in(temp: &tempfile::TempDir) -> GlobalAgentRulesDocument {
    GlobalAgentRulesDocument::with_path(GlobalAgentRulesDocument::standard_path_for_home(
        temp.path(),
    ))
}

#[test]
fn standard_path_is_agents_md_under_home() {
    let path = GlobalAgentRulesDocument::standard_path_for_home("/Users/example");
    assert_eq!(
        path,
        std::path::PathBuf::from(format!("/Users/example/{AGENTS_SUBDIR}/{AGENTS_FILE_NAME}"))
    );
}

#[test]
fn missing_file_loads_as_missing() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);

    assert_eq!(document.load().unwrap(), GlobalAgentRulesState::Missing);
}

#[test]
fn create_writes_owner_only_file_atomically() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);

    let hash = document.create("prefer 4-space indentation\n").unwrap();
    let state = document.load().unwrap();

    match state {
        GlobalAgentRulesState::Present {
            content,
            content_hash,
        } => {
            assert_eq!(content, "prefer 4-space indentation\n");
            assert_eq!(content_hash, hash);
        }
        GlobalAgentRulesState::Missing => panic!("file should exist after create"),
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        let parent = document.path().parent().unwrap();
        assert_eq!(
            fs::metadata(parent).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(document.path()).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn create_conflicts_when_file_already_exists() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    document.create("first").unwrap();

    let error = document.create("second").unwrap_err();
    assert!(matches!(error, GlobalAgentRulesError::Conflict { .. }));
    assert_eq!(fs::read_to_string(document.path()).unwrap(), "first");
}

#[test]
fn edit_save_replaces_content_when_hash_matches() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    let first_hash = document.create("first draft").unwrap();

    let second_hash = document
        .save("second draft", ExpectedContent::Hash(first_hash))
        .unwrap();
    assert_eq!(
        document.load().unwrap(),
        GlobalAgentRulesState::Present {
            content: "second draft".into(),
            content_hash: second_hash,
        }
    );
    assert_eq!(fs::read_to_string(document.path()).unwrap(), "second draft");
}

#[test]
fn stale_editor_save_reports_conflict_without_overwriting() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    let original_hash = document.create("editor view").unwrap();

    // External change (another process / editor).
    fs::write(document.path(), "external change").unwrap();

    let error = document
        .save("stale save", ExpectedContent::Hash(original_hash))
        .unwrap_err();
    assert!(matches!(error, GlobalAgentRulesError::Conflict { .. }));
    assert_eq!(
        fs::read_to_string(document.path()).unwrap(),
        "external change"
    );
}

#[test]
fn delete_removes_file_when_hash_matches() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    let hash = document.create("to delete").unwrap();

    document.delete(ExpectedContent::Hash(hash)).unwrap();
    assert_eq!(document.load().unwrap(), GlobalAgentRulesState::Missing);
    assert!(!document.path().exists());
}

#[test]
fn delete_confirmation_rejects_stale_hash() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    let hash = document.create("original").unwrap();
    fs::write(document.path(), "externally rewritten").unwrap();

    let error = document.delete(ExpectedContent::Hash(hash)).unwrap_err();
    assert!(matches!(error, GlobalAgentRulesError::Conflict { .. }));
    assert_eq!(
        fs::read_to_string(document.path()).unwrap(),
        "externally rewritten"
    );
}

#[cfg(unix)]
#[test]
fn malformed_permissions_surface_as_unsupported_or_io() {
    use std::os::unix::fs::{symlink, PermissionsExt as _};

    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    let parent = document.path().parent().unwrap();
    fs::create_dir_all(parent).unwrap();

    // Symlink destination is unsupported (not a regular file).
    let target = temp.path().join("target.md");
    fs::write(&target, "target").unwrap();
    symlink(&target, document.path()).unwrap();

    let error = document.load().unwrap_err();
    assert!(matches!(
        error,
        GlobalAgentRulesError::UnsupportedFileType { .. }
    ));

    // Replace symlink with a regular file that is unreadable.
    fs::remove_file(document.path()).unwrap();
    document.create("secret").unwrap();
    fs::set_permissions(document.path(), fs::Permissions::from_mode(0o000)).unwrap();

    let error = document.load().unwrap_err();
    assert!(matches!(error, GlobalAgentRulesError::Io(_)));

    // Restore so the tempdir can clean up on some platforms.
    fs::set_permissions(document.path(), fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn restart_reloads_persisted_content() {
    let temp = tempfile::tempdir().unwrap();
    let path = GlobalAgentRulesDocument::standard_path_for_home(temp.path());

    let first_session = GlobalAgentRulesDocument::with_path(path.clone());
    let hash = first_session
        .create("# Global rules\n\nAlways run tests.\n")
        .unwrap();

    // Simulate restart: new document handle, same path.
    let second_session = GlobalAgentRulesDocument::with_path(path);
    assert_eq!(
        second_session.load().unwrap(),
        GlobalAgentRulesState::Present {
            content: "# Global rules\n\nAlways run tests.\n".into(),
            content_hash: hash,
        }
    );
    assert_eq!(content_hash(second_session.path()).unwrap(), Some(hash));
}

#[test]
fn expected_content_tracks_loaded_state() {
    assert!(matches!(
        GlobalAgentRulesDocument::expected_content(&GlobalAgentRulesState::Missing),
        ExpectedContent::Missing
    ));

    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    let hash = document.create("x").unwrap();
    let state = document.load().unwrap();
    assert!(matches!(
        GlobalAgentRulesDocument::expected_content(&state),
        ExpectedContent::Hash(h) if h == hash
    ));
}
