use std::fs::{self, File, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use serde::Serialize;
use tempfile::Builder;
use thiserror::Error;
use warp_core::paths::LegacyRoots;
use warpui_extras::owner_only_file::{
    atomic_replace, ensure_owner_only_dir, ensure_owner_only_file, ExpectedContent,
    OwnerOnlyFileError,
};

mod logs;
mod manifest;
mod platform_secure_storage;
mod report;
pub(crate) mod settings;
mod settings_rules;
mod sqlite;

use logs::copy_log_files;
use manifest::{EntryKind, LegacyRoot, ManifestEntry, MANIFEST_VERSION, MIGRATION_MANIFEST};
pub(crate) use platform_secure_storage::{
    current_secure_storage_service, migrate_current_home_if_needed,
};
use report::{
    EntryReport, EntryStatus, MigrationMarker, MigrationReport, SecretReport, SecretStatus,
};
use settings::translate_legacy_settings;
use settings_rules::SETTINGS_RULES;
use sqlite::migrate_sqlite;

const MIGRATION_REPORT_FILE: &str = "migration-report.json";
const MIGRATION_MARKER_FILE: &str = "migration-complete.json";
const RETAINED_SECRET_KEYS: &[&str] = &["AiApiKeys", "FileBasedMcpCredentials"];

pub(crate) struct MigrationRequest<'a> {
    destination: PathBuf,
    legacy: LegacyRoots,
    secrets: &'a dyn MigrationSecretStore,
    #[cfg(test)]
    failure_after: Option<&'static str>,
}

impl<'a> MigrationRequest<'a> {
    pub(crate) fn new(
        destination: PathBuf,
        legacy: LegacyRoots,
        secrets: &'a dyn MigrationSecretStore,
    ) -> Self {
        Self {
            destination,
            legacy,
            secrets,
            #[cfg(test)]
            failure_after: None,
        }
    }

    #[cfg(test)]
    fn with_failure_after(mut self, entry_id: &'static str) -> Self {
        self.failure_after = Some(entry_id);
        self
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MigrationOutcome {
    ExistingDestination,
    InProgress,
    Migrated { manifest_version: u32 },
}

pub(crate) trait MigrationSecretStore {
    fn read_legacy(&self, key: &str) -> Result<Option<String>, MigrationSecretError>;
    fn read_destination(
        &self,
        key: &str,
        staging_root: &Path,
    ) -> Result<Option<String>, MigrationSecretError>;
    fn write_destination(
        &self,
        key: &str,
        value: &str,
        staging_root: &Path,
    ) -> Result<(), MigrationSecretError>;
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum MigrationSecretError {
    #[error("secure storage is unavailable")]
    Unavailable,
}

#[derive(Debug, Error)]
pub(crate) enum MigrationError {
    #[error("migration filesystem operation failed: {0}")]
    Io(#[from] io::Error),
    #[error("owner-only migration write failed: {0}")]
    OwnerOnly(#[from] OwnerOnlyFileError),
    #[error("could not serialize the redacted migration report: {0}")]
    Report(#[from] serde_json::Error),
    #[error("secure-storage migration failed for key {key}: {source}")]
    Secret {
        key: &'static str,
        source: MigrationSecretError,
    },
    #[error("destination secure storage already contains a different value for key {key}")]
    SecretConflict { key: &'static str },
    #[cfg(test)]
    #[error("injected migration failure after {after}")]
    InjectedFailure { after: &'static str },
    #[error("SQLite migration failed for {path}: {source}")]
    Sqlite {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },
}

pub(crate) fn migrate_legacy_home(
    request: MigrationRequest<'_>,
) -> Result<MigrationOutcome, MigrationError> {
    if fs::symlink_metadata(&request.destination).is_ok() {
        return Ok(MigrationOutcome::ExistingDestination);
    }

    let parent = request.destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "ZYH home must have a parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;
    let Some(_lock) = MigrationLock::acquire(&request.destination)? else {
        return Ok(MigrationOutcome::InProgress);
    };
    if fs::symlink_metadata(&request.destination).is_ok() {
        return Ok(MigrationOutcome::ExistingDestination);
    }

    let staging = Builder::new()
        .prefix(".zyh-migration-")
        .tempdir_in(parent)?;
    ensure_owner_only_dir(staging.path())?;

    let mut report = MigrationReport {
        manifest_version: MANIFEST_VERSION,
        entries: Vec::with_capacity(MIGRATION_MANIFEST.len()),
        omitted_setting_keys: Vec::new(),
        unknown_setting_keys: Vec::new(),
        skipped_paths: Vec::new(),
        secure_storage: Vec::with_capacity(RETAINED_SECRET_KEYS.len()),
    };

    for entry in MIGRATION_MANIFEST {
        migrate_entry(entry, &request.legacy, staging.path(), &mut report)?;
        #[cfg(test)]
        if request.failure_after == Some(entry.id) {
            return Err(MigrationError::InjectedFailure { after: entry.id });
        }
    }
    migrate_secrets(request.secrets, staging.path(), &mut report)?;

    write_json(&staging.path().join(MIGRATION_REPORT_FILE), &report)?;
    write_json(
        &staging.path().join(MIGRATION_MARKER_FILE),
        &MigrationMarker {
            manifest_version: MANIFEST_VERSION,
            complete: true,
        },
    )?;

    let staging_path = staging.keep();
    fs::rename(&staging_path, &request.destination)?;
    sync_directory(parent)?;

    Ok(MigrationOutcome::Migrated {
        manifest_version: MANIFEST_VERSION,
    })
}

fn migrate_entry(
    entry: &ManifestEntry,
    legacy: &LegacyRoots,
    staging: &Path,
    report: &mut MigrationReport,
) -> Result<(), MigrationError> {
    let source = legacy_root(legacy, entry.root).join(entry.source);
    let destination = staging.join(entry.destination);
    let metadata = match fs::symlink_metadata(&source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            report.entries.push(EntryReport {
                id: entry.id,
                status: EntryStatus::Missing,
            });
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };

    if metadata.file_type().is_symlink() {
        report.entries.push(EntryReport {
            id: entry.id,
            status: EntryStatus::SkippedSymlink,
        });
        report.skipped_paths.push(entry.destination.to_owned());
        return Ok(());
    }

    let status = match entry.kind {
        EntryKind::File if metadata.is_file() => {
            copy_file(&source, &destination)?;
            EntryStatus::Copied
        }
        EntryKind::Directory if metadata.is_dir() => {
            copy_directory(&source, &destination, entry.destination, report)?;
            EntryStatus::Copied
        }
        EntryKind::Settings { backup_name } if metadata.is_file() => {
            migrate_settings(&source, &destination, staging, backup_name, report)?
        }
        EntryKind::Sqlite if metadata.is_file() => {
            migrate_sqlite(&source, &destination).map_err(|source_error| {
                MigrationError::Sqlite {
                    path: source,
                    source: source_error,
                }
            })?;
            EntryStatus::CopiedAndCleaned
        }
        EntryKind::LogFiles if metadata.is_dir() => {
            if copy_log_files(
                &source,
                &destination,
                legacy.log_file_name(),
                entry.destination,
                report,
            )? {
                EntryStatus::Copied
            } else {
                EntryStatus::Missing
            }
        }
        EntryKind::File
        | EntryKind::Directory
        | EntryKind::Settings { .. }
        | EntryKind::Sqlite
        | EntryKind::LogFiles => {
            report.skipped_paths.push(entry.destination.to_owned());
            EntryStatus::SkippedUnsupported
        }
    };
    report.entries.push(EntryReport {
        id: entry.id,
        status,
    });
    Ok(())
}

fn migrate_settings(
    source: &Path,
    destination: &Path,
    staging: &Path,
    backup_name: &str,
    report: &mut MigrationReport,
) -> Result<EntryStatus, MigrationError> {
    let source_bytes = read_source_file(source)?;
    atomic_replace(
        &staging.join("migration").join(backup_name),
        &source_bytes,
        ExpectedContent::Missing,
    )?;
    let Ok(source_text) = std::str::from_utf8(&source_bytes) else {
        return Ok(EntryStatus::Malformed);
    };
    let Ok(translation) = translate_legacy_settings(source_text, SETTINGS_RULES) else {
        return Ok(EntryStatus::Malformed);
    };

    report.omitted_setting_keys.extend(translation.omitted_keys);
    report.unknown_setting_keys.extend(translation.unknown_keys);
    atomic_replace(
        destination,
        translation.settings.to_string().as_bytes(),
        ExpectedContent::Missing,
    )?;
    Ok(EntryStatus::Translated)
}

fn copy_directory(
    source: &Path,
    destination: &Path,
    report_prefix: &str,
    report: &mut MigrationReport,
) -> Result<(), MigrationError> {
    ensure_owner_only_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let report_path = format!("{report_prefix}/{}", entry.file_name().to_string_lossy());
        let metadata = fs::symlink_metadata(&source_path)?;

        if metadata.file_type().is_symlink() {
            report.skipped_paths.push(report_path);
        } else if metadata.is_dir() {
            copy_directory(&source_path, &destination_path, &report_path, report)?;
        } else if metadata.is_file() {
            copy_file(&source_path, &destination_path)?;
        } else {
            report.skipped_paths.push(report_path);
        }
    }
    Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), MigrationError> {
    let bytes = read_source_file(source)?;
    atomic_replace(destination, &bytes, ExpectedContent::Missing)?;
    Ok(())
}

fn read_source_file(path: &Path) -> Result<Vec<u8>, MigrationError> {
    let mut file = open_source_file(path)?;
    if !file.metadata()?.is_file() {
        return Err(
            io::Error::new(io::ErrorKind::InvalidData, "source is not a regular file").into(),
        );
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(unix)]
fn open_source_file(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;

    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_source_file(path: &Path) -> io::Result<File> {
    OpenOptions::new().read(true).open(path)
}

fn migrate_secrets(
    secrets: &dyn MigrationSecretStore,
    staging_root: &Path,
    report: &mut MigrationReport,
) -> Result<(), MigrationError> {
    for &key in RETAINED_SECRET_KEYS {
        let Some(legacy_value) = secrets
            .read_legacy(key)
            .map_err(|source| MigrationError::Secret { key, source })?
        else {
            report.secure_storage.push(SecretReport {
                key,
                status: SecretStatus::Missing,
            });
            continue;
        };

        match secrets
            .read_destination(key, staging_root)
            .map_err(|source| MigrationError::Secret { key, source })?
        {
            Some(destination_value) if destination_value != legacy_value => {
                return Err(MigrationError::SecretConflict { key });
            }
            Some(_) => {}
            None => secrets
                .write_destination(key, &legacy_value, staging_root)
                .map_err(|source| MigrationError::Secret { key, source })?,
        }

        let verified = secrets
            .read_destination(key, staging_root)
            .map_err(|source| MigrationError::Secret { key, source })?
            .is_some_and(|destination_value| destination_value == legacy_value);
        if !verified {
            return Err(MigrationError::Secret {
                key,
                source: MigrationSecretError::Unavailable,
            });
        }
        report.secure_storage.push(SecretReport {
            key,
            status: SecretStatus::CopiedAndVerified,
        });
    }
    Ok(())
}

fn legacy_root(legacy: &LegacyRoots, root: LegacyRoot) -> &Path {
    match root {
        LegacyRoot::HomeConfig => legacy.home_config_dir(),
        LegacyRoot::Config => legacy.config_dir(),
        LegacyRoot::Data => legacy.data_dir(),
        LegacyRoot::SecureState => legacy.secure_state_dir(),
        LegacyRoot::Logs => legacy.logs_dir(),
        LegacyRoot::TuiConfig => legacy.tui_config_dir(),
        LegacyRoot::TuiState => legacy.tui_state_dir(),
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), MigrationError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    atomic_replace(path, &bytes, ExpectedContent::Missing)?;
    Ok(())
}

struct MigrationLock {
    _file: File,
}

impl MigrationLock {
    fn acquire(destination: &Path) -> Result<Option<Self>, MigrationError> {
        let mut lock_name = destination.as_os_str().to_os_string();
        lock_name.push(".migration.lock");
        let path = PathBuf::from(lock_name);
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true);
        let mut file = options.open(&path)?;
        ensure_owner_only_file(&path)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Ok(None),
            Err(fs::TryLockError::Error(error)) => return Err(error.into()),
        }

        file.set_len(0)?;
        file.write_all(b"ZYH home migration in progress\n")?;
        file.sync_all()?;
        Ok(Some(Self { _file: file }))
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "zyh_home_migration_tests.rs"]
mod tests;
