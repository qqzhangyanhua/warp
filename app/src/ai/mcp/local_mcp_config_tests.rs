use std::collections::HashMap;
use std::fs;

use warpui_extras::owner_only_file::{content_hash, ExpectedContent};

use super::{
    commit_local_mcp_config, is_pure_placeholder, merge_secrets, parse_servers_from_user_json,
    redact_server_map, remote_credentials_bound_to_origin, resolve_placeholders,
    secret_storage_key, ExtractedSecret, LocalMcpConfigDocument, LocalMcpConfigError,
    LocalMcpConfigScope, LocalMcpConfigState, SecretKind,
};

fn document_in(temp: &tempfile::TempDir) -> LocalMcpConfigDocument {
    LocalMcpConfigDocument::for_scope(&LocalMcpConfigScope::Global {
        home_config_dir: temp.path().to_path_buf(),
    })
}

#[test]
fn global_scope_path_is_mcp_json_under_zyh_home() {
    let scope = LocalMcpConfigScope::Global {
        home_config_dir: std::path::PathBuf::from("/Users/example/.zyh"),
    };
    assert_eq!(
        scope.path(),
        std::path::PathBuf::from("/Users/example/.zyh/.mcp.json")
    );
}

#[test]
fn project_scope_path_is_mcp_json_under_zyh_project_dir() {
    let scope = LocalMcpConfigScope::project("/repos/app");
    assert_eq!(
        scope.path(),
        std::path::PathBuf::from("/repos/app/.zyh/.mcp.json")
    );
}

#[test]
fn missing_file_loads_as_missing() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    assert_eq!(document.load().unwrap(), LocalMcpConfigState::Missing);
}

#[test]
fn create_and_load_local_command_server() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    let content = r#"{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
    }
  }
}
"#;
    let hash = document.create(content).unwrap();
    let state = document.load().unwrap();
    match state {
        LocalMcpConfigState::Present {
            content: loaded,
            content_hash,
            servers,
            wrapper,
        } => {
            assert_eq!(content_hash, hash);
            assert_eq!(wrapper, "mcpServers");
            assert!(servers.contains_key("filesystem"));
            assert!(loaded.contains("filesystem"));
            assert!(loaded.contains("npx"));
        }
        LocalMcpConfigState::Missing => panic!("expected present config"),
    }
}

#[test]
fn upsert_redacts_literal_env_and_headers_to_placeholders() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);

    let mut servers = parse_servers_from_user_json(
        r#"{
          "mcpServers": {
            "remote": {
              "url": "https://mcp.example.com/sse",
              "headers": {
                "Authorization": "Bearer super-secret-token"
              },
              "env": {
                "API_KEY": "sk-live-plaintext"
              }
            }
          }
        }"#,
    )
    .unwrap();

    let (_, secrets) = document
        .upsert_servers(servers.clone(), ExpectedContent::Missing)
        .unwrap();

    assert_eq!(secrets.len(), 2);
    assert!(secrets.iter().any(|s| {
        s.server == "remote"
            && s.kind == SecretKind::Env
            && s.name == "API_KEY"
            && s.value == "sk-live-plaintext"
            && s.storage_key() == "remote/env/API_KEY"
    }));
    assert!(secrets.iter().any(|s| {
        s.server == "remote"
            && s.kind == SecretKind::Header
            && s.name == "Authorization"
            && s.value == "Bearer super-secret-token"
    }));

    let on_disk = fs::read_to_string(document.path()).unwrap();
    assert!(!on_disk.contains("sk-live-plaintext"));
    assert!(!on_disk.contains("super-secret-token"));
    assert!(on_disk.contains("${API_KEY}"));
    assert!(on_disk.contains("${Authorization}"));
    assert!(on_disk.contains("https://mcp.example.com/sse"));

    // Project files must never receive plaintext from a second write of the same secrets map.
    servers = parse_servers_from_user_json(&on_disk).unwrap();
    let redacted = redact_server_map(servers).unwrap();
    assert!(redacted.secrets.is_empty());
}

#[test]
fn pure_placeholders_are_preserved_without_extraction() {
    let servers = parse_servers_from_user_json(
        r#"{
          "mcpServers": {
            "cli": {
              "command": "tool",
              "env": { "TOKEN": "${TOKEN}" }
            }
          }
        }"#,
    )
    .unwrap();
    let redacted = redact_server_map(servers).unwrap();
    assert!(redacted.secrets.is_empty());
    assert_eq!(
        redacted.servers["cli"]["env"]["TOKEN"],
        serde_json::json!("${TOKEN}")
    );
    assert!(is_pure_placeholder("${TOKEN}"));
    assert!(!is_pure_placeholder("Bearer ${TOKEN}"));
    assert!(!is_pure_placeholder("literal"));
}

#[test]
fn resolve_placeholders_prefers_environment_then_secrets() {
    const VAR: &str = "ZYH_MCP_TEST_RESOLVE_VAR";
    std::env::remove_var(VAR);
    let secrets = HashMap::from([(VAR.to_owned(), "from-storage".to_owned())]);
    let json = format!(r#"{{"env":{{"K":"${{{VAR}}}"}}}}"#);
    let resolved = resolve_placeholders(&json, &secrets).unwrap();
    assert!(resolved.contains("from-storage"));

    std::env::set_var(VAR, "from-env");
    let resolved = resolve_placeholders(&json, &secrets).unwrap();
    assert!(resolved.contains("from-env"));
    assert!(!resolved.contains("from-storage"));
    std::env::remove_var(VAR);
}

#[test]
fn resolve_placeholders_errors_when_missing() {
    let err =
        resolve_placeholders(r#"{"k":"${MISSING_ZYH_MCP_VAR}"}"#, &HashMap::new()).unwrap_err();
    assert!(matches!(err, LocalMcpConfigError::Io(_)));
}

#[test]
fn malformed_config_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    let err = document.create("not-json").unwrap_err();
    assert!(matches!(err, LocalMcpConfigError::InvalidJson(_)));

    let err = document.create(r#"{"foo": 1}"#).unwrap_err();
    assert!(matches!(err, LocalMcpConfigError::MissingServerMap));
}

#[test]
fn external_edit_conflicts_on_stale_save() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    let content = r#"{
  "mcpServers": {
    "a": { "command": "echo" }
  }
}
"#;
    let hash = document.create(content).unwrap();
    fs::write(document.path(), content.replace("echo", "cat")).unwrap();

    let err = document
        .save(
            r#"{ "mcpServers": { "a": { "command": "ls" } } }"#,
            ExpectedContent::Hash(hash),
        )
        .unwrap_err();
    assert!(matches!(err, LocalMcpConfigError::Conflict { .. }));
    assert!(fs::read_to_string(document.path()).unwrap().contains("cat"));
}

#[test]
fn upsert_with_stale_expected_after_external_edit_fails_and_preserves_disk() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    document
        .create(
            r#"{
  "mcpServers": {
    "a": { "command": "echo" }
  }
}
"#,
        )
        .unwrap();
    let expected = LocalMcpConfigDocument::expected_content(&document.load().unwrap());

    // External edit replaces the file with a different server set.
    fs::write(
        document.path(),
        r#"{
  "mcpServers": {
    "external": { "command": "cat" }
  }
}
"#,
    )
    .unwrap();

    let incoming = parse_servers_from_user_json(
        r#"{ "mcpServers": { "b": { "command": "ls" } } }"#,
    )
    .unwrap();
    let err = document.upsert_servers(incoming, expected).unwrap_err();
    assert!(matches!(err, LocalMcpConfigError::Conflict { .. }));

    let on_disk = fs::read_to_string(document.path()).unwrap();
    assert!(
        on_disk.contains("external") && on_disk.contains("cat"),
        "stale upsert must not clobber external content: {on_disk}"
    );
    assert!(!on_disk.contains("\"b\""), "stale upsert must not write: {on_disk}");
}

#[test]
fn delete_with_stale_expected_after_external_edit_preserves_disk() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    document
        .create(
            r#"{
  "mcpServers": {
    "a": { "command": "echo" }
  }
}
"#,
        )
        .unwrap();
    let expected = LocalMcpConfigDocument::expected_content(&document.load().unwrap());

    fs::write(
        document.path(),
        r#"{
  "mcpServers": {
    "kept": { "command": "true" }
  }
}
"#,
    )
    .unwrap();

    let err = document.delete_server("a", expected).unwrap_err();
    assert!(matches!(err, LocalMcpConfigError::Conflict { .. }));
    let on_disk = fs::read_to_string(document.path()).unwrap();
    assert!(on_disk.contains("kept") && on_disk.contains("true"));
}

#[test]
fn delete_server_and_remove_empty_file() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    document
        .create(
            r#"{
  "mcpServers": {
    "only": { "command": "echo" }
  }
}
"#,
        )
        .unwrap();
    let state = document.load().unwrap();
    let expected = LocalMcpConfigDocument::expected_content(&state);
    let result = document.delete_server("only", expected).unwrap();
    assert!(result.is_none());
    assert!(!document.path().exists());
}

#[test]
fn merge_secrets_overwrites_same_namespaced_key() {
    let existing = HashMap::from([("svc/env/API_KEY".to_owned(), "old".to_owned())]);
    let extracted = vec![ExtractedSecret {
        server: "svc".to_owned(),
        kind: SecretKind::Env,
        name: "API_KEY".to_owned(),
        value: "new".to_owned(),
    }];
    let merged = merge_secrets(&existing, &extracted);
    assert_eq!(merged.get("svc/env/API_KEY").map(String::as_str), Some("new"));
}

#[test]
fn secrets_from_different_servers_do_not_collide() {
    let servers = parse_servers_from_user_json(
        r#"{
          "mcpServers": {
            "alpha": { "command": "a", "env": { "API_KEY": "secret-a" } },
            "beta": { "command": "b", "env": { "API_KEY": "secret-b" } }
          }
        }"#,
    )
    .unwrap();
    let redacted = redact_server_map(servers).unwrap();
    let merged = merge_secrets(&HashMap::new(), &redacted.secrets);
    assert_eq!(merged.get("alpha/env/API_KEY").map(String::as_str), Some("secret-a"));
    assert_eq!(merged.get("beta/env/API_KEY").map(String::as_str), Some("secret-b"));

    let on_disk = serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": redacted.servers
    }))
    .unwrap();
    let resolved = resolve_placeholders(&on_disk, &merged).unwrap();
    assert!(resolved.contains("secret-a") && resolved.contains("secret-b"));
    assert!(!resolved.contains("${API_KEY}"));
}

#[test]
fn commit_does_not_write_file_when_secret_persist_fails() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    let incoming = parse_servers_from_user_json(
        r#"{
          "mcpServers": {
            "cli": {
              "command": "tool",
              "env": { "TOKEN": "plaintext-token" }
            }
          }
        }"#,
    )
    .unwrap();

    let err = commit_local_mcp_config(
        &document,
        incoming,
        ExpectedContent::Missing,
        &HashMap::new(),
        |_| {
            Err(LocalMcpConfigError::Io(std::io::Error::other(
                "secure storage unavailable",
            )))
        },
    )
    .unwrap_err();
    assert!(matches!(err, LocalMcpConfigError::Io(_)));
    assert!(
        !document.path().exists(),
        "file must remain absent when secret persist fails"
    );
}

#[test]
fn editor_json_for_server_returns_placeholders_only() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    document
        .create(
            r#"{
  "mcpServers": {
    "cli": {
      "command": "tool",
      "env": { "TOKEN": "${TOKEN}" }
    }
  }
}
"#,
        )
        .unwrap();
    let json = super::editor_json_for_server(document.path(), "cli").unwrap();
    assert!(json.contains("${TOKEN}"));
    assert!(!json.contains("plaintext"));
}

#[test]
fn editor_json_for_server_errors_when_missing() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    let err = super::editor_json_for_server(document.path(), "missing").unwrap_err();
    assert!(matches!(err, LocalMcpConfigError::ServerNotFound(_)));
}

#[test]
fn commit_writes_placeholders_after_secrets_succeed() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    let incoming = parse_servers_from_user_json(
        r#"{
          "mcpServers": {
            "cli": {
              "command": "tool",
              "env": { "TOKEN": "plaintext-token" }
            }
          }
        }"#,
    )
    .unwrap();

    let mut stored = HashMap::new();
    let (_hash, merged) = commit_local_mcp_config(
        &document,
        incoming,
        ExpectedContent::Missing,
        &HashMap::new(),
        |secrets| {
            stored = secrets.clone();
            Ok(())
        },
    )
    .unwrap();

    assert_eq!(
        stored.get(&secret_storage_key("cli", SecretKind::Env, "TOKEN")),
        Some(&"plaintext-token".to_owned())
    );
    assert_eq!(merged, stored);
    let on_disk = fs::read_to_string(document.path()).unwrap();
    assert!(on_disk.contains("${TOKEN}"));
    assert!(!on_disk.contains("plaintext-token"));
}

#[test]
fn remote_credentials_are_not_bound_to_provider_origin() {
    let headers = HashMap::from([(
        "Authorization".to_owned(),
        "Bearer mcp-only-token".to_owned(),
    )]);
    assert!(remote_credentials_bound_to_origin(
        "https://mcp.example.com/v1",
        &headers,
        "https://api.openai.com/v1",
    ));
    // Same origin would mean credentials could leak to the provider surface.
    assert!(!remote_credentials_bound_to_origin(
        "https://api.openai.com/mcp",
        &headers,
        "https://api.openai.com/v1",
    ));
}

#[test]
fn remote_mcp_credential_target_is_mcp_origin_only() {
    let servers = parse_servers_from_user_json(
        r#"{
          "mcpServers": {
            "remote": {
              "url": "https://mcp.example.com/sse",
              "headers": {
                "Authorization": "${Authorization}",
                "X-Client": "zyh"
              }
            },
            "cli": {
              "command": "npx",
              "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
            }
          }
        }"#,
    )
    .unwrap();

    let remote = super::remote_mcp_credential_target(&servers["remote"])
        .unwrap()
        .expect("remote server");
    assert_eq!(remote.mcp_origin, "https://mcp.example.com");
    assert_eq!(
        remote.header_names,
        vec!["Authorization".to_owned(), "X-Client".to_owned()]
    );
    assert!(super::remote_credentials_exclude_provider_origin(
        &remote,
        "https://api.openai.com/v1/chat/completions",
    ));
    assert!(!super::remote_credentials_exclude_provider_origin(
        &remote,
        "https://mcp.example.com/v1",
    ));

    assert!(super::remote_mcp_credential_target(&servers["cli"])
        .unwrap()
        .is_none());
}

#[test]
fn local_command_server_parses_from_mcp_json() {
    let json = r#"{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
      "env": { "TOKEN": "${TOKEN}" }
    }
  }
}
"#;
    let servers = parse_servers_from_user_json(json).unwrap();
    assert_eq!(servers.len(), 1);
    let server = &servers["filesystem"];
    assert_eq!(server["command"], "npx");
    assert_eq!(
        server["args"],
        serde_json::json!(["-y", "@modelcontextprotocol/server-filesystem", "/tmp"])
    );
    assert!(super::remote_mcp_credential_target(server)
        .unwrap()
        .is_none());

    // After placeholder resolve with secrets, still a command server (stdio path).
    let secrets = HashMap::from([(
        secret_storage_key("filesystem", SecretKind::Env, "TOKEN"),
        "secret".to_owned(),
    )]);
    let resolved = resolve_placeholders(json, &secrets).unwrap();
    let resolved_servers = parse_servers_from_user_json(&resolved).unwrap();
    assert_eq!(resolved_servers["filesystem"]["env"]["TOKEN"], "secret");
    assert_eq!(resolved_servers["filesystem"]["command"], "npx");
}

#[test]
fn restart_survives_via_disk_content_hash() {
    let temp = tempfile::tempdir().unwrap();
    let document = document_in(&temp);
    document
        .create(
            r#"{
  "mcpServers": {
    "keep": { "command": "true", "env": { "TOKEN": "${TOKEN}" } }
  }
}
"#,
        )
        .unwrap();
    let hash_before = content_hash(document.path()).unwrap();
    // Simulate process restart: new document handle, same path.
    let document = LocalMcpConfigDocument::with_path(document.path());
    let state = document.load().unwrap();
    match state {
        LocalMcpConfigState::Present {
            content_hash: hash_after,
            servers,
            ..
        } => {
            assert_eq!(Some(hash_after), hash_before);
            assert!(servers.contains_key("keep"));
            assert_eq!(
                servers["keep"]["env"]["TOKEN"],
                serde_json::json!("${TOKEN}")
            );
        }
        LocalMcpConfigState::Missing => panic!("config must survive restart"),
    }
}
