//! Cloud wrappers around local MCP payloads.
//!
//! Pure MCP transport/template data lives in [`local_models`]. This module
//! retains `CloudMCPServer` / `CloudTemplatableMCPServer` and `JsonModel`
//! adapters for remaining cloud object stack consumers until those units are
//! deleted (#41).

use cloud_objects::cloud_object::{
    GenericCloudObject, GenericServerObject, GenericStringModel, JsonObjectType,
};
use cloud_objects::ids::GenericStringObjectId;

pub use local_models::{
    CLIServer, FromStoredJsonError, GalleryData, JSONMCPServer, JSONTransportType, JsonTemplate,
    MCPServer, MCPServerState, ServerSentEvents, StaticEnvVar, StaticHeader, TemplatableMCPServer,
    TemplateVariable, TransportType,
};

use crate::{JsonModel, JsonSerializer};

impl JsonModel for MCPServer {
    fn json_object_type() -> JsonObjectType {
        JsonObjectType::MCPServer
    }
}

pub type CloudMCPServer = GenericCloudObject<GenericStringObjectId, CloudMCPServerModel>;
pub type CloudMCPServerModel = GenericStringModel<MCPServer, JsonSerializer>;
pub type ServerMCPServer = GenericServerObject<GenericStringObjectId, CloudMCPServerModel>;

impl JsonModel for TemplatableMCPServer {
    fn json_object_type() -> JsonObjectType {
        JsonObjectType::TemplatableMCPServer
    }
}

pub type CloudTemplatableMCPServer =
    GenericCloudObject<GenericStringObjectId, CloudTemplatableMCPServerModel>;
pub type CloudTemplatableMCPServerModel = GenericStringModel<TemplatableMCPServer, JsonSerializer>;
pub type ServerTemplatableMCPServer =
    GenericServerObject<GenericStringObjectId, CloudTemplatableMCPServerModel>;
