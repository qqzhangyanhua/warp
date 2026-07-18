use std::collections::HashMap;

use serde::Serialize;
use serde_json::json;
use uuid::Uuid;
use warp_multi_agent_api as api;

use super::tools::mcp_provider_name;
use crate::ai::agent::api::convert_conversation::convert_tool_call_result_to_input;
use crate::ai::agent::api::convert_to::convert_context;
use crate::ai::agent::task::TaskId;
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

enum ToolResultSource<'a> {
    Stored(&'a api::message::ToolCallResult),
    Current(&'a AIAgentInput),
}

pub(super) fn chat_messages_from_tasks(
    tasks: &[api::Task],
    inputs: &[AIAgentInput],
) -> Vec<ChatMessage> {
    let mut document_versions = HashMap::new();
    let mut messages = Vec::new();
    for task in tasks {
        let task_id = TaskId::new(task.id.clone());
        let mut pending_tool_calls = HashMap::<String, Vec<usize>>::new();
        let mut tool_results = HashMap::<usize, ToolResultSource<'_>>::new();
        for (index, message) in task.messages.iter().enumerate() {
            match message.message.as_ref() {
                Some(api::message::Message::ToolCall(tool_call)) => pending_tool_calls
                    .entry(tool_call.tool_call_id.clone())
                    .or_default()
                    .push(index),
                Some(api::message::Message::ToolCallResult(result)) => {
                    if let Some(call_index) = pending_tool_calls
                        .get_mut(result.tool_call_id.as_str())
                        .and_then(Vec::pop)
                    {
                        tool_results.insert(call_index, ToolResultSource::Stored(result));
                    }
                }
                _ => {}
            }
        }
        for input in inputs {
            let AIAgentInput::ActionResult { result, .. } = input else {
                continue;
            };
            if result.task_id.to_string() != task.id {
                continue;
            }
            if let Some(call_index) = pending_tool_calls
                .get_mut(result.id.to_string().as_str())
                .and_then(Vec::pop)
            {
                tool_results.insert(call_index, ToolResultSource::Current(input));
            }
        }

        let mut message_index = 0;
        while message_index < task.messages.len() {
            if matches!(
                task.messages[message_index].message.as_ref(),
                Some(api::message::Message::ToolCall(_))
            ) {
                let request_id = &task.messages[message_index].request_id;
                let group_start = message_index;
                while message_index < task.messages.len()
                    && task.messages[message_index].request_id == *request_id
                    && matches!(
                        task.messages[message_index].message.as_ref(),
                        Some(api::message::Message::ToolCall(_))
                    )
                {
                    message_index += 1;
                }

                let tool_calls = task.messages[group_start..message_index]
                    .iter()
                    .enumerate()
                    .filter_map(|(offset, message)| match message.message.as_ref() {
                        Some(api::message::Message::ToolCall(tool_call)) => {
                            Some((group_start + offset, tool_call))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let group_is_resolved = tool_calls.len() == message_index - group_start
                    && !tool_calls.is_empty()
                    && tool_calls
                        .iter()
                        .all(|(index, _)| tool_results.contains_key(index));
                if !group_is_resolved {
                    continue;
                }

                let chat_tool_calls = tool_calls
                    .iter()
                    .map(|(_, tool_call)| chat_tool_call(tool_call))
                    .collect::<Option<Vec<_>>>();
                let group_tool_call_map = tool_calls
                    .iter()
                    .map(|(_, tool_call)| (tool_call.tool_call_id.clone(), *tool_call))
                    .collect::<HashMap<_, _>>();
                let tool_result_messages = tool_calls
                    .iter()
                    .map(|(index, _)| match tool_results.get(index)? {
                        ToolResultSource::Stored(result) => {
                            let input = convert_tool_call_result_to_input(
                                &task_id,
                                result,
                                &group_tool_call_map,
                                &mut document_versions,
                            )?;
                            chat_message_from_tool_result_input(&input)
                        }
                        ToolResultSource::Current(input) => {
                            chat_message_from_tool_result_input(input)
                        }
                    })
                    .collect::<Option<Vec<_>>>();
                if let (Some(tool_calls), Some(tool_results)) =
                    (chat_tool_calls, tool_result_messages)
                {
                    messages.push(ChatMessage {
                        role: ChatRole::Assistant,
                        content: None,
                        tool_calls,
                        tool_call_id: None,
                    });
                    messages.extend(tool_results);
                }
                continue;
            }

            let message = &task.messages[message_index];
            if let Some(message) = chat_message_from_api_message(message) {
                messages.push(message);
            }
            message_index += 1;
        }
    }
    messages
}

fn chat_message_from_api_message(message: &api::Message) -> Option<ChatMessage> {
    match message.message.as_ref()? {
        api::message::Message::UserQuery(user_query) if !user_query.query.trim().is_empty() => {
            Some(text_message(ChatRole::User, user_query.query.clone()))
        }
        api::message::Message::AgentOutput(output) if !output.text.trim().is_empty() => {
            Some(text_message(ChatRole::Assistant, output.text.clone()))
        }
        _ => None,
    }
}

pub(super) fn chat_messages_from_user_inputs(inputs: &[AIAgentInput]) -> Vec<ChatMessage> {
    inputs
        .iter()
        .filter_map(|input| match input {
            AIAgentInput::UserQuery { query, .. } if !query.trim().is_empty() => {
                Some(text_message(ChatRole::User, query.clone()))
            }
            _ => None,
        })
        .collect()
}

fn chat_message_from_tool_result_input(input: &AIAgentInput) -> Option<ChatMessage> {
    match input {
        AIAgentInput::ActionResult { result, .. } => Some(ChatMessage {
            role: ChatRole::Tool,
            content: Some(result.to_string()),
            tool_calls: vec![],
            tool_call_id: Some(result.id.to_string()),
        }),
        _ => None,
    }
}

pub(super) fn local_messages_from_inputs(
    task_id: &str,
    request_id: &str,
    inputs: &[AIAgentInput],
) -> Vec<api::Message> {
    inputs
        .iter()
        .filter_map(|input| match input {
            AIAgentInput::UserQuery {
                query,
                context,
                referenced_attachments,
                user_query_mode,
                intended_agent,
                ..
            } => Some(api::Message {
                id: Uuid::new_v4().to_string(),
                task_id: task_id.to_string(),
                request_id: request_id.to_string(),
                message: Some(api::message::Message::UserQuery(api::message::UserQuery {
                    query: query.clone(),
                    context: Some(convert_context(context.as_ref())),
                    referenced_attachments: referenced_attachments
                        .iter()
                        .map(|(name, attachment)| (name.clone(), attachment.clone().into()))
                        .collect(),
                    mode: Some(user_query_mode.clone().into()),
                    intended_agent: intended_agent.clone().map(Into::into).unwrap_or_default(),
                })),
                ..Default::default()
            }),
            AIAgentInput::ActionResult { result, context } => {
                let result_type = local_tool_call_result(result)?;
                Some(api::Message {
                    id: Uuid::new_v4().to_string(),
                    task_id: task_id.to_string(),
                    request_id: request_id.to_string(),
                    message: Some(api::message::Message::ToolCallResult(
                        api::message::ToolCallResult {
                            tool_call_id: result.id.to_string(),
                            result: Some(result_type),
                            context: Some(convert_context(context.as_ref())),
                        },
                    )),
                    ..Default::default()
                })
            }
            _ => None,
        })
        .collect()
}

#[allow(deprecated)]
fn local_tool_call_result(
    result: &crate::ai::agent::AIAgentActionResult,
) -> Option<api::message::tool_call_result::Result> {
    use api::message::tool_call_result::Result as MessageResult;
    use api::request::input::tool_call_result::Result as InputResult;
    use api::request::input::user_inputs::user_input::Input as UserInput;

    if matches!(
        &result.result,
        crate::ai::agent::AIAgentActionResultType::RequestCommandOutput(
            crate::ai::agent::RequestCommandOutputResult::CancelledBeforeExecution
        ) | crate::ai::agent::AIAgentActionResultType::ReadFiles(
            crate::ai::agent::ReadFilesResult::Cancelled
        ) | crate::ai::agent::AIAgentActionResultType::RequestFileEdits(
            crate::ai::agent::RequestFileEditsResult::Cancelled
        ) | crate::ai::agent::AIAgentActionResultType::CallMCPTool(
            crate::ai::agent::CallMCPToolResult::Cancelled
        )
    ) {
        return Some(MessageResult::Cancel(()));
    }

    let UserInput::ToolCallResult(result) = result.clone().try_into().ok()? else {
        return None;
    };
    match result.result? {
        InputResult::RunShellCommand(result) => Some(MessageResult::RunShellCommand(result)),
        InputResult::ReadFiles(result) => Some(MessageResult::ReadFiles(result)),
        InputResult::ApplyFileDiffs(result) => Some(MessageResult::ApplyFileDiffs(result)),
        InputResult::CallMcpTool(result) => Some(MessageResult::CallMcpTool(result)),
        _ => None,
    }
}

pub(super) fn agent_output_message(
    task_id: &str,
    message_id: &str,
    request_id: &str,
    content: String,
) -> api::Message {
    api::Message {
        id: message_id.to_string(),
        task_id: task_id.to_string(),
        request_id: request_id.to_string(),
        message: Some(api::message::Message::AgentOutput(
            api::message::AgentOutput { text: content },
        )),
        ..Default::default()
    }
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
