//! ZYH-managed local MCP configuration (`~/.zyh/.mcp.json` and project `.zyh/.mcp.json`).
//!
//! Create/update/delete use owner-only atomic writes with content-hash conflict
//! detection. Sensitive env/header values written by the app go to OS secure
//! storage and appear on disk only as `${NAME}` placeholders.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{fs, io};

use regex::Regex;
use serde_json::{Map, Value};
use thiserror::Error;
use warpui_extras::owner_only_file::{
    atomic_create, atomic_replace, content_hash, ContentHash, ExpectedContent, OwnerOnlyFileError,
};

/// Secure-storage key for the JSON map of ZYH-managed MCP secret values.
pub const ZYH_MCP_SECRETS_STORAGE_KEY: &str = "ZyhMcpServerSecrets";

const MCP_SERVERS_WRAPPER: &str = "mcpServers";
const SERVERS_WRAPPER: &str = "servers";

/// Scope of a ZYH-managed MCP config file.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Project scope is used by callers and tests; keep both variants public.
pub enum LocalMcpConfigScope {
    Global { home_config_dir: PathBuf },
    Project { project_root: PathBuf },
}

/// In-memory snapshot of a ZYH MCP config file.
#[derive(Debug, Clone, PartialEq)]
pub enum LocalMcpConfigState {
    Missing,
    Present {
        content: String,
        content_hash: ContentHash,
        servers: Map<String, Value>,
        wrapper: String,
    },
}

/// Kind of sensitive field extracted from a server definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    Env,
    Header,
}

impl SecretKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Env => "env",
            Self::Header => "header",
        }
    }
}

/// Secret extracted while redacting a config write (`${name}` on disk).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedSecret {
    /// Server name that owned this env/header entry.
    pub server: String,
    pub kind: SecretKind,
    /// Env or header field name (also used as `${name}` placeholder).
    pub name: String,
    pub value: String,
}

impl ExtractedSecret {
    /// Secure-storage key: `{server}/{kind}/{name}` so servers do not collide.
    pub fn storage_key(&self) -> String {
        secret_storage_key(&self.server, self.kind, &self.name)
    }
}

/// Build the durable secret map key for a ZYH-managed MCP secret.
pub fn secret_storage_key(server: &str, kind: SecretKind, name: &str) -> String {
    format!("{server}/{}/{name}", kind.as_str())
}

/// Server map prepared for disk with secrets stripped to placeholders.
#[derive(Debug, Clone, PartialEq)]
pub struct RedactedMcpConfig {
    pub servers: Map<String, Value>,
    pub secrets: Vec<ExtractedSecret>,
}

/// Errors from loading or mutating ZYH-managed MCP config files.
#[derive(Debug, Error)]
pub enum LocalMcpConfigError {
    #[error("MCP configuration path is unavailable")]
    PathUnavailable,
    #[error("MCP configuration is not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("MCP configuration must be a JSON object with mcpServers or servers")]
    MissingServerMap,
    #[error("MCP server '{0}' was not found in the configuration")]
    ServerNotFound(String),
    #[error("the MCP configuration file changed before it could be written: {path}")]
    Conflict { path: PathBuf },
    #[error("refusing to operate on a symlink or non-regular file: {path}")]
    UnsupportedFileType { path: PathBuf },
    #[error("MCP configuration file operation failed: {0}")]
    Io(#[from] io::Error),
}

impl From<OwnerOnlyFileError> for LocalMcpConfigError {
    fn from(error: OwnerOnlyFileError) -> Self {
        match error {
            OwnerOnlyFileError::Io(error) => Self::Io(error),
            OwnerOnlyFileError::Conflict { path } => Self::Conflict { path },
            OwnerOnlyFileError::UnsupportedFileType { path } => Self::UnsupportedFileType { path },
        }
    }
}

impl LocalMcpConfigScope {
    pub fn path(&self) -> PathBuf {
        match self {
            Self::Global { home_config_dir } => home_config_dir.join(".mcp.json"),
            Self::Project { project_root } => project_root
                .join(warp_core::paths::ZYH_PROJECT_CONFIG_DIR)
                .join(".mcp.json"),
        }
    }

    pub fn global() -> Result<Self, LocalMcpConfigError> {
        let home_config_dir =
            warp_core::paths::warp_home_config_dir().ok_or(LocalMcpConfigError::PathUnavailable)?;
        Ok(Self::Global { home_config_dir })
    }

    pub fn project(project_root: impl Into<PathBuf>) -> Self {
        Self::Project {
            project_root: project_root.into(),
        }
    }
}

/// Document handle for a ZYH-managed `.mcp.json` file.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Public file API; create/save used by tests and future project writers.
pub struct LocalMcpConfigDocument {
    path: PathBuf,
}

impl LocalMcpConfigDocument {
    pub fn for_scope(scope: &LocalMcpConfigScope) -> Self {
        Self { path: scope.path() }
    }

    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn display_path(&self) -> String {
        self.path.display().to_string()
    }

    pub fn load(&self) -> Result<LocalMcpConfigState, LocalMcpConfigError> {
        match content_hash(&self.path)? {
            None => Ok(LocalMcpConfigState::Missing),
            Some(file_hash) => {
                let content = fs::read_to_string(&self.path)?;
                let (wrapper, servers) = parse_server_map(&content)?;
                Ok(LocalMcpConfigState::Present {
                    content,
                    content_hash: file_hash,
                    servers,
                    wrapper,
                })
            }
        }
    }

    pub fn expected_content(state: &LocalMcpConfigState) -> ExpectedContent {
        match state {
            LocalMcpConfigState::Missing => ExpectedContent::Missing,
            LocalMcpConfigState::Present { content_hash, .. } => {
                ExpectedContent::Hash(*content_hash)
            }
        }
    }

    pub fn create(&self, content: &str) -> Result<ContentHash, LocalMcpConfigError> {
        let _ = parse_server_map(content)?;
        Ok(atomic_create(&self.path, content.as_bytes())?)
    }

    pub fn save(
        &self,
        content: &str,
        expected: ExpectedContent,
    ) -> Result<ContentHash, LocalMcpConfigError> {
        let _ = parse_server_map(content)?;
        Ok(atomic_replace(&self.path, content.as_bytes(), expected)?.content_hash)
    }

    /// Upsert servers; literal env/header values become placeholders and are
    /// returned for secure-storage writes (never written to the file).
    ///
    /// The same `expected` token is enforced at write time via
    /// [`atomic_create`] / [`atomic_replace`] so concurrent external edits
    /// cannot be clobbered after a stale in-memory merge.
    pub fn upsert_servers(
        &self,
        incoming: Map<String, Value>,
        expected: ExpectedContent,
    ) -> Result<(ContentHash, Vec<ExtractedSecret>), LocalMcpConfigError> {
        let disk_hash = content_hash(&self.path)?;
        if !expected_matches(expected, disk_hash) {
            return Err(LocalMcpConfigError::Conflict {
                path: self.path.clone(),
            });
        }

        let (wrapper, mut servers) = match disk_hash {
            None => (MCP_SERVERS_WRAPPER.to_owned(), Map::new()),
            Some(_) => {
                let content = fs::read_to_string(&self.path)?;
                parse_server_map(&content)?
            }
        };

        let redacted = redact_server_map(incoming)?;
        for (name, server) in redacted.servers {
            servers.insert(name, server);
        }

        let content = serialize_config(&wrapper, &servers)?;
        let write_expected = match disk_hash {
            None => ExpectedContent::Missing,
            Some(hash) => ExpectedContent::Hash(hash),
        };
        // Prefer the caller's expected when it is more specific than Missing/Hash
        // derived from disk; both must already match (checked above).
        let write_expected = match expected {
            ExpectedContent::Any => write_expected,
            other => other,
        };
        let hash = match write_expected {
            ExpectedContent::Missing => atomic_create(&self.path, content.as_bytes())?,
            ExpectedContent::Hash(_) | ExpectedContent::Any => {
                atomic_replace(&self.path, content.as_bytes(), write_expected)?.content_hash
            }
        };
        Ok((hash, redacted.secrets))
    }

    pub fn delete_server(
        &self,
        server_name: &str,
        expected: ExpectedContent,
    ) -> Result<Option<ContentHash>, LocalMcpConfigError> {
        let disk_hash = content_hash(&self.path)?;
        if !expected_matches(expected, disk_hash) {
            return Err(LocalMcpConfigError::Conflict {
                path: self.path.clone(),
            });
        }

        match disk_hash {
            None => Ok(None),
            Some(hash) => {
                let content = fs::read_to_string(&self.path)?;
                let (wrapper, mut servers) = parse_server_map(&content)?;
                if servers.remove(server_name).is_none() {
                    return Err(LocalMcpConfigError::ServerNotFound(server_name.to_owned()));
                }
                if servers.is_empty() {
                    // Re-check the same expected hash immediately before unlink.
                    if !expected_matches(ExpectedContent::Hash(hash), content_hash(&self.path)?) {
                        return Err(LocalMcpConfigError::Conflict {
                            path: self.path.clone(),
                        });
                    }
                    match fs::remove_file(&self.path) {
                        Ok(()) => Ok(None),
                        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                        Err(error) => Err(error.into()),
                    }
                } else {
                    let content = serialize_config(&wrapper, &servers)?;
                    let write_expected = match expected {
                        ExpectedContent::Any => ExpectedContent::Hash(hash),
                        other => other,
                    };
                    let new_hash =
                        atomic_replace(&self.path, content.as_bytes(), write_expected)?.content_hash;
                    Ok(Some(new_hash))
                }
            }
        }
    }

    pub fn server_document(name: &str, server: &Value) -> Result<String, LocalMcpConfigError> {
        let mut servers = Map::new();
        servers.insert(name.to_owned(), server.clone());
        serialize_config(MCP_SERVERS_WRAPPER, &servers)
    }
}

/// Load editor JSON for one ZYH-managed server from disk placeholders only.
///
/// Never falls back to in-memory resolved installation values (which may hold
/// plaintext secrets after spawn-time substitution).
pub fn editor_json_for_server(
    path: &Path,
    server_name: &str,
) -> Result<String, LocalMcpConfigError> {
    let document = LocalMcpConfigDocument::with_path(path);
    match document.load()? {
        LocalMcpConfigState::Missing => Err(LocalMcpConfigError::ServerNotFound(
            server_name.to_owned(),
        )),
        LocalMcpConfigState::Present { servers, .. } => {
            let server = servers
                .get(server_name)
                .ok_or_else(|| LocalMcpConfigError::ServerNotFound(server_name.to_owned()))?;
            LocalMcpConfigDocument::server_document(server_name, server)
        }
    }
}

/// Parse user JSON into a server name map (`mcpServers`, `servers`, or flat map).
pub fn parse_servers_from_user_json(json: &str) -> Result<Map<String, Value>, LocalMcpConfigError> {
    let (_, servers) = parse_server_map(json)?;
    Ok(servers)
}

/// Redact literal secrets from a server map destined for disk.
pub fn redact_server_map(
    servers: Map<String, Value>,
) -> Result<RedactedMcpConfig, LocalMcpConfigError> {
    let mut secrets = Vec::new();
    let mut output = Map::new();

    for (name, server) in servers {
        let Some(server_obj) = server.as_object() else {
            return Err(LocalMcpConfigError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("server '{name}' must be a JSON object"),
            )));
        };
        let redacted = redact_server(&name, server_obj, &mut secrets);
        output.insert(name, Value::Object(redacted));
    }

    Ok(RedactedMcpConfig {
        servers: output,
        secrets,
    })
}

/// Merge extracted secrets into an existing map (later values win).
pub fn merge_secrets(
    existing: &HashMap<String, String>,
    extracted: &[ExtractedSecret],
) -> HashMap<String, String> {
    let mut merged = existing.clone();
    for secret in extracted {
        merged.insert(secret.storage_key(), secret.value.clone());
    }
    merged
}

/// Commit redacted servers to disk only after `persist_secrets` succeeds.
///
/// Order:
/// 1. Validate expected content matches disk.
/// 2. Redact and merge secrets.
/// 3. Call `persist_secrets` with the full secret map (must succeed).
/// 4. Atomically write the redacted file with the same expected token.
///
/// If step 3 fails, the file is left unchanged. If step 4 fails after step 3,
/// secrets may be ahead of the file; callers should surface that error (secrets
/// without matching placeholders is safer than placeholders without secrets).
pub fn commit_local_mcp_config(
    document: &LocalMcpConfigDocument,
    incoming: Map<String, Value>,
    expected: ExpectedContent,
    existing_secrets: &HashMap<String, String>,
    persist_secrets: impl FnOnce(&HashMap<String, String>) -> Result<(), LocalMcpConfigError>,
) -> Result<(ContentHash, HashMap<String, String>), LocalMcpConfigError> {
    let disk_hash = content_hash(document.path())?;
    if !expected_matches(expected, disk_hash) {
        return Err(LocalMcpConfigError::Conflict {
            path: document.path().to_path_buf(),
        });
    }

    let redacted = redact_server_map(incoming)?;
    let merged = merge_secrets(existing_secrets, &redacted.secrets);
    persist_secrets(&merged)?;

    let (hash, _) = document.upsert_servers(redacted.servers, expected)?;
    Ok((hash, merged))
}

/// Resolve `${NAME}` placeholders in MCP config JSON.
///
/// When `json` is a server map document, each server resolves env/headers from
/// (1) process environment, (2) namespaced keys `{server}/env|header/{name}`,
/// (3) legacy bare `{name}` keys. Otherwise falls back to flat string
/// substitution (environment, then bare secret keys) for non-document inputs.
pub fn resolve_placeholders(
    json: &str,
    secrets: &HashMap<String, String>,
) -> Result<String, LocalMcpConfigError> {
    match parse_server_map(json) {
        Ok((wrapper, servers)) => {
            let mut resolved_servers = Map::new();
            for (server_name, server) in servers {
                resolved_servers.insert(
                    server_name.clone(),
                    resolve_server_value(&server_name, server, secrets)?,
                );
            }
            serialize_config(&wrapper, &resolved_servers)
        }
        Err(_) => resolve_flat_placeholders(json, secrets),
    }
}

fn resolve_flat_placeholders(
    json: &str,
    secrets: &HashMap<String, String>,
) -> Result<String, LocalMcpConfigError> {
    let re = placeholder_regex();
    let mut result = json.to_owned();
    for capture in re.captures_iter(json) {
        let Some(var_match) = capture.get(1) else {
            continue;
        };
        let var_name = var_match.as_str();
        let value = std::env::var(var_name)
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| secrets.get(var_name).filter(|v| !v.is_empty()).cloned());
        match value {
            Some(value) => {
                let placeholder = format!("${{{var_name}}}");
                result = result.replace(&placeholder, &value);
            }
            None => {
                return Err(LocalMcpConfigError::Io(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Missing or empty secret or environment variable: {var_name}"),
                )));
            }
        }
    }
    Ok(result)
}

fn resolve_server_value(
    server_name: &str,
    server: Value,
    secrets: &HashMap<String, String>,
) -> Result<Value, LocalMcpConfigError> {
    let Some(object) = server.as_object() else {
        return Ok(server);
    };
    let mut output = Map::new();
    for (key, value) in object {
        match key.as_str() {
            "env" => {
                output.insert(
                    key.clone(),
                    Value::Object(resolve_string_map(
                        server_name,
                        SecretKind::Env,
                        value,
                        secrets,
                    )?),
                );
            }
            "headers" => {
                output.insert(
                    key.clone(),
                    Value::Object(resolve_string_map(
                        server_name,
                        SecretKind::Header,
                        value,
                        secrets,
                    )?),
                );
            }
            _ => {
                output.insert(key.clone(), value.clone());
            }
        }
    }
    Ok(Value::Object(output))
}

fn resolve_string_map(
    server_name: &str,
    kind: SecretKind,
    value: &Value,
    secrets: &HashMap<String, String>,
) -> Result<Map<String, Value>, LocalMcpConfigError> {
    let Some(map) = value.as_object() else {
        return Err(LocalMcpConfigError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{server_name}.{} must be an object", kind.as_str()),
        )));
    };
    let mut output = Map::new();
    for (field, field_value) in map {
        match field_value.as_str() {
            Some(text) => {
                let resolved = resolve_value_placeholders(server_name, kind, field, text, secrets)?;
                output.insert(field.clone(), Value::String(resolved));
            }
            None => {
                output.insert(field.clone(), field_value.clone());
            }
        }
    }
    Ok(output)
}

fn resolve_value_placeholders(
    server_name: &str,
    kind: SecretKind,
    field: &str,
    text: &str,
    secrets: &HashMap<String, String>,
) -> Result<String, LocalMcpConfigError> {
    let re = placeholder_regex();
    let mut result = text.to_owned();
    for capture in re.captures_iter(text) {
        let Some(var_match) = capture.get(1) else {
            continue;
        };
        let var_name = var_match.as_str();
        let namespaced = secret_storage_key(server_name, kind, var_name);
        // Prefer field-name match for pure `${FIELD}` placeholders written by redact.
        let field_namespaced = secret_storage_key(server_name, kind, field);
        let value = std::env::var(var_name)
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| secrets.get(&namespaced).filter(|v| !v.is_empty()).cloned())
            .or_else(|| {
                if var_name == field {
                    secrets
                        .get(&field_namespaced)
                        .filter(|v| !v.is_empty())
                        .cloned()
                } else {
                    None
                }
            })
            .or_else(|| secrets.get(var_name).filter(|v| !v.is_empty()).cloned());
        match value {
            Some(value) => {
                let placeholder = format!("${{{var_name}}}");
                result = result.replace(&placeholder, &value);
            }
            None => {
                return Err(LocalMcpConfigError::Io(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Missing or empty secret or environment variable: {var_name}"),
                )));
            }
        }
    }
    Ok(result)
}

/// True when `value` is solely a `${NAME}` placeholder.
pub fn is_pure_placeholder(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("${")
        && trimmed.ends_with('}')
        && trimmed.matches("${").count() == 1
        && placeholder_regex().is_match(trimmed)
}

/// True when remote MCP headers are bound to `server_url`, not `provider_origin`.
#[allow(dead_code)] // Asserted in unit tests; documents the MCP-vs-Provider origin contract.
pub fn remote_credentials_bound_to_origin(
    server_url: &str,
    headers: &HashMap<String, String>,
    provider_origin: &str,
) -> bool {
    if headers.is_empty() {
        return true;
    }
    let Ok(mcp_url) = url::Url::parse(server_url) else {
        return false;
    };
    let Ok(provider_url) = url::Url::parse(provider_origin) else {
        return false;
    };
    mcp_url.origin() != provider_url.origin()
}

fn parse_server_map(json: &str) -> Result<(String, Map<String, Value>), LocalMcpConfigError> {
    let trimmed = json.trim();
    let json_for_parsing = if trimmed.starts_with('{') {
        trimmed.to_owned()
    } else {
        format!("{{{trimmed}}}")
    };
    let value: Value = serde_json::from_str(&json_for_parsing)?;
    let object = value
        .as_object()
        .ok_or(LocalMcpConfigError::MissingServerMap)?;

    for wrapper in [MCP_SERVERS_WRAPPER, SERVERS_WRAPPER] {
        if let Some(servers) = object.get(wrapper).and_then(Value::as_object) {
            return Ok((wrapper.to_owned(), servers.clone()));
        }
    }
    if let Some(mcp) = object.get("mcp").and_then(Value::as_object) {
        if let Some(servers) = mcp.get(SERVERS_WRAPPER).and_then(Value::as_object) {
            return Ok((format!("mcp.{SERVERS_WRAPPER}"), servers.clone()));
        }
    }

    // Flat map: each top-level key is a server definition with command or url.
    if !object.is_empty()
        && object.values().all(|v| {
            v.as_object().is_some_and(|o| {
                o.contains_key("command") || o.contains_key("url") || o.contains_key("serverUrl")
            })
        })
    {
        return Ok((MCP_SERVERS_WRAPPER.to_owned(), object.clone()));
    }

    Err(LocalMcpConfigError::MissingServerMap)
}

fn serialize_config(
    wrapper: &str,
    servers: &Map<String, Value>,
) -> Result<String, LocalMcpConfigError> {
    let mut root = Map::new();
    if wrapper == format!("mcp.{SERVERS_WRAPPER}") {
        let mut mcp = Map::new();
        mcp.insert(SERVERS_WRAPPER.to_owned(), Value::Object(servers.clone()));
        root.insert("mcp".to_owned(), Value::Object(mcp));
    } else {
        root.insert(wrapper.to_owned(), Value::Object(servers.clone()));
    }
    let mut text = serde_json::to_string_pretty(&Value::Object(root))?;
    text.push('\n');
    Ok(text)
}

fn redact_server(
    server_name: &str,
    server: &Map<String, Value>,
    secrets: &mut Vec<ExtractedSecret>,
) -> Map<String, Value> {
    let mut output = Map::new();
    for (key, value) in server {
        match key.as_str() {
            "env" => {
                if let Some(map) = value.as_object() {
                    output.insert(
                        key.clone(),
                        Value::Object(redact_string_map(
                            server_name,
                            SecretKind::Env,
                            map,
                            secrets,
                        )),
                    );
                } else {
                    output.insert(key.clone(), value.clone());
                }
            }
            "headers" => {
                if let Some(map) = value.as_object() {
                    output.insert(
                        key.clone(),
                        Value::Object(redact_string_map(
                            server_name,
                            SecretKind::Header,
                            map,
                            secrets,
                        )),
                    );
                } else {
                    output.insert(key.clone(), value.clone());
                }
            }
            _ => {
                output.insert(key.clone(), value.clone());
            }
        }
    }
    output
}

fn redact_string_map(
    server_name: &str,
    kind: SecretKind,
    map: &Map<String, Value>,
    secrets: &mut Vec<ExtractedSecret>,
) -> Map<String, Value> {
    let mut output = Map::new();
    for (key, value) in map {
        match value.as_str() {
            Some(literal) if !literal.is_empty() && !is_pure_placeholder(literal) => {
                secrets.push(ExtractedSecret {
                    server: server_name.to_owned(),
                    kind,
                    name: key.clone(),
                    value: literal.to_owned(),
                });
                output.insert(key.clone(), Value::String(format!("${{{key}}}")));
            }
            Some(placeholder) => {
                output.insert(key.clone(), Value::String(placeholder.to_owned()));
            }
            None => {
                output.insert(key.clone(), value.clone());
            }
        }
    }
    output
}

fn expected_matches(expected: ExpectedContent, actual: Option<ContentHash>) -> bool {
    match expected {
        ExpectedContent::Any => true,
        ExpectedContent::Missing => actual.is_none(),
        ExpectedContent::Hash(expected_hash) => actual == Some(expected_hash),
    }
}

fn placeholder_regex() -> Regex {
    Regex::new(r"\$\{([^}]+)\}").expect("placeholder regex is valid")
}

#[cfg(test)]
#[path = "local_mcp_config_tests.rs"]
mod tests;
