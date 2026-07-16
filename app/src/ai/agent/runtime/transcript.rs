use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use warp_multi_agent_api as api;

use super::resources::{ResourceSnapshotBuilder, ResourceSnapshotError};
use crate::ai::agent::conversation::AIConversation;

mod tool_request;

use tool_request::project_tool_request;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum RuntimeContentBlock {
    Text {
        text: String,
    },
    Image {
        mime_type: ImageMimeType,
        data_base64: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(super) enum ImageMimeType {
    #[serde(rename = "image/gif")]
    Gif,
    #[serde(rename = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/webp")]
    Webp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TranscriptRole {
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum TranscriptItem {
    Message {
        message_id: String,
        role: TranscriptRole,
        content: Vec<RuntimeContentBlock>,
    },
    ResourceSnapshot {
        resource_id: String,
        name: String,
        content: Vec<RuntimeContentBlock>,
    },
    ToolRequest {
        tool_call_id: String,
        tool_id: String,
        tool_name: String,
        arguments: serde_json::Map<String, serde_json::Value>,
    },
    ToolResult {
        tool_call_id: String,
        result: ToolResultProjection,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ToolResultProjection {
    Success {
        content: Vec<RuntimeContentBlock>,
        truncated: bool,
    },
    Denied {
        denied_by: ToolDenialSource,
        content: Vec<RuntimeContentBlock>,
        truncated: bool,
    },
    Error {
        error_code: ToolErrorCode,
        may_have_executed: bool,
        content: Vec<RuntimeContentBlock>,
        truncated: bool,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ToolDenialSource {
    Policy,
    User,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ToolErrorCode {
    InvalidToolRequest,
    ToolExecutionFailed,
    ToolRequestLimitExceeded,
    ToolOutcomeUnknown,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(super) struct RunScopedToolCallId {
    run_id: String,
    tool_call_id: String,
}

impl RunScopedToolCallId {
    pub(super) fn new(run_id: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Self {
            run_id: run_id.into(),
            tool_call_id: tool_call_id.into(),
        }
    }

    fn for_message(message: &api::Message, tool_call_id: &str) -> Self {
        Self::new(&message.request_id, tool_call_id)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum TranscriptError {
    #[error(transparent)]
    ResourceSnapshot(#[from] ResourceSnapshotError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RuntimeTranscript {
    conversation_id: String,
    revision: u64,
    items: Vec<TranscriptItem>,
}

impl RuntimeTranscript {
    pub(super) fn project(
        conversation: &AIConversation,
        revision: u64,
        interrupted_message_ids: &HashSet<String>,
        tool_result_projections: &HashMap<RunScopedToolCallId, ToolResultProjection>,
    ) -> Result<Self, TranscriptError> {
        let messages = conversation.all_linearized_messages();
        let paired_tool_call_ids = paired_tool_call_ids(&messages, tool_result_projections);
        let snapshots = ResourceSnapshotBuilder::default().build(messages.iter().copied())?;
        let mut snapshots_by_message =
            snapshots
                .into_iter()
                .fold(HashMap::<_, Vec<_>>::new(), |mut grouped, snapshot| {
                    grouped
                        .entry(snapshot.initiating_message_id.clone())
                        .or_default()
                        .push(snapshot);
                    grouped
                });
        let mut items = Vec::new();
        for message in messages {
            if let Some(item) = project_message(message, interrupted_message_ids) {
                items.push(item);
            }
            if let Some(item) =
                project_tool_activity(message, &paired_tool_call_ids, tool_result_projections)
            {
                items.push(item);
            }
            if let Some(snapshots) = snapshots_by_message.remove(&message.id) {
                items.extend(snapshots.into_iter().map(|snapshot| {
                    TranscriptItem::ResourceSnapshot {
                        resource_id: snapshot.resource_id,
                        name: snapshot.name,
                        content: snapshot.content,
                    }
                }));
            }
        }
        Ok(Self {
            conversation_id: conversation.id().to_string(),
            revision,
            items,
        })
    }

    pub(super) fn conversation_id(&self) -> &str {
        &self.conversation_id
    }

    pub(super) fn revision(&self) -> u64 {
        self.revision
    }

    pub(super) fn set_revision(&mut self, revision: u64) {
        self.revision = revision;
    }

    pub(super) fn items(&self) -> &[TranscriptItem] {
        &self.items
    }

    pub(super) fn append_recovered_tool_activity(
        &mut self,
        revision: u64,
        items: Vec<TranscriptItem>,
    ) {
        self.revision = revision;
        self.items.extend(items);
    }
}

fn paired_tool_call_ids(
    messages: &[&api::Message],
    tool_result_projections: &HashMap<RunScopedToolCallId, ToolResultProjection>,
) -> HashSet<RunScopedToolCallId> {
    let mut projectable_requests = HashSet::new();
    let mut paired = HashSet::new();
    for message in messages {
        if let Some(api::message::Message::ToolCall(tool_call)) = message.message.as_ref() {
            if project_tool_request(tool_call).is_some() {
                projectable_requests.insert(RunScopedToolCallId::for_message(
                    message,
                    &tool_call.tool_call_id,
                ));
            }
        }
        if let Some(api::message::Message::ToolCallResult(result)) = message.message.as_ref() {
            let identity = RunScopedToolCallId::for_message(message, &result.tool_call_id);
            if projectable_requests.contains(&identity)
                && tool_result_projections.contains_key(&identity)
            {
                paired.insert(identity);
            }
        }
    }
    paired
}

fn project_tool_activity(
    message: &api::Message,
    paired_tool_call_ids: &HashSet<RunScopedToolCallId>,
    tool_result_projections: &HashMap<RunScopedToolCallId, ToolResultProjection>,
) -> Option<TranscriptItem> {
    let content = message.message.as_ref()?;
    if let api::message::Message::ToolCall(tool_call) = content {
        let identity = RunScopedToolCallId::for_message(message, &tool_call.tool_call_id);
        if paired_tool_call_ids.contains(&identity) {
            return project_tool_request(tool_call);
        }
    }
    if let api::message::Message::ToolCallResult(result) = content {
        let identity = RunScopedToolCallId::for_message(message, &result.tool_call_id);
        if paired_tool_call_ids.contains(&identity) {
            return Some(TranscriptItem::ToolResult {
                tool_call_id: result.tool_call_id.clone(),
                result: tool_result_projections.get(&identity)?.clone(),
            });
        }
    }
    None
}

fn project_message(
    message: &api::Message,
    interrupted_message_ids: &HashSet<String>,
) -> Option<TranscriptItem> {
    let (role, text) = match message.message.as_ref()? {
        api::message::Message::UserQuery(query) => (TranscriptRole::User, query.query.as_str()),
        api::message::Message::AgentOutput(output)
            if !interrupted_message_ids.contains(&message.id) =>
        {
            (TranscriptRole::Assistant, output.text.as_str())
        }
        api::message::Message::AgentOutput(_)
        | api::message::Message::ToolCall(_)
        | api::message::Message::ToolCallResult(_)
        | api::message::Message::ServerEvent(_)
        | api::message::Message::SystemQuery(_)
        | api::message::Message::UpdateTodos(_)
        | api::message::Message::AgentReasoning(_)
        | api::message::Message::Summarization(_)
        | api::message::Message::CodeReview(_)
        | api::message::Message::UpdateReviewComments(_)
        | api::message::Message::WebSearch(_)
        | api::message::Message::WebFetch(_)
        | api::message::Message::DebugOutput(_)
        | api::message::Message::ArtifactEvent(_)
        | api::message::Message::InvokeSkill(_)
        | api::message::Message::MessagesReceivedFromAgents(_)
        | api::message::Message::ModelUsed(_)
        | api::message::Message::EventsFromAgents(_)
        | api::message::Message::PassiveSuggestionResult(_)
        | api::message::Message::OrchestrationConfigSnapshot(_) => return None,
    };
    if text.is_empty() {
        return None;
    }
    Some(TranscriptItem::Message {
        message_id: message.id.clone(),
        role,
        content: vec![RuntimeContentBlock::Text {
            text: text.to_string(),
        }],
    })
}
