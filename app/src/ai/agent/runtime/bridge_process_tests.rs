use super::serialize_tool_result_frame;

#[test]
fn tool_result_frame_retains_the_stored_projection_bytes() {
    let projection = br#"{ "content" : [{"type":"text","text":"fixed"}], "truncated" : false, "status" : "success" }"#;

    let frame =
        serialize_tool_result_frame("conversation-1", "run-1", "call-1", projection).unwrap();

    assert_eq!(
        frame,
        r#"{"type":"tool_result","conversation_id":"conversation-1","run_id":"run-1","tool_call_id":"call-1", "content" : [{"type":"text","text":"fixed"}], "truncated" : false, "status" : "success" }"#
    );
}
