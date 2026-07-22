use std::fs;
use std::path::Path;

use warp_core::channel::Channel;
use warp_core::paths::{LegacyInstallation, LegacyPlatform, LegacyRoots};

use super::{
    legacy_roots, migrate_legacy_home, write, MigrationOutcome, MigrationRequest, TestSecretStore,
};
use crate::zyh_home_migration::{MigrationError, PublicationFailure};

#[test]
fn partial_failure_leaves_no_destination_and_a_rerun_can_complete() {
    let tempdir = tempfile::tempdir().unwrap();
    let legacy = LegacyRoots::resolve(
        &tempdir.path().join("home"),
        LegacyPlatform::MacOs,
        LegacyInstallation::new(Channel::Stable, "dev.warp.Warp"),
    );
    write(&legacy.config_dir().join("keybindings.yaml"), b"bindings");
    write(&legacy.data_dir().join("themes/dark.yaml"), b"theme");
    let destination = tempdir.path().join("home").join(".zyh");
    let secrets = TestSecretStore::default();

    let error = migrate_legacy_home(
        MigrationRequest::new(destination.clone(), legacy.clone(), &secrets)
            .with_failure_after("keybindings"),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        MigrationError::InjectedFailure {
            after: "keybindings"
        }
    ));
    assert!(!destination.exists());
    assert_eq!(
        fs::read(legacy.config_dir().join("keybindings.yaml")).unwrap(),
        b"bindings"
    );
    assert_no_staging_directory(&destination);

    let outcome =
        migrate_legacy_home(MigrationRequest::new(destination.clone(), legacy, &secrets)).unwrap();
    assert!(matches!(outcome, MigrationOutcome::Migrated { .. }));
    assert_eq!(
        fs::read(destination.join("keybindings.yaml")).unwrap(),
        b"bindings"
    );
}

#[test]
fn publication_rename_failure_leaves_no_destination_or_staging_directory() {
    let tempdir = tempfile::tempdir().unwrap();
    let legacy = legacy_roots(&tempdir);
    let destination = tempdir.path().join("home").join(".zyh");

    migrate_legacy_home(
        MigrationRequest::new(destination.clone(), legacy, &TestSecretStore::default())
            .with_publication_failure(PublicationFailure::Rename),
    )
    .unwrap_err();

    assert!(!destination.exists());
    assert_no_staging_directory(&destination);
}

#[test]
fn publication_parent_sync_failure_rolls_back_destination() {
    let tempdir = tempfile::tempdir().unwrap();
    let legacy = legacy_roots(&tempdir);
    let destination = tempdir.path().join("home").join(".zyh");

    migrate_legacy_home(
        MigrationRequest::new(destination.clone(), legacy, &TestSecretStore::default())
            .with_publication_failure(PublicationFailure::ParentSync),
    )
    .unwrap_err();

    assert!(!destination.exists());
    assert_no_staging_directory(&destination);
}

fn assert_no_staging_directory(destination: &Path) {
    assert!(fs::read_dir(destination.parent().unwrap())
        .unwrap()
        .all(|entry| {
            let name = entry.unwrap().file_name();
            !name.to_string_lossy().starts_with(".zyh-migration-")
        }));
}
