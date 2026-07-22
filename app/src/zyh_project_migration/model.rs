use std::io;
use std::path::PathBuf;

use thiserror::Error;

use super::mcp::McpSanitizationError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreviewStatus {
    Ready,
    AlreadyPresent,
    Conflict,
    SkippedSymlink,
    Unsupported,
}

#[derive(Clone)]
pub(crate) struct MigrationPreviewEntry {
    pub(crate) source: PathBuf,
    pub(crate) destination: Option<PathBuf>,
    pub(crate) status: PreviewStatus,
    pub(crate) omissions: Vec<String>,
    pub(super) snapshot: Option<FileSnapshot>,
}

impl std::fmt::Debug for MigrationPreviewEntry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MigrationPreviewEntry")
            .field("source", &self.source)
            .field("destination", &self.destination)
            .field("status", &self.status)
            .field("omissions", &self.omissions)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MigrationPreview {
    pub(crate) manifest_version: u32,
    pub(crate) repository_root: PathBuf,
    pub(crate) entries: Vec<MigrationPreviewEntry>,
}

#[derive(Clone)]
pub(super) struct FileSnapshot {
    pub(super) source_hash: FileHash,
    pub(super) destination: DestinationSnapshot,
    pub(super) prepared_content: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FileHash(pub(super) [u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DestinationSnapshot {
    Missing,
    Regular(FileHash),
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MigrationResultStatus {
    Copied,
    AlreadyPresent,
    Conflict,
    SkippedSymlink,
    Unsupported,
    Stale,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MigrationResultEntry {
    pub(crate) source: PathBuf,
    pub(crate) destination: Option<PathBuf>,
    pub(crate) status: MigrationResultStatus,
    pub(crate) omissions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MigrationResult {
    pub(crate) manifest_version: u32,
    pub(crate) entries: Vec<MigrationResultEntry>,
}

#[derive(Debug, Error)]
pub(crate) enum ProjectMigrationError {
    #[error("the selected path is not in a Git repository: {0}")]
    Repository(#[from] git2::Error),
    #[error("bare repositories do not have project configuration")]
    BareRepository,
    #[error("legacy project configuration does not exist at {0}")]
    MissingLegacyConfiguration(PathBuf),
    #[error("legacy project configuration is not a real directory: {0}")]
    InvalidLegacyConfiguration(PathBuf),
    #[error("could not inspect project configuration at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not walk project configuration at {path}: {source}")]
    Walk {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
    #[error("could not sanitize MCP configuration at {path}: {source}")]
    MalformedMcp {
        path: PathBuf,
        #[source]
        source: McpSanitizationError,
    },
}
