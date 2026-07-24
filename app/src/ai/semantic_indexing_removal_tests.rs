use super::{
    may_use_hosted_semantic_indexing, may_use_hosted_semantic_search,
    semantic_search_unavailable_message, HOSTED_SEMANTIC_INDEXING_REMOVED,
    SEMANTIC_SEARCH_REMOVED_GUIDANCE,
};

#[test]
fn hosted_semantic_indexing_is_removed() {
    assert!(HOSTED_SEMANTIC_INDEXING_REMOVED);
    assert!(!may_use_hosted_semantic_indexing());
    assert!(!may_use_hosted_semantic_search());
}

#[test]
fn guidance_points_to_local_search_and_mcp_not_a_vector_store() {
    let message = semantic_search_unavailable_message();
    assert_eq!(message, SEMANTIC_SEARCH_REMOVED_GUIDANCE);
    assert!(message.contains("grep") || message.contains("ripgrep") || message.contains("outline"));
    assert!(message.contains("MCP"));
    // Must not advertise a local vector index replacement.
    assert!(!message.to_ascii_lowercase().contains("vector"));
    assert!(!message.to_ascii_lowercase().contains("embedding store"));
}

#[test]
fn guidance_does_not_mention_warp_cloud_upload() {
    let message = semantic_search_unavailable_message().to_ascii_lowercase();
    assert!(!message.contains("upload"));
    assert!(!message.contains("quota"));
}
