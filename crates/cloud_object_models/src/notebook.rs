//! Cloud wrappers around local Notebook payloads.
//!
//! Pure [`Notebook`] / [`SerializedNotebook`] data lives in [`local_models`].
//! This module retains `CloudNotebookModel` (with typed `AIDocumentId`),
//! `NotebookId`, and `CloudNotebook` aliases for remaining cloud object stack
//! consumers until those units are deleted (#41).

#[cfg(not(target_family = "wasm"))]
pub mod persistence;

use ai::document::AIDocumentId;
use anyhow::{Context, Result};
use cloud_objects::cloud_object::{
    GenericCloudObject, GenericServerObject, ObjectType, ServerObjectModel,
};
use cloud_objects::ids::{ServerId, SyncId};
use serde::{Deserialize, Serialize};

pub use local_models::{Notebook, SerializedNotebook};

/// Cloud object model for a notebook. Field layout matches [`Notebook`]; the
/// document id stays typed as [`AIDocumentId`] for app call sites.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CloudNotebookModel {
    pub title: String,
    pub data: String,
    pub ai_document_id: Option<AIDocumentId>,
    /// This is the server-generated conversation token, not the client-side AIConversationId.
    pub conversation_id: Option<String>,
}

impl CloudNotebookModel {
    /// Convert to the pure local notebook payload (string document ids).
    pub fn to_local(&self) -> Notebook {
        Notebook {
            title: self.title.clone(),
            data: self.data.clone(),
            ai_document_id: self.ai_document_id.as_ref().map(|id| id.to_string()),
            conversation_id: self.conversation_id.clone(),
        }
    }

    /// Build a cloud model from a local notebook payload.
    pub fn try_from_local(notebook: Notebook) -> Result<Self> {
        let ai_document_id = notebook
            .ai_document_id
            .map(AIDocumentId::try_from)
            .transpose()
            .context("invalid ai_document_id on local notebook")?;
        Ok(Self {
            title: notebook.title,
            data: notebook.data,
            ai_document_id,
            conversation_id: notebook.conversation_id,
        })
    }

    /// Sync-queue / create-object wire form (body + link ids only).
    pub fn to_serialized(&self) -> SerializedNotebook {
        self.to_local().to_serialized()
    }
}

impl From<&CloudNotebookModel> for Notebook {
    fn from(model: &CloudNotebookModel) -> Self {
        model.to_local()
    }
}

impl From<CloudNotebookModel> for Notebook {
    fn from(model: CloudNotebookModel) -> Self {
        model.to_local()
    }
}

impl TryFrom<Notebook> for CloudNotebookModel {
    type Error = anyhow::Error;

    fn try_from(notebook: Notebook) -> Result<Self> {
        Self::try_from_local(notebook)
    }
}

impl ServerObjectModel for CloudNotebookModel {
    fn object_type(&self) -> ObjectType {
        ObjectType::Notebook
    }
}

/// This is the notebook_id in the database associated with this notebook.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct NotebookId(ServerId);
cloud_objects::server_id_traits! { NotebookId, "Notebook" }

impl From<NotebookId> for SyncId {
    fn from(id: NotebookId) -> Self {
        Self::ServerId(id.into())
    }
}

/// `CloudNotebook` is a notebook retrieved from the server.
pub type CloudNotebook = GenericCloudObject<NotebookId, CloudNotebookModel>;
pub type ServerNotebook = GenericServerObject<NotebookId, CloudNotebookModel>;
