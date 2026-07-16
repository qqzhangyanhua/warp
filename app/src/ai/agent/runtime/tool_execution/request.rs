use serde_json::json;
use warp_multi_agent_api as api;

use super::super::protocol::RuntimeToolRequest;
use super::ToolExecutionError;
use crate::ai::agent::task::TaskId;
use crate::ai::agent::{AIAgentAction, AIAgentActionId};

#[allow(deprecated)]
pub(super) fn typed_action(
    request: &RuntimeToolRequest,
    task_id: &str,
    tool: api::message::tool_call::Tool,
) -> Result<AIAgentAction, ToolExecutionError> {
    use api::message::tool_call::Tool as ApiTool;

    let action = match tool {
        ApiTool::RunShellCommand(tool) => tool.into(),
        ApiTool::ReadFiles(tool) => tool.into(),
        ApiTool::ApplyFileDiffs(tool) => tool.into(),
        ApiTool::CallMcpTool(tool) => tool
            .try_into()
            .map_err(|_| ToolExecutionError::InvalidTypedAction)?,
        ApiTool::SearchCodebase(_)
        | ApiTool::Server(_)
        | ApiTool::SuggestPlan(_)
        | ApiTool::SuggestCreatePlan(_)
        | ApiTool::Grep(_)
        | ApiTool::FileGlob(_)
        | ApiTool::ReadMcpResource(_)
        | ApiTool::WriteToLongRunningShellCommand(_)
        | ApiTool::SuggestNewConversation(_)
        | ApiTool::FileGlobV2(_)
        | ApiTool::SuggestPrompt(_)
        | ApiTool::OpenCodeReview(_)
        | ApiTool::InitProject(_)
        | ApiTool::Subagent(_)
        | ApiTool::ReadDocuments(_)
        | ApiTool::EditDocuments(_)
        | ApiTool::CreateDocuments(_)
        | ApiTool::ReadShellCommandOutput(_)
        | ApiTool::UseComputer(_)
        | ApiTool::InsertReviewComments(_)
        | ApiTool::ReadSkill(_)
        | ApiTool::RequestComputerUse(_)
        | ApiTool::FetchConversation(_)
        | ApiTool::StartAgent(_)
        | ApiTool::SendMessageToAgent(_)
        | ApiTool::TransferShellCommandControlToUser(_)
        | ApiTool::AskUserQuestion(_)
        | ApiTool::StartAgentV2(_)
        | ApiTool::UploadFileArtifact(_)
        | ApiTool::RunAgents(_)
        | ApiTool::WaitForEvents(_)
        | ApiTool::StartRecording(_)
        | ApiTool::StopRecording(_) => return Err(ToolExecutionError::InvalidTypedAction),
    };
    Ok(AIAgentAction {
        id: AIAgentActionId::from(request.tool_call_id.clone()),
        task_id: TaskId::new(task_id.to_string()),
        action,
        requires_result: true,
    })
}

pub(super) fn request_payload(request: &RuntimeToolRequest, task_id: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "version": 1,
        "task_id": task_id,
        "tool_id": request.tool_id,
        "tool_name": request.tool_name,
        "arguments": request.arguments,
    }))
    .expect("Tool Request payload must serialize")
}
