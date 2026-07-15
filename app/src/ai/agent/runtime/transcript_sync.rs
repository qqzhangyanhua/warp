use std::fmt;

use serde::Serialize;
use thiserror::Error;

use super::transcript::{RuntimeTranscript, TranscriptItem};

const MAX_JAVASCRIPT_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) struct TranscriptSync {
    lines: Vec<String>,
}

impl TranscriptSync {
    pub(super) fn new(
        sync_id: &str,
        transcript: &RuntimeTranscript,
        max_frame_bytes: usize,
        max_transaction_bytes: usize,
    ) -> Result<Self, TranscriptSyncError> {
        if sync_id.is_empty()
            || transcript.revision() > MAX_JAVASCRIPT_SAFE_INTEGER
            || transcript.items().len() > u32::MAX as usize
        {
            return Err(TranscriptSyncError::InvalidIdentity);
        }

        let item_lines = transcript
            .items()
            .iter()
            .enumerate()
            .map(|(index, item)| {
                serialize(&TranscriptSyncItem {
                    message_type: "transcript_sync_item",
                    sync_id,
                    index: index as u32,
                    item,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let total_bytes = item_lines.iter().try_fold(0_usize, |total, line| {
            total
                .checked_add(line.len())
                .ok_or(TranscriptSyncError::TransactionTooLarge {
                    max_bytes: max_transaction_bytes,
                })
        })?;
        if total_bytes > max_transaction_bytes {
            return Err(TranscriptSyncError::TransactionTooLarge {
                max_bytes: max_transaction_bytes,
            });
        }

        let mut lines = Vec::with_capacity(item_lines.len() + 2);
        lines.push(serialize(&TranscriptSyncBegin {
            message_type: "transcript_sync_begin",
            sync_id,
            conversation_id: transcript.conversation_id(),
            revision: transcript.revision(),
            item_count: item_lines.len() as u32,
            total_bytes: total_bytes as u64,
        })?);
        lines.extend(item_lines);
        lines.push(serialize(&TranscriptSyncCommit {
            message_type: "transcript_sync_commit",
            sync_id,
        })?);

        if lines.iter().any(|line| line.len() > max_frame_bytes) {
            return Err(TranscriptSyncError::FrameTooLarge {
                max_bytes: max_frame_bytes,
            });
        }
        Ok(Self { lines })
    }

    pub(super) fn lines(&self) -> &[String] {
        &self.lines
    }
}

impl fmt::Debug for TranscriptSync {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TranscriptSync")
            .field("frame_count", &self.lines.len())
            .finish()
    }
}

#[derive(Serialize)]
struct TranscriptSyncBegin<'a> {
    #[serde(rename = "type")]
    message_type: &'static str,
    sync_id: &'a str,
    conversation_id: &'a str,
    revision: u64,
    item_count: u32,
    total_bytes: u64,
}

#[derive(Serialize)]
struct TranscriptSyncItem<'a> {
    #[serde(rename = "type")]
    message_type: &'static str,
    sync_id: &'a str,
    index: u32,
    item: &'a TranscriptItem,
}

#[derive(Serialize)]
struct TranscriptSyncCommit<'a> {
    #[serde(rename = "type")]
    message_type: &'static str,
    sync_id: &'a str,
}

fn serialize(value: &impl Serialize) -> Result<String, TranscriptSyncError> {
    serde_json::to_string(value).map_err(|_| TranscriptSyncError::Serialization)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum TranscriptSyncError {
    #[error("Runtime Transcript sync identity is invalid")]
    InvalidIdentity,
    #[error("Runtime Transcript sync could not be serialized")]
    Serialization,
    #[error("Runtime Transcript sync frame exceeds its negotiated byte limit")]
    FrameTooLarge { max_bytes: usize },
    #[error("Runtime Transcript sync exceeds its negotiated total byte limit")]
    TransactionTooLarge { max_bytes: usize },
}
