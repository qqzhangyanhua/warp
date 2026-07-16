use std::collections::{HashMap, HashSet};

use prost_types::value::Kind;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use thiserror::Error;
use uuid::Uuid;
use warp_multi_agent_api as api;

use crate::ai::agent::provider_tool_name::mcp_provider_name;
use crate::ai::agent::MCPContext;

pub(super) const TOOL_REQUEST_LIMIT: u32 = 32;

#[derive(Clone)]
pub(super) struct ToolCatalog {
    entries: Vec<ToolCatalogEntry>,
    routes: HashMap<String, ToolRoute>,
}

impl ToolCatalog {
    pub(super) fn initial(mcp_context: Option<&MCPContext>) -> Result<Self, ToolCatalogError> {
        let mut entries = builtin_entries();
        let mut routes = HashMap::from([
            (
                "builtin.run_shell_command".to_string(),
                ToolRoute::RunShellCommand,
            ),
            ("builtin.read_files".to_string(), ToolRoute::ReadFiles),
            (
                "builtin.apply_file_diffs".to_string(),
                ToolRoute::ApplyFileDiffs,
            ),
        ]);
        if let Some(context) = mcp_context {
            for server in &context.servers {
                let server_id = Uuid::parse_str(&server.id)
                    .map_err(|_| ToolCatalogError::InvalidMcpServerId)?;
                for tool in &server.tools {
                    let id = format!("mcp:{server_id}:{}", tool.name);
                    entries.push(ToolCatalogEntry {
                        id: id.clone(),
                        name: mcp_provider_name(server_id, tool.name.as_ref()),
                        description: tool.description.as_deref().unwrap_or_default().to_string(),
                        input_schema: tool.input_schema.as_ref().clone(),
                    });
                    routes.insert(
                        id,
                        ToolRoute::Mcp {
                            server_id,
                            tool_name: tool.name.to_string(),
                        },
                    );
                }
            }
        }
        validate_unique_entries(&entries)?;
        Ok(Self { entries, routes })
    }

    pub(super) fn entries(&self) -> &[ToolCatalogEntry] {
        &self.entries
    }

    #[cfg(test)]
    pub(super) fn route(&self, tool_id: &str) -> Option<&ToolRoute> {
        self.routes.get(tool_id)
    }

    pub(super) fn resolve(
        &self,
        tool_id: &str,
        tool_name: &str,
        arguments: &Map<String, Value>,
    ) -> Result<api::message::tool_call::Tool, ToolRequestError> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.id == tool_id)
            .ok_or(ToolRequestError::UnavailableTool)?;
        if entry.name != tool_name {
            return Err(ToolRequestError::ToolNameMismatch);
        }
        let schema = Value::Object(entry.input_schema.clone());
        let validator = jsonschema::validator_for(&schema)
            .map_err(|_| ToolRequestError::InvalidCatalogSchema)?;
        if !validator.is_valid(&Value::Object(arguments.clone())) {
            return Err(ToolRequestError::InvalidArguments);
        }
        match self.routes.get(tool_id) {
            Some(ToolRoute::RunShellCommand) => shell_tool(arguments),
            Some(ToolRoute::ReadFiles) => read_files_tool(arguments),
            Some(ToolRoute::ApplyFileDiffs) => apply_diffs_tool(arguments),
            Some(ToolRoute::Mcp {
                server_id,
                tool_name,
            }) => mcp_tool(*server_id, tool_name, arguments),
            None => Err(ToolRequestError::UnavailableTool),
        }
    }
}

#[derive(Clone, Serialize)]
pub(super) struct ToolCatalogEntry {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) input_schema: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ToolRoute {
    RunShellCommand,
    ReadFiles,
    ApplyFileDiffs,
    Mcp { server_id: Uuid, tool_name: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum ToolCatalogError {
    #[error("Configured MCP server installation ID is invalid")]
    InvalidMcpServerId,
    #[error("Tool Catalog contains a duplicate stable identity or Provider-visible name")]
    DuplicateTool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum ToolRequestError {
    #[error("Requested tool is unavailable")]
    UnavailableTool,
    #[error("Requested tool name does not match its stable identity")]
    ToolNameMismatch,
    #[error("Tool Catalog contains an invalid input schema")]
    InvalidCatalogSchema,
    #[error("Tool Request arguments do not match the Tool Catalog")]
    InvalidArguments,
}

#[derive(Deserialize)]
struct RunShellCommandArgs {
    command: String,
    #[serde(default = "default_true")]
    wait_until_complete: bool,
}

#[derive(Deserialize)]
struct ReadFilesArgs {
    files: Vec<ReadFileArgs>,
}

#[derive(Deserialize)]
struct ReadFileArgs {
    name: String,
    #[serde(default)]
    line_ranges: Vec<LineRangeArgs>,
}

#[derive(Deserialize)]
struct LineRangeArgs {
    start: u32,
    end: u32,
}

#[derive(Deserialize)]
struct ApplyFileDiffsArgs {
    summary: String,
    diffs: Vec<FileDiffArgs>,
}

#[derive(Deserialize)]
struct FileDiffArgs {
    file_path: String,
    search: String,
    replace: String,
}

fn validate_unique_entries(entries: &[ToolCatalogEntry]) -> Result<(), ToolCatalogError> {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    if entries
        .iter()
        .all(|entry| ids.insert(&entry.id) && names.insert(&entry.name))
    {
        Ok(())
    } else {
        Err(ToolCatalogError::DuplicateTool)
    }
}

fn builtin_entries() -> Vec<ToolCatalogEntry> {
    vec![
        entry(
            "builtin.run_shell_command",
            "run_shell_command",
            "Run a shell command in the active terminal after Warp permission approval.",
            json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "wait_until_complete": { "type": "boolean", "default": true }
                },
                "required": ["command"],
                "additionalProperties": false
            }),
        ),
        entry(
            "builtin.read_files",
            "read_files",
            "Read one or more files after Warp permission approval.",
            json!({
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string", "minLength": 1 },
                                "line_ranges": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "start": { "type": "integer", "minimum": 1 },
                                            "end": { "type": "integer", "minimum": 1 }
                                        },
                                        "required": ["start", "end"],
                                        "additionalProperties": false
                                    }
                                }
                            },
                            "required": ["name"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["files"],
                "additionalProperties": false
            }),
        ),
        entry(
            "builtin.apply_file_diffs",
            "apply_file_diffs",
            "Apply search-and-replace file edits after Warp permission approval.",
            json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string", "minLength": 1 },
                    "diffs": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string", "minLength": 1 },
                                "search": { "type": "string" },
                                "replace": { "type": "string" }
                            },
                            "required": ["file_path", "search", "replace"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["summary", "diffs"],
                "additionalProperties": false
            }),
        ),
    ]
}

fn entry(id: &str, name: &str, description: &str, schema: Value) -> ToolCatalogEntry {
    ToolCatalogEntry {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        input_schema: schema
            .as_object()
            .expect("built-in tool schema must be an object")
            .clone(),
    }
}

fn shell_tool(
    arguments: &Map<String, Value>,
) -> Result<api::message::tool_call::Tool, ToolRequestError> {
    let args: RunShellCommandArgs = deserialize_arguments(arguments)?;
    Ok(api::message::tool_call::Tool::RunShellCommand(
        api::message::tool_call::RunShellCommand {
            command: args.command,
            is_risky: true,
            wait_until_complete_value: Some(
                api::message::tool_call::run_shell_command::WaitUntilCompleteValue::WaitUntilComplete(
                    args.wait_until_complete,
                ),
            ),
            ..Default::default()
        },
    ))
}

fn read_files_tool(
    arguments: &Map<String, Value>,
) -> Result<api::message::tool_call::Tool, ToolRequestError> {
    let args: ReadFilesArgs = deserialize_arguments(arguments)?;
    Ok(api::message::tool_call::Tool::ReadFiles(
        api::message::tool_call::ReadFiles {
            files: args
                .files
                .into_iter()
                .map(|file| api::message::tool_call::read_files::File {
                    name: file.name,
                    line_ranges: file
                        .line_ranges
                        .into_iter()
                        .map(|range| api::FileContentLineRange {
                            start: range.start,
                            end: range.end,
                        })
                        .collect(),
                })
                .collect(),
        },
    ))
}

fn apply_diffs_tool(
    arguments: &Map<String, Value>,
) -> Result<api::message::tool_call::Tool, ToolRequestError> {
    let args: ApplyFileDiffsArgs = deserialize_arguments(arguments)?;
    Ok(api::message::tool_call::Tool::ApplyFileDiffs(
        api::message::tool_call::ApplyFileDiffs {
            summary: args.summary,
            diffs: args
                .diffs
                .into_iter()
                .map(|diff| api::message::tool_call::apply_file_diffs::FileDiff {
                    file_path: diff.file_path,
                    search: diff.search,
                    replace: diff.replace,
                })
                .collect(),
            ..Default::default()
        },
    ))
}

fn mcp_tool(
    server_id: Uuid,
    tool_name: &str,
    arguments: &Map<String, Value>,
) -> Result<api::message::tool_call::Tool, ToolRequestError> {
    Ok(api::message::tool_call::Tool::CallMcpTool(
        api::message::tool_call::CallMcpTool {
            server_id: server_id.to_string(),
            name: tool_name.to_string(),
            args: Some(prost_types::Struct {
                fields: arguments
                    .iter()
                    .map(|(name, value)| Ok((name.clone(), json_to_prost(value)?)))
                    .collect::<Result<_, ToolRequestError>>()?,
            }),
        },
    ))
}

fn deserialize_arguments<T: for<'de> Deserialize<'de>>(
    arguments: &Map<String, Value>,
) -> Result<T, ToolRequestError> {
    serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|_| ToolRequestError::InvalidArguments)
}

fn json_to_prost(value: &Value) -> Result<prost_types::Value, ToolRequestError> {
    let kind = match value {
        Value::Null => Kind::NullValue(0),
        Value::Bool(value) => Kind::BoolValue(*value),
        Value::Number(value) => {
            Kind::NumberValue(value.as_f64().ok_or(ToolRequestError::InvalidArguments)?)
        }
        Value::String(value) => Kind::StringValue(value.clone()),
        Value::Array(values) => Kind::ListValue(prost_types::ListValue {
            values: values.iter().map(json_to_prost).collect::<Result<_, _>>()?,
        }),
        Value::Object(values) => Kind::StructValue(prost_types::Struct {
            fields: values
                .iter()
                .map(|(name, value)| Ok((name.clone(), json_to_prost(value)?)))
                .collect::<Result<_, ToolRequestError>>()?,
        }),
    };
    Ok(prost_types::Value { kind: Some(kind) })
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
#[path = "tool_catalog_tests.rs"]
mod tests;
