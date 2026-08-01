use std::borrow::Cow;
use std::sync::Arc;

use serde_json::json;
use uuid::Uuid;
use warp_core::features::FeatureFlag;

use super::{PersonalMemoryToolRequest, ResolvedTool, ToolCatalog, ToolRequestError, ToolRoute};
use crate::ai::agent::{MCPContext, MCPServer};
use crate::ai::personal_memory::MemoryCapability;

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

#[test]
fn ordinary_input_has_no_personal_memory_tools() {
    let catalog = ToolCatalog::initial(None).unwrap();

    assert!(catalog
        .entries()
        .iter()
        .all(|entry| !entry.id.starts_with("personal_memory.")));
}

#[test]
fn disabled_personal_memory_flag_hides_tool_catalog_entries() {
    let _flag = FeatureFlag::PersonalMemory.override_enabled(false);
    let user_text = "帮我记住我的 GitHub 帐号是 zyh-work";
    let catalog = ToolCatalog::for_user_input(None, MemoryCapability::derive(user_text)).unwrap();

    assert!(catalog
        .entries()
        .iter()
        .all(|entry| !entry.id.starts_with("personal_memory.")));
}

#[test]
fn explicit_create_exposes_only_create_and_binds_exact_user_spans() {
    let user_text = "帮我记住我的 GitHub 帐号是 zyh-work";
    let _flag = FeatureFlag::PersonalMemory.override_enabled(true);
    let catalog = ToolCatalog::for_user_input(None, MemoryCapability::derive(user_text)).unwrap();
    let memory_entries = catalog
        .entries()
        .iter()
        .filter(|entry| entry.id.starts_with("personal_memory."))
        .collect::<Vec<_>>();

    assert_eq!(memory_entries.len(), 1);
    assert_eq!(memory_entries[0].id, "personal_memory.create");
    let arguments = json!({
        "fact_text": "我的 GitHub 帐号是 zyh-work",
        "value_text": "zyh-work",
        "topic": "GitHub 帐号"
    })
    .as_object()
    .unwrap()
    .clone();
    assert!(matches!(
        catalog
            .resolve(
                "personal_memory.create",
                "remember_personal_fact",
                &arguments,
            )
            .unwrap(),
        ResolvedTool::PersonalMemory(PersonalMemoryToolRequest::Create(_))
    ));

    let altered = json!({
        "fact_text": "我的 GitHub 帐号是 changed-by-model",
        "value_text": "changed-by-model",
        "topic": "GitHub 帐号"
    })
    .as_object()
    .unwrap()
    .clone();
    assert_eq!(
        catalog.resolve("personal_memory.create", "remember_personal_fact", &altered,),
        Err(ToolRequestError::InvalidArguments)
    );
}

#[test]
fn explicit_query_exposes_only_query_and_rejects_unscoped_text() {
    let user_text = "我的 GitHub 帐号记得么？";
    let _flag = FeatureFlag::PersonalMemory.override_enabled(true);
    let catalog = ToolCatalog::for_user_input(None, MemoryCapability::derive(user_text)).unwrap();

    assert!(catalog
        .entries()
        .iter()
        .any(|entry| entry.id == "personal_memory.query"));
    assert!(catalog
        .entries()
        .iter()
        .all(|entry| entry.id != "personal_memory.create"));
    let arguments = json!({ "query_text": "GitHub 帐号" })
        .as_object()
        .unwrap()
        .clone();
    assert!(matches!(
        catalog
            .resolve(
                "personal_memory.query",
                "recall_personal_memory",
                &arguments,
            )
            .unwrap(),
        ResolvedTool::PersonalMemory(PersonalMemoryToolRequest::Query { .. })
    ));
    let unscoped = json!({ "query_text": "all memories" })
        .as_object()
        .unwrap()
        .clone();
    assert_eq!(
        catalog.resolve("personal_memory.query", "recall_personal_memory", &unscoped,),
        Err(ToolRequestError::InvalidArguments)
    );
}
