//! Server-backed full-source embeddings and semantic retrieval are removed.
//!
//! ZYH does not upload source fragments, Merkle trees, embedding requests,
//! rerank requests, or index metadata to a hosted service. Local and SSH-side
//! file tree, repository detection, file outline, grep, and ripgrep remain.
//! Agent semantic search is available only through an explicitly configured
//! MCP tool — never as a built-in hosted index.

/// Product flag: hosted semantic codebase indexing is not supported.
pub const HOSTED_SEMANTIC_INDEXING_REMOVED: bool = true;

/// User-facing guidance when an Agent requests built-in semantic code search.
pub const SEMANTIC_SEARCH_REMOVED_GUIDANCE: &str = "Built-in semantic code search is no longer available. \
Use local file search (tree, outline, grep, or ripgrep), SSH-side search on the connected host, \
or an explicitly configured MCP search tool.";

/// Whether the app may create, sync, or query a hosted codebase index.
pub fn may_use_hosted_semantic_indexing() -> bool {
    !HOSTED_SEMANTIC_INDEXING_REMOVED
}

/// Whether Agent tools may perform hosted semantic code search (embedding or server outline retrieval).
pub fn may_use_hosted_semantic_search() -> bool {
    !HOSTED_SEMANTIC_INDEXING_REMOVED
}

/// Stable error message for Agent tool failures.
pub fn semantic_search_unavailable_message() -> String {
    SEMANTIC_SEARCH_REMOVED_GUIDANCE.to_string()
}

#[cfg(test)]
#[path = "semantic_indexing_removal_tests.rs"]
mod tests;
