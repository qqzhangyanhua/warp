use std::path::{Component, Path, PathBuf};
use std::{fs, io};

use super::model::{DestinationSnapshot, FileSnapshot};
use super::{destination_has_invalid_ancestor, hash_bytes, hash_file, MigrationPreviewEntry};

pub(super) enum SnapshotRevalidationError {
    Stale,
    Io { path: PathBuf, source: io::Error },
}

pub(super) fn revalidate_snapshot(
    repository_root: &Path,
    entry: &MigrationPreviewEntry,
    snapshot: &FileSnapshot,
) -> Result<Vec<u8>, SnapshotRevalidationError> {
    let destination = entry
        .destination
        .as_ref()
        .ok_or(SnapshotRevalidationError::Stale)?;
    if path_has_symlink_or_non_directory_ancestor(repository_root, &entry.source).map_err(
        |source| SnapshotRevalidationError::Io {
            path: entry.source.clone(),
            source,
        },
    )? {
        return Err(SnapshotRevalidationError::Stale);
    }
    let source_path = repository_root.join(&entry.source);
    match fs::symlink_metadata(&source_path) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => return Err(SnapshotRevalidationError::Stale),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(SnapshotRevalidationError::Stale);
        }
        Err(source) => {
            return Err(SnapshotRevalidationError::Io {
                path: entry.source.clone(),
                source,
            });
        }
    }
    let source_bytes = fs::read(source_path).map_err(|source| SnapshotRevalidationError::Io {
        path: entry.source.clone(),
        source,
    })?;
    if hash_bytes(&source_bytes) != snapshot.source_hash {
        return Err(SnapshotRevalidationError::Stale);
    }
    let destination_path = repository_root.join(destination);
    if destination_snapshot(&destination_path).map_err(|source| SnapshotRevalidationError::Io {
        path: destination.clone(),
        source,
    })? != snapshot.destination
    {
        return Err(SnapshotRevalidationError::Stale);
    }
    Ok(source_bytes)
}

fn destination_snapshot(path: &Path) -> io::Result<DestinationSnapshot> {
    if destination_has_invalid_ancestor(path)? {
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

fn path_has_symlink_or_non_directory_ancestor(
    repository_root: &Path,
    relative: &Path,
) -> io::Result<bool> {
    let Some(parent) = relative.parent() else {
        return Ok(false);
    };
    let mut current = repository_root.to_path_buf();
    for component in parent.components() {
        let Component::Normal(component) = component else {
            return Ok(true);
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_dir() => {}
            Ok(_) => return Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(true),
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}
