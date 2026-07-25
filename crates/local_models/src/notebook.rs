//! Local Notebook document payloads.
//!
//! These types intentionally do not depend on cloud ownership, GraphQL, server
//! auth, or hosted-service crates. Document and conversation ids are opaque
//! strings so on-disk Markdown / JSON and the historical sync-queue wire form
//! round-trip without cloud UUID wrappers. Cloud object wrappers live in
//! `cloud_object_models` until that stack is deleted (#41).
//!
//! File-backed Notebooks use path + Markdown body elsewhere; this module owns
//! the shared title/body/id payload shape used when a notebook still flows
//! through the transitional cloud model.

use serde::{Deserialize, Serialize};

/// Local notebook document payload (title + body + optional link ids).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notebook {
    pub title: String,
    pub data: String,
    /// Optional linked AI document id (legacy `AIDocumentId` Display/UUID form).
    #[serde(default)]
    pub ai_document_id: Option<String>,
    /// Optional server-generated conversation token (legacy cloud field).
    #[serde(default)]
    pub conversation_id: Option<String>,
}

/// Serialized notebook body for create/update wire paths and sync queue items.
///
/// Title is carried separately on create requests; this payload holds body and
/// optional link ids only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerializedNotebook {
    pub data: String,
    #[serde(default)]
    pub ai_document_id: Option<String>,
    #[serde(default)]
    pub conversation_id: Option<String>,
}

impl Notebook {
    pub fn new(title: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            data: data.into(),
            ai_document_id: None,
            conversation_id: None,
        }
    }

    pub fn with_ai_document_id(mut self, id: impl Into<String>) -> Self {
        self.ai_document_id = Some(id.into());
        self
    }

    pub fn with_conversation_id(mut self, id: impl Into<String>) -> Self {
        self.conversation_id = Some(id.into());
        self
    }

    /// Whether this notebook is linked to a plan / AI document.
    pub fn is_plan(&self) -> bool {
        self.ai_document_id.is_some()
    }

    /// Wire form used by create-object / sync-queue serialization.
    pub fn to_serialized(&self) -> SerializedNotebook {
        SerializedNotebook {
            data: self.data.clone(),
            ai_document_id: self.ai_document_id.clone(),
            conversation_id: self.conversation_id.clone(),
        }
    }

    /// Rebuild a full notebook from a title plus the body-only wire form.
    pub fn from_serialized(title: impl Into<String>, serialized: SerializedNotebook) -> Self {
        Self {
            title: title.into(),
            data: serialized.data,
            ai_document_id: serialized.ai_document_id,
            conversation_id: serialized.conversation_id,
        }
    }
}

#[cfg(test)]
#[path = "notebook_tests.rs"]
mod tests;
