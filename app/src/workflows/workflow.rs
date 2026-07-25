//! Workflow payload types are owned by [`local_models`] (no cloud GraphQL).
//! Cloud wrappers (`CloudWorkflow`) remain re-exported from `cloud_object_models`
//! until the cloud stack is deleted.

pub use local_models::{Argument, ArgumentType, ObjectRef, Workflow};

