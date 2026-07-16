use super::{RuntimeFailureCode, TextRunOutcome, TextRunResult};
use crate::persistence::model::AgentConversationData;

#[test]
fn only_failed_text_runs_require_process_rebuild() {
    assert!(result(TextRunOutcome::Failed {
        error_code: RuntimeFailureCode::RuntimeFailure,
        diagnostic_id: "failed".to_string(),
    })
    .requires_process_rebuild());
    assert!(!result(TextRunOutcome::LimitReached {
        tool_request_limit: 32,
    })
    .requires_process_rebuild());
    assert!(!result(TextRunOutcome::Completed).requires_process_rebuild());
}

fn result(outcome: TextRunOutcome) -> TextRunResult {
    TextRunResult {
        outcome,
        revision: 0,
        tasks: Vec::new(),
        conversation_data: serde_json::from_str::<AgentConversationData>(
            r#"{"server_conversation_token":null}"#,
        )
        .unwrap(),
    }
}
