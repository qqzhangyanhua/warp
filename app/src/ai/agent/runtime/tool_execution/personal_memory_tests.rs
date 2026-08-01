use ::ai::agent::PERSONAL_MEMORY_STORE_ID;
use serde_json::json;
use warp_core::features::FeatureFlag;

use super::*;
use crate::ai::personal_memory::{
    CreatePersonalMemoryInput, MemoryCapability, PersonalMemoryService,
};

#[tokio::test]
async fn explicit_memory_create_executes_locally_without_adapter_permission() {
    let user_text = "帮我记住我的 GitHub 帐号是 zyh-work";
    let catalog = {
        let _flag = FeatureFlag::PersonalMemory.override_enabled(true);
        ToolCatalog::for_user_input(None, MemoryCapability::derive(user_text)).unwrap()
    };
    let harness = Harness::new_with_catalog(ToolPermissionDecision::DeniedByPolicy, catalog);
    let mut state = harness.state(0);
    let first = harness
        .authority
        .handle(
            personal_memory_request(
                "call-1",
                "personal_memory.create",
                "remember_personal_fact",
                json!({
                    "fact_text": "我的 GitHub 帐号是 zyh-work",
                    "value_text": "zyh-work",
                    "topic": "GitHub 帐号"
                }),
            ),
            &mut state,
        )
        .await
        .unwrap();
    let redelivered = harness
        .authority
        .handle(
            personal_memory_request(
                "call-1",
                "personal_memory.create",
                "remember_personal_fact",
                json!({
                    "fact_text": "我的 GitHub 帐号是 zyh-work",
                    "value_text": "zyh-work",
                    "topic": "GitHub 帐号"
                }),
            ),
            &mut state,
        )
        .await
        .unwrap();
    let records = PersonalMemoryService::new(harness.writer.sender.clone())
        .list()
        .await
        .unwrap();

    assert_eq!(first.projection, redelivered.projection);
    assert!(projection_text(&first.projection).contains("zyh-work"));
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].value_text, "zyh-work");
    assert_eq!(harness.adapter.permission_count.load(Ordering::SeqCst), 0);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);
    assert_eq!(state.revision, 1);
    harness.finish();
}

#[tokio::test]
async fn explicit_memory_query_commits_verbatim_match_and_source_metadata() {
    let user_text = "我的 GitHub 帐号记得么？";
    let catalog = {
        let _flag = FeatureFlag::PersonalMemory.override_enabled(true);
        ToolCatalog::for_user_input(None, MemoryCapability::derive(user_text)).unwrap()
    };
    let harness = Harness::new_with_catalog(ToolPermissionDecision::DeniedByPolicy, catalog);
    let service = PersonalMemoryService::new(harness.writer.sender.clone());
    service
        .create(CreatePersonalMemoryInput::exact(
            "记住我的 GitHub 帐号是 zyh-work".to_string(),
            "zyh-work".to_string(),
            "GitHub 帐号".to_string(),
        ))
        .await
        .unwrap();
    let mut state = harness.state(0);

    let result = harness
        .authority
        .handle(
            personal_memory_request(
                "call-1",
                "personal_memory.query",
                "recall_personal_memory",
                json!({ "query_text": "GitHub 帐号" }),
            ),
            &mut state,
        )
        .await
        .unwrap();

    assert!(projection_text(&result.projection).contains("zyh-work"));
    let fetched_memories = &state.tasks[0].messages.last().unwrap().fetched_memories;
    assert_eq!(fetched_memories.len(), 1);
    assert_eq!(
        fetched_memories[0].memory_store_id,
        PERSONAL_MEMORY_STORE_ID
    );
    assert_eq!(harness.adapter.permission_count.load(Ordering::SeqCst), 0);
    assert_eq!(harness.adapter.execution_count.load(Ordering::SeqCst), 0);
    harness.finish();
}

fn personal_memory_request(
    tool_call_id: &str,
    tool_id: &str,
    tool_name: &str,
    arguments: serde_json::Value,
) -> RuntimeToolRequest {
    RuntimeToolRequest {
        frame_fingerprint: [2; 32],
        conversation_id: CONVERSATION_ID.to_string(),
        run_id: RUN_ID.to_string(),
        tool_call_id: tool_call_id.to_string(),
        tool_id: tool_id.to_string(),
        tool_name: tool_name.to_string(),
        arguments: arguments.as_object().unwrap().clone(),
    }
}

fn projection_text(projection: &ToolResultProjection) -> &str {
    match projection {
        ToolResultProjection::Success { content, .. } => match &content[0] {
            RuntimeContentBlock::Text { text } => text,
            RuntimeContentBlock::Image { .. } => panic!("memory projection must be text"),
        },
        _ => panic!("memory operation should succeed"),
    }
}
