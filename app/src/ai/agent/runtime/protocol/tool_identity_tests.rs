use super::*;
use crate::ai::agent::runtime::protocol::ProtocolError;

#[test]
fn accepts_completed_tool_redelivery_but_rejects_live_duplicates_and_identity_reuse() {
    let mut lifecycle = TOOL_LIFECYCLE.lines();
    let request = lifecycle
        .next()
        .expect("tool lifecycle fixture must contain a request");
    let result = lifecycle
        .next()
        .expect("tool lifecycle fixture must contain a result");
    let changed_request = request.replace(r#""pwd""#, r#""whoami""#);
    let mut session = ProtocolSession::new(1_048_576, HandshakePolicy::current());
    session
        .receive_inbound(VALID_BRIDGE_HELLO.trim())
        .expect("valid Bridge hello should validate compatibility");
    session
        .authorize_outbound_line(ACCEPTED_HANDSHAKE_RESULT.trim())
        .expect("accepted handshake result should complete the handshake");

    session
        .receive_inbound(request)
        .expect("first tool request should be accepted");
    let duplicate = session
        .receive_inbound(request)
        .expect_err("a live duplicate must fail before its first result");
    assert_eq!(duplicate, SessionError::ToolCallIdentityDuplicated);
    session
        .authorize_outbound_line(result)
        .expect("the matching result should complete the request");
    session
        .receive_inbound(request)
        .expect("completed request redelivery should be idempotent");
    let error = session
        .receive_inbound(&changed_request)
        .expect_err("tool identity reuse for a different request must fail");

    assert_eq!(error, SessionError::ToolCallIdentityReused);
}

#[test]
fn rejects_empty_tool_call_identity() {
    let request = TOOL_LIFECYCLE
        .lines()
        .next()
        .unwrap()
        .replace("\"call-001\"", "\"\"");
    let mut session = ProtocolSession::new(1_048_576, HandshakePolicy::current());
    session.receive_inbound(VALID_BRIDGE_HELLO.trim()).unwrap();
    session
        .authorize_outbound_line(ACCEPTED_HANDSHAKE_RESULT.trim())
        .unwrap();

    assert_eq!(
        session.receive_inbound(&request),
        Err(SessionError::Protocol(ProtocolError::InvalidMessage))
    );
}
