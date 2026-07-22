use std::path::Path;

use warp_core::channel::ChannelState;
use warp_core::paths::{AppHome, AppHomeProfile, LegacyRoots};
use warpui_extras::secure_storage::{self, Error as SecureStorageError, Model, SecureStorage};

use super::{migrate_legacy_home, MigrationRequest, MigrationSecretError, MigrationSecretStore};

const ZYH_PRODUCTION_SECRET_SERVICE: &str = "dev.zyh.ZYH";
const ZYH_DEVELOPMENT_SECRET_SERVICE: &str = "dev.zyh.ZYH-Development";

pub(crate) fn migrate_current_home_if_needed() -> anyhow::Result<()> {
    if AppHomeProfile::current() != AppHomeProfile::Production {
        return Ok(());
    }

    let destination = AppHome::current()?.root().to_path_buf();
    let legacy = LegacyRoots::current()
        .ok_or_else(|| anyhow::anyhow!("could not resolve the active legacy application roots"))?;
    let secrets = PlatformMigrationSecretStore::new(
        &ChannelState::data_domain(),
        ZYH_PRODUCTION_SECRET_SERVICE,
        &legacy,
    );
    migrate_legacy_home(MigrationRequest::new(destination, legacy, &secrets))?;
    Ok(())
}

pub(crate) fn current_secure_storage_service() -> &'static str {
    match AppHomeProfile::current() {
        AppHomeProfile::Production => ZYH_PRODUCTION_SECRET_SERVICE,
        AppHomeProfile::Development => ZYH_DEVELOPMENT_SECRET_SERVICE,
        AppHomeProfile::Integration => "dev.zyh.ZYH-Integration",
    }
}

struct PlatformMigrationSecretStore {
    legacy: Model,
    #[cfg(not(target_os = "windows"))]
    destination: Model,
    #[cfg(target_os = "windows")]
    destination_service: String,
}

impl PlatformMigrationSecretStore {
    fn new(
        legacy_service: &str,
        destination_service: &str,
        #[cfg_attr(
            not(any(target_os = "linux", target_os = "freebsd", target_os = "windows")),
            allow(unused_variables)
        )]
        legacy_roots: &LegacyRoots,
    ) -> Self {
        cfg_if::cfg_if! {
            if #[cfg(any(target_os = "linux", target_os = "freebsd"))] {
                Self {
                    legacy: secure_storage::open_with_fallback(
                        legacy_service,
                        legacy_roots.state_dir().to_path_buf(),
                    ),
                    destination: secure_storage::open(destination_service),
                }
            } else if #[cfg(target_os = "windows")] {
                Self {
                    legacy: secure_storage::open_with_dir(
                        legacy_service,
                        legacy_roots.state_dir().to_path_buf(),
                    ),
                    destination_service: destination_service.to_owned(),
                }
            } else {
                Self {
                    legacy: secure_storage::open(legacy_service),
                    destination: secure_storage::open(destination_service),
                }
            }
        }
    }

    fn read(
        storage: &dyn SecureStorage,
        key: &str,
    ) -> Result<Option<String>, MigrationSecretError> {
        match storage.read_value(key) {
            Ok(value) => Ok(Some(value)),
            Err(SecureStorageError::NotFound) => Ok(None),
            Err(SecureStorageError::DecodeError(_) | SecureStorageError::Unknown(_)) => {
                Err(MigrationSecretError::Unavailable)
            }
            #[cfg(target_os = "windows")]
            Err(
                SecureStorageError::IOError(_)
                | SecureStorageError::WindowsAPIError(_)
                | SecureStorageError::InvalidLocation,
            ) => Err(MigrationSecretError::Unavailable),
        }
    }

    #[cfg(target_os = "windows")]
    fn with_destination<T>(
        &self,
        staging_root: &Path,
        callback: impl FnOnce(&dyn SecureStorage) -> T,
    ) -> T {
        let destination =
            secure_storage::open_with_dir(&self.destination_service, staging_root.to_path_buf());
        callback(destination.as_ref())
    }

    #[cfg(not(target_os = "windows"))]
    fn with_destination<T>(&self, _: &Path, callback: impl FnOnce(&dyn SecureStorage) -> T) -> T {
        callback(self.destination.as_ref())
    }
}

impl MigrationSecretStore for PlatformMigrationSecretStore {
    fn read_legacy(&self, key: &str) -> Result<Option<String>, MigrationSecretError> {
        Self::read(self.legacy.as_ref(), key)
    }

    fn read_destination(
        &self,
        key: &str,
        staging_root: &Path,
    ) -> Result<Option<String>, MigrationSecretError> {
        self.with_destination(staging_root, |destination| Self::read(destination, key))
    }

    fn write_destination(
        &self,
        key: &str,
        value: &str,
        staging_root: &Path,
    ) -> Result<(), MigrationSecretError> {
        self.with_destination(staging_root, |destination| {
            destination
                .write_value_with_owner_only_fallback(key, value)
                .map_err(|_| MigrationSecretError::Unavailable)
        })
    }
}
