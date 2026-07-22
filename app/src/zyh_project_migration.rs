use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::{fs, io};

use sha2::{Digest as _, Sha256};
use walkdir::WalkDir;
use warp_core::paths::{LEGACY_PROJECT_CONFIG_DIR, ZYH_PROJECT_CONFIG_DIR};
use warpui_extras::owner_only_file::{atomic_replace, ExpectedContent};

mod manifest;
mod mcp;
pub(crate) mod modal;
mod model;

use manifest::{ManifestEntryKind, PROJECT_MIGRATION_MANIFEST};
use model::{DestinationSnapshot, FileHash, FileSnapshot};
pub(crate) use model::{
    MigrationPreview, MigrationPreviewEntry, MigrationResult, MigrationResultEntry,
    MigrationResultStatus, PreviewStatus, ProjectMigrationError,
};

pub(crate) fn preview_project_migration(
    path: &Path,
) -> Result<MigrationPreview, ProjectMigrationError> {
    let repository = git2::Repository::discover(path)?;
    let repository_root = repository
        .workdir()
        .ok_or(ProjectMigrationError::BareRepository)?
        .to_path_buf();
    let source_root = repository_root.join(LEGACY_PROJECT_CONFIG_DIR);
    let source_metadata = match fs::symlink_metadata(&source_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ProjectMigrationError::MissingLegacyConfiguration(
                source_root,
            ));
        }
        Err(source) => {
            return Err(ProjectMigrationError::Io {
                path: source_root,
                source,
            });
        }
    };
    if !source_metadata.file_type().is_dir() {
        return Err(ProjectMigrationError::InvalidLegacyConfiguration(
            source_root,
        ));
    }

    let mut entries = Vec::new();
    for manifest_entry in PROJECT_MIGRATION_MANIFEST {
        match manifest_entry.kind {
            ManifestEntryKind::Directory => preview_directory(
                &repository_root,
                Path::new(manifest_entry.path),
                &mut entries,
            )?,
            ManifestEntryKind::SanitizedMcp => preview_mcp(&repository_root, &mut entries)?,
        }
    }
    preview_unsupported_entries(&repository_root, &mut entries)?;
    entries.sort_by(|left, right| left.source.cmp(&right.source));

    Ok(MigrationPreview {
        manifest_version: manifest::MANIFEST_VERSION,
        repository_root,
        entries,
    })
}

pub(crate) fn execute_project_migration(preview: MigrationPreview) -> MigrationResult {
    let entries = preview
        .entries
        .into_iter()
        .map(|entry| execute_entry(&preview.repository_root, entry))
        .collect();

    MigrationResult {
        manifest_version: preview.manifest_version,
        entries,
    }
}

fn preview_directory(
    repository_root: &Path,
    relative_directory: &Path,
    entries: &mut Vec<MigrationPreviewEntry>,
) -> Result<(), ProjectMigrationError> {
    let source_root = repository_root.join(LEGACY_PROJECT_CONFIG_DIR);
    let directory = source_root.join(relative_directory);
    let metadata = match fs::symlink_metadata(&directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(ProjectMigrationError::Io {
                path: relative_directory.to_path_buf(),
                source,
            });
        }
    };
    if metadata.file_type().is_symlink() {
        entries.push(non_copy_entry(
            relative_source(relative_directory),
            Some(relative_destination(relative_directory)),
            PreviewStatus::SkippedSymlink,
        ));
        return Ok(());
    }
    if !metadata.file_type().is_dir() {
        entries.push(non_copy_entry(
            relative_source(relative_directory),
            Some(relative_destination(relative_directory)),
            PreviewStatus::Unsupported,
        ));
        return Ok(());
    }

    for item in WalkDir::new(&directory).follow_links(false).min_depth(1) {
        let item = item.map_err(|source| ProjectMigrationError::Walk {
            path: relative_directory.to_path_buf(),
            source,
        })?;
        let relative = item
            .path()
            .strip_prefix(&source_root)
            .expect("walked entry must remain under the legacy project root");
        let file_type = item.file_type();
        if file_type.is_dir() {
            continue;
        }
        if file_type.is_symlink() {
            entries.push(non_copy_entry(
                relative_source(relative),
                Some(relative_destination(relative)),
                PreviewStatus::SkippedSymlink,
            ));
        } else if file_type.is_file() {
            entries.push(preview_file(repository_root, relative)?);
        } else {
            entries.push(non_copy_entry(
                relative_source(relative),
                Some(relative_destination(relative)),
                PreviewStatus::Unsupported,
            ));
        }
    }
    Ok(())
}

fn preview_file(
    repository_root: &Path,
    relative: &Path,
) -> Result<MigrationPreviewEntry, ProjectMigrationError> {
    let source = relative_source(relative);
    let destination = relative_destination(relative);
    let source_hash = hash_file(&repository_root.join(&source)).map_err(|source_error| {
        ProjectMigrationError::Io {
            path: source.clone(),
            source: source_error,
        }
    })?;
    let destination_path = repository_root.join(&destination);
    let (status, destination_snapshot) = destination_status(&destination_path, source_hash)?;

    Ok(MigrationPreviewEntry {
        source,
        destination: Some(destination),
        status,
        omissions: Vec::new(),
        snapshot: Some(FileSnapshot {
            source_hash,
            destination: destination_snapshot,
            prepared_content: None,
        }),
    })
}

fn preview_mcp(
    repository_root: &Path,
    entries: &mut Vec<MigrationPreviewEntry>,
) -> Result<(), ProjectMigrationError> {
    let relative = Path::new(".mcp.json");
    let source = relative_source(relative);
    let source_path = repository_root.join(&source);
    let metadata = match fs::symlink_metadata(&source_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source_error) => {
            return Err(ProjectMigrationError::Io {
                path: source,
                source: source_error,
            });
        }
    };
    let destination = relative_destination(relative);
    if metadata.file_type().is_symlink() {
        entries.push(non_copy_entry(
            source,
            Some(destination),
            PreviewStatus::SkippedSymlink,
        ));
        return Ok(());
    }
    if !metadata.file_type().is_file() {
        entries.push(non_copy_entry(
            source,
            Some(destination),
            PreviewStatus::Unsupported,
        ));
        return Ok(());
    }

    let source_bytes =
        fs::read(&source_path).map_err(|source_error| ProjectMigrationError::Io {
            path: source.clone(),
            source: source_error,
        })?;
    let sanitized = mcp::sanitize_mcp(&source_bytes).map_err(|source_error| {
        ProjectMigrationError::MalformedMcp {
            path: source.clone(),
            source: source_error,
        }
    })?;
    let output_hash = hash_bytes(&sanitized.bytes);
    let destination_path = repository_root.join(&destination);
    let (status, destination_snapshot) = destination_status(&destination_path, output_hash)?;
    entries.push(MigrationPreviewEntry {
        source,
        destination: Some(destination),
        status,
        omissions: sanitized.omissions,
        snapshot: Some(FileSnapshot {
            source_hash: hash_bytes(&source_bytes),
            destination: destination_snapshot,
            prepared_content: Some(sanitized.bytes),
        }),
    });
    Ok(())
}

fn preview_unsupported_entries(
    repository_root: &Path,
    entries: &mut Vec<MigrationPreviewEntry>,
) -> Result<(), ProjectMigrationError> {
    let approved: HashSet<&str> = PROJECT_MIGRATION_MANIFEST
        .iter()
        .map(|entry| entry.path)
        .collect();
    let source_root = repository_root.join(LEGACY_PROJECT_CONFIG_DIR);
    let directory = fs::read_dir(&source_root).map_err(|source| ProjectMigrationError::Io {
        path: PathBuf::from(LEGACY_PROJECT_CONFIG_DIR),
        source,
    })?;
    for item in directory {
        let item = item.map_err(|source| ProjectMigrationError::Io {
            path: PathBuf::from(LEGACY_PROJECT_CONFIG_DIR),
            source,
        })?;
        if item
            .file_name()
            .to_str()
            .is_some_and(|name| approved.contains(name))
        {
            continue;
        }
        entries.push(non_copy_entry(
            Path::new(LEGACY_PROJECT_CONFIG_DIR).join(item.file_name()),
            None,
            PreviewStatus::Unsupported,
        ));
    }
    Ok(())
}

fn destination_status(
    destination: &Path,
    source_hash: FileHash,
) -> Result<(PreviewStatus, DestinationSnapshot), ProjectMigrationError> {
    if destination_has_invalid_ancestor(destination) {
        return Ok((PreviewStatus::Conflict, DestinationSnapshot::Other));
    }
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_file() => {
            let destination_hash =
                hash_file(destination).map_err(|source| ProjectMigrationError::Io {
                    path: destination.to_path_buf(),
                    source,
                })?;
            let status = if destination_hash == source_hash {
                PreviewStatus::AlreadyPresent
            } else {
                PreviewStatus::Conflict
            };
            Ok((status, DestinationSnapshot::Regular(destination_hash)))
        }
        Ok(_) => Ok((PreviewStatus::Conflict, DestinationSnapshot::Other)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Ok((PreviewStatus::Ready, DestinationSnapshot::Missing))
        }
        Err(source) => Err(ProjectMigrationError::Io {
            path: destination.to_path_buf(),
            source,
        }),
    }
}

fn execute_entry(repository_root: &Path, entry: MigrationPreviewEntry) -> MigrationResultEntry {
    let source_bytes = entry
        .snapshot
        .as_ref()
        .map(|snapshot| revalidate_snapshot(repository_root, &entry, snapshot));
    let status = match source_bytes {
        Some(Err(())) => MigrationResultStatus::Stale,
        Some(Ok(source_bytes)) => match entry.status {
            PreviewStatus::Ready => execute_ready_entry(repository_root, &entry, &source_bytes),
            PreviewStatus::AlreadyPresent => MigrationResultStatus::AlreadyPresent,
            PreviewStatus::Conflict => MigrationResultStatus::Conflict,
            PreviewStatus::SkippedSymlink | PreviewStatus::Unsupported => {
                MigrationResultStatus::Stale
            }
        },
        None => match entry.status {
            PreviewStatus::Ready | PreviewStatus::AlreadyPresent | PreviewStatus::Conflict => {
                MigrationResultStatus::Stale
            }
            PreviewStatus::SkippedSymlink => MigrationResultStatus::SkippedSymlink,
            PreviewStatus::Unsupported => MigrationResultStatus::Unsupported,
        },
    };
    MigrationResultEntry {
        source: entry.source,
        destination: entry.destination,
        status,
        omissions: entry.omissions,
    }
}

fn execute_ready_entry(
    repository_root: &Path,
    entry: &MigrationPreviewEntry,
    source_bytes: &[u8],
) -> MigrationResultStatus {
    let Some(destination) = entry.destination.as_ref() else {
        return MigrationResultStatus::Failed("copy candidate has no destination".to_owned());
    };
    let destination_path = repository_root.join(destination);
    let output = entry
        .snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.prepared_content.as_deref())
        .unwrap_or(source_bytes);
    match atomic_replace(&destination_path, output, ExpectedContent::Missing) {
        Ok(_) => MigrationResultStatus::Copied,
        Err(error) => MigrationResultStatus::Failed(error.to_string()),
    }
}

fn revalidate_snapshot(
    repository_root: &Path,
    entry: &MigrationPreviewEntry,
    snapshot: &FileSnapshot,
) -> Result<Vec<u8>, ()> {
    let destination = entry.destination.as_ref().ok_or(())?;
    if path_has_symlink_or_non_directory_ancestor(repository_root, &entry.source) {
        return Err(());
    }
    let source_path = repository_root.join(&entry.source);
    match fs::symlink_metadata(&source_path) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        _ => return Err(()),
    }
    let source_bytes = fs::read(source_path).map_err(|_| ())?;
    if hash_bytes(&source_bytes) != snapshot.source_hash {
        return Err(());
    }
    let destination_path = repository_root.join(destination);
    if destination_snapshot(&destination_path).map_err(|_| ())? != snapshot.destination {
        return Err(());
    }
    Ok(source_bytes)
}

fn destination_snapshot(path: &Path) -> io::Result<DestinationSnapshot> {
    if destination_has_invalid_ancestor(path) {
        return Ok(DestinationSnapshot::Other);
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => {
            hash_file(path).map(DestinationSnapshot::Regular)
        }
        Ok(_) => Ok(DestinationSnapshot::Other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(DestinationSnapshot::Missing),
        Err(error) => Err(error),
    }
}

fn destination_has_invalid_ancestor(destination: &Path) -> bool {
    destination
        .parent()
        .is_some_and(|parent| existing_path_is_symlink_or_non_directory(parent))
}

fn existing_path_is_symlink_or_non_directory(path: &Path) -> bool {
    let mut current = path;
    loop {
        match fs::symlink_metadata(current) {
            Ok(metadata) => return !metadata.file_type().is_dir(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return true,
        }
        let Some(parent) = current.parent() else {
            return false;
        };
        current = parent;
    }
}

fn path_has_symlink_or_non_directory_ancestor(repository_root: &Path, relative: &Path) -> bool {
    let Some(parent) = relative.parent() else {
        return false;
    };
    let mut current = repository_root.to_path_buf();
    for component in parent.components() {
        let Component::Normal(component) = component else {
            return true;
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_dir() => {}
            _ => return true,
        }
    }
    false
}

fn hash_file(path: &Path) -> io::Result<FileHash> {
    fs::read(path).map(|bytes| hash_bytes(&bytes))
}

fn hash_bytes(bytes: &[u8]) -> FileHash {
    FileHash(Sha256::digest(bytes).into())
}

fn relative_source(relative: &Path) -> PathBuf {
    Path::new(LEGACY_PROJECT_CONFIG_DIR).join(relative)
}

fn relative_destination(relative: &Path) -> PathBuf {
    Path::new(ZYH_PROJECT_CONFIG_DIR).join(relative)
}

fn non_copy_entry(
    source: PathBuf,
    destination: Option<PathBuf>,
    status: PreviewStatus,
) -> MigrationPreviewEntry {
    MigrationPreviewEntry {
        source,
        destination,
        status,
        omissions: Vec::new(),
        snapshot: None,
    }
}

#[cfg(test)]
#[path = "zyh_project_migration_tests.rs"]
mod tests;
