use std::fs;
use std::path::{Path, PathBuf};

use warpui_extras::owner_only_file::{content_hash, ExpectedContent};

use super::{
    default_save_filename, first_save, open_notebook, relative_base_directory,
    resolve_relative_path, save_notebook, save_or_create, validate_markdown_path,
    LocalMarkdownError, UnsavedMarkdownNotebook,
};

#[test]
fn new_notebook_remains_unsaved_until_path_chosen() {
    let unsaved = UnsavedMarkdownNotebook::new("# Hello\n").with_suggested_title("Notes");
    assert_eq!(unsaved.content, "# Hello\n");
    assert_eq!(unsaved.suggested_filename_stem(), "Notes");
    assert_eq!(default_save_filename(&unsaved), "Notes.md");
    // No path until first_save succeeds.
    assert!(unsaved.suggested_title.is_some());
}

#[test]
fn cancel_first_save_keeps_unsaved_content() {
    let unsaved = UnsavedMarkdownNotebook::new("draft body")
        .with_suggested_title("Draft")
        .cancel_save();
    assert_eq!(unsaved.content, "draft body");
    assert_eq!(unsaved.suggested_title.as_deref(), Some("Draft"));
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
    let new_hash = save_notebook(
        &path,
        updated,
        ExpectedContent::Hash(reopened.content_hash),
    )
    .unwrap();

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

    let error = save_notebook(
        &path,
        "stale local\n",
        ExpectedContent::Hash(entry.content_hash),
    )
    .unwrap_err();

    assert!(matches!(error, LocalMarkdownError::Conflict { .. }));
    assert_eq!(content_hash(&path).unwrap().unwrap(), external_hash);
    assert_eq!(fs::read_to_string(&path).unwrap(), "external\n");
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
fn relative_content_resolves_from_notebook_location() {
    let notebook = PathBuf::from("/docs/project/readme.md");
    assert_eq!(
        relative_base_directory(&notebook),
        Some(Path::new("/docs/project"))
    );
    assert_eq!(
        resolve_relative_path(&notebook, "images/diagram.png"),
        PathBuf::from("/docs/project/images/diagram.png")
    );
    assert_eq!(
        resolve_relative_path(&notebook, "../shared/logo.png"),
        PathBuf::from("/docs/project/../shared/logo.png")
    );
    // Absolute / URL / data URI pass through.
    assert_eq!(
        resolve_relative_path(&notebook, "/abs/path.png"),
        PathBuf::from("/abs/path.png")
    );
    assert_eq!(
        resolve_relative_path(&notebook, "https://example.com/a.png"),
        PathBuf::from("https://example.com/a.png")
    );
    assert_eq!(
        resolve_relative_path(&notebook, "data:image/png;base64,abc"),
        PathBuf::from("data:image/png;base64,abc")
    );
}

#[test]
fn restart_reload_restores_last_saved_content() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("persist.md");
    first_save(&path, "# before restart\n").unwrap();

    // Simulate app restart by opening from path only.
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
