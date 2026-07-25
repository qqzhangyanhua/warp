//! Cloud wrappers around local [`Workflow`] payloads.
//!
//! Pure workflow data lives in [`local_models`]. This module retains
//! `CloudWorkflow` / `WorkflowId` for remaining cloud object stack consumers
//! until those units are deleted (#41).

#[cfg(not(target_family = "wasm"))]
pub mod persistence;

use cloud_objects::cloud_object::{
    GenericCloudObject, GenericServerObject, ObjectType, ServerObjectModel,
};
use cloud_objects::ids::{ClientId, GenericStringObjectId, HashableId, ServerId, SyncId};
use serde::{Deserialize, Serialize};

pub use local_models::{Argument, ArgumentType, ObjectRef, Workflow};

/// Parse a local [`ObjectRef`] into a cloud [`SyncId`] (legacy wire encoding).
pub fn object_ref_to_sync_id(object_ref: &ObjectRef) -> SyncId {
    let s = object_ref.as_str();
    if let Some(client_id) = ClientId::from_hash(s) {
        SyncId::ClientId(client_id)
    } else {
        SyncId::ServerId(ServerId::from_string_lossy(s.to_owned()))
    }
}

/// Encode a cloud [`SyncId`] as a local [`ObjectRef`].
pub fn sync_id_to_object_ref(sync_id: SyncId) -> ObjectRef {
    ObjectRef::new(sync_id.to_string())
}

/// Extension helpers that bridge local workflows to cloud SyncIds.
pub trait WorkflowCloudIds {
    fn get_enum_sync_ids(&self) -> Vec<SyncId>;
    fn get_server_enum_ids(&self) -> Vec<GenericStringObjectId>;
    fn default_env_vars_sync_id(&self) -> Option<SyncId>;
    fn replace_sync_object_id(&mut self, old_id: SyncId, new_id: SyncId) -> bool;
}

impl WorkflowCloudIds for Workflow {
    fn get_enum_sync_ids(&self) -> Vec<SyncId> {
        self.get_enum_ids()
            .iter()
            .map(object_ref_to_sync_id)
            .collect()
    }

    fn get_server_enum_ids(&self) -> Vec<GenericStringObjectId> {
        self.get_enum_sync_ids()
            .into_iter()
            .filter_map(|id| id.into_server())
            .map(Into::into)
            .collect()
    }

    fn default_env_vars_sync_id(&self) -> Option<SyncId> {
        self.default_env_vars().as_ref().map(object_ref_to_sync_id)
    }

    fn replace_sync_object_id(&mut self, old_id: SyncId, new_id: SyncId) -> bool {
        self.replace_object_id(
            &sync_id_to_object_ref(old_id),
            sync_id_to_object_ref(new_id),
        )
    }
}

/// The model for a `CloudWorkflow`.
#[derive(Clone, Debug, PartialEq)]
pub struct CloudWorkflowModel {
    pub data: Workflow,
}

impl CloudWorkflowModel {
    pub fn new(workflow: Workflow) -> Self {
        Self { data: workflow }
    }

    pub fn get_enum_ids(&self) -> Vec<SyncId> {
        self.data.get_enum_sync_ids()
    }
}

impl ServerObjectModel for CloudWorkflowModel {
    fn object_type(&self) -> ObjectType {
        ObjectType::Workflow
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct WorkflowId(ServerId);
cloud_objects::server_id_traits! { WorkflowId, "Workflow" }

/// `CloudWorkflow` is a workflow retrieved from the server.
pub type CloudWorkflow = GenericCloudObject<WorkflowId, CloudWorkflowModel>;
pub type ServerWorkflow = GenericServerObject<WorkflowId, CloudWorkflowModel>;

/// Extract the local workflow payload from a cloud workflow wrapper.
pub fn workflow_from_cloud(cloud_workflow: &CloudWorkflow) -> Workflow {
    cloud_workflow.model().data.clone()
}

#[cfg(test)]
#[path = "workflow_tests.rs"]
mod tests;
