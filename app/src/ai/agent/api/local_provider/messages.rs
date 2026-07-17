use serde::Serialize;
use serde_json::json;
use uuid::Uuid;
use warp_multi_agent_api as api;

use super::tools::mcp_provider_name;
use crate::ai::agent::AIAgentInput;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct ChatMessage {
    pub(super) role: ChatRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) tool_calls: Vec<ChatToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(super) enum ChatRole {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct ChatToolCall {
    id: String,
    r#type: &'static str,
    function: ChatFunctionCall,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ChatFunctionCall {
    name: String,
    arguments: String,
}

pub(super) fn chat_messages_from_tasks(tasks: &[api::Task]) -> Vec<ChatMessage> {
    tasks
        .iter()
        .flat_map(|task| task.messages.iter())
        .filter_map(chat_message_from_api_message)
        .collect()
}

fn chat_message_from_api_message(message: &api::Message) -> Option<ChatMessage> {
    match message.message.as_ref()? {
        api::message::Message::UserQuery(user_query) if !user_query.query.trim().is_empty() => {
            Some(text_message(ChatRole::User, user_query.query.clone()))
        }
        api::message::Message::AgentOutput(output) if !output.text.trim().is_empty() => {
            Some(text_message(ChatRole::Assistant, output.text.clone()))
        }
        api::message::Message::ToolCall(tool_call) => {
            let tool_call = chat_tool_call(tool_call)?;
            Some(ChatMessage {
                role: ChatRole::Assistant,
                content: None,
                tool_calls: vec![tool_call],
                tool_call_id: None,
            })
        }
        _ => None,
    }
}

pub(super) fn chat_messages_from_inputs(inputs: &[AIAgentInput]) -> Vec<ChatMessage> {
    inputs
        .iter()
        .filter_map(|input| match input {
            AIAgentInput::UserQuery { query, .. } if !query.trim().is_empty() => {
                Some(text_message(ChatRole::User, query.clone()))
            }
            AIAgentInput::ActionResult { result, .. } => Some(ChatMessage {
                role: ChatRole::Tool,
                content: Some(result.to_string()),
                tool_calls: vec![],
                tool_call_id: Some(result.id.to_string()),
            }),
            _ => None,
        })
        .collect()
}

fn text_message(role: ChatRole, content: String) -> ChatMessage {
    ChatMessage {
        role,
        content: Some(content),
        tool_calls: vec![],
        tool_call_id: None,
    }
}

fn chat_tool_call(tool_call: &api::message::ToolCall) -> Option<ChatToolCall> {
    let (name, arguments) = match tool_call.tool.as_ref()? {
        api::message::tool_call::Tool::RunShellCommand(command) => (
            "run_shell_command".to_string(),
            json!({
                "command": command.command,
                "wait_until_complete": command.wait_until_complete_value.as_ref().is_none_or(
                    |value| matches!(
                        value,
                        api::message::tool_call::run_shell_command::WaitUntilCompleteValue::WaitUntilComplete(true)
                    )
                )
            }),
        ),
        api::message::tool_call::Tool::ReadFiles(read) => (
            "read_files".to_string(),
            json!({
                "files": read.files.iter().map(|file| json!({
                    "name": file.name,
                    "line_ranges": file.line_ranges.iter().map(|range| json!({
                        "start": range.start,
                        "end": range.end
                    })).collect::<Vec<_>>()
                })).collect::<Vec<_>>()
            }),
        ),
        api::message::tool_call::Tool::ApplyFileDiffs(apply) => (
            "apply_file_diffs".to_string(),
            json!({
                "summary": apply.summary,
                "diffs": apply.diffs.iter().map(|diff| json!({
                    "file_path": diff.file_path,
                    "search": diff.search,
                    "replace": diff.replace
                })).collect::<Vec<_>>()
            }),
        ),
        api::message::tool_call::Tool::CallMcpTool(call) => (
            mcp_provider_name(Uuid::parse_str(&call.server_id).ok()?, &call.name),
            prost_struct_to_json(call.args.as_ref()?),
        ),
        _ => return None,
    };

    Some(ChatToolCall {
        id: tool_call.tool_call_id.clone(),
        r#type: "function",
        function: ChatFunctionCall {
            name: name.to_string(),
            arguments: arguments.to_string(),
        },
    })
}

fn prost_struct_to_json(value: &prost_types::Struct) -> serde_json::Value {
    serde_json::Value::Object(
        value
            .fields
            .iter()
            .map(|(name, value)| (name.clone(), prost_value_to_json(value)))
            .collect(),
    )
}

fn prost_value_to_json(value: &prost_types::Value) -> serde_json::Value {
    use prost_types::value::Kind;

    match value.kind.as_ref() {
        Some(Kind::NullValue(_)) | None => serde_json::Value::Null,
        Some(Kind::NumberValue(value)) => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Some(Kind::StringValue(value)) => serde_json::Value::String(value.clone()),
        Some(Kind::BoolValue(value)) => serde_json::Value::Bool(*value),
        Some(Kind::StructValue(value)) => prost_struct_to_json(value),
        Some(Kind::ListValue(value)) => {
            serde_json::Value::Array(value.values.iter().map(prost_value_to_json).collect())
        }
    }
}
