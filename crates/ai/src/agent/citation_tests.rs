use super::{AIAgentCitation, PERSONAL_MEMORY_STORE_ID};

#[test]
fn local_personal_memory_store_uses_distinct_citation() {
    let citation = AIAgentCitation::from_fetched_memory(
        PERSONAL_MEMORY_STORE_ID.to_string(),
        "record-1".to_string(),
        "my account is zyh-work".to_string(),
    );

    assert_eq!(
        citation,
        AIAgentCitation::PersonalMemory {
            record_id: "record-1".to_string(),
            content: "my account is zyh-work".to_string(),
        }
    );
}

#[test]
fn other_memory_stores_keep_remote_agent_memory_behavior() {
    let citation = AIAgentCitation::from_fetched_memory(
        "remote-store".to_string(),
        "memory-1".to_string(),
        "content".to_string(),
    );

    assert_eq!(
        citation,
        AIAgentCitation::AgentMemory {
            memory_store_id: "remote-store".to_string(),
            memory_id: "memory-1".to_string(),
            content: "content".to_string(),
        }
    );
}
