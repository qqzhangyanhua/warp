//! Local Markdown Notebook file IO.
//!
//! Notebooks are ordinary Markdown files on disk. Create and replace use
//! owner-only atomic writes with content-hash conflict detection. Session
//! state (unsaved / dirty / conflict) lives in
//! [`crate::notebooks::file::local_session`]. Relative links and images are
//! resolved by the editor via `document_path`, not here.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use warpui_extras::owner_only_file::{
    atomic_create, atomic_replace, content_hash, ContentHash, ExpectedContent, OwnerOnlyFileError,
};

/// In-memory record of a local Markdown Notebook file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalMarkdownNotebook {
    pub path: PathBuf,
    pub content: String,
    pub content_hash: ContentHash,
}

/// Errors from loading or mutating local Markdown Notebook files.
#[derive(Debug, Error)]
pub enum LocalMarkdownError {
    #[error("notebook path is empty or invalid")]
    InvalidPath,
    #[error("notebook path must use a Markdown extension (.md or .markdown): {path}")]
    InvalidExtension { path: PathBuf },
    #[error("a notebook file already exists at {path}")]
    PathCollision { path: PathBuf },
    #[error("the notebook file changed before it could be written: {path}")]
    Conflict { path: PathBuf },
    #[error("refusing to operate on a symlink or non-regular file: {path}")]
    UnsupportedFileType { path: PathBuf },
    #[error("notebook file operation failed: {0}")]
    Io(#[from] io::Error),
}

impl From<OwnerOnlyFileError> for LocalMarkdownError {
    fn from(error: OwnerOnlyFileError) -> Self {
        match error {
            OwnerOnlyFileError::Io(error) => Self::Io(error),
            OwnerOnlyFileError::Conflict { path } => Self::Conflict { path },
            OwnerOnlyFileError::UnsupportedFileType { path } => Self::UnsupportedFileType { path },
        }
    }
}

/// Validate that `path` is an acceptable local Markdown Notebook destination.
pub fn validate_markdown_path(path: &Path) -> Result<(), LocalMarkdownError> {
    if path.as_os_str().is_empty() {
        return Err(LocalMarkdownError::InvalidPath);
    }
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_none_or(|name| name.is_empty() || name == "." || name == "..")
    {
        return Err(LocalMarkdownError::InvalidPath);
    }
    if !is_markdown_extension(path) {
        return Err(LocalMarkdownError::InvalidExtension {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

/// Whether `path` uses a Markdown file extension.
pub fn is_markdown_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

/// Suggested default filename for a save dialog from a display title.
pub fn default_save_filename(title_hint: &str) -> String {
    let stem = title_hint.trim();
    let stem = if stem.is_empty() { "Untitled" } else { stem };
    format!("{stem}.md")
}

/// Open an existing Markdown file from disk.
pub fn open_notebook(path: &Path) -> Result<LocalMarkdownNotebook, LocalMarkdownError> {
    validate_markdown_path(path)?;
    let hash = content_hash(path)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("notebook file not found: {}", path.display()),
        )
    })?;
    let content = fs::read_to_string(path)?;
    Ok(LocalMarkdownNotebook {
        path: path.to_path_buf(),
        content,
        content_hash: hash,
    })
}

/// First save of an unsaved Notebook to a chosen path.
///
/// Fails with [`LocalMarkdownError::PathCollision`] when the destination already
/// exists. Does not overwrite.
pub fn first_save(
    path: &Path,
    content: &str,
) -> Result<LocalMarkdownNotebook, LocalMarkdownError> {
    validate_markdown_path(path)?;
    if path.exists() {
        return Err(LocalMarkdownError::PathCollision {
            path: path.to_path_buf(),
        });
    }
    let content_hash = atomic_create(path, content.as_bytes())?;
    Ok(LocalMarkdownNotebook {
        path: path.to_path_buf(),
        content: content.to_string(),
        content_hash,
    })
}

/// Atomically replace an existing Notebook file when `expected` still matches.
///
/// Callers that already hold a content hash must pass
/// [`ExpectedContent::Hash`]. Prefer that over [`ExpectedContent::Any`] so
/// external edits are not silently overwritten.
pub fn save_notebook(
    path: &Path,
    content: &str,
    expected: ExpectedContent,
) -> Result<ContentHash, LocalMarkdownError> {
    validate_markdown_path(path)?;
    Ok(atomic_replace(path, content.as_bytes(), expected)?.content_hash)
}

/// Bound save that always conflict-checks against a known hash (fail-closed).
pub fn save_bound(
    path: &Path,
    content: &str,
    expected_hash: ContentHash,
) -> Result<ContentHash, LocalMarkdownError> {
    save_notebook(path, content, ExpectedContent::Hash(expected_hash))
}

/// Save either for the first time (`expected_hash == None` → create) or as an
/// update (`Some(hash)` → hash-checked replace). Never uses `ExpectedContent::Any`.
pub fn save_or_create(
    path: &Path,
    content: &str,
    expected_hash: Option<ContentHash>,
) -> Result<LocalMarkdownNotebook, LocalMarkdownError> {
    match expected_hash {
        None => first_save(path, content),
        Some(hash) => {
            let content_hash = save_bound(path, content, hash)?;
            Ok(LocalMarkdownNotebook {
                path: path.to_path_buf(),
                content: content.to_string(),
                content_hash,
            })
        }
    }
}

#[cfg(test)]
#[path = "local_markdown_tests.rs"]
mod tests;
