use warp_multi_agent_api as api;

use super::append_message_action;

#[test]
fn append_message_action_targets_agent_output_text() {
    let action = append_message_action("task-id", "message-id", "request-id", "delta".to_string());

    let Some(api::client_action::Action::AppendToMessageContent(append)) = action.action else {
        panic!("expected append-message action");
    };
    assert_eq!(
        append.mask.expect("append mask should exist").paths,
        vec!["agent_output.text".to_string()]
    );
}
