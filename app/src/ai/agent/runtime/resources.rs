use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use thiserror::Error;
use warp_multi_agent_api as api;

use super::transcript::{ImageMimeType, RuntimeContentBlock};

const DEFAULT_MAX_SNAPSHOTS: usize = 64;
const DEFAULT_MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;
const DEFAULT_MAX_TOTAL_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ResourceSnapshot {
    pub initiating_message_id: String,
    pub resource_id: String,
    pub name: String,
    pub content: Vec<RuntimeContentBlock>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ResourceSnapshotBuilder {
    max_snapshots: usize,
    max_snapshot_bytes: usize,
    max_total_bytes: usize,
}

impl Default for ResourceSnapshotBuilder {
    fn default() -> Self {
        Self {
            max_snapshots: DEFAULT_MAX_SNAPSHOTS,
            max_snapshot_bytes: DEFAULT_MAX_SNAPSHOT_BYTES,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum ResourceSnapshotError {
    #[error("Agent resource catalog contains a local path reference")]
    UnsupportedPathReference,
    #[error("Agent resource catalog contains unsupported content")]
    UnsupportedContent,
    #[error("Agent resource catalog contains an unsupported image MIME type")]
    UnsupportedImageMimeType,
    #[error("A Resource Snapshot exceeds its byte limit")]
    SnapshotTooLarge,
    #[error("The Agent Resource Catalog exceeds its item limit")]
    TooManySnapshots,
    #[error("The Agent Resource Catalog exceeds its total byte limit")]
    TotalBytesExceeded,
}

impl ResourceSnapshotBuilder {
    pub(super) fn build<'a>(
        self,
        messages: impl IntoIterator<Item = &'a api::Message>,
    ) -> Result<Vec<ResourceSnapshot>, ResourceSnapshotError> {
        let mut snapshots = Vec::new();
        let mut total_bytes = 0;
        for message in messages {
            match message.message.as_ref() {
                Some(api::message::Message::UserQuery(query)) => {
                    self.push_user_query_resources(
                        &mut snapshots,
                        &mut total_bytes,
                        &message.id,
                        query,
                    )?;
                }
                Some(api::message::Message::InvokeSkill(invocation)) => {
                    self.push_skill(&mut snapshots, &mut total_bytes, &message.id, invocation)?;
                }
                Some(
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
                    | api::message::Message::MessagesReceivedFromAgents(_)
                    | api::message::Message::ModelUsed(_)
                    | api::message::Message::EventsFromAgents(_)
                    | api::message::Message::PassiveSuggestionResult(_)
                    | api::message::Message::OrchestrationConfigSnapshot(_),
                )
                | None => {}
            }
        }
        Ok(snapshots)
    }

    fn push_user_query_resources(
        self,
        snapshots: &mut Vec<ResourceSnapshot>,
        total_bytes: &mut usize,
        message_id: &str,
        query: &api::message::UserQuery,
    ) -> Result<(), ResourceSnapshotError> {
        if let Some(context) = &query.context {
            for (rules_index, rules) in context.project_rules.iter().enumerate() {
                for (file_index, file) in rules.active_rule_files.iter().enumerate() {
                    self.push_text(
                        snapshots,
                        total_bytes,
                        message_id,
                        format!("{message_id}:rule:{rules_index}:{file_index}"),
                        file.file_path.clone(),
                        file.content.clone(),
                    )?;
                }
            }
            for (index, file) in context.files.iter().enumerate() {
                let Some(file) = &file.content else {
                    continue;
                };
                self.push_text(
                    snapshots,
                    total_bytes,
                    message_id,
                    format!("{message_id}:file:{index}"),
                    file.file_path.clone(),
                    file.content.clone(),
                )?;
            }
            for (index, selection) in context.selected_text.iter().enumerate() {
                self.push_text(
                    snapshots,
                    total_bytes,
                    message_id,
                    format!("{message_id}:selection:{index}"),
                    "Selected text".to_string(),
                    selection.text.clone(),
                )?;
            }
            for (index, image) in context.images.iter().enumerate() {
                let mime_type = parse_image_mime_type(&image.mime_type)?;
                self.push_snapshot(
                    snapshots,
                    total_bytes,
                    ResourceSnapshot {
                        initiating_message_id: message_id.to_string(),
                        resource_id: format!("{message_id}:image:{index}"),
                        name: format!("Image {index}"),
                        content: vec![RuntimeContentBlock::Image {
                            mime_type,
                            data_base64: BASE64_STANDARD.encode(&image.data),
                        }],
                    },
                )?;
            }
        }

        let mut attachments = query.referenced_attachments.iter().collect::<Vec<_>>();
        attachments.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (index, (name, attachment)) in attachments.into_iter().enumerate() {
            let resource_id = format!("{message_id}:attachment:{index}");
            let snapshot =
                snapshot_from_attachment(message_id, resource_id, name.clone(), attachment)?;
            self.push_snapshot(snapshots, total_bytes, snapshot)?;
        }
        Ok(())
    }

    fn push_skill(
        self,
        snapshots: &mut Vec<ResourceSnapshot>,
        total_bytes: &mut usize,
        message_id: &str,
        invocation: &api::message::InvokeSkill,
    ) -> Result<(), ResourceSnapshotError> {
        let Some(skill) = &invocation.skill else {
            return Err(ResourceSnapshotError::UnsupportedContent);
        };
        let Some(content) = &skill.content else {
            return Err(ResourceSnapshotError::UnsupportedContent);
        };
        let name = skill
            .descriptor
            .as_ref()
            .map(|descriptor| descriptor.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| content.file_path.clone());
        self.push_text(
            snapshots,
            total_bytes,
            message_id,
            format!("{message_id}:skill"),
            name,
            content.content.clone(),
        )
    }

    fn push_text(
        self,
        snapshots: &mut Vec<ResourceSnapshot>,
        total_bytes: &mut usize,
        initiating_message_id: &str,
        resource_id: String,
        name: String,
        text: String,
    ) -> Result<(), ResourceSnapshotError> {
        self.push_snapshot(
            snapshots,
            total_bytes,
            ResourceSnapshot {
                initiating_message_id: initiating_message_id.to_string(),
                resource_id,
                name,
                content: vec![RuntimeContentBlock::Text { text }],
            },
        )
    }

    fn push_snapshot(
        self,
        snapshots: &mut Vec<ResourceSnapshot>,
        total_bytes: &mut usize,
        snapshot: ResourceSnapshot,
    ) -> Result<(), ResourceSnapshotError> {
        if snapshots.len() >= self.max_snapshots {
            return Err(ResourceSnapshotError::TooManySnapshots);
        }
        let snapshot_bytes = snapshot.content.iter().map(content_bytes).sum::<usize>();
        if snapshot_bytes > self.max_snapshot_bytes {
            return Err(ResourceSnapshotError::SnapshotTooLarge);
        }
        *total_bytes = total_bytes
            .checked_add(snapshot_bytes)
            .ok_or(ResourceSnapshotError::TotalBytesExceeded)?;
        if *total_bytes > self.max_total_bytes {
            return Err(ResourceSnapshotError::TotalBytesExceeded);
        }
        snapshots.push(snapshot);
        Ok(())
    }
}

fn snapshot_from_attachment(
    initiating_message_id: &str,
    resource_id: String,
    name: String,
    attachment: &api::Attachment,
) -> Result<ResourceSnapshot, ResourceSnapshotError> {
    let text = match attachment.value.as_ref() {
        Some(api::attachment::Value::PlainText(text)) => text.clone(),
        Some(api::attachment::Value::ExecutedShellCommand(command)) => {
            format!("$ {}\n{}", command.command, command.output)
        }
        Some(api::attachment::Value::DocumentContent(document)) => document.content.clone(),
        Some(api::attachment::Value::DiffSet(diff)) => diff
            .hunks
            .iter()
            .map(|hunk| format!("{}\n{}", hunk.file_path, hunk.diff_content))
            .collect::<Vec<_>>()
            .join("\n"),
        #[allow(deprecated)]
        Some(api::attachment::Value::DiffHunk(diff)) => {
            format!("{}\n{}", diff.file_path, diff.diff_content)
        }
        Some(api::attachment::Value::FilePathReference(_)) => {
            return Err(ResourceSnapshotError::UnsupportedPathReference);
        }
        Some(
            api::attachment::Value::RunningShellCommand(_) | api::attachment::Value::DriveObject(_),
        )
        | None => return Err(ResourceSnapshotError::UnsupportedContent),
    };
    Ok(ResourceSnapshot {
        initiating_message_id: initiating_message_id.to_string(),
        resource_id,
        name,
        content: vec![RuntimeContentBlock::Text { text }],
    })
}

fn parse_image_mime_type(mime_type: &str) -> Result<ImageMimeType, ResourceSnapshotError> {
    match mime_type {
        "image/gif" => Ok(ImageMimeType::Gif),
        "image/jpeg" => Ok(ImageMimeType::Jpeg),
        "image/png" => Ok(ImageMimeType::Png),
        "image/webp" => Ok(ImageMimeType::Webp),
        _ => Err(ResourceSnapshotError::UnsupportedImageMimeType),
    }
}

fn content_bytes(content: &RuntimeContentBlock) -> usize {
    match content {
        RuntimeContentBlock::Text { text } => text.len(),
        RuntimeContentBlock::Image { data_base64, .. } => data_base64.len(),
    }
}
