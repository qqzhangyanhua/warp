use std::fs;
use std::path::Path;

use warpui_extras::owner_only_file::{content_hash, ExpectedContent};

use super::{
    default_save_filename, first_save, open_notebook, save_bound, save_notebook, save_or_create,
    validate_markdown_path, LocalMarkdownError,
};

#[test]
fn default_save_filename_uses_title_hint() {
    assert_eq!(default_save_filename("Notes"), "Notes.md");
    assert_eq!(default_save_filename("  "), "Untitled.md");
    assert_eq!(default_save_filename(""), "Untitled.md");
}

#[test]
fn first_save_writes_markdown_atomically() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("notes.md");
    let content = "# First save\n\nBody\n";

    let notebook = first_save(&path, content).unwrap();

    assert_eq!(notebook.path, path);
    assert_eq!(notebook.content, content);
    assert_eq!(fs::read_to_string(&path).unwrap(), content);
    assert_eq!(
        content_hash(&path).unwrap().unwrap(),
        notebook.content_hash
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn first_save_reports_collision_without_overwriting() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("exists.md");
    fs::write(&path, "original\n").unwrap();

    let error = first_save(&path, "replacement\n").unwrap_err();
    assert!(matches!(error, LocalMarkdownError::PathCollision { .. }));
    assert_eq!(fs::read_to_string(&path).unwrap(), "original\n");
}

#[test]
fn reopen_edit_and_save_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("roundtrip.md");
    let created = first_save(&path, "# v1\n").unwrap();

    let reopened = open_notebook(&path).unwrap();
    assert_eq!(reopened.content, "# v1\n");
    assert_eq!(reopened.content_hash, created.content_hash);

    let updated = "# v2\n\nedited\n";
    let new_hash = save_bound(&path, updated, reopened.content_hash).unwrap();

    let again = open_notebook(&path).unwrap();
    assert_eq!(again.content, updated);
    assert_eq!(again.content_hash, new_hash);
}

#[test]
fn external_edit_conflict_rejects_stale_save() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("conflict.md");
    let entry = first_save(&path, "local\n").unwrap();

    fs::write(&path, "external\n").unwrap();
    let external_hash = content_hash(&path).unwrap().unwrap();

    let error = save_bound(&path, "stale local\n", entry.content_hash).unwrap_err();

    assert!(matches!(error, LocalMarkdownError::Conflict { .. }));
    assert_eq!(content_hash(&path).unwrap().unwrap(), external_hash);
    assert_eq!(fs::read_to_string(&path).unwrap(), "external\n");
}

#[test]
fn save_or_create_none_never_overwrites_existing() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("exists.md");
    fs::write(&path, "keep\n").unwrap();

    let error = save_or_create(&path, "clobber\n", None).unwrap_err();
    assert!(matches!(error, LocalMarkdownError::PathCollision { .. }));
    assert_eq!(fs::read_to_string(&path).unwrap(), "keep\n");
}

#[test]
fn invalid_path_rejected() {
    assert!(matches!(
        validate_markdown_path(Path::new("")),
        Err(LocalMarkdownError::InvalidPath)
    ));
    assert!(matches!(
        validate_markdown_path(Path::new("notes.txt")),
        Err(LocalMarkdownError::InvalidExtension { .. })
    ));
    assert!(matches!(
        first_save(Path::new("notes.txt"), "x"),
        Err(LocalMarkdownError::InvalidExtension { .. })
    ));
    assert!(matches!(
        open_notebook(Path::new("notes.txt")),
        Err(LocalMarkdownError::InvalidExtension { .. })
    ));
}

#[test]
fn restart_reload_restores_last_saved_content() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("persist.md");
    first_save(&path, "# before restart\n").unwrap();

    let restored = open_notebook(&path).unwrap();
    assert_eq!(restored.content, "# before restart\n");
    assert_eq!(restored.path, path);
}

#[test]
fn save_or_create_first_then_update() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("flow.md");

    let created = save_or_create(&path, "one\n", None).unwrap();
    assert_eq!(created.content, "one\n");

    let updated = save_or_create(&path, "two\n", Some(created.content_hash)).unwrap();
    assert_eq!(updated.content, "two\n");
    assert_eq!(fs::read_to_string(&path).unwrap(), "two\n");
}

#[test]
fn open_missing_file_is_io_error() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("missing.md");
    let error = open_notebook(&path).unwrap_err();
    assert!(matches!(error, LocalMarkdownError::Io(_)));
}

#[test]
fn save_notebook_any_is_available_but_bound_api_requires_hash() {
    // Document the contract: IO layer can take Any, but save_bound always hashes.
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("any.md");
    first_save(&path, "a\n").unwrap();
    let hash = save_notebook(&path, "b\n", ExpectedContent::Any).unwrap();
    assert_eq!(save_bound(&path, "c\n", hash).unwrap(), content_hash(&path).unwrap().unwrap());
    assert_eq!(fs::read_to_string(&path).unwrap(), "c\n");
}
