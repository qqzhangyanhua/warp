use warp_multi_agent_api as api;

use super::super::protocol::RuntimeToolRequest;

pub(super) fn tool_request_message(
    request: &RuntimeToolRequest,
    task_id: &str,
    tool: Option<api::message::tool_call::Tool>,
) -> api::Message {
    runtime_message(
        format!("tool-request:{}:{}", request.run_id, request.tool_call_id),
        request,
        task_id,
        api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: request.tool_call_id.clone(),
            tool,
        }),
    )
}

pub(super) fn tool_result_message(
    request: &RuntimeToolRequest,
    task_id: &str,
    result: Option<api::message::tool_call_result::Result>,
) -> api::Message {
    runtime_message(
        format!("tool-result:{}:{}", request.run_id, request.tool_call_id),
        request,
        task_id,
        api::message::Message::ToolCallResult(api::message::ToolCallResult {
            tool_call_id: request.tool_call_id.clone(),
            context: None,
            result,
        }),
    )
}

fn runtime_message(
    id: String,
    request: &RuntimeToolRequest,
    task_id: &str,
    message: api::message::Message,
) -> api::Message {
    api::Message {
        id,
        task_id: task_id.to_string(),
        request_id: request.run_id.clone(),
        message: Some(message),
        ..Default::default()
    }
}
