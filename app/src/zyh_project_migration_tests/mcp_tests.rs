use std::fs;
use std::path::Path;

use super::super::{
    execute_project_migration, preview_project_migration, MigrationOmissionReason,
    MigrationResultStatus, PreviewStatus,
};
use super::write;

#[test]
fn mcp_migration_preserves_local_configuration_without_secret_values_or_cloud_references() {
    let tempdir = tempfile::tempdir().unwrap();
    git2::Repository::init(tempdir.path()).unwrap();
    write(
        &tempdir.path().join(".warp/.mcp.json"),
        br#"{
  "mcpServers": {
    "local": {
      "command": "node",
      "args": ["server.js"],
      "env": {
        "TOKEN": "literal-secret",
        "FROM_ENV": "${FROM_ENV}"
      },
      "cloudId": "cloud-object-123"
    },
    "remote": {
      "url": "https://mcp.example.test/run?token=url-secret",
      "headers": {
        "Authorization": "Bearer literal-secret",
        "X-From-Env": "${MCP_HEADER}"
      }
    }
  }
}"#,
    );

    let preview = preview_project_migration(tempdir.path()).unwrap();
    let mcp_entry = preview
        .entries
        .iter()
        .find(|entry| entry.source == Path::new(".warp/.mcp.json"))
        .unwrap();
    assert_eq!(mcp_entry.status, PreviewStatus::Ready);
    assert_eq!(
        mcp_entry.destination.as_deref(),
        Some(Path::new(".zyh/.mcp.json"))
    );
    for path in [
        "mcpServers.local.env.TOKEN",
        "mcpServers.local.cloudId",
        "mcpServers.remote.headers.Authorization",
    ] {
        assert!(mcp_entry.omissions.iter().any(|omission| {
            omission.path == path && omission.reason == MigrationOmissionReason::SensitiveValue
        }));
    }

    let result = execute_project_migration(preview);
    assert!(result.entries.iter().any(|entry| {
        entry.source == Path::new(".warp/.mcp.json")
            && entry.status == MigrationResultStatus::Copied
    }));
    let migrated = fs::read_to_string(tempdir.path().join(".zyh/.mcp.json")).unwrap();
    assert!(!migrated.contains("literal-secret"));
    assert!(!migrated.contains("url-secret"));
    assert!(!migrated.contains("cloud-object-123"));
    let migrated: serde_json::Value = serde_json::from_str(&migrated).unwrap();
    assert_eq!(
        migrated["mcpServers"]["local"]["env"]["FROM_ENV"],
        "${FROM_ENV}"
    );
    assert_eq!(
        migrated["mcpServers"]["remote"]["headers"]["X-From-Env"],
        "${MCP_HEADER}"
    );
    assert_eq!(
        migrated["mcpServers"]["remote"]["url"],
        "https://mcp.example.test/run"
    );
}

#[test]
fn mcp_server_with_credential_argument_is_omitted_without_leaking_its_value() {
    let tempdir = tempfile::tempdir().unwrap();
    git2::Repository::init(tempdir.path()).unwrap();
    write(
        &tempdir.path().join(".warp/.mcp.json"),
        br#"{
  "mcpServers": {
    "unsafe": {
      "command": "server",
      "args": ["--token", "argument-secret"]
    },
    "safe": {
      "command": "safe-server",
      "args": ["--stdio"]
    }
  }
}"#,
    );

    let preview = preview_project_migration(tempdir.path()).unwrap();
    let mcp_entry = preview
        .entries
        .iter()
        .find(|entry| entry.source == Path::new(".warp/.mcp.json"))
        .unwrap();
    assert!(mcp_entry.omissions.iter().any(|omission| {
        omission.path == "mcpServers.unsafe.args"
            && omission.reason == MigrationOmissionReason::SensitiveValue
    }));
    assert!(!format!("{mcp_entry:?}").contains("argument-secret"));

    execute_project_migration(preview);
    let migrated = fs::read_to_string(tempdir.path().join(".zyh/.mcp.json")).unwrap();
    assert!(!migrated.contains("argument-secret"));
    let migrated: serde_json::Value = serde_json::from_str(&migrated).unwrap();
    assert!(migrated["mcpServers"].get("unsafe").is_none());
    assert_eq!(migrated["mcpServers"]["safe"]["command"], "safe-server");
}

#[test]
fn mcp_argument_secrets_and_cloud_references_omit_servers_and_preserve_transport() {
    let tempdir = tempfile::tempdir().unwrap();
    git2::Repository::init(tempdir.path()).unwrap();
    write(
        &tempdir.path().join(".warp/.mcp.json"),
        br#"{
  "mcpServers": {
    "access-token": { "command": "server", "args": ["--access-token", "access-secret"] },
    "authorization-header": { "command": "server", "args": ["--header", "Authorization: Bearer header-secret"] },
    "cloud-id": { "command": "server", "args": ["--cloud-id", "managed-object-123"] },
    "client-secret": { "command": "server", "args": ["--client-secret", "client-value"] },
    "github-token": { "command": "server", "args": ["--github-token=github-value"] },
    "auth": { "command": "server", "args": ["--auth", "auth-value"] },
    "safe": {
      "type": "stdio",
      "transport": "local",
      "command": "safe-server",
      "args": ["--stdio"]
    }
  }
}"#,
    );

    let preview = preview_project_migration(tempdir.path()).unwrap();
    let mcp_entry = preview
        .entries
        .iter()
        .find(|entry| entry.source == Path::new(".warp/.mcp.json"))
        .unwrap();
    for server in [
        "access-token",
        "authorization-header",
        "cloud-id",
        "client-secret",
        "github-token",
        "auth",
    ] {
        assert!(mcp_entry.omissions.iter().any(|omission| {
            omission.path == format!("mcpServers.{server}.args")
                && omission.reason == MigrationOmissionReason::SensitiveValue
        }));
    }
    let debug = format!("{mcp_entry:?}");
    for secret in [
        "access-secret",
        "header-secret",
        "managed-object-123",
        "client-value",
        "github-value",
        "auth-value",
    ] {
        assert!(!debug.contains(secret));
    }

    execute_project_migration(preview);
    let migrated = fs::read_to_string(tempdir.path().join(".zyh/.mcp.json")).unwrap();
    for secret in [
        "access-secret",
        "header-secret",
        "managed-object-123",
        "client-value",
        "github-value",
        "auth-value",
    ] {
        assert!(!migrated.contains(secret));
    }
    let migrated: serde_json::Value = serde_json::from_str(&migrated).unwrap();
    for server in [
        "access-token",
        "authorization-header",
        "cloud-id",
        "client-secret",
        "github-token",
        "auth",
    ] {
        assert!(migrated["mcpServers"].get(server).is_none());
    }
    assert_eq!(migrated["mcpServers"]["safe"]["type"], "stdio");
    assert_eq!(migrated["mcpServers"]["safe"]["transport"], "local");
}
