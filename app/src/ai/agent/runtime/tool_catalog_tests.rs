use std::borrow::Cow;
use std::sync::Arc;

use serde_json::json;
use uuid::Uuid;

use super::{ToolCatalog, ToolRoute};
use crate::ai::agent::{MCPContext, MCPServer};

#[test]
#[allow(deprecated)]
fn preserves_configured_mcp_identity_name_schema_and_route() {
    let mut tool = rmcp::model::Tool::default();
    tool.name = Cow::Borrowed("web.search");
    tool.description = Some(Cow::Borrowed("Search project documentation"));
    tool.input_schema = Arc::new(
        json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query"],
            "additionalProperties": false
        })
        .as_object()
        .unwrap()
        .clone(),
    );
    let context = MCPContext {
        resources: vec![],
        tools: vec![],
        servers: vec![MCPServer {
            id: "123e4567-e89b-12d3-a456-426614174000".to_string(),
            name: "docs".to_string(),
            description: String::new(),
            resources: vec![],
            tools: vec![tool],
        }],
    };

    let catalog = ToolCatalog::initial(Some(&context)).unwrap();
    let entry = catalog.entries().last().unwrap();

    assert_eq!(
        entry.id,
        "mcp:123e4567-e89b-12d3-a456-426614174000:web.search"
    );
    assert_eq!(
        entry.name,
        "mcp_123e4567e89b12d3a456426614174000_web_search_75898d89"
    );
    assert_eq!(entry.input_schema["required"], json!(["query"]));
    assert_eq!(
        catalog.route(&entry.id),
        Some(&ToolRoute::Mcp {
            server_id: Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap(),
            tool_name: "web.search".to_string(),
        })
    );
}
