use std::collections::HashMap;

use super::{
    local_mcp_surface, materialize_local_mcp_servers, LocalMcpTransportKind, McpSettingsCardKind,
};

#[test]
fn local_surface_excludes_gallery_cloud_and_managed_sources() {
    let policy = local_mcp_surface();
    assert!(!policy.allows_gallery());
    assert!(!policy.allows_cloud_objects());
    assert!(!policy.allows_managed_resolution());
    assert!(!policy.managed_resolution_unavailable_message().is_empty());
}

#[test]
fn only_file_based_settings_cards_are_retained() {
    let policy = local_mcp_surface();
    assert!(policy.allows_settings_card(McpSettingsCardKind::FileBased));
    assert!(!policy.allows_settings_card(McpSettingsCardKind::Gallery));
    assert!(!policy.allows_settings_card(McpSettingsCardKind::CloudTemplate));
    assert!(!policy.allows_settings_card(McpSettingsCardKind::CloudInstallation));
}

#[test]
fn materialize_exposes_command_and_remote_servers_for_agent_path() {
    let json = r#"{
      "mcpServers": {
        "filesystem": {
          "command": "npx",
          "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
          "env": { "TOKEN": "${TOKEN}" }
        },
        "remote": {
          "url": "https://mcp.example.com/sse",
          "headers": { "Authorization": "${Authorization}" }
        }
      }
    }"#;
    let secrets = HashMap::from([
        ("filesystem/env/TOKEN".to_owned(), "tok".to_owned()),
        (
            "remote/header/Authorization".to_owned(),
            "Bearer secret".to_owned(),
        ),
    ]);
    let servers = materialize_local_mcp_servers(json, &secrets).unwrap();
    assert_eq!(servers.len(), 2);
    assert_eq!(servers[0].name, "filesystem");
    assert_eq!(
        servers[0].transport,
        LocalMcpTransportKind::Command {
            command: "npx".to_owned()
        }
    );
    assert_eq!(servers[1].name, "remote");
    assert_eq!(
        servers[1].transport,
        LocalMcpTransportKind::Remote {
            mcp_origin: "https://mcp.example.com".to_owned()
        }
    );
}
