use serde_json::{Map, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum McpSanitizationError {
    #[error("MCP configuration is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("MCP configuration must contain an object named mcpServers or servers")]
    MissingServerMap,
}

pub(super) struct SanitizedMcp {
    pub(super) bytes: Vec<u8>,
    pub(super) omissions: Vec<String>,
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
        .map(|key| key.to_owned())
        .collect::<Vec<_>>();
    let mut sanitized_servers = Map::new();
    for (name, server) in servers {
        let path = format!("{wrapper}.{name}");
        let Some(server) = server.as_object() else {
            omissions.push(path);
            continue;
        };
        if let Some(sanitized) = sanitize_server(server, &path, &mut omissions) {
            sanitized_servers.insert(name.to_owned(), Value::Object(sanitized));
        }
    }
    omissions.sort();

    let mut output = Map::new();
    output.insert(wrapper.to_owned(), Value::Object(sanitized_servers));
    let mut bytes = serde_json::to_vec_pretty(&Value::Object(output))?;
    bytes.push(b'\n');
    Ok(SanitizedMcp { bytes, omissions })
}

fn sanitize_server(
    server: &Map<String, Value>,
    path: &str,
    omissions: &mut Vec<String>,
) -> Option<Map<String, Value>> {
    if server
        .get("args")
        .and_then(Value::as_array)
        .is_some_and(|args| contains_credential_argument(args))
    {
        omissions.push(format!("{path}.args"));
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
            None => omissions.push(format!("{path}.url")),
        }
    }
    copy_string_array(server, "args", path, &mut output, omissions);
    copy_string(server, "working_directory", path, &mut output, omissions);
    copy_reference_map(server, "env", path, &mut output, omissions);
    copy_reference_map(server, "headers", path, &mut output, omissions);

    let approved = [
        "command",
        "url",
        "args",
        "working_directory",
        "env",
        "headers",
    ];
    for key in server.keys() {
        if !approved.contains(&key.as_str()) {
            omissions.push(format!("{path}.{key}"));
        }
    }

    if has_transport {
        Some(output)
    } else {
        omissions.push(path.to_owned());
        None
    }
}

fn contains_credential_argument(args: &[Value]) -> bool {
    const CREDENTIAL_ARGUMENTS: &[&str] = &[
        "token",
        "api-key",
        "apikey",
        "secret",
        "password",
        "credential",
        "credentials",
    ];

    args.iter().filter_map(Value::as_str).any(|argument| {
        let normalized = argument
            .trim_start_matches('-')
            .to_ascii_lowercase()
            .replace('_', "-");
        let name = normalized.split('=').next().unwrap_or(&normalized);
        CREDENTIAL_ARGUMENTS.contains(&name)
    })
}

fn copy_string(
    source: &Map<String, Value>,
    key: &str,
    path: &str,
    output: &mut Map<String, Value>,
    omissions: &mut Vec<String>,
) {
    match source.get(key) {
        Some(Value::String(value)) => {
            output.insert(key.to_owned(), Value::String(value.to_owned()));
        }
        Some(_) => omissions.push(format!("{path}.{key}")),
        None => {}
    }
}

fn copy_string_array(
    source: &Map<String, Value>,
    key: &str,
    path: &str,
    output: &mut Map<String, Value>,
    omissions: &mut Vec<String>,
) {
    match source.get(key) {
        Some(Value::Array(values)) if values.iter().all(Value::is_string) => {
            output.insert(key.to_owned(), Value::Array(values.to_owned()));
        }
        Some(_) => omissions.push(format!("{path}.{key}")),
        None => {}
    }
}

fn copy_reference_map(
    source: &Map<String, Value>,
    key: &str,
    path: &str,
    output: &mut Map<String, Value>,
    omissions: &mut Vec<String>,
) {
    let Some(value) = source.get(key) else {
        return;
    };
    let Some(values) = value.as_object() else {
        omissions.push(format!("{path}.{key}"));
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
            None => omissions.push(format!("{path}.{key}.{name}")),
        }
    }
    if !sanitized.is_empty() {
        output.insert(key.to_owned(), Value::Object(sanitized));
    }
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
