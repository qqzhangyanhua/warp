//! File-backed Global Agent Rules document.
//!
//! The sole source of truth for Global Rules is the standard path
//! `~/.agents/AGENTS.md`. Create, save, and delete use owner-only atomic
//! writes and content-hash conflict detection. Cached cloud Rules are never
//! read or imported here.

use std::path::{Path, PathBuf};
use std::{fs, io};

use thiserror::Error;
use warpui_extras::owner_only_file::{
    atomic_create, atomic_replace, content_hash, ContentHash, ExpectedContent, OwnerOnlyFileError,
};

pub(crate) const AGENTS_SUBDIR: &str = ".agents";
pub(crate) const AGENTS_FILE_NAME: &str = "AGENTS.md";

/// In-memory snapshot of the on-disk global rules document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobalAgentRulesState {
    /// The standard file does not exist.
    Missing,
    /// The file exists and was read successfully.
    Present {
        content: String,
        content_hash: ContentHash,
    },
}

/// Errors from loading or mutating the global rules document.
#[derive(Debug, Error)]
pub enum GlobalAgentRulesError {
    #[error("home directory is unavailable")]
    HomeNotFound,
    #[error("the global rules file changed before it could be written: {path}")]
    Conflict { path: PathBuf },
    #[error("refusing to operate on a symlink or non-regular file: {path}")]
    UnsupportedFileType { path: PathBuf },
    #[error("global rules file operation failed: {0}")]
    Io(#[from] io::Error),
}

impl From<OwnerOnlyFileError> for GlobalAgentRulesError {
    fn from(error: OwnerOnlyFileError) -> Self {
        match error {
            OwnerOnlyFileError::Io(error) => Self::Io(error),
            OwnerOnlyFileError::Conflict { path } => Self::Conflict { path },
            OwnerOnlyFileError::UnsupportedFileType { path } => Self::UnsupportedFileType { path },
        }
    }
}

/// Document handle for the standard global Agent rules file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalAgentRulesDocument {
    path: PathBuf,
}

impl GlobalAgentRulesDocument {
    /// Absolute path of the standard global rules file under `home`.
    pub fn standard_path_for_home(home: impl AsRef<Path>) -> PathBuf {
        home.as_ref().join(AGENTS_SUBDIR).join(AGENTS_FILE_NAME)
    }

    /// Document for the current user's home directory.
    pub fn standard() -> Result<Self, GlobalAgentRulesError> {
        let home = dirs::home_dir().ok_or(GlobalAgentRulesError::HomeNotFound)?;
        Ok(Self::with_path(Self::standard_path_for_home(home)))
    }

    /// Document at an explicit path. Used by tests and callers that already
    /// resolved a home root.
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Display path used in the Rules surface (absolute).
    pub fn display_path(&self) -> String {
        self.path.display().to_string()
    }

    /// Load the current on-disk state without mutating it.
    pub fn load(&self) -> Result<GlobalAgentRulesState, GlobalAgentRulesError> {
        match content_hash(&self.path)? {
            None => Ok(GlobalAgentRulesState::Missing),
            Some(content_hash) => {
                let content = fs::read_to_string(&self.path)?;
                Ok(GlobalAgentRulesState::Present {
                    content,
                    content_hash,
                })
            }
        }
    }

    /// Create the file when it is absent. Fails with [`GlobalAgentRulesError::Conflict`]
    /// if the file already exists.
    pub fn create(&self, content: &str) -> Result<ContentHash, GlobalAgentRulesError> {
        Ok(atomic_create(&self.path, content.as_bytes())?)
    }

    /// Atomically replace the file when `expected` still matches on-disk content.
    ///
    /// Use [`ExpectedContent::Missing`] for first save of a create flow that
    /// already reserved the path via [`Self::create`], or
    /// [`ExpectedContent::Hash`] after a successful load.
    pub fn save(
        &self,
        content: &str,
        expected: ExpectedContent,
    ) -> Result<ContentHash, GlobalAgentRulesError> {
        Ok(atomic_replace(&self.path, content.as_bytes(), expected)?.content_hash)
    }

    /// Delete the file when `expected` still matches. Missing file with
    /// [`ExpectedContent::Missing`] or [`ExpectedContent::Any`] is success.
    pub fn delete(&self, expected: ExpectedContent) -> Result<(), GlobalAgentRulesError> {
        let actual = content_hash(&self.path)?;
        if !expected_matches(expected, actual) {
            return Err(GlobalAgentRulesError::Conflict {
                path: self.path.clone(),
            });
        }

        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Expected-content token for a subsequent save/delete based on a loaded state.
    pub fn expected_content(state: &GlobalAgentRulesState) -> ExpectedContent {
        match state {
            GlobalAgentRulesState::Missing => ExpectedContent::Missing,
            GlobalAgentRulesState::Present { content_hash, .. } => {
                ExpectedContent::Hash(*content_hash)
            }
        }
    }
}

fn expected_matches(expected: ExpectedContent, actual: Option<ContentHash>) -> bool {
    match expected {
        ExpectedContent::Any => true,
        ExpectedContent::Missing => actual.is_none(),
        ExpectedContent::Hash(expected_hash) => actual == Some(expected_hash),
    }
}

#[cfg(test)]
#[path = "global_agent_rules_document_tests.rs"]
mod tests;
