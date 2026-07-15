use std::sync::mpsc::SyncSender;

use futures::channel::oneshot;
use warp_multi_agent_api as api;

use super::TextRunRequest;
use crate::ai::agent::runtime::bridge_process::BridgeProcessError;
use crate::ai::agent::runtime::supervisor::RuntimeError;
use crate::persistence::model::{AgentConversationData, AgentRuntimeBinding};
use crate::persistence::{CommitAgentRuntimeMutation, ModelEvent};

pub(super) struct CommittedOutput {
    pub(super) tasks: Vec<api::Task>,
    pub(super) conversation_data: AgentConversationData,
    pub(super) revision: u64,
}

pub(super) struct OutputCommitRequest<'a> {
    pub(super) conversation_id: &'a str,
    pub(super) run_id: &'a str,
    pub(super) output_task_id: &'a str,
    pub(super) commit_id: &'a str,
    pub(super) message_id: &'a str,
    pub(super) expected_revision: u64,
    pub(super) text: String,
    pub(super) tasks: &'a [api::Task],
    pub(super) conversation_data: &'a AgentConversationData,
}

pub(super) async fn commit_output(
    persistence: &SyncSender<ModelEvent>,
    request: OutputCommitRequest<'_>,
) -> Result<CommittedOutput, RuntimeError> {
    let OutputCommitRequest {
        conversation_id,
        run_id,
        output_task_id,
        commit_id,
        message_id,
        expected_revision,
        text,
        tasks,
        conversation_data,
    } = request;
    if text.is_empty() {
        return Err(RuntimeError::InvalidAssistantOutput);
    }
    let mut updated_tasks = tasks.to_vec();
    let task = updated_tasks
        .iter_mut()
        .find(|task| task.id == output_task_id)
        .ok_or(RuntimeError::InvalidAssistantOutput)?;
    match task
        .messages
        .iter()
        .find(|message| message.id == message_id)
    {
        Some(message)
            if message.request_id == run_id
                && matches!(
                    message.message.as_ref(),
                    Some(api::message::Message::AgentOutput(output)) if output.text == text
                ) => {}
        Some(_) => return Err(RuntimeError::InvalidAssistantOutput),
        None => task.messages.push(api::Message {
            id: message_id.to_string(),
            task_id: output_task_id.to_string(),
            request_id: run_id.to_string(),
            message: Some(api::message::Message::AgentOutput(
                api::message::AgentOutput { text },
            )),
            ..Default::default()
        }),
    }

    let (acknowledgement, acknowledged) = oneshot::channel();
    persistence
        .send(ModelEvent::CommitAgentRuntimeMutation(
            CommitAgentRuntimeMutation {
                conversation_id: conversation_id.to_string(),
                run_id: run_id.to_string(),
                commit_id: commit_id.to_string(),
                expected_revision,
                updated_tasks: updated_tasks.clone(),
                conversation_data: conversation_data.clone(),
                sidecar_mutation: None,
                acknowledgement,
            },
        ))
        .map_err(|_| RuntimeError::PersistenceUnavailable)?;
    let revision = acknowledged
        .await
        .map_err(|_| RuntimeError::PersistenceAcknowledgementDropped)??;
    let mut conversation_data = conversation_data.clone();
    conversation_data.runtime_binding = Some(AgentRuntimeBinding::Pi);
    conversation_data.runtime_transcript_revision = Some(revision);
    Ok(CommittedOutput {
        tasks: updated_tasks,
        conversation_data,
        revision,
    })
}

pub(super) async fn commit_interrupted_output(
    persistence: &SyncSender<ModelEvent>,
    conversation_id: &str,
    request: &mut TextRunRequest,
    revision: &mut u64,
    partial_text: &mut String,
    partial_event_id: &mut Option<String>,
) -> Result<(), RuntimeError> {
    if partial_text.is_empty() {
        return Ok(());
    }
    let event_id = partial_event_id
        .take()
        .ok_or(BridgeProcessError::ProtocolViolation)?;
    let commit_id = format!("interrupted:{}:{event_id}", request.run_id);
    let message_id = format!("interrupted:{}", request.run_id);
    let committed = commit_output(
        persistence,
        OutputCommitRequest {
            conversation_id,
            run_id: &request.run_id,
            output_task_id: &request.output_task_id,
            commit_id: &commit_id,
            message_id: &message_id,
            expected_revision: *revision,
            text: std::mem::take(partial_text),
            tasks: &request.tasks,
            conversation_data: &request.conversation_data,
        },
    )
    .await?;
    request.tasks = committed.tasks;
    request.conversation_data = committed.conversation_data;
    *revision = committed.revision;
    Ok(())
}
