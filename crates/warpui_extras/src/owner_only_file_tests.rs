use std::fs;

use super::{atomic_replace, content_hash, ExpectedContent, OwnerOnlyFileError};

#[test]
fn creates_and_replaces_with_one_last_known_good_backup() {
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("private").join("settings.toml");

    let first = atomic_replace(&path, b"first", ExpectedContent::Missing).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"first");
    assert!(!first.backup_path.exists());

    let second =
        atomic_replace(&path, b"second", ExpectedContent::Hash(first.content_hash)).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"second");
    assert_eq!(fs::read(&second.backup_path).unwrap(), b"first");

    atomic_replace(&path, b"third", ExpectedContent::Hash(second.content_hash)).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"third");
    assert_eq!(fs::read(&second.backup_path).unwrap(), b"second");
}

#[test]
fn rejects_stale_or_unexpected_destination_without_changing_it() {
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("settings.toml");
    fs::write(&path, b"external edit").unwrap();

    let stale = content_hash(&tempdir.path().join("missing")).unwrap();
    assert!(stale.is_none());

    let error = atomic_replace(&path, b"replacement", ExpectedContent::Missing).unwrap_err();
    assert!(matches!(error, OwnerOnlyFileError::Conflict { .. }));
    assert_eq!(fs::read(&path).unwrap(), b"external edit");

    let wrong_hash = content_hash(&path).unwrap().unwrap();
    fs::write(&path, b"newer external edit").unwrap();
    let error =
        atomic_replace(&path, b"replacement", ExpectedContent::Hash(wrong_hash)).unwrap_err();
    assert!(matches!(error, OwnerOnlyFileError::Conflict { .. }));
    assert_eq!(fs::read(&path).unwrap(), b"newer external edit");
}

#[cfg(unix)]
#[test]
fn created_directories_and_files_are_owner_only() {
    use std::os::unix::fs::PermissionsExt as _;

    let tempdir = tempfile::tempdir().unwrap();
    let private_dir = tempdir.path().join("private");
    let path = private_dir.join("settings.toml");

    atomic_replace(&path, b"private", ExpectedContent::Missing).unwrap();

    assert_eq!(
        fs::metadata(private_dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[cfg(unix)]
#[test]
fn rejects_symlink_destinations() {
    use std::os::unix::fs::symlink;

    let tempdir = tempfile::tempdir().unwrap();
    let target = tempdir.path().join("target");
    let link = tempdir.path().join("link");
    fs::write(&target, b"target").unwrap();
    symlink(&target, &link).unwrap();

    let error = atomic_replace(&link, b"replacement", ExpectedContent::Any).unwrap_err();
    assert!(matches!(
        error,
        OwnerOnlyFileError::UnsupportedFileType { .. }
    ));
    assert_eq!(fs::read(target).unwrap(), b"target");
}
