use serde_json::{json, Map, Value};
use uuid::Uuid;
use warp_multi_agent_api as api;

use super::TranscriptItem;
use crate::ai::agent::provider_tool_name::mcp_provider_name;

pub(super) fn project_tool_request(tool_call: &api::message::ToolCall) -> Option<TranscriptItem> {
    let tool = tool_call.tool.as_ref()?;
    if let api::message::tool_call::Tool::RunShellCommand(command) = tool {
        let mut arguments = Map::from_iter([(
            "command".to_string(),
            Value::String(command.command.clone()),
        )]);
        if let Some(
            api::message::tool_call::run_shell_command::WaitUntilCompleteValue::WaitUntilComplete(
                wait_until_complete,
            ),
        ) = command.wait_until_complete_value
        {
            arguments.insert(
                "wait_until_complete".to_string(),
                Value::Bool(wait_until_complete),
            );
        }
        return Some(request(
            tool_call,
            "builtin.run_shell_command",
            "run_shell_command",
            arguments,
        ));
    }
    if let api::message::tool_call::Tool::ReadFiles(read) = tool {
        return Some(request(
            tool_call,
            "builtin.read_files",
            "read_files",
            object(json!({
                "files": read.files.iter().map(|file| json!({
                    "name": file.name,
                    "line_ranges": file.line_ranges.iter().map(|range| json!({
                        "start": range.start,
                        "end": range.end,
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })),
        ));
    }
    if let api::message::tool_call::Tool::ApplyFileDiffs(apply) = tool {
        return Some(request(
            tool_call,
            "builtin.apply_file_diffs",
            "apply_file_diffs",
            object(json!({
                "summary": apply.summary,
                "diffs": apply.diffs.iter().map(|diff| json!({
                    "file_path": diff.file_path,
                    "search": diff.search,
                    "replace": diff.replace,
                })).collect::<Vec<_>>(),
                "new_files": apply.new_files.iter().map(|file| json!({
                    "file_path": file.file_path,
                    "content": file.content,
                })).collect::<Vec<_>>(),
                "deleted_files": apply.deleted_files.iter().map(|file| json!({
                    "file_path": file.file_path,
                })).collect::<Vec<_>>(),
                "v4a_updates": apply.v4a_updates.iter().map(|update| json!({
                    "file_path": update.file_path,
                    "move_to": update.move_to,
                    "hunks": update.hunks.iter().map(|hunk| json!({
                        "change_context": hunk.change_context,
                        "pre_context": hunk.pre_context,
                        "old": hunk.old,
                        "new": hunk.new,
                        "post_context": hunk.post_context,
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })),
        ));
    }
    if let api::message::tool_call::Tool::CallMcpTool(call) = tool {
        let server_id = Uuid::parse_str(&call.server_id).ok()?;
        let arguments = prost_struct_to_json(call.args.as_ref()?);
        return Some(request(
            tool_call,
            format!("mcp:{server_id}:{}", call.name),
            mcp_provider_name(server_id, &call.name),
            arguments,
        ));
    }
    None
}

fn request(
    tool_call: &api::message::ToolCall,
    tool_id: impl Into<String>,
    tool_name: impl Into<String>,
    arguments: Map<String, Value>,
) -> TranscriptItem {
    TranscriptItem::ToolRequest {
        tool_call_id: tool_call.tool_call_id.clone(),
        tool_id: tool_id.into(),
        tool_name: tool_name.into(),
        arguments,
    }
}

fn object(value: Value) -> Map<String, Value> {
    value
        .as_object()
        .expect("statically constructed tool arguments must be an object")
        .clone()
}

fn prost_struct_to_json(value: &prost_types::Struct) -> Map<String, Value> {
    value
        .fields
        .iter()
        .map(|(name, value)| (name.clone(), prost_value_to_json(value)))
        .collect()
}

fn prost_value_to_json(value: &prost_types::Value) -> Value {
    use prost_types::value::Kind;

    match value.kind.as_ref() {
        Some(Kind::NullValue(_)) | None => Value::Null,
        Some(Kind::NumberValue(value)) => serde_json::Number::from_f64(*value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        Some(Kind::StringValue(value)) => Value::String(value.clone()),
        Some(Kind::BoolValue(value)) => Value::Bool(*value),
        Some(Kind::StructValue(value)) => Value::Object(prost_struct_to_json(value)),
        Some(Kind::ListValue(value)) => {
            Value::Array(value.values.iter().map(prost_value_to_json).collect())
        }
    }
}
