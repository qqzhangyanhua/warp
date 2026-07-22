use std::fs;
use std::path::Path;

use super::{
    execute_project_migration, preview_project_migration, MigrationResultStatus, PreviewStatus,
};

#[test]
fn preview_is_read_only_and_confirmation_copies_only_supported_files() {
    let tempdir = tempfile::tempdir().unwrap();
    git2::Repository::init(tempdir.path()).unwrap();
    write(
        &tempdir.path().join(".warp/workflows/deploy.yaml"),
        b"name: deploy",
    );
    write(
        &tempdir.path().join(".warp/skills/review/SKILL.md"),
        b"# Review",
    );
    write(
        &tempdir.path().join(".warp/not-approved.txt"),
        b"unsupported",
    );

    let preview = preview_project_migration(tempdir.path()).unwrap();

    assert!(!tempdir.path().join(".zyh").exists());
    assert!(preview.entries.iter().any(|entry| {
        entry.source == Path::new(".warp/workflows/deploy.yaml")
            && entry.destination.as_deref() == Some(Path::new(".zyh/workflows/deploy.yaml"))
            && entry.status == PreviewStatus::Ready
    }));
    assert!(preview.entries.iter().any(|entry| {
        entry.source == Path::new(".warp/not-approved.txt")
            && entry.destination.is_none()
            && entry.status == PreviewStatus::Unsupported
    }));

    let result = execute_project_migration(preview);

    assert_eq!(
        fs::read(tempdir.path().join(".zyh/workflows/deploy.yaml")).unwrap(),
        b"name: deploy"
    );
    assert_eq!(
        fs::read(tempdir.path().join(".zyh/skills/review/SKILL.md")).unwrap(),
        b"# Review"
    );
    assert!(!tempdir.path().join(".zyh/not-approved.txt").exists());
    assert_eq!(
        fs::read(tempdir.path().join(".warp/workflows/deploy.yaml")).unwrap(),
        b"name: deploy"
    );
    assert!(result.entries.iter().any(|entry| {
        entry.source == Path::new(".warp/workflows/deploy.yaml")
            && entry.status == MigrationResultStatus::Copied
    }));
    assert!(result.entries.iter().any(|entry| {
        entry.source == Path::new(".warp/not-approved.txt")
            && entry.status == MigrationResultStatus::Unsupported
    }));
}

#[cfg(unix)]
#[test]
fn confirmation_rejects_source_replaced_by_same_content_symlink() {
    use std::os::unix::fs::symlink;

    let tempdir = tempfile::tempdir().unwrap();
    git2::Repository::init(tempdir.path()).unwrap();
    let source = tempdir.path().join(".warp/workflows/deploy.yaml");
    let replacement = tempdir.path().join("replacement.yaml");
    write(&source, b"name: deploy");
    write(&replacement, b"name: deploy");
    let preview = preview_project_migration(tempdir.path()).unwrap();

    fs::remove_file(&source).unwrap();
    symlink(&replacement, &source).unwrap();
    let result = execute_project_migration(preview);

    assert!(!tempdir.path().join(".zyh/workflows/deploy.yaml").exists());
    assert!(result.entries.iter().any(|entry| {
        entry.source == Path::new(".warp/workflows/deploy.yaml")
            && entry.status == MigrationResultStatus::Stale
    }));
}

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
    assert!(mcp_entry
        .omissions
        .contains(&"mcpServers.local.env.TOKEN".to_owned()));
    assert!(mcp_entry
        .omissions
        .contains(&"mcpServers.local.cloudId".to_owned()));
    assert!(mcp_entry
        .omissions
        .contains(&"mcpServers.remote.headers.Authorization".to_owned()));

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
    assert!(mcp_entry
        .omissions
        .contains(&"mcpServers.unsafe.args".to_owned()));
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
    "access-token": {
      "command": "server",
      "args": ["--access-token", "access-secret"]
    },
    "authorization-header": {
      "command": "server",
      "args": ["--header", "Authorization: Bearer header-secret"]
    },
    "cloud-id": {
      "command": "server",
      "args": ["--cloud-id", "managed-object-123"]
    },
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
    for server in ["access-token", "authorization-header", "cloud-id"] {
        assert!(mcp_entry
            .omissions
            .contains(&format!("mcpServers.{server}.args")));
    }
    let debug = format!("{mcp_entry:?}");
    assert!(!debug.contains("access-secret"));
    assert!(!debug.contains("header-secret"));
    assert!(!debug.contains("managed-object-123"));

    execute_project_migration(preview);
    let migrated = fs::read_to_string(tempdir.path().join(".zyh/.mcp.json")).unwrap();
    assert!(!migrated.contains("access-secret"));
    assert!(!migrated.contains("header-secret"));
    assert!(!migrated.contains("managed-object-123"));
    let migrated: serde_json::Value = serde_json::from_str(&migrated).unwrap();
    assert!(migrated["mcpServers"].get("access-token").is_none());
    assert!(migrated["mcpServers"].get("authorization-header").is_none());
    assert!(migrated["mcpServers"].get("cloud-id").is_none());
    assert_eq!(migrated["mcpServers"]["safe"]["type"], "stdio");
    assert_eq!(migrated["mcpServers"]["safe"]["transport"], "local");
}

#[test]
fn conflicting_destination_is_reported_and_never_overwritten() {
    let tempdir = tempfile::tempdir().unwrap();
    git2::Repository::init(tempdir.path()).unwrap();
    write(
        &tempdir.path().join(".warp/workflows/deploy.yaml"),
        b"legacy",
    );
    write(
        &tempdir.path().join(".zyh/workflows/deploy.yaml"),
        b"destination",
    );

    let preview = preview_project_migration(tempdir.path()).unwrap();
    assert!(preview.entries.iter().any(|entry| {
        entry.source == Path::new(".warp/workflows/deploy.yaml")
            && entry.status == PreviewStatus::Conflict
    }));
    let result = execute_project_migration(preview);

    assert_eq!(
        fs::read(tempdir.path().join(".zyh/workflows/deploy.yaml")).unwrap(),
        b"destination"
    );
    assert!(result.entries.iter().any(|entry| {
        entry.source == Path::new(".warp/workflows/deploy.yaml")
            && entry.status == MigrationResultStatus::Conflict
    }));
}

#[test]
fn repeated_migration_reports_identical_files_without_rewriting_them() {
    let tempdir = tempfile::tempdir().unwrap();
    git2::Repository::init(tempdir.path()).unwrap();
    write(
        &tempdir.path().join(".warp/workflows/deploy.yaml"),
        b"workflow",
    );
    execute_project_migration(preview_project_migration(tempdir.path()).unwrap());

    let destination = tempdir.path().join(".zyh/workflows/deploy.yaml");
    let modified = fs::metadata(&destination).unwrap().modified().unwrap();
    let preview = preview_project_migration(tempdir.path()).unwrap();
    assert!(preview.entries.iter().any(|entry| {
        entry.source == Path::new(".warp/workflows/deploy.yaml")
            && entry.status == PreviewStatus::AlreadyPresent
    }));
    let result = execute_project_migration(preview);

    assert_eq!(
        fs::metadata(destination).unwrap().modified().unwrap(),
        modified
    );
    assert!(result.entries.iter().any(|entry| {
        entry.source == Path::new(".warp/workflows/deploy.yaml")
            && entry.status == MigrationResultStatus::AlreadyPresent
    }));
}

#[test]
fn stale_source_fails_exact_file_while_independent_files_still_copy() {
    let tempdir = tempfile::tempdir().unwrap();
    git2::Repository::init(tempdir.path()).unwrap();
    let stale_source = tempdir.path().join(".warp/workflows/stale.yaml");
    write(&stale_source, b"stale");
    write(
        &tempdir.path().join(".warp/workflows/copied.yaml"),
        b"copied",
    );
    let preview = preview_project_migration(tempdir.path()).unwrap();
    fs::remove_file(stale_source).unwrap();

    let result = execute_project_migration(preview);

    assert_eq!(
        fs::read(tempdir.path().join(".zyh/workflows/copied.yaml")).unwrap(),
        b"copied"
    );
    assert!(!tempdir.path().join(".zyh/workflows/stale.yaml").exists());
    assert!(result.entries.iter().any(|entry| {
        entry.source == Path::new(".warp/workflows/stale.yaml")
            && entry.status == MigrationResultStatus::Stale
    }));
    assert!(result.entries.iter().any(|entry| {
        entry.source == Path::new(".warp/workflows/copied.yaml")
            && entry.status == MigrationResultStatus::Copied
    }));
}

#[cfg(unix)]
#[test]
fn source_read_error_is_reported_as_a_file_failure() {
    use std::os::unix::fs::PermissionsExt as _;

    let tempdir = tempfile::tempdir().unwrap();
    git2::Repository::init(tempdir.path()).unwrap();
    let source = tempdir.path().join(".warp/workflows/unreadable.yaml");
    write(&source, b"workflow");
    let preview = preview_project_migration(tempdir.path()).unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o000)).unwrap();

    let result = execute_project_migration(preview);

    fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
    let entry = result
        .entries
        .iter()
        .find(|entry| entry.source == Path::new(".warp/workflows/unreadable.yaml"))
        .unwrap();
    let MigrationResultStatus::Failed(error) = &entry.status else {
        panic!("expected an explicit file failure, got {:?}", entry.status);
    };
    assert!(error.contains(".warp/workflows/unreadable.yaml"));
    assert!(!tempdir
        .path()
        .join(".zyh/workflows/unreadable.yaml")
        .exists());
}

#[test]
fn already_present_entry_changed_after_preview_is_stale() {
    let tempdir = tempfile::tempdir().unwrap();
    git2::Repository::init(tempdir.path()).unwrap();
    let source = tempdir.path().join(".warp/workflows/deploy.yaml");
    let destination = tempdir.path().join(".zyh/workflows/deploy.yaml");
    write(&source, b"same");
    write(&destination, b"same");
    let preview = preview_project_migration(tempdir.path()).unwrap();

    write(&source, b"changed");
    let result = execute_project_migration(preview);

    assert!(result.entries.iter().any(|entry| {
        entry.source == Path::new(".warp/workflows/deploy.yaml")
            && entry.status == MigrationResultStatus::Stale
    }));
    assert_eq!(fs::read(destination).unwrap(), b"same");
}

#[test]
fn conflict_removed_after_preview_is_stale() {
    let tempdir = tempfile::tempdir().unwrap();
    git2::Repository::init(tempdir.path()).unwrap();
    write(
        &tempdir.path().join(".warp/workflows/deploy.yaml"),
        b"source",
    );
    let destination = tempdir.path().join(".zyh/workflows/deploy.yaml");
    write(&destination, b"conflict");
    let preview = preview_project_migration(tempdir.path()).unwrap();

    fs::remove_file(&destination).unwrap();
    let result = execute_project_migration(preview);

    assert!(result.entries.iter().any(|entry| {
        entry.source == Path::new(".warp/workflows/deploy.yaml")
            && entry.status == MigrationResultStatus::Stale
    }));
    assert!(!destination.exists());
}

#[test]
fn malformed_mcp_fails_preview_without_creating_destination() {
    let tempdir = tempfile::tempdir().unwrap();
    git2::Repository::init(tempdir.path()).unwrap();
    write(&tempdir.path().join(".warp/.mcp.json"), b"not-json");

    let error = preview_project_migration(tempdir.path()).unwrap_err();

    assert!(matches!(
        error,
        super::ProjectMigrationError::MalformedMcp { .. }
    ));
    assert!(!tempdir.path().join(".zyh").exists());
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}
