//! Retained ZYH local MCP product surface.
//!
//! Encodes which MCP configuration and resolution sources remain after the
//! permanent local product cut (ADR-0009 / issue #29). Settings UI and managed
//! resolution call into this policy instead of scattering product flags.

/// Kind of MCP entry the settings list might show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpSettingsCardKind {
    /// Discovered from ZYH or third-party local config files.
    FileBased,
    /// Cloud gallery catalog item.
    Gallery,
    /// Cloud-synced templatable MCP template.
    CloudTemplate,
    /// Cloud-synced templatable installation.
    CloudInstallation,
}

/// Policy for the retained local MCP path.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalMcpSurfacePolicy;

impl LocalMcpSurfacePolicy {
    /// Gallery catalog is not part of the local product path.
    pub fn allows_gallery(self) -> bool {
        false
    }

    /// Cloud template / installation objects are not part of the local path.
    pub fn allows_cloud_objects(self) -> bool {
        false
    }

    /// Server-side managed MCP resolution and proxy tokens are not available.
    pub fn allows_managed_resolution(self) -> bool {
        false
    }

    /// Whether a settings card kind may appear on the retained surface.
    pub fn allows_settings_card(self, kind: McpSettingsCardKind) -> bool {
        match kind {
            McpSettingsCardKind::FileBased => true,
            McpSettingsCardKind::Gallery
            | McpSettingsCardKind::CloudTemplate
            | McpSettingsCardKind::CloudInstallation => false,
        }
    }

    /// Error message when managed MCP resolution is attempted.
    pub fn managed_resolution_unavailable_message(self) -> &'static str {
        "Managed MCP resolution is not available in the ZYH local product"
    }
}

/// Process-wide policy instance used by settings and resolution call sites.
pub fn local_mcp_surface() -> LocalMcpSurfacePolicy {
    LocalMcpSurfacePolicy
}

/// Transport kind exposed to the Agent after local config materialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalMcpTransportKind {
    /// stdio / command MCP server.
    Command { command: String },
    /// Remote MCP over HTTP/SSE; credentials may only target this origin.
    Remote { mcp_origin: String },
}

/// One server ready for the retained local MCP path (config → typed install).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedLocalMcpServer {
    pub name: String,
    pub transport: LocalMcpTransportKind,
}

/// Materialize local `.mcp.json` text into typed servers for the Agent surface.
///
/// Resolves `${…}` placeholders via environment and the provided secrets map,
/// then parses through the same config-file JSON path used at spawn time.
pub fn materialize_local_mcp_servers(
    json: &str,
    secrets: &std::collections::HashMap<String, String>,
) -> Result<Vec<MaterializedLocalMcpServer>, super::local_mcp_config::LocalMcpConfigError> {
    use super::local_mcp_config::{
        parse_servers_from_user_json, remote_mcp_credential_target, resolve_placeholders,
    };

    let resolved = resolve_placeholders(json, secrets)?;
    let servers = parse_servers_from_user_json(&resolved)?;
    let mut out = Vec::with_capacity(servers.len());
    for (name, server) in servers {
        let transport = if let Some(target) = remote_mcp_credential_target(&server)? {
            LocalMcpTransportKind::Remote {
                mcp_origin: target.mcp_origin,
            }
        } else if let Some(command) = server.get("command").and_then(|v| v.as_str()) {
            LocalMcpTransportKind::Command {
                command: command.to_owned(),
            }
        } else {
            continue;
        };
        out.push(MaterializedLocalMcpServer { name, transport });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[cfg(test)]
#[path = "local_mcp_surface_tests.rs"]
mod tests;
