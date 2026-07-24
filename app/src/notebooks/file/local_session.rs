//! Local Markdown Notebook session state machine.
//!
//! Owns unsaved / bound / dirty / conflict transitions for a file-backed
//! Notebook. Disk IO stays in [`crate::notebooks::local_markdown`]; this module
//! only decides whether a save is allowed and which content hash to expect.

use std::path::{Path, PathBuf};

use warpui_extras::owner_only_file::{ContentHash, ExpectedContent};

/// Edit/conflict status for a path-bound Notebook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditState {
    Clean,
    Dirty,
    Conflict,
}

/// In-memory lifecycle for one local Markdown Notebook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalNotebookSession {
    /// No path chosen yet. First save must pick a Markdown path.
    Unsaved { title: String },
    /// Bound to a local Markdown path with a known content hash.
    Bound {
        path: PathBuf,
        content_hash: ContentHash,
        edit: EditState,
    },
}

/// What the session requires before a write may proceed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavePlan {
    /// User must choose a path (first save / save-as of unsaved).
    NeedsPath,
    /// Write to `path` only when `expected` still matches disk.
    Write {
        path: PathBuf,
        expected: ExpectedContent,
    },
    /// External conflict: refuse to write until refresh.
    BlockedByConflict { path: PathBuf },
}

impl LocalNotebookSession {
    /// New Notebook with no path. Title is for display and save-picker default.
    pub fn new_unsaved(title: impl Into<String>) -> Self {
        let title = title.into();
        let title = if title.trim().is_empty() {
            "Untitled".to_string()
        } else {
            title
        };
        Self::Unsaved { title }
    }

    /// Session after a successful disk open or first save.
    pub fn bound(path: impl Into<PathBuf>, content_hash: ContentHash) -> Self {
        Self::Bound {
            path: path.into(),
            content_hash,
            edit: EditState::Clean,
        }
    }

    pub fn is_unsaved(&self) -> bool {
        matches!(self, Self::Unsaved { .. })
    }

    pub fn is_dirty(&self) -> bool {
        matches!(
            self,
            Self::Bound {
                edit: EditState::Dirty,
                ..
            }
        )
    }

    pub fn has_conflict(&self) -> bool {
        matches!(
            self,
            Self::Bound {
                edit: EditState::Conflict,
                ..
            }
        )
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Unsaved { .. } => None,
            Self::Bound { path, .. } => Some(path.as_path()),
        }
    }

    pub fn content_hash(&self) -> Option<ContentHash> {
        match self {
            Self::Unsaved { .. } => None,
            Self::Bound { content_hash, .. } => Some(*content_hash),
        }
    }

    pub fn title_hint(&self) -> &str {
        match self {
            Self::Unsaved { title } => title.as_str(),
            Self::Bound { path, .. } => path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Untitled"),
        }
    }

    /// Parent directory used as the base for relative links and images.
    pub fn document_path_for_relative_content(&self) -> Option<&Path> {
        self.path()
    }

    /// User edited the in-memory buffer.
    pub fn mark_edited(&mut self) {
        match self {
            Self::Unsaved { .. } => {}
            Self::Bound {
                edit: edit @ (EditState::Clean | EditState::Dirty),
                ..
            } => {
                *edit = EditState::Dirty;
            }
            Self::Bound {
                edit: EditState::Conflict,
                ..
            } => {
                // Stay in conflict until the user refreshes; further edits remain local.
            }
        }
    }

    /// Decide how a save request should proceed. Does not touch the filesystem.
    pub fn save_plan(&self) -> SavePlan {
        match self {
            Self::Unsaved { .. } => SavePlan::NeedsPath,
            Self::Bound {
                path,
                edit: EditState::Conflict,
                ..
            } => SavePlan::BlockedByConflict {
                path: path.clone(),
            },
            Self::Bound {
                path,
                content_hash,
                edit: EditState::Clean | EditState::Dirty,
            } => SavePlan::Write {
                path: path.clone(),
                // Fail-closed: always require the last known hash. Never Any.
                expected: ExpectedContent::Hash(*content_hash),
            },
        }
    }

    /// Apply a successful create or replace. Leaves the session Bound and Clean.
    pub fn apply_save_ok(&mut self, path: impl Into<PathBuf>, content_hash: ContentHash) {
        *self = Self::Bound {
            path: path.into(),
            content_hash,
            edit: EditState::Clean,
        };
    }

    /// A write was rejected because disk no longer matches the expected hash.
    pub fn apply_save_conflict(&mut self) {
        if let Self::Bound { edit, .. } = self {
            *edit = EditState::Conflict;
        }
    }

    /// File watcher reported on-disk content.
    ///
    /// - Same hash: ignore.
    /// - Clean session: accept new hash (caller reloads buffer).
    /// - Dirty or already conflict: enter Conflict (caller must not clobber buffer).
    pub fn apply_external_update(&mut self, disk_hash: ContentHash) -> ExternalUpdateOutcome {
        match self {
            Self::Unsaved { .. } => ExternalUpdateOutcome::Ignore,
            Self::Bound {
                content_hash,
                edit: EditState::Clean,
                ..
            } if *content_hash == disk_hash => ExternalUpdateOutcome::Ignore,
            Self::Bound {
                content_hash,
                edit: EditState::Clean,
                ..
            } => {
                *content_hash = disk_hash;
                ExternalUpdateOutcome::AcceptReload
            }
            Self::Bound {
                content_hash,
                edit,
                ..
            } => {
                if *content_hash == disk_hash && *edit == EditState::Dirty {
                    // Our own save may race with the watcher; matching hash clears dirty.
                    *edit = EditState::Clean;
                    return ExternalUpdateOutcome::Ignore;
                }
                *edit = EditState::Conflict;
                ExternalUpdateOutcome::Conflict
            }
        }
    }

    /// User accepted disk version after a conflict (or explicit reload).
    pub fn apply_refresh(&mut self, content_hash: ContentHash) {
        if let Self::Bound { content_hash: h, edit, .. } = self {
            *h = content_hash;
            *edit = EditState::Clean;
        }
    }
}

/// How the UI should react to an external file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalUpdateOutcome {
    Ignore,
    AcceptReload,
    Conflict,
}

#[cfg(test)]
#[path = "local_session_tests.rs"]
mod tests;
