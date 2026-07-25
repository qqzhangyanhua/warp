//! ZYH-owned local data models.
//!
//! These types intentionally do not depend on cloud ownership, GraphQL, server
//! auth, or hosted-service crates. Wire formats preserve legacy SyncId string
//! encodings so YAML/JSON on disk remains readable.

pub mod workflow;

pub use workflow::{Argument, ArgumentType, ObjectRef, Workflow};
