use super::{
    HandshakeError, HandshakePolicy, ProtocolCapability, ProtocolCodec, ProtocolMessage,
    ProtocolSession, SessionError,
};

const VALID_BRIDGE_HELLO: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tools/warp-bridge/protocol/fixtures/valid/bridge-hello.jsonl"
));
const BRIDGE_HELLO_WITH_UNKNOWN_FIELD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tools/warp-bridge/protocol/fixtures/invalid/bridge-hello-unknown-field.jsonl"
));
const ACCEPTED_HANDSHAKE_RESULT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tools/warp-bridge/protocol/fixtures/valid/handshake-result-accepted.jsonl"
));
const REJECTED_HANDSHAKE_RESULT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tools/warp-bridge/protocol/fixtures/valid/handshake-result-rejected.jsonl"
));
const TRANSCRIPT_SYNC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tools/warp-bridge/protocol/fixtures/valid/transcript-sync.jsonl"
));
const TEXT_RUN_LIFECYCLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tools/warp-bridge/protocol/fixtures/valid/text-run-lifecycle.jsonl"
));
const TOOL_LIFECYCLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tools/warp-bridge/protocol/fixtures/valid/tool-lifecycle.jsonl"
));
const RUN_TERMINAL_OUTCOMES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tools/warp-bridge/protocol/fixtures/valid/run-terminal-outcomes.jsonl"
));

#[test]
fn parses_valid_bridge_hello_fixture() {
    let message = ProtocolMessage::parse_line(VALID_BRIDGE_HELLO.trim())
        .expect("valid bridge hello fixture should parse");

    let ProtocolMessage::BridgeHello(hello) = message else {
        panic!("expected bridge hello");
    };
    assert_eq!(hello.protocol_version, 2);
    assert_eq!(hello.bridge_version, "0.1.0");
    assert_eq!(hello.prompt_version, "warp.v1");
    assert_eq!(
        hello.core_schema_hash,
        "sha256:7a44caef7fc85b2719d1c3ae7f98bab98f221287a4de6541d6386d1f590c578c"
    );
    assert_eq!(hello.capabilities.len(), 1);
    assert_eq!(hello.capabilities[0].name, "usage.v1");
    assert_eq!(hello.capabilities[0].version, 1);
    assert_eq!(
        hello.capabilities[0].schema_hash,
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
}

#[test]
fn accepts_bridge_hello_with_current_core_schema() {
    let message = ProtocolMessage::parse_line(VALID_BRIDGE_HELLO.trim())
        .expect("valid bridge hello fixture should parse");
    let ProtocolMessage::BridgeHello(hello) = message else {
        panic!("expected bridge hello");
    };

    HandshakePolicy::current()
        .validate(&hello)
        .expect("current protocol and schema should be compatible");
}

#[test]
fn rejects_bridge_hello_with_mismatched_core_identity() {
    let message = ProtocolMessage::parse_line(VALID_BRIDGE_HELLO.trim())
        .expect("valid bridge hello fixture should parse");
    let ProtocolMessage::BridgeHello(mut hello) = message else {
        panic!("expected bridge hello");
    };

    hello.protocol_version = 3;
    assert_eq!(
        HandshakePolicy::current().validate(&hello),
        Err(HandshakeError::ProtocolVersionMismatch {
            expected: 2,
            actual: 3,
        })
    );

    hello.protocol_version = 2;
    hello.core_schema_hash =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
    assert_eq!(
        HandshakePolicy::current().validate(&hello),
        Err(HandshakeError::CoreSchemaMismatch)
    );
}

#[test]
fn rejects_bridge_hello_without_required_capability() {
    let message = ProtocolMessage::parse_line(VALID_BRIDGE_HELLO.trim())
        .expect("valid bridge hello fixture should parse");
    let ProtocolMessage::BridgeHello(hello) = message else {
        panic!("expected bridge hello");
    };
    let policy = HandshakePolicy::current().requiring_capability(ProtocolCapability {
        name: "parallel_tools.v1".to_string(),
        version: 1,
        schema_hash: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
            .to_string(),
    });

    assert_eq!(
        policy.validate(&hello),
        Err(HandshakeError::MissingRequiredCapability {
            name: "parallel_tools.v1".to_string(),
        })
    );
}

#[test]
fn rejects_required_capability_with_mismatched_version_or_schema() {
    let message = ProtocolMessage::parse_line(VALID_BRIDGE_HELLO.trim())
        .expect("valid bridge hello fixture should parse");
    let ProtocolMessage::BridgeHello(hello) = message else {
        panic!("expected bridge hello");
    };

    for required in [
        ProtocolCapability {
            name: "usage.v1".to_string(),
            version: 2,
            schema_hash: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                .to_string(),
        },
        ProtocolCapability {
            name: "usage.v1".to_string(),
            version: 1,
            schema_hash: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .to_string(),
        },
    ] {
        let policy = HandshakePolicy::current().requiring_capability(required);
        assert_eq!(
            policy.validate(&hello),
            Err(HandshakeError::MissingRequiredCapability {
                name: "usage.v1".to_string(),
            })
        );
    }
}

#[test]
fn rejects_bridge_hello_with_unknown_field() {
    let result = ProtocolMessage::parse_line(BRIDGE_HELLO_WITH_UNKNOWN_FIELD.trim());

    assert!(result.is_err(), "unknown protocol fields must fail closed");
}

#[test]
fn parses_accepted_handshake_result_limits() {
    let message = ProtocolMessage::parse_line(ACCEPTED_HANDSHAKE_RESULT.trim())
        .expect("accepted handshake result fixture should parse");

    let ProtocolMessage::HandshakeResult(result) = message else {
        panic!("expected handshake result");
    };
    assert_eq!(result.max_frame_bytes(), Some(1_048_576));
    assert_eq!(result.max_transcript_bytes(), Some(16_777_216));
}

#[test]
fn parses_rejected_handshake_result_without_content() {
    let message = ProtocolMessage::parse_line(REJECTED_HANDSHAKE_RESULT.trim())
        .expect("rejected handshake result fixture should parse");

    let ProtocolMessage::HandshakeResult(result) = message else {
        panic!("expected handshake result");
    };
    assert_eq!(result.error_code(), Some("core_schema_mismatch"));
    assert_eq!(result.diagnostic_id(), Some("diag-001"));
}

#[test]
fn parses_complete_transactional_transcript_sync() {
    let messages = TRANSCRIPT_SYNC
        .lines()
        .map(ProtocolMessage::parse_line)
        .collect::<Result<Vec<_>, _>>()
        .expect("complete transcript sync fixture should parse");

    assert_eq!(messages.len(), 4);
    assert!(matches!(
        &messages[0],
        ProtocolMessage::TranscriptSyncBegin(begin)
            if begin.sync_id == "sync-001"
                && begin.revision == 7
                && begin.item_count == 1
    ));
    assert!(matches!(
        &messages[1],
        ProtocolMessage::TranscriptSyncItem(item)
            if item.sync_id == "sync-001" && item.index == 0
    ));
    assert!(matches!(
        &messages[2],
        ProtocolMessage::TranscriptSyncCommit(commit) if commit.sync_id == "sync-001"
    ));
    assert!(matches!(
        &messages[3],
        ProtocolMessage::TranscriptSyncResult(result)
            if result.sync_id() == "sync-001" && result.accepted_revision() == Some(7)
    ));
}

#[test]
fn parses_text_run_with_commit_barrier_and_terminal_outcome() {
    let messages = TEXT_RUN_LIFECYCLE
        .lines()
        .map(ProtocolMessage::parse_line)
        .collect::<Result<Vec<_>, _>>()
        .expect("text run lifecycle fixture should parse");

    assert_eq!(messages.len(), 7);
    assert!(matches!(
        &messages[0],
        ProtocolMessage::RunStart(start)
            if start.run_id == "run-001" && start.transcript_revision == 7
    ));
    assert!(matches!(
        &messages[3],
        ProtocolMessage::AssistantMessageCommit(commit)
            if commit.commit_id == "commit-001" && commit.expected_revision == 7
    ));
    assert!(matches!(
        &messages[4],
        ProtocolMessage::CommitResult(result)
            if result.commit_id() == "commit-001" && result.committed_revision() == 8
    ));
    assert!(matches!(&messages[5], ProtocolMessage::RunCancel(_)));
    assert!(matches!(
        &messages[6],
        ProtocolMessage::RunFinished(finished) if finished.is_completed()
    ));
}

#[test]
fn parses_structured_tool_results_including_indeterminate_execution() {
    let messages = TOOL_LIFECYCLE
        .lines()
        .map(ProtocolMessage::parse_line)
        .collect::<Result<Vec<_>, _>>()
        .expect("tool lifecycle fixture should parse");

    assert_eq!(messages.len(), 4);
    assert!(matches!(
        &messages[0],
        ProtocolMessage::ToolRequest(request) if request.tool_call_id == "call-001"
    ));
    assert!(matches!(
        &messages[2],
        ProtocolMessage::ToolResult(result) if result.is_denied()
    ));
    assert!(matches!(
        &messages[3],
        ProtocolMessage::ToolResult(result)
            if result.error_code() == Some("tool_outcome_unknown")
                && result.may_have_executed() == Some(true)
    ));
}

#[test]
fn parses_typed_content_free_terminal_outcomes() {
    let messages = RUN_TERMINAL_OUTCOMES
        .lines()
        .map(ProtocolMessage::parse_line)
        .collect::<Result<Vec<_>, _>>()
        .expect("terminal outcome fixtures should parse");

    assert_eq!(messages.len(), 4);
    assert!(matches!(
        &messages[0],
        ProtocolMessage::RunFinished(finished) if finished.is_cancelled()
    ));
    assert!(matches!(
        &messages[1],
        ProtocolMessage::RunFinished(finished)
            if finished.error_code() == Some("provider_transport_error")
    ));
    assert!(matches!(
        &messages[2],
        ProtocolMessage::RunFinished(finished)
            if finished.error_code() == Some("provider_redirect_not_allowed")
    ));
    assert!(matches!(
        &messages[3],
        ProtocolMessage::RunFinished(finished) if finished.tool_request_limit() == Some(32)
    ));
}

#[test]
fn rejects_oversized_frame_without_echoing_content() {
    let secret = "secret-provider-body";
    let line = format!(r#"{{"type":"{secret}"}}"#);

    let error = ProtocolCodec::new(8)
        .parse_line(&line)
        .expect_err("oversized frame must be rejected before parsing");
    let diagnostic = format!("{error:?} {error}");

    assert!(!diagnostic.contains(secret));
}

#[test]
fn bounded_reader_rejects_before_buffering_the_whole_frame() {
    use std::io::Cursor;

    let mut reader = Cursor::new(vec![b'x'; 1024]);
    let error = ProtocolCodec::new(8)
        .read_frame(&mut reader)
        .expect_err("oversized frame must stop at the codec limit");

    assert_eq!(
        error,
        super::ProtocolError::FrameTooLarge { max_frame_bytes: 8 }
    );
    assert!(reader.position() <= 9, "reader consumed an unbounded frame");
}

#[test]
fn refuses_run_configuration_until_bridge_handshake_succeeds() {
    let run_start = TEXT_RUN_LIFECYCLE
        .lines()
        .next()
        .expect("run start fixture must contain a first line");
    let mut session = ProtocolSession::new(1_048_576, HandshakePolicy::current());

    let error = session
        .authorize_outbound_line(run_start)
        .expect_err("run configuration must not be authorized before handshake");
    assert_eq!(error, SessionError::HandshakeRequired);
    assert!(!format!("{error:?} {error}").contains("fixture-key"));

    session
        .receive_inbound(VALID_BRIDGE_HELLO.trim())
        .expect("valid Bridge hello should validate compatibility");
    assert_eq!(
        session.authorize_outbound_line(run_start),
        Err(SessionError::HandshakeResultRequired)
    );
    session
        .authorize_outbound_line(ACCEPTED_HANDSHAKE_RESULT.trim())
        .expect("accepted handshake result should complete the handshake");
    session
        .authorize_outbound_line(run_start)
        .expect("run configuration should be authorized after handshake");
}

#[test]
fn accepts_identical_tool_redelivery_but_rejects_identity_reuse_with_new_arguments() {
    let request = TOOL_LIFECYCLE
        .lines()
        .next()
        .expect("tool lifecycle fixture must contain a request");
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
    session
        .receive_inbound(request)
        .expect("identical tool request redelivery should be idempotent");
    let error = session
        .receive_inbound(&changed_request)
        .expect_err("tool identity reuse for a different request must fail");

    assert_eq!(error, SessionError::ToolCallIdentityReused);
}

#[test]
fn protocol_errors_and_debug_output_redact_supplied_content() {
    let run_start = TEXT_RUN_LIFECYCLE
        .lines()
        .next()
        .expect("run start fixture must contain a first line");
    let message = ProtocolMessage::parse_line(run_start).expect("run start fixture should parse");
    let debug = format!("{message:?}");
    for content in ["fixture-key", "local-model", "/workspace", "127.0.0.1"] {
        assert!(!debug.contains(content), "Debug leaked protocol content");
    }

    let malformed = r#"{"type":"secret-provider-body""#;
    let error = ProtocolMessage::parse_line(malformed)
        .expect_err("malformed JSON must produce a protocol error");
    assert!(!format!("{error:?} {error}").contains("secret-provider-body"));
}

#[test]
fn sensitive_payload_debug_output_is_content_free() {
    let text_messages = TEXT_RUN_LIFECYCLE
        .lines()
        .map(ProtocolMessage::parse_line)
        .collect::<Result<Vec<_>, _>>()
        .expect("text lifecycle fixture should parse");
    let tool_messages = TOOL_LIFECYCLE
        .lines()
        .map(ProtocolMessage::parse_line)
        .collect::<Result<Vec<_>, _>>()
        .expect("tool lifecycle fixture should parse");

    let debug = format!(
        "{:?} {:?} {:?} {:?} {:?}",
        match &text_messages[0] {
            ProtocolMessage::RunStart(payload) => payload,
            _ => panic!("expected run start"),
        },
        match &text_messages[2] {
            ProtocolMessage::TextDelta(payload) => payload,
            _ => panic!("expected text delta"),
        },
        match &text_messages[3] {
            ProtocolMessage::AssistantMessageCommit(payload) => payload,
            _ => panic!("expected assistant message commit"),
        },
        match &tool_messages[0] {
            ProtocolMessage::ToolRequest(payload) => payload,
            _ => panic!("expected tool request"),
        },
        match &tool_messages[1] {
            ProtocolMessage::ToolResult(payload) => payload,
            _ => panic!("expected tool result"),
        },
    );

    for content in [
        "fixture-key",
        "local-model",
        "/workspace",
        "127.0.0.1",
        "Running tests now.",
        "pwd",
        "/workspace/project",
    ] {
        assert!(!debug.contains(content), "Debug leaked protocol content");
    }
}

#[test]
fn rust_boundary_matches_all_shared_conformance_fixtures() {
    let fixtures_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tools/warp-bridge/protocol/fixtures");

    for entry in
        std::fs::read_dir(fixtures_root.join("valid")).expect("valid fixtures should exist")
    {
        let path = entry
            .expect("valid fixture entry should be readable")
            .path();
        let fixture = std::fs::read_to_string(&path).expect("valid fixture should be readable");
        for line in fixture.lines().filter(|line| !line.is_empty()) {
            ProtocolMessage::parse_line(line)
                .unwrap_or_else(|_| panic!("valid fixture failed: {}", path.display()));
        }
    }

    for entry in
        std::fs::read_dir(fixtures_root.join("invalid")).expect("invalid fixtures should exist")
    {
        let path = entry
            .expect("invalid fixture entry should be readable")
            .path();
        let fixture = std::fs::read_to_string(&path).expect("invalid fixture should be readable");
        for line in fixture.lines().filter(|line| !line.is_empty()) {
            assert!(
                ProtocolMessage::parse_line(line).is_err(),
                "invalid fixture passed: {}",
                path.display()
            );
        }
    }
}
