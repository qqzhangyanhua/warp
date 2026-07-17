use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use warp_multi_agent_api as api;

use crate::ai::agent::MCPContext;

const MAX_PROVIDER_TOOL_NAME_LEN: usize = 64;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct ChatToolDefinition {
    r#type: &'static str,
    function: ChatFunctionDefinition,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ChatToolCallDelta {
    index: usize,
    id: Option<String>,
    function: Option<ChatFunctionCallDelta>,
}

#[derive(Debug, Default, Deserialize)]
struct ChatFunctionCallDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Default)]
struct PartialToolCall {
    id: Option<String>,
    name: String,
    arguments: String,
}

#[derive(Debug, Default)]
pub(super) struct ToolCallAssembler {
    calls: BTreeMap<usize, PartialToolCall>,
}

#[derive(Debug, Clone)]
struct MCPToolRoute {
    server_id: String,
    tool_name: String,
}

#[derive(Debug, Clone)]
pub(super) struct ToolCatalog {
    definitions: Vec<ChatToolDefinition>,
    mcp_routes: HashMap<String, MCPToolRoute>,
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ChatFunctionDefinition {
    name: String,
    description: String,
    parameters: Value,
}

impl ChatToolDefinition {
    fn function(name: &str, description: &str, parameters: Value) -> Self {
        Self {
            r#type: "function",
            function: ChatFunctionDefinition {
                name: name.to_string(),
                description: description.to_string(),
                parameters,
            },
        }
    }
}

pub(super) fn local_tool_definitions() -> Vec<ChatToolDefinition> {
    vec![
        ChatToolDefinition::function(
            "run_shell_command",
            "Run a shell command in the active terminal after permission approval.",
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
        ChatToolDefinition::function(
            "read_files",
            "Read one or more local files after permission approval.",
            json!({
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
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
        ChatToolDefinition::function(
            "apply_file_diffs",
            "Apply search-and-replace file edits after permission approval.",
            json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string" },
                    "diffs": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" },
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

impl ToolCatalog {
    pub(super) fn new(mcp_context: Option<&MCPContext>) -> Self {
        let mut definitions = local_tool_definitions();
        let mut mcp_routes = HashMap::new();

        if let Some(context) = mcp_context {
            for server in &context.servers {
                let Ok(server_uuid) = Uuid::parse_str(&server.id) else {
                    continue;
                };
                for tool in &server.tools {
                    let provider_name = mcp_provider_name(server_uuid, tool.name.as_ref());
                    let description = tool.description.as_deref().unwrap_or_default();
                    let parameters = Value::Object(tool.input_schema.as_ref().clone());
                    definitions.push(ChatToolDefinition::function(
                        &provider_name,
                        description,
                        parameters,
                    ));
                    mcp_routes.insert(
                        provider_name,
                        MCPToolRoute {
                            server_id: server.id.clone(),
                            tool_name: tool.name.to_string(),
                        },
                    );
                }
            }
        }

        Self {
            definitions,
            mcp_routes,
        }
    }

    pub(super) fn definitions(&self) -> Vec<ChatToolDefinition> {
        self.definitions.clone()
    }

    fn resolve(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<api::message::tool_call::Tool, &'static str> {
        if let Some(route) = self.mcp_routes.get(name) {
            return mcp_tool(route, arguments);
        }
        built_in_tool(name, arguments)
    }
}

pub(super) fn mcp_provider_name(server_id: Uuid, tool_name: &str) -> String {
    let sanitized = tool_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let hash = fnv1a_32(format!("{server_id}\0{tool_name}").as_bytes());
    let prefix = format!("mcp_{}", server_id.simple());
    let suffix = format!("_{hash:08x}");
    let available_name_len =
        MAX_PROVIDER_TOOL_NAME_LEN.saturating_sub(prefix.len() + suffix.len() + 1);
    let sanitized = sanitized
        .chars()
        .take(available_name_len)
        .collect::<String>();
    format!("{prefix}_{sanitized}{suffix}")
}

fn fnv1a_32(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c9dc5, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(0x01000193)
    })
}

impl ToolCallAssembler {
    pub(super) fn push(&mut self, deltas: Vec<ChatToolCallDelta>) -> Result<(), &'static str> {
        for delta in deltas {
            let call = self.calls.entry(delta.index).or_default();
            if let Some(id) = delta.id {
                if call.id.as_ref().is_some_and(|existing| existing != &id) {
                    return Err("Provider changed a tool call ID while streaming");
                }
                call.id = Some(id);
            }
            if let Some(function) = delta.function {
                if let Some(name) = function.name {
                    call.name.push_str(&name);
                }
                if let Some(arguments) = function.arguments {
                    call.arguments.push_str(&arguments);
                }
            }
        }
        Ok(())
    }

    pub(super) fn finish(
        self,
        task_id: &str,
        request_id: &str,
        catalog: &ToolCatalog,
    ) -> Result<Vec<api::Message>, &'static str> {
        self.calls
            .into_values()
            .map(|call| tool_call_message(call, task_id, request_id, catalog))
            .collect()
    }
}

fn tool_call_message(
    call: PartialToolCall,
    task_id: &str,
    request_id: &str,
    catalog: &ToolCatalog,
) -> Result<api::Message, &'static str> {
    let tool_call_id = call
        .id
        .filter(|id| !id.is_empty())
        .ok_or("Provider returned a tool call without an ID. Check OpenAI compatibility.")?;
    let arguments: Value = serde_json::from_str(&call.arguments)
        .map_err(|_| "Provider returned malformed tool arguments. The tool was not executed.")?;
    if !arguments.is_object() {
        return Err("Provider tool arguments must be a JSON object. The tool was not executed.");
    }
    let tool = catalog.resolve(&call.name, arguments)?;

    Ok(api::Message {
        id: Uuid::new_v4().to_string(),
        task_id: task_id.to_string(),
        request_id: request_id.to_string(),
        message: Some(api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id,
            tool: Some(tool),
        })),
        ..Default::default()
    })
}

fn mcp_tool(
    route: &MCPToolRoute,
    arguments: Value,
) -> Result<api::message::tool_call::Tool, &'static str> {
    let Value::Object(arguments) = arguments else {
        return Err(
            "Provider MCP tool arguments must be a JSON object. The tool was not executed.",
        );
    };
    let fields = arguments
        .into_iter()
        .map(|(name, value)| json_to_prost(value).map(|value| (name, value)))
        .collect::<Result<BTreeMap<_, _>, _>>()?;

    Ok(api::message::tool_call::Tool::CallMcpTool(
        api::message::tool_call::CallMcpTool {
            name: route.tool_name.clone(),
            args: Some(prost_types::Struct { fields }),
            server_id: route.server_id.clone(),
        },
    ))
}

fn json_to_prost(value: Value) -> Result<prost_types::Value, &'static str> {
    use prost_types::value::Kind;

    let kind = match value {
        Value::Null => Kind::NullValue(0),
        Value::Bool(value) => Kind::BoolValue(value),
        Value::Number(value) => Kind::NumberValue(value.as_f64().ok_or(
            "Provider MCP tool arguments contained an unsupported number. The tool was not executed.",
        )?),
        Value::String(value) => Kind::StringValue(value),
        Value::Array(values) => Kind::ListValue(prost_types::ListValue {
            values: values
                .into_iter()
                .map(json_to_prost)
                .collect::<Result<Vec<_>, _>>()?,
        }),
        Value::Object(values) => Kind::StructValue(prost_types::Struct {
            fields: values
                .into_iter()
                .map(|(name, value)| json_to_prost(value).map(|value| (name, value)))
                .collect::<Result<BTreeMap<_, _>, _>>()?,
        }),
    };
    Ok(prost_types::Value { kind: Some(kind) })
}

fn built_in_tool(
    name: &str,
    arguments: Value,
) -> Result<api::message::tool_call::Tool, &'static str> {
    match name {
        "run_shell_command" => {
            let args: RunShellCommandArgs = serde_json::from_value(arguments).map_err(|_| {
                "Provider returned invalid run_shell_command arguments. The command was not executed."
            })?;
            Ok(api::message::tool_call::Tool::RunShellCommand(
                api::message::tool_call::RunShellCommand {
                    command: args.command,
                    is_read_only: false,
                    uses_pager: false,
                    citations: vec![],
                    is_risky: true,
                    wait_until_complete_value: Some(
                        api::message::tool_call::run_shell_command::WaitUntilCompleteValue::WaitUntilComplete(
                            args.wait_until_complete,
                        ),
                    ),
                    risk_category: 0,
                },
            ))
        }
        "read_files" => {
            let args: ReadFilesArgs = serde_json::from_value(arguments)
                .map_err(|_| "Provider returned invalid read_files arguments. No file was read.")?;
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
        "apply_file_diffs" => {
            let args: ApplyFileDiffsArgs = serde_json::from_value(arguments).map_err(|_| {
                "Provider returned invalid apply_file_diffs arguments. No file was changed."
            })?;
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
                    new_files: vec![],
                    deleted_files: vec![],
                    v4a_updates: vec![],
                },
            ))
        }
        _ => Err("Provider requested an unavailable local tool. The tool was not executed."),
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
#[path = "tool_loop_tests.rs"]
mod tests;
