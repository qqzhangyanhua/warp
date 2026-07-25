//! ZYH-owned local data models.
//!
//! These types intentionally do not depend on cloud ownership, GraphQL, server
//! auth, or hosted-service crates. Wire formats preserve legacy SyncId string
//! encodings so YAML/JSON on disk remains readable.

pub mod ai_fact;
pub mod mcp;
pub mod workflow;

pub use ai_fact::{AIFact, AIMemory, SuggestedLoggingId};
pub use mcp::{
    CLIServer, FromStoredJsonError, GalleryData, JSONMCPServer, JSONTransportType, JsonTemplate,
    MCPServer, MCPServerState, ServerSentEvents, StaticEnvVar, StaticHeader, TemplatableMCPServer,
    TemplateVariable, TransportType,
};
pub use workflow::{Argument, ArgumentType, ObjectRef, Workflow};
