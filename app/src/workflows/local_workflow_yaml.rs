//! File-backed local Workflow YAML lifecycle.
//!
//! User Workflows live under the ZYH home `workflows/` directory. Project
//! Workflows live under the ZYH project directory's `workflows/` directory.
//! Create, save, rename, and delete use owner-only atomic writes with content-hash
//! conflict detection. Filename collisions are reported without overwriting.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use warpui_extras::owner_only_file::{
    atomic_create, atomic_replace, content_hash, ContentHash, ExpectedContent, OwnerOnlyFileError,
};

use super::workflow::Workflow;
use crate::workflows::local_workflows::workflows_dir;

/// Scope for a local Workflow YAML file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalWorkflowScope {
    /// User-level Workflows under the ZYH application home.
    User { home_data_dir: PathBuf },
    /// Project-level Workflows under a repository's ZYH project directory.
    Project { project_root: PathBuf },
}

/// In-memory record of a local Workflow YAML file.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalWorkflowEntry {
    pub path: PathBuf,
    pub workflow: Workflow,
    pub content_hash: ContentHash,
}

/// Errors from loading or mutating local Workflow YAML files.
#[derive(Debug, Error)]
pub enum LocalWorkflowYamlError {
    #[error("workflow filename is empty or invalid after sanitization")]
    InvalidFilename,
    #[error("a workflow file already exists at {path}")]
    FilenameCollision { path: PathBuf },
    #[error("the workflow file changed before it could be written: {path}")]
    Conflict { path: PathBuf },
    #[error("refusing to operate on a symlink or non-regular file: {path}")]
    UnsupportedFileType { path: PathBuf },
    #[error("invalid workflow YAML at {path}: {message}")]
    InvalidYaml { path: PathBuf, message: String },
    #[error("workflow file operation failed: {0}")]
    Io(#[from] io::Error),
}

impl From<OwnerOnlyFileError> for LocalWorkflowYamlError {
    fn from(error: OwnerOnlyFileError) -> Self {
        match error {
            OwnerOnlyFileError::Io(error) => Self::Io(error),
            OwnerOnlyFileError::Conflict { path } => Self::Conflict { path },
            OwnerOnlyFileError::UnsupportedFileType { path } => Self::UnsupportedFileType { path },
        }
    }
}

impl LocalWorkflowScope {
    /// Directory that holds YAML Workflow files for this scope.
    pub fn directory(&self) -> PathBuf {
        match self {
            Self::User { home_data_dir } => workflows_dir(home_data_dir),
            Self::Project { project_root } => {
                workflows_dir(project_root.join(warp_core::paths::ZYH_PROJECT_CONFIG_DIR))
            }
        }
    }

    /// User scope rooted at the current ZYH data directory.
    pub fn user() -> Self {
        Self::User {
            home_data_dir: warp_core::paths::data_dir(),
        }
    }

    /// Project scope rooted at `project_root` (repository workdir).
    pub fn project(project_root: impl Into<PathBuf>) -> Self {
        Self::Project {
            project_root: project_root.into(),
        }
    }
}

/// Sanitize a Workflow name into a YAML filename stem.
///
/// Preserves ASCII letters, digits, hyphens, and underscores. Other characters
/// become underscores. Empty results are rejected by callers via
/// [`LocalWorkflowYamlError::InvalidFilename`].
pub fn sanitize_workflow_filename_stem(name: &str) -> String {
    let mut sanitized = String::with_capacity(name.len());
    let mut last_was_underscore = false;

    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            sanitized.push(c);
            last_was_underscore = c == '_';
        } else if !last_was_underscore && !sanitized.is_empty() {
            sanitized.push('_');
            last_was_underscore = true;
        }
    }

    sanitized.trim_matches('_').to_string()
}

/// Absolute path for a new Workflow file from a display name.
pub fn path_for_workflow_name(directory: &Path, name: &str) -> Result<PathBuf, LocalWorkflowYamlError> {
    let stem = sanitize_workflow_filename_stem(name);
    if stem.is_empty() {
        return Err(LocalWorkflowYamlError::InvalidFilename);
    }
    Ok(directory.join(format!("{stem}.yaml")))
}

/// Load every single-document Workflow YAML under `directory`.
///
/// Multi-document files are supported for discovery: each document becomes an
/// entry sharing the same path and content hash. Create/save always write a
/// single document per file.
pub fn list_workflows(directory: &Path) -> Result<Vec<LocalWorkflowEntry>, LocalWorkflowYamlError> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    if !directory.is_dir() {
        return Err(LocalWorkflowYamlError::UnsupportedFileType {
            path: directory.to_path_buf(),
        });
    }

    let mut entries = Vec::new();
    let mut paths: Vec<PathBuf> = fs::read_dir(directory)?
        .filter_map(|item| item.ok())
        .map(|item| item.path())
        .filter(|path| is_yaml_file(path))
        .collect();
    paths.sort();

    for path in paths {
        match load_workflows_from_file(&path) {
            Ok(file_entries) => entries.extend(file_entries),
            Err(LocalWorkflowYamlError::InvalidYaml { path, message }) => {
                log::warn!("Skipping invalid workflow YAML at {}: {message}", path.display());
            }
            Err(error) => return Err(error),
        }
    }
    Ok(entries)
}

/// Load Workflow documents from a single YAML file.
pub fn load_workflows_from_file(
    path: &Path,
) -> Result<Vec<LocalWorkflowEntry>, LocalWorkflowYamlError> {
    let hash = content_hash(path)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("workflow file not found: {}", path.display()),
        )
    })?;
    let bytes = fs::read(path)?;
    let workflows = parse_workflow_yaml(&bytes).map_err(|message| {
        LocalWorkflowYamlError::InvalidYaml {
            path: path.to_path_buf(),
            message,
        }
    })?;

    Ok(workflows
        .into_iter()
        .map(|workflow| LocalWorkflowEntry {
            path: path.to_path_buf(),
            workflow,
            content_hash: hash,
        })
        .collect())
}

/// Load the first Workflow document from `path`.
pub fn load_workflow(path: &Path) -> Result<LocalWorkflowEntry, LocalWorkflowYamlError> {
    let mut entries = load_workflows_from_file(path)?;
    if entries.is_empty() {
        Err(LocalWorkflowYamlError::InvalidYaml {
            path: path.to_path_buf(),
            message: "YAML file contains no workflow documents".to_string(),
        })
    } else {
        Ok(entries.remove(0))
    }
}

/// Create a new single-document Workflow file. Fails if the target path exists.
pub fn create_workflow(
    directory: &Path,
    workflow: &Workflow,
) -> Result<LocalWorkflowEntry, LocalWorkflowYamlError> {
    let path = path_for_workflow_name(directory, workflow.name())?;
    if path.exists() {
        return Err(LocalWorkflowYamlError::FilenameCollision { path });
    }

    let yaml = serialize_workflow_yaml(workflow)?;
    let content_hash = atomic_create(&path, yaml.as_bytes())?;
    Ok(LocalWorkflowEntry {
        path,
        workflow: workflow.clone(),
        content_hash,
    })
}

/// Atomically replace an existing Workflow file when `expected` still matches.
pub fn save_workflow(
    path: &Path,
    workflow: &Workflow,
    expected: ExpectedContent,
) -> Result<ContentHash, LocalWorkflowYamlError> {
    let yaml = serialize_workflow_yaml(workflow)?;
    Ok(atomic_replace(path, yaml.as_bytes(), expected)?.content_hash)
}

/// Rename a Workflow file to a stem derived from `new_name`.
///
/// Reports [`LocalWorkflowYamlError::FilenameCollision`] when the destination
/// exists and is not the source path. Content is conflict-checked before rename.
pub fn rename_workflow(
    path: &Path,
    new_name: &str,
    expected: ExpectedContent,
) -> Result<LocalWorkflowEntry, LocalWorkflowYamlError> {
    let directory = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "workflow path must have a parent directory",
        )
    })?;
    let destination = path_for_workflow_name(directory, new_name)?;

    let actual = content_hash(path)?;
    if !expected_matches(expected, actual) {
        return Err(LocalWorkflowYamlError::Conflict {
            path: path.to_path_buf(),
        });
    }

    if destination != path {
        if destination.exists() {
            return Err(LocalWorkflowYamlError::FilenameCollision {
                path: destination,
            });
        }
        fs::rename(path, &destination)?;
    }

    let mut entry = load_workflow(&destination)?;
    // Keep the workflow display name in sync with the requested name when the
    // caller is renaming via the editor name field.
    if entry.workflow.name() != new_name {
        entry.workflow.set_name(new_name);
        entry.content_hash = save_workflow(
            &destination,
            &entry.workflow,
            ExpectedContent::Hash(entry.content_hash),
        )?;
    }
    Ok(entry)
}

/// Delete a Workflow file when `expected` still matches.
pub fn delete_workflow(path: &Path, expected: ExpectedContent) -> Result<(), LocalWorkflowYamlError> {
    let actual = content_hash(path)?;
    if !expected_matches(expected, actual) {
        return Err(LocalWorkflowYamlError::Conflict {
            path: path.to_path_buf(),
        });
    }

    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Serialize a Workflow to YAML bytes suitable for a single-document file.
pub fn serialize_workflow_yaml(workflow: &Workflow) -> Result<String, LocalWorkflowYamlError> {
    serde_yaml::to_string(workflow).map_err(|error| LocalWorkflowYamlError::InvalidYaml {
        path: PathBuf::from("<serialize>"),
        message: error.to_string(),
    })
}

fn parse_workflow_yaml(bytes: &[u8]) -> Result<Vec<Workflow>, String> {
    let mut workflows = Vec::new();
    for document in serde_yaml::Deserializer::from_slice(bytes) {
        let workflow = Workflow::deserialize(document).map_err(|error| error.to_string())?;
        workflows.push(workflow);
    }
    Ok(workflows)
}

fn is_yaml_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
}

fn expected_matches(expected: ExpectedContent, actual: Option<ContentHash>) -> bool {
    match expected {
        ExpectedContent::Any => true,
        ExpectedContent::Missing => actual.is_none(),
        ExpectedContent::Hash(expected_hash) => actual == Some(expected_hash),
    }
}

// Workflow::deserialize is used via serde; import the trait.
use serde::Deserialize as _;

#[cfg(test)]
#[path = "local_workflow_yaml_tests.rs"]
mod tests;
