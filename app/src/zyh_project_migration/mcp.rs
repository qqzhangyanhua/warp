use serde_json::{Map, Value};
use thiserror::Error;

use super::model::{MigrationOmission, MigrationOmissionReason};

#[derive(Debug, Error)]
pub(crate) enum McpSanitizationError {
    #[error("MCP configuration is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("MCP configuration must contain an object named mcpServers or servers")]
    MissingServerMap,
}

pub(super) struct SanitizedMcp {
    pub(super) bytes: Vec<u8>,
    pub(super) omissions: Vec<MigrationOmission>,
}

pub(super) fn sanitize_mcp(bytes: &[u8]) -> Result<SanitizedMcp, McpSanitizationError> {
    let source: Value = serde_json::from_slice(bytes)?;
    let source_object = source
        .as_object()
        .ok_or(McpSanitizationError::MissingServerMap)?;
    let wrapper = ["mcpServers", "servers"]
        .into_iter()
        .find(|key| source_object.get(*key).is_some_and(Value::is_object))
        .ok_or(McpSanitizationError::MissingServerMap)?;
    let servers = source_object[wrapper]
        .as_object()
        .expect("selected MCP server map must be an object");

    let mut omissions = source_object
        .keys()
        .filter(|key| key.as_str() != wrapper)
        .map(|key| MigrationOmission {
            path: key.to_owned(),
            reason: MigrationOmissionReason::UnsupportedField,
        })
        .collect::<Vec<_>>();
    let mut sanitized_servers = Map::new();
    for (name, server) in servers {
        let path = format!("{wrapper}.{name}");
        let Some(server) = server.as_object() else {
            omit(&mut omissions, path, MigrationOmissionReason::InvalidValue);
            continue;
        };
        if let Some(sanitized) = sanitize_server(server, &path, &mut omissions) {
            sanitized_servers.insert(name.to_owned(), Value::Object(sanitized));
        }
    }
    omissions.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.reason.cmp(&right.reason))
    });

    let mut output = Map::new();
    output.insert(wrapper.to_owned(), Value::Object(sanitized_servers));
    let mut bytes = serde_json::to_vec_pretty(&Value::Object(output))?;
    bytes.push(b'\n');
    Ok(SanitizedMcp { bytes, omissions })
}

fn sanitize_server(
    server: &Map<String, Value>,
    path: &str,
    omissions: &mut Vec<MigrationOmission>,
) -> Option<Map<String, Value>> {
    if server
        .get("args")
        .and_then(Value::as_array)
        .is_some_and(|args| contains_credential_argument(args))
    {
        omit(
            omissions,
            format!("{path}.args"),
            MigrationOmissionReason::SensitiveValue,
        );
        return None;
    }

    let mut output = Map::new();
    let mut has_transport = false;

    if let Some(command) = server.get("command").and_then(Value::as_str) {
        output.insert("command".to_owned(), Value::String(command.to_owned()));
        has_transport = true;
    }
    if let Some(url) = server.get("url").and_then(Value::as_str) {
        match sanitize_url(url) {
            Some(url) => {
                output.insert("url".to_owned(), Value::String(url));
                has_transport = true;
            }
            None => omit(
                omissions,
                format!("{path}.url"),
                MigrationOmissionReason::InvalidValue,
            ),
        }
    }
    copy_string_array(server, "args", path, &mut output, omissions);
    copy_string(server, "type", path, &mut output, omissions);
    copy_string(server, "transport", path, &mut output, omissions);
    copy_string(server, "working_directory", path, &mut output, omissions);
    copy_reference_map(server, "env", path, &mut output, omissions);
    copy_reference_map(server, "headers", path, &mut output, omissions);

    let approved = [
        "command",
        "url",
        "args",
        "type",
        "transport",
        "working_directory",
        "env",
        "headers",
    ];
    for key in server.keys() {
        if !approved.contains(&key.as_str()) {
            let reason = if contains_sensitive_name(key) {
                MigrationOmissionReason::SensitiveValue
            } else {
                MigrationOmissionReason::UnsupportedField
            };
            omit(omissions, format!("{path}.{key}"), reason);
        }
    }

    if has_transport {
        Some(output)
    } else {
        omit(omissions, path, MigrationOmissionReason::InvalidValue);
        None
    }
}

fn contains_credential_argument(args: &[Value]) -> bool {
    args.iter().filter_map(Value::as_str).any(|argument| {
        let normalized = argument.trim().to_ascii_lowercase().replace('_', "-");
        if normalized.contains("authorization:")
            || normalized
                .split(|character: char| character.is_ascii_whitespace() || character == '=')
                .any(|part| part == "bearer")
        {
            return true;
        }
        let flag = normalized
            .trim_start_matches('-')
            .split('=')
            .next()
            .unwrap_or(&normalized);
        contains_sensitive_name(flag)
    })
}

fn contains_sensitive_name(name: &str) -> bool {
    const SENSITIVE_NAMES: &[&str] = &[
        "api-key",
        "apikey",
        "auth",
        "authorization",
        "credential",
        "credentials",
        "header",
        "headers",
        "password",
        "secret",
        "token",
    ];

    let normalized = name.to_ascii_lowercase().replace('_', "-");
    SENSITIVE_NAMES.contains(&normalized.as_str())
        || normalized
            .split('-')
            .any(|part| SENSITIVE_NAMES.contains(&part))
        || ["auth", "credential", "password", "secret", "token"]
            .into_iter()
            .any(|suffix| normalized.ends_with(suffix))
        || normalized.contains("cloud")
        || normalized.contains("managed")
}

fn copy_string(
    source: &Map<String, Value>,
    key: &str,
    path: &str,
    output: &mut Map<String, Value>,
    omissions: &mut Vec<MigrationOmission>,
) {
    match source.get(key) {
        Some(Value::String(value)) => {
            output.insert(key.to_owned(), Value::String(value.to_owned()));
        }
        Some(_) => omit(
            omissions,
            format!("{path}.{key}"),
            MigrationOmissionReason::InvalidValue,
        ),
        None => {}
    }
}

fn copy_string_array(
    source: &Map<String, Value>,
    key: &str,
    path: &str,
    output: &mut Map<String, Value>,
    omissions: &mut Vec<MigrationOmission>,
) {
    match source.get(key) {
        Some(Value::Array(values)) if values.iter().all(Value::is_string) => {
            output.insert(key.to_owned(), Value::Array(values.to_owned()));
        }
        Some(_) => omit(
            omissions,
            format!("{path}.{key}"),
            MigrationOmissionReason::InvalidValue,
        ),
        None => {}
    }
}

fn copy_reference_map(
    source: &Map<String, Value>,
    key: &str,
    path: &str,
    output: &mut Map<String, Value>,
    omissions: &mut Vec<MigrationOmission>,
) {
    let Some(value) = source.get(key) else {
        return;
    };
    let Some(values) = value.as_object() else {
        omit(
            omissions,
            format!("{path}.{key}"),
            MigrationOmissionReason::InvalidValue,
        );
        return;
    };
    let mut sanitized = Map::new();
    for (name, value) in values {
        match value
            .as_str()
            .filter(|value| is_environment_reference(value))
        {
            Some(value) => {
                sanitized.insert(name.to_owned(), Value::String(value.to_owned()));
            }
            None => omit(
                omissions,
                format!("{path}.{key}.{name}"),
                MigrationOmissionReason::SensitiveValue,
            ),
        }
    }
    if !sanitized.is_empty() {
        output.insert(key.to_owned(), Value::Object(sanitized));
    }
}

fn omit(
    omissions: &mut Vec<MigrationOmission>,
    path: impl Into<String>,
    reason: MigrationOmissionReason,
) {
    omissions.push(MigrationOmission {
        path: path.into(),
        reason,
    });
}

fn is_environment_reference(value: &str) -> bool {
    let reference = value.strip_prefix("Bearer ").unwrap_or(value);
    let Some(variable) = reference
        .strip_prefix("${")
        .and_then(|reference| reference.strip_suffix('}'))
    else {
        return false;
    };
    !variable.is_empty()
        && variable
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn sanitize_url(value: &str) -> Option<String> {
    let mut url = url::Url::parse(value).ok()?;
    url.set_username("").ok()?;
    url.set_password(None).ok()?;
    url.set_query(None);
    url.set_fragment(None);
    Some(url.into())
}
