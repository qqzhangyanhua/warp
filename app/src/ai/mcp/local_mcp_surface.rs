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

#[cfg(test)]
#[path = "local_mcp_surface_tests.rs"]
mod tests;
