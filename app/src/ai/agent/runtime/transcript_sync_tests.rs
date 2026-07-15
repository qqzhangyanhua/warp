use std::collections::{HashMap, HashSet};

use serde_json::json;
use warp_multi_agent_api as api;

use super::transcript::RuntimeTranscript;
use super::transcript_sync::{TranscriptSync, TranscriptSyncError};
use crate::ai::agent::conversation::{AIConversation, AIConversationId};

#[test]
fn builds_bounded_begin_items_commit_transaction() {
    let transcript = transcript_with_user_message("Inspect the workspace", 7);

    let sync = TranscriptSync::new("sync-1", &transcript, 1024, 4096).unwrap();

    assert_eq!(sync.lines().len(), 3);
    let begin: serde_json::Value = serde_json::from_str(&sync.lines()[0]).unwrap();
    let item: serde_json::Value = serde_json::from_str(&sync.lines()[1]).unwrap();
    let commit: serde_json::Value = serde_json::from_str(&sync.lines()[2]).unwrap();
    assert_eq!(
        begin,
        json!({
            "type": "transcript_sync_begin",
            "sync_id": "sync-1",
            "conversation_id": transcript.conversation_id(),
            "revision": 7,
            "item_count": 1,
            "total_bytes": sync.lines()[1].len(),
        })
    );
    assert_eq!(
        item,
        json!({
            "type": "transcript_sync_item",
            "sync_id": "sync-1",
            "index": 0,
            "item": {
                "kind": "message",
                "message_id": "user-1",
                "role": "user",
                "content": [{"type": "text", "text": "Inspect the workspace"}],
            }
        })
    );
    assert_eq!(
        commit,
        json!({"type": "transcript_sync_commit", "sync_id": "sync-1"})
    );
}

#[test]
fn rejects_frames_and_transactions_above_negotiated_limits() {
    let transcript = transcript_with_user_message("Inspect the workspace", 7);

    assert_eq!(
        TranscriptSync::new("sync-1", &transcript, 32, 4096).unwrap_err(),
        TranscriptSyncError::FrameTooLarge { max_bytes: 32 }
    );
    assert_eq!(
        TranscriptSync::new("sync-1", &transcript, 1024, 32).unwrap_err(),
        TranscriptSyncError::TransactionTooLarge { max_bytes: 32 }
    );
}

fn transcript_with_user_message(text: &str, revision: u64) -> RuntimeTranscript {
    let conversation = AIConversation::new_restored(
        AIConversationId::new(),
        vec![api::Task {
            id: "root-task".to_string(),
            messages: vec![api::Message {
                id: "user-1".to_string(),
                task_id: "root-task".to_string(),
                request_id: "run-1".to_string(),
                message: Some(api::message::Message::UserQuery(api::message::UserQuery {
                    query: text.to_string(),
                    ..Default::default()
                })),
                ..Default::default()
            }],
            ..Default::default()
        }],
        None,
    )
    .unwrap();
    RuntimeTranscript::project(&conversation, revision, &HashSet::new(), &HashMap::new()).unwrap()
}
