/// Minimal stub of the former legacy MCP server manager module.
///
/// The only thing still needed from this module is the legacy secure-storage
/// key constant in `oauth`, which is read during the one-time migration of
/// legacy MCP servers to the templatable model.
// TODO(issue #23): Remove legacy managed MCP OAuth credentials.
#[allow(dead_code)]
pub mod oauth;
