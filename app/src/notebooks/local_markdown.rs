//! Local Markdown Notebook lifecycle.
//!
//! Notebooks are ordinary Markdown files on disk. A new Notebook stays unsaved
//! until the user chooses a local path on first save. Subsequent saves use
//! owner-only atomic replacement with content-hash conflict detection. There is
//! no cloud Notebook ID, owner, sharing, revision, or cloud cache.

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

/// Unsaved Notebook that has not yet chosen a path.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnsavedMarkdownNotebook {
    pub content: String,
    pub suggested_title: Option<String>,
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

impl UnsavedMarkdownNotebook {
    /// Create an unsaved Notebook with optional initial Markdown body.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            suggested_title: None,
        }
    }

    /// Create an unsaved Notebook with a suggested title for first-save naming.
    pub fn with_suggested_title(mut self, title: impl Into<String>) -> Self {
        let title = title.into();
        if !title.trim().is_empty() {
            self.suggested_title = Some(title);
        }
        self
    }

    /// Default filename stem for the save picker (no extension).
    pub fn suggested_filename_stem(&self) -> &str {
        self.suggested_title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .unwrap_or("Untitled")
    }

    /// Cancel first save: notebook remains unsaved with current content.
    pub fn cancel_save(self) -> Self {
        self
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
pub fn save_notebook(
    path: &Path,
    content: &str,
    expected: ExpectedContent,
) -> Result<ContentHash, LocalMarkdownError> {
    validate_markdown_path(path)?;
    Ok(atomic_replace(path, content.as_bytes(), expected)?.content_hash)
}

/// Save either for the first time or as an update, depending on whether a path
/// and expected hash are known.
pub fn save_or_create(
    path: &Path,
    content: &str,
    expected_hash: Option<ContentHash>,
) -> Result<LocalMarkdownNotebook, LocalMarkdownError> {
    match expected_hash {
        None => first_save(path, content),
        Some(hash) => {
            let content_hash = save_notebook(path, content, ExpectedContent::Hash(hash))?;
            Ok(LocalMarkdownNotebook {
                path: path.to_path_buf(),
                content: content.to_string(),
                content_hash,
            })
        }
    }
}

/// Resolve a relative link or image path against the Notebook file location.
///
/// Absolute paths, URLs, and data URIs are returned unchanged. Relative paths
/// join the Notebook file's parent directory.
pub fn resolve_relative_path(notebook_path: &Path, reference: &str) -> PathBuf {
    if reference.is_empty() {
        return PathBuf::new();
    }
    if reference.starts_with("http://")
        || reference.starts_with("https://")
        || reference.starts_with("data:")
        || reference.starts_with('/')
        || Path::new(reference).is_absolute()
    {
        return PathBuf::from(reference);
    }
    let base = notebook_path.parent().unwrap_or_else(|| Path::new("."));
    base.join(reference)
}

/// Directory used as the base for relative content resolution.
pub fn relative_base_directory(notebook_path: &Path) -> Option<&Path> {
    notebook_path.parent()
}

/// Suggested default filename for a save dialog from an unsaved Notebook.
pub fn default_save_filename(unsaved: &UnsavedMarkdownNotebook) -> String {
    format!("{}.md", unsaved.suggested_filename_stem())
}

#[cfg(test)]
#[path = "local_markdown_tests.rs"]
mod tests;
