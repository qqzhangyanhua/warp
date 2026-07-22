use std::collections::HashMap;
use std::sync::Mutex;

use super::{
    legacy_roots, migrate_legacy_home, MigrationOutcome, MigrationRequest, MigrationSecretError,
    TestSecretStore,
};

#[test]
fn equal_destination_secret_is_verified_without_overwrite() {
    let tempdir = tempfile::tempdir().unwrap();
    let legacy = legacy_roots(&tempdir);
    let destination = tempdir.path().join("home").join(".zyh");
    let secrets = TestSecretStore {
        legacy: HashMap::from([("AiApiKeys".to_owned(), "same-secret".to_owned())]),
        destination: Mutex::new(HashMap::from([(
            "AiApiKeys".to_owned(),
            "same-secret".to_owned(),
        )])),
        ..TestSecretStore::default()
    };

    let outcome =
        migrate_legacy_home(MigrationRequest::new(destination, legacy, &secrets)).unwrap();

    assert!(matches!(outcome, MigrationOutcome::Migrated { .. }));
    assert_eq!(secrets.destination.lock().unwrap().len(), 1);
}

#[test]
fn conflicting_destination_secret_aborts_publication() {
    let tempdir = tempfile::tempdir().unwrap();
    let legacy = legacy_roots(&tempdir);
    let destination = tempdir.path().join("home").join(".zyh");
    let secrets = TestSecretStore {
        legacy: HashMap::from([("AiApiKeys".to_owned(), "legacy-secret".to_owned())]),
        destination: Mutex::new(HashMap::from([(
            "AiApiKeys".to_owned(),
            "destination-secret".to_owned(),
        )])),
        ..TestSecretStore::default()
    };

    let error = migrate_legacy_home(MigrationRequest::new(destination.clone(), legacy, &secrets))
        .unwrap_err();

    assert!(matches!(
        error,
        super::super::MigrationError::SecretConflict { key: "AiApiKeys" }
    ));
    assert!(!destination.exists());
    assert_eq!(
        secrets
            .destination
            .lock()
            .unwrap()
            .get("AiApiKeys")
            .map(String::as_str),
        Some("destination-secret")
    );
}

#[test]
fn failed_secret_readback_aborts_publication() {
    let tempdir = tempfile::tempdir().unwrap();
    let legacy = legacy_roots(&tempdir);
    let destination = tempdir.path().join("home").join(".zyh");
    let secrets = TestSecretStore {
        legacy: HashMap::from([("AiApiKeys".to_owned(), "provider-secret".to_owned())]),
        hide_destination_reads: true,
        ..TestSecretStore::default()
    };

    let error = migrate_legacy_home(MigrationRequest::new(destination.clone(), legacy, &secrets))
        .unwrap_err();

    assert!(matches!(
        error,
        super::super::MigrationError::Secret {
            key: "AiApiKeys",
            source: MigrationSecretError::Unavailable,
        }
    ));
    assert!(!destination.exists());
}
