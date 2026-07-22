use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};
use tempfile::Builder;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContentHash([u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpectedContent {
    Any,
    Missing,
    Hash(ContentHash),
}

#[derive(Debug, Error)]
pub enum OwnerOnlyFileError {
    #[error("owner-only file operation failed: {0}")]
    Io(#[from] io::Error),
    #[error("the file changed before it could be replaced: {path}")]
    Conflict { path: PathBuf },
    #[error("refusing to replace a symlink or non-regular file: {path}")]
    UnsupportedFileType { path: PathBuf },
}

#[derive(Debug, PartialEq, Eq)]
pub struct AtomicWriteResult {
    pub content_hash: ContentHash,
    pub backup_path: PathBuf,
}

pub fn content_hash(path: &Path) -> Result<Option<ContentHash>, OwnerOnlyFileError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(Some(hash_bytes(&fs::read(path)?))),
        Ok(_) => Err(OwnerOnlyFileError::UnsupportedFileType {
            path: path.to_path_buf(),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn ensure_owner_only_dir(path: &Path) -> Result<(), OwnerOnlyFileError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => {
            return Err(OwnerOnlyFileError::UnsupportedFileType {
                path: path.to_path_buf(),
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir_all(path)?,
        Err(error) => return Err(error.into()),
    }

    set_owner_only_dir_permissions(path)?;
    Ok(())
}

pub fn ensure_owner_only_file(path: &Path) -> Result<(), OwnerOnlyFileError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => set_owner_only_file_permissions(path)?,
        Ok(_) => {
            return Err(OwnerOnlyFileError::UnsupportedFileType {
                path: path.to_path_buf(),
            });
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

pub fn atomic_replace(
    path: &Path,
    bytes: &[u8],
    expected: ExpectedContent,
) -> Result<AtomicWriteResult, OwnerOnlyFileError> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "owner-only file path must have a parent directory",
        )
    })?;
    ensure_owner_only_dir(parent)?;

    let previous_hash = content_hash(path)?;
    if !expected.matches(previous_hash) {
        return Err(OwnerOnlyFileError::Conflict {
            path: path.to_path_buf(),
        });
    }

    let backup_path = backup_path(path);
    if previous_hash.is_some() {
        let previous_bytes = fs::read(path)?;
        validate_destination(&backup_path)?;
        persist_owner_only(&backup_path, &previous_bytes)?;
    }

    persist_owner_only(path, bytes)?;

    Ok(AtomicWriteResult {
        content_hash: hash_bytes(bytes),
        backup_path,
    })
}

impl ExpectedContent {
    fn matches(self, actual: Option<ContentHash>) -> bool {
        match self {
            Self::Any => true,
            Self::Missing => actual.is_none(),
            Self::Hash(expected) => actual == Some(expected),
        }
    }
}

fn persist_owner_only(path: &Path, bytes: &[u8]) -> Result<(), OwnerOnlyFileError> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "owner-only file path must have a parent directory",
        )
    })?;
    let mut temporary = Builder::new().prefix(".zyh-write-").tempfile_in(parent)?;
    set_owner_only_file_permissions(temporary.path())?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    sync_parent(parent)?;
    Ok(())
}

fn validate_destination(path: &Path) -> Result<(), OwnerOnlyFileError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) => Err(OwnerOnlyFileError::UnsupportedFileType {
            path: path.to_path_buf(),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".bak");
    PathBuf::from(name)
}

fn hash_bytes(bytes: &[u8]) -> ContentHash {
    ContentHash(Sha256::digest(bytes).into())
}

#[cfg(unix)]
fn set_owner_only_dir_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(unix)]
fn set_owner_only_file_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> io::Result<()> {
    fs::File::open(path)?.sync_all()
}

#[cfg(target_os = "windows")]
fn set_owner_only_dir_permissions(path: &Path) -> io::Result<()> {
    set_owner_only_windows_acl(path)
}

#[cfg(target_os = "windows")]
fn set_owner_only_file_permissions(path: &Path) -> io::Result<()> {
    set_owner_only_windows_acl(path)
}

#[cfg(target_os = "windows")]
fn sync_parent(_: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn set_owner_only_windows_acl(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows::Win32::Security::{
        SetFileSecurityW, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR,
    };

    let descriptor_text: Vec<u16> = "D:P(A;;FA;;;OW)\0".encode_utf16().collect();
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();

    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(descriptor_text.as_ptr()),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
        .map_err(windows_error)?;

        let result = SetFileSecurityW(
            PCWSTR(path.as_ptr()),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        )
        .ok()
        .map_err(windows_error);
        let _ = LocalFree(Some(HLOCAL(descriptor.0)));
        result
    }
}

#[cfg(target_os = "windows")]
fn windows_error(error: windows::core::Error) -> io::Error {
    io::Error::from_raw_os_error(error.code().0)
}

#[cfg(test)]
#[path = "owner_only_file_tests.rs"]
mod tests;
