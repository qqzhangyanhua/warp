//! Cloud wrappers around local Agent Rule / memory payloads.
//!
//! Pure [`AIFact`] / [`AIMemory`] data lives in [`local_models`]. This module
//! retains `CloudAIFact` and `JsonModel` adapters for remaining cloud object
//! stack consumers until those units are deleted (#41).

use cloud_objects::cloud_object::{
    GenericCloudObject, GenericServerObject, GenericStringModel, JsonObjectType,
};
use cloud_objects::ids::GenericStringObjectId;

pub use local_models::{AIFact, AIMemory, SuggestedLoggingId};

use crate::{JsonModel, JsonSerializer};

impl JsonModel for AIFact {
    fn json_object_type() -> JsonObjectType {
        JsonObjectType::AIFact
    }
}

pub type CloudAIFact = GenericCloudObject<GenericStringObjectId, CloudAIFactModel>;
pub type CloudAIFactModel = GenericStringModel<AIFact, JsonSerializer>;
pub type ServerAIFact = GenericServerObject<GenericStringObjectId, CloudAIFactModel>;
