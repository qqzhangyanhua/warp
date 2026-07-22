use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::sync::Mutex;

use diesel::connection::SimpleConnection as _;
use diesel::prelude::*;
use warp_core::channel::Channel;
use warp_core::paths::{LegacyIdentity, LegacyPlatform, LegacyRoots};

use super::{
    migrate_legacy_home, MigrationOutcome, MigrationRequest, MigrationSecretError,
    MigrationSecretStore,
};

#[derive(Default)]
struct TestSecretStore {
    legacy: HashMap<String, String>,
    destination: Mutex<HashMap<String, String>>,
    hide_destination_reads: bool,
}

impl MigrationSecretStore for TestSecretStore {
    fn read_legacy(&self, key: &str) -> Result<Option<String>, MigrationSecretError> {
        Ok(self.legacy.get(key).cloned())
    }

    fn read_destination(
        &self,
        key: &str,
        _staging_root: &std::path::Path,
    ) -> Result<Option<String>, MigrationSecretError> {
        if self.hide_destination_reads {
            return Ok(None);
        }
        Ok(self.destination.lock().unwrap().get(key).cloned())
    }

    fn write_destination(
        &self,
        key: &str,
        value: &str,
        _staging_root: &std::path::Path,
    ) -> Result<(), MigrationSecretError> {
        self.destination
            .lock()
            .unwrap()
            .insert(key.to_owned(), value.to_owned());
        Ok(())
    }
}

#[test]
fn fresh_migration_copies_manifest_data_without_touching_legacy_sources() {
    let tempdir = tempfile::tempdir().unwrap();
    let legacy = LegacyRoots::resolve(
        &tempdir.path().join("home"),
        LegacyPlatform::MacOs,
        LegacyIdentity::new(Channel::Stable, "dev.warp.Warp"),
    );
    let destination = tempdir.path().join("home").join(".zyh");

    write(
        &legacy.config_dir().join("settings.toml"),
        br#"
[appearance.text]
font_size = 15
[account]
is_settings_sync_enabled = true
"#,
    );
    write(&legacy.config_dir().join("keybindings.yaml"), b"bindings");
    write(&legacy.data_dir().join("themes/dark.yaml"), b"theme");
    write(
        &legacy.data_dir().join("workflows/deploy.yaml"),
        b"workflow",
    );
    write(
        &legacy.home_config_dir().join(".mcp.json"),
        br#"{"mcpServers":{}}"#,
    );
    write(
        &legacy.data_dir().join("tab_configs/dev.toml"),
        b"name = 'dev'",
    );
    write(
        &legacy.data_dir().join("launch_configurations/dev.yaml"),
        b"launch",
    );
    write(
        &legacy.home_config_dir().join("skills/review/SKILL.md"),
        b"skill",
    );
    write(
        &legacy.home_config_dir().join("plugins/local/index.js"),
        b"plugin",
    );
    write(
        &legacy.data_dir().join("ssh_hosts.json"),
        br#"{"version":1,"hosts":[]}"#,
    );
    write(&legacy.state_dir().join("logs/app.log"), b"log");
    write(
        &legacy.tui_config_dir().join("settings.toml"),
        b"[terminal]\ncopy_on_select = true\n",
    );

    let secrets = TestSecretStore {
        legacy: HashMap::from([("AiApiKeys".to_owned(), "provider-secret".to_owned())]),
        ..TestSecretStore::default()
    };
    let outcome = migrate_legacy_home(MigrationRequest::new(
        destination.clone(),
        legacy.clone(),
        &secrets,
    ))
    .unwrap();

    assert_eq!(
        outcome,
        MigrationOutcome::Migrated {
            manifest_version: 1
        }
    );
    assert_eq!(
        fs::read(destination.join("keybindings.yaml")).unwrap(),
        b"bindings"
    );
    assert_eq!(
        fs::read(destination.join("themes/dark.yaml")).unwrap(),
        b"theme"
    );
    assert_eq!(
        fs::read(destination.join("workflows/deploy.yaml")).unwrap(),
        b"workflow"
    );
    assert_eq!(
        fs::read(destination.join(".mcp.json")).unwrap(),
        br#"{"mcpServers":{}}"#
    );
    assert_eq!(
        fs::read(destination.join("tab_configs/dev.toml")).unwrap(),
        b"name = 'dev'"
    );
    assert_eq!(
        fs::read(destination.join("launch_configurations/dev.yaml")).unwrap(),
        b"launch"
    );
    assert_eq!(
        fs::read(destination.join("skills/review/SKILL.md")).unwrap(),
        b"skill"
    );
    assert_eq!(
        fs::read(destination.join("plugins/local/index.js")).unwrap(),
        b"plugin"
    );
    assert_eq!(
        fs::read(destination.join("ssh_hosts.json")).unwrap(),
        br#"{"version":1,"hosts":[]}"#
    );
    assert_eq!(fs::read(destination.join("logs/app.log")).unwrap(), b"log");
    assert!(destination.join("tui/settings.toml").is_file());

    let settings = fs::read_to_string(destination.join("settings.toml")).unwrap();
    assert!(settings.contains("font_size = 15"));
    assert!(!settings.contains("is_settings_sync_enabled"));
    assert_eq!(
        fs::read(destination.join("migration/settings.toml.legacy")).unwrap(),
        fs::read(legacy.config_dir().join("settings.toml")).unwrap()
    );

    let report = fs::read_to_string(destination.join("migration-report.json")).unwrap();
    assert!(report.contains("account.is_settings_sync_enabled"));
    assert!(!report.contains("provider-secret"));
    assert!(destination.join("migration-complete.json").is_file());
    assert_eq!(
        secrets
            .destination
            .lock()
            .unwrap()
            .get("AiApiKeys")
            .map(String::as_str),
        Some("provider-secret")
    );

    assert_eq!(
        fs::read(legacy.data_dir().join("themes/dark.yaml")).unwrap(),
        b"theme"
    );
    assert!(!fs::symlink_metadata(destination.join("themes"))
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn sqlite_migration_uses_a_consistent_backup_and_removes_cloud_rows() {
    let tempdir = tempfile::tempdir().unwrap();
    let legacy = LegacyRoots::resolve(
        &tempdir.path().join("home"),
        LegacyPlatform::MacOs,
        LegacyIdentity::new(Channel::Stable, "dev.warp.Warp"),
    );
    let source_database = legacy.state_dir().join("warp.sqlite");
    fs::create_dir_all(source_database.parent().unwrap()).unwrap();
    let mut source = crate::persistence::setup_database(&source_database).unwrap();
    source
        .batch_execute(
            r#"
            INSERT INTO ai_queries (exchange_id, conversation_id, start_ts, output_status, input)
            VALUES ('exchange', 'conversation', CURRENT_TIMESTAMP, '{}', 'retained');
            INSERT INTO users (firebase_uid) VALUES ('cloud-user');
            "#,
        )
        .unwrap();

    let destination = tempdir.path().join("home").join(".zyh");
    migrate_legacy_home(MigrationRequest::new(
        destination.clone(),
        legacy,
        &TestSecretStore::default(),
    ))
    .unwrap();

    let mut migrated =
        SqliteConnection::establish(destination.join("warp.sqlite").to_str().unwrap()).unwrap();
    assert_eq!(query_count(&mut migrated, "ai_queries"), 1);
    assert_eq!(query_count(&mut migrated, "users"), 0);
}

#[test]
fn partial_failure_leaves_no_destination_and_a_rerun_can_complete() {
    let tempdir = tempfile::tempdir().unwrap();
    let legacy = LegacyRoots::resolve(
        &tempdir.path().join("home"),
        LegacyPlatform::MacOs,
        LegacyIdentity::new(Channel::Stable, "dev.warp.Warp"),
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
        super::MigrationError::InjectedFailure {
            after: "keybindings"
        }
    ));
    assert!(!destination.exists());
    assert_eq!(
        fs::read(legacy.config_dir().join("keybindings.yaml")).unwrap(),
        b"bindings"
    );
    assert!(fs::read_dir(destination.parent().unwrap())
        .unwrap()
        .all(|entry| {
            let name = entry.unwrap().file_name();
            !name.to_string_lossy().starts_with(".zyh-migration-")
        }));

    let outcome =
        migrate_legacy_home(MigrationRequest::new(destination.clone(), legacy, &secrets)).unwrap();
    assert!(matches!(outcome, MigrationOutcome::Migrated { .. }));
    assert_eq!(
        fs::read(destination.join("keybindings.yaml")).unwrap(),
        b"bindings"
    );
}

#[test]
fn stale_lock_file_does_not_block_a_retry() {
    let tempdir = tempfile::tempdir().unwrap();
    let legacy = LegacyRoots::resolve(
        &tempdir.path().join("home"),
        LegacyPlatform::MacOs,
        LegacyIdentity::new(Channel::Stable, "dev.warp.Warp"),
    );
    let destination = tempdir.path().join("home").join(".zyh");
    let lock_path = destination.parent().unwrap().join(".zyh.migration.lock");
    write(&lock_path, b"stale lock from an interrupted launch");

    let outcome = migrate_legacy_home(MigrationRequest::new(
        destination.clone(),
        legacy,
        &TestSecretStore::default(),
    ))
    .unwrap();

    assert!(matches!(outcome, MigrationOutcome::Migrated { .. }));
    assert!(destination.join("migration-complete.json").is_file());
}

#[test]
fn existing_destination_is_a_hard_noop_even_when_empty() {
    let tempdir = tempfile::tempdir().unwrap();
    let legacy = LegacyRoots::resolve(
        &tempdir.path().join("home"),
        LegacyPlatform::MacOs,
        LegacyIdentity::new(Channel::Stable, "dev.warp.Warp"),
    );
    write(&legacy.config_dir().join("keybindings.yaml"), b"legacy");
    let destination = tempdir.path().join("home").join(".zyh");
    fs::create_dir_all(&destination).unwrap();

    let outcome = migrate_legacy_home(MigrationRequest::new(
        destination.clone(),
        legacy,
        &TestSecretStore::default(),
    ))
    .unwrap();

    assert_eq!(outcome, MigrationOutcome::ExistingDestination);
    assert!(fs::read_dir(&destination).unwrap().next().is_none());
}

#[test]
fn malformed_settings_are_backed_up_without_creating_active_settings() {
    let tempdir = tempfile::tempdir().unwrap();
    let legacy = LegacyRoots::resolve(
        &tempdir.path().join("home"),
        LegacyPlatform::MacOs,
        LegacyIdentity::new(Channel::Stable, "dev.warp.Warp"),
    );
    let malformed = b"api_key = 'secret-value'\n[broken";
    write(&legacy.config_dir().join("settings.toml"), malformed);
    let destination = tempdir.path().join("home").join(".zyh");

    migrate_legacy_home(MigrationRequest::new(
        destination.clone(),
        legacy,
        &TestSecretStore::default(),
    ))
    .unwrap();

    assert!(!destination.join("settings.toml").exists());
    assert_eq!(
        fs::read(destination.join("migration/settings.toml.legacy")).unwrap(),
        malformed
    );
    let report = fs::read_to_string(destination.join("migration-report.json")).unwrap();
    assert!(report.contains("\"status\": \"malformed\""));
    assert!(!report.contains("secret-value"));
}

#[cfg(unix)]
#[test]
fn source_symlinks_are_reported_and_never_followed() {
    use std::os::unix::fs::symlink;

    let tempdir = tempfile::tempdir().unwrap();
    let legacy = LegacyRoots::resolve(
        &tempdir.path().join("home"),
        LegacyPlatform::MacOs,
        LegacyIdentity::new(Channel::Stable, "dev.warp.Warp"),
    );
    let outside_file = tempdir.path().join("outside-keybindings.yaml");
    write(&outside_file, b"outside secret");
    fs::create_dir_all(legacy.config_dir()).unwrap();
    symlink(&outside_file, legacy.config_dir().join("keybindings.yaml")).unwrap();
    let outside_directory = tempdir.path().join("outside-theme");
    write(&outside_directory.join("secret.yaml"), b"theme secret");
    fs::create_dir_all(legacy.data_dir().join("themes")).unwrap();
    symlink(&outside_directory, legacy.data_dir().join("themes/linked")).unwrap();
    let destination = tempdir.path().join("home").join(".zyh");

    migrate_legacy_home(MigrationRequest::new(
        destination.clone(),
        legacy,
        &TestSecretStore::default(),
    ))
    .unwrap();

    assert!(!destination.join("keybindings.yaml").exists());
    assert!(!destination.join("themes/linked").exists());
    assert_eq!(fs::read(&outside_file).unwrap(), b"outside secret");
    let report = fs::read_to_string(destination.join("migration-report.json")).unwrap();
    assert!(report.contains("keybindings.yaml"));
    assert!(report.contains("themes/linked"));
    assert!(!report.contains("outside secret"));
    assert!(!report.contains("theme secret"));
}

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
        super::MigrationError::SecretConflict { key: "AiApiKeys" }
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
        super::MigrationError::Secret {
            key: "AiApiKeys",
            source: MigrationSecretError::Unavailable,
        }
    ));
    assert!(!destination.exists());
}

#[test]
fn concurrent_migration_reports_in_progress_then_allows_retry() {
    let tempdir = tempfile::tempdir().unwrap();
    let legacy = legacy_roots(&tempdir);
    let destination = tempdir.path().join("home").join(".zyh");
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    let lock_path = destination.parent().unwrap().join(".zyh.migration.lock");
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(lock_path)
        .unwrap();
    lock.try_lock().unwrap();

    let outcome = migrate_legacy_home(MigrationRequest::new(
        destination.clone(),
        legacy.clone(),
        &TestSecretStore::default(),
    ))
    .unwrap();
    assert_eq!(outcome, MigrationOutcome::InProgress);
    assert!(!destination.exists());

    drop(lock);
    let outcome = migrate_legacy_home(MigrationRequest::new(
        destination,
        legacy,
        &TestSecretStore::default(),
    ))
    .unwrap();
    assert!(matches!(outcome, MigrationOutcome::Migrated { .. }));
}

#[test]
fn missing_legacy_source_still_creates_a_complete_empty_home() {
    let tempdir = tempfile::tempdir().unwrap();
    let legacy = legacy_roots(&tempdir);
    let destination = tempdir.path().join("home").join(".zyh");

    let outcome = migrate_legacy_home(MigrationRequest::new(
        destination.clone(),
        legacy,
        &TestSecretStore::default(),
    ))
    .unwrap();

    assert!(matches!(outcome, MigrationOutcome::Migrated { .. }));
    assert!(destination.join("migration-complete.json").is_file());
    let report = fs::read_to_string(destination.join("migration-report.json")).unwrap();
    assert!(report.contains("\"status\": \"missing\""));
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    count: i64,
}

fn query_count(connection: &mut SqliteConnection, table: &str) -> i64 {
    diesel::sql_query(format!("SELECT COUNT(*) AS count FROM {table}"))
        .get_result::<Count>(connection)
        .unwrap()
        .count
}

fn write(path: &std::path::Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn legacy_roots(tempdir: &tempfile::TempDir) -> LegacyRoots {
    LegacyRoots::resolve(
        &tempdir.path().join("home"),
        LegacyPlatform::MacOs,
        LegacyIdentity::new(Channel::Stable, "dev.warp.Warp"),
    )
}
