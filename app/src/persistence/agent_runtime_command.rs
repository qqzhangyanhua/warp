use futures::channel::oneshot;
use prost_reflect::{DynamicMessage, MapKey, ReflectMessage as _, Value};
use sha2::{Digest as _, Sha256};
use warp_multi_agent_api as api;

use super::model::{
    AgentConversationData, AgentRuntimeBinding, VersionedCompleteToolOutcome,
    VersionedToolResultProjection, COMPLETE_TOOL_OUTCOME_ENCODING_VERSION,
    TOOL_RESULT_PROJECTION_ENCODING_VERSION,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteToolOutcomePayload {
    encoding_version: i32,
    bytes: Vec<u8>,
}

impl CompleteToolOutcomePayload {
    pub fn current(bytes: Vec<u8>) -> Self {
        Self {
            encoding_version: COMPLETE_TOOL_OUTCOME_ENCODING_VERSION,
            bytes,
        }
    }

    pub fn from_parts(encoding_version: i32, bytes: Vec<u8>) -> Option<Self> {
        VersionedCompleteToolOutcome::from_parts(encoding_version, &bytes)?;
        Some(Self {
            encoding_version,
            bytes,
        })
    }

    pub(super) fn versioned(&self) -> VersionedCompleteToolOutcome<'_> {
        VersionedCompleteToolOutcome::from_parts(self.encoding_version, &self.bytes)
            .expect("Complete Tool Outcome payload version was validated at construction")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResultProjectionPayload {
    encoding_version: i32,
    bytes: Vec<u8>,
}

impl ToolResultProjectionPayload {
    pub fn current(bytes: Vec<u8>) -> Self {
        Self {
            encoding_version: TOOL_RESULT_PROJECTION_ENCODING_VERSION,
            bytes,
        }
    }

    pub fn from_parts(encoding_version: i32, bytes: Vec<u8>) -> Option<Self> {
        VersionedToolResultProjection::from_parts(encoding_version, &bytes)?;
        Some(Self {
            encoding_version,
            bytes,
        })
    }

    pub(super) fn versioned(&self) -> VersionedToolResultProjection<'_> {
        VersionedToolResultProjection::from_parts(self.encoding_version, &self.bytes)
            .expect("Tool Result Projection payload version was validated at construction")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentRuntimeSidecarMutation {
    CompleteToolExecution {
        tool_call_id: String,
        complete_outcome: CompleteToolOutcomePayload,
        tool_result_projection: ToolResultProjectionPayload,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CommitAgentRuntimeMutationError {
    #[error("Conversation Record does not exist")]
    ConversationNotFound,
    #[error("Conversation Record is not Pi-bound")]
    RuntimeBindingMismatch,
    #[error("Agent Run Record does not exist")]
    RunNotFound,
    #[error("Commit identity was already used for a different Agent Runtime mutation")]
    CommitIdentityConflict,
    #[error("Conversation Record revision conflict: expected {expected}, found {actual}")]
    RevisionConflict { expected: u64, actual: u64 },
    #[error("Conversation Record revision cannot be represented by SQLite")]
    RevisionOverflow,
    #[error("Failed to commit Agent Runtime mutation")]
    Persistence,
}

impl From<diesel::result::Error> for CommitAgentRuntimeMutationError {
    fn from(_: diesel::result::Error) -> Self {
        Self::Persistence
    }
}

#[derive(Debug)]
pub struct CommitAgentRuntimeMutation {
    pub conversation_id: String,
    pub run_id: String,
    pub commit_id: String,
    pub expected_revision: u64,
    pub updated_tasks: Vec<api::Task>,
    pub conversation_data: AgentConversationData,
    pub sidecar_mutation: Option<AgentRuntimeSidecarMutation>,
    pub acknowledgement: oneshot::Sender<Result<u64, CommitAgentRuntimeMutationError>>,
}

impl CommitAgentRuntimeMutation {
    pub(super) fn payload_fingerprint(&self) -> Result<[u8; 32], CommitAgentRuntimeMutationError> {
        let mut hasher = Sha256::new();
        hash_bytes(&mut hasher, b"warp-agent-runtime-commit-v1");
        hash_bytes(&mut hasher, self.conversation_id.as_bytes());
        hash_bytes(&mut hasher, self.run_id.as_bytes());
        hasher.update(self.expected_revision.to_le_bytes());

        let mut tasks = self.updated_tasks.iter().collect::<Vec<_>>();
        tasks.sort_by(|left, right| left.id.cmp(&right.id));
        hasher.update((tasks.len() as u64).to_le_bytes());
        for task in tasks {
            hash_proto_message(&mut hasher, &task.transcode_to_dynamic());
        }

        let committed_revision = self
            .expected_revision
            .checked_add(1)
            .ok_or(CommitAgentRuntimeMutationError::RevisionOverflow)?;
        let mut conversation_data = self.conversation_data.clone();
        conversation_data.runtime_binding = Some(AgentRuntimeBinding::Pi);
        conversation_data.runtime_transcript_revision = Some(committed_revision);
        let mut conversation_data = serde_json::to_value(conversation_data)
            .map_err(|_| CommitAgentRuntimeMutationError::Persistence)?;
        normalize_unordered_conversation_data(&mut conversation_data);
        hash_json_value(&mut hasher, &conversation_data);

        match &self.sidecar_mutation {
            None => hasher.update([0]),
            Some(AgentRuntimeSidecarMutation::CompleteToolExecution {
                tool_call_id,
                complete_outcome,
                tool_result_projection,
            }) => {
                hasher.update([1]);
                hash_bytes(&mut hasher, tool_call_id.as_bytes());
                hasher.update(complete_outcome.encoding_version.to_le_bytes());
                hash_bytes(&mut hasher, &complete_outcome.bytes);
                hasher.update(tool_result_projection.encoding_version.to_le_bytes());
                hash_bytes(&mut hasher, &tool_result_projection.bytes);
            }
        }

        Ok(hasher.finalize().into())
    }
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn hash_proto_message(hasher: &mut Sha256, message: &DynamicMessage) {
    let mut fields = message.fields().collect::<Vec<_>>();
    fields.sort_by_key(|(field, _)| field.number());
    hasher.update((fields.len() as u64).to_le_bytes());
    for (field, value) in fields {
        hasher.update(field.number().to_le_bytes());
        hash_proto_value(hasher, value);
    }

    let unknown_fields = message.unknown_fields().collect::<Vec<_>>();
    hasher.update((unknown_fields.len() as u64).to_le_bytes());
    for field in unknown_fields {
        let mut encoded = Vec::with_capacity(field.encoded_len());
        field.encode(&mut encoded);
        hash_bytes(hasher, &encoded);
    }
}

fn hash_proto_value(hasher: &mut Sha256, value: &Value) {
    match value {
        Value::Bool(value) => hasher.update([0, u8::from(*value)]),
        Value::I32(value) => hash_number(hasher, 1, value.to_le_bytes()),
        Value::I64(value) => hash_number(hasher, 2, value.to_le_bytes()),
        Value::U32(value) => hash_number(hasher, 3, value.to_le_bytes()),
        Value::U64(value) => hash_number(hasher, 4, value.to_le_bytes()),
        Value::F32(value) => hash_number(hasher, 5, value.to_bits().to_le_bytes()),
        Value::F64(value) => hash_number(hasher, 6, value.to_bits().to_le_bytes()),
        Value::String(value) => hash_tagged_bytes(hasher, 7, value.as_bytes()),
        Value::Bytes(value) => hash_tagged_bytes(hasher, 8, value),
        Value::EnumNumber(value) => hash_number(hasher, 9, value.to_le_bytes()),
        Value::Message(value) => {
            hasher.update([10]);
            hash_proto_message(hasher, value);
        }
        Value::List(values) => {
            hasher.update([11]);
            hasher.update((values.len() as u64).to_le_bytes());
            for value in values {
                hash_proto_value(hasher, value);
            }
        }
        Value::Map(values) => {
            hasher.update([12]);
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            hasher.update((entries.len() as u64).to_le_bytes());
            for (key, value) in entries {
                hash_proto_map_key(hasher, key);
                hash_proto_value(hasher, value);
            }
        }
    }
}

fn hash_proto_map_key(hasher: &mut Sha256, key: &MapKey) {
    match key {
        MapKey::Bool(value) => hasher.update([0, u8::from(*value)]),
        MapKey::I32(value) => hash_number(hasher, 1, value.to_le_bytes()),
        MapKey::I64(value) => hash_number(hasher, 2, value.to_le_bytes()),
        MapKey::U32(value) => hash_number(hasher, 3, value.to_le_bytes()),
        MapKey::U64(value) => hash_number(hasher, 4, value.to_le_bytes()),
        MapKey::String(value) => hash_tagged_bytes(hasher, 5, value.as_bytes()),
    }
}

fn hash_number<const N: usize>(hasher: &mut Sha256, tag: u8, bytes: [u8; N]) {
    hasher.update([tag]);
    hasher.update(bytes);
}

fn hash_tagged_bytes(hasher: &mut Sha256, tag: u8, bytes: &[u8]) {
    hasher.update([tag]);
    hash_bytes(hasher, bytes);
}

fn normalize_unordered_conversation_data(value: &mut serde_json::Value) {
    let Some(reverted_action_ids) = value
        .get_mut("reverted_action_ids")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    reverted_action_ids.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
}

fn hash_json_value(hasher: &mut Sha256, value: &serde_json::Value) {
    match value {
        serde_json::Value::Null => hasher.update([0]),
        serde_json::Value::Bool(value) => hasher.update([1, u8::from(*value)]),
        serde_json::Value::Number(value) => {
            hasher.update([2]);
            hash_bytes(hasher, value.to_string().as_bytes());
        }
        serde_json::Value::String(value) => {
            hasher.update([3]);
            hash_bytes(hasher, value.as_bytes());
        }
        serde_json::Value::Array(values) => {
            hasher.update([4]);
            hasher.update((values.len() as u64).to_le_bytes());
            for value in values {
                hash_json_value(hasher, value);
            }
        }
        serde_json::Value::Object(values) => {
            hasher.update([5]);
            let mut fields = values.iter().collect::<Vec<_>>();
            fields.sort_by(|(left, _), (right, _)| left.cmp(right));
            hasher.update((fields.len() as u64).to_le_bytes());
            for (field, value) in fields {
                hash_bytes(hasher, field.as_bytes());
                hash_json_value(hasher, value);
            }
        }
    }
}
