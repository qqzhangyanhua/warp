use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::{deserialize_arguments, entry, ToolCatalogEntry, ToolRequestError, ToolRoute};
use crate::ai::personal_memory::{CreatePersonalMemoryInput, MemoryCapability};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::ai::agent::runtime) enum PersonalMemoryToolRequest {
    Create(CreatePersonalMemoryInput),
    Query { query_text: String },
}

#[derive(Deserialize)]
struct PersonalMemoryCreateArgs {
    fact_text: String,
    value_text: String,
    topic: String,
}

#[derive(Deserialize)]
struct PersonalMemoryQueryArgs {
    query_text: String,
}

pub(super) fn catalog_entry(capability: MemoryCapability) -> (ToolCatalogEntry, ToolRoute) {
    match capability {
        MemoryCapability::Create {
            initiating_user_text,
        } => (
            entry(
                "personal_memory.create",
                "remember_personal_fact",
                "Store one fact only because the current user explicitly requested it. Copy fact_text and value_text byte-for-byte from the current user message. Use topic only as a short retrieval key.",
                json!({
                    "type": "object",
                    "properties": {
                        "fact_text": { "type": "string", "minLength": 1, "maxLength": 4096 },
                        "value_text": { "type": "string", "minLength": 1, "maxLength": 2048 },
                        "topic": { "type": "string", "minLength": 1, "maxLength": 512 }
                    },
                    "required": ["fact_text", "value_text", "topic"],
                    "additionalProperties": false
                }),
            ),
            ToolRoute::PersonalMemoryCreate {
                initiating_user_text,
            },
        ),
        MemoryCapability::Query {
            initiating_user_text,
        } => (
            entry(
                "personal_memory.query",
                "recall_personal_memory",
                "Search local Personal Memory only for the fact explicitly requested by the current user. Copy query_text from the current user message. Return stored values verbatim and never guess on no match.",
                json!({
                    "type": "object",
                    "properties": {
                        "query_text": { "type": "string", "minLength": 1, "maxLength": 512 }
                    },
                    "required": ["query_text"],
                    "additionalProperties": false
                }),
            ),
            ToolRoute::PersonalMemoryQuery {
                initiating_user_text,
            },
        ),
    }
}

pub(super) fn create_tool(
    initiating_user_text: &str,
    arguments: &Map<String, Value>,
) -> Result<PersonalMemoryToolRequest, ToolRequestError> {
    let args: PersonalMemoryCreateArgs = deserialize_arguments(arguments)?;
    if !initiating_user_text.contains(&args.fact_text)
        || !initiating_user_text.contains(&args.value_text)
    {
        return Err(ToolRequestError::InvalidArguments);
    }
    Ok(PersonalMemoryToolRequest::Create(
        CreatePersonalMemoryInput::exact(args.fact_text, args.value_text, args.topic),
    ))
}

pub(super) fn query_tool(
    initiating_user_text: &str,
    arguments: &Map<String, Value>,
) -> Result<PersonalMemoryToolRequest, ToolRequestError> {
    let args: PersonalMemoryQueryArgs = deserialize_arguments(arguments)?;
    if !initiating_user_text.contains(&args.query_text) {
        return Err(ToolRequestError::InvalidArguments);
    }
    Ok(PersonalMemoryToolRequest::Query {
        query_text: args.query_text,
    })
}
