use std::collections::HashMap;

use sha2::{Digest as _, Sha256};
use thiserror::Error;

mod wire;

use wire::*;

const CORE_PROTOCOL_VERSION: u32 = 1;
const CORE_SCHEMA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../tools/warp-bridge/protocol/core-v1.schema.json"
));

struct ProtocolCodec {
    max_frame_bytes: usize,
}

impl ProtocolCodec {
    fn new(max_frame_bytes: usize) -> Self {
        Self { max_frame_bytes }
    }

    fn parse_line(&self, line: &str) -> Result<ProtocolMessage, ProtocolError> {
        if line.len() > self.max_frame_bytes {
            return Err(ProtocolError::FrameTooLarge {
                max_frame_bytes: self.max_frame_bytes,
            });
        }
        ProtocolMessage::parse_line(line)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
enum SessionError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Handshake(#[from] HandshakeError),
    #[error("Bridge handshake must succeed before sensitive protocol messages are sent")]
    HandshakeRequired,
    #[error("Expected Bridge hello as the first inbound protocol message")]
    ExpectedBridgeHello,
    #[error("Bridge hello is not valid after handshake completion")]
    UnexpectedBridgeHello,
    #[error("Tool call identity was reused for a different request")]
    ToolCallIdentityReused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HandshakeState {
    AwaitingBridgeHello,
    Ready,
}

struct ProtocolSession {
    codec: ProtocolCodec,
    handshake_policy: HandshakePolicy,
    handshake_state: HandshakeState,
    tool_request_fingerprints: HashMap<(String, String), [u8; 32]>,
}

impl ProtocolSession {
    fn new(max_frame_bytes: usize, handshake_policy: HandshakePolicy) -> Self {
        Self {
            codec: ProtocolCodec::new(max_frame_bytes),
            handshake_policy,
            handshake_state: HandshakeState::AwaitingBridgeHello,
            tool_request_fingerprints: HashMap::new(),
        }
    }

    fn receive_inbound(&mut self, line: &str) -> Result<ProtocolMessage, SessionError> {
        let message = self.codec.parse_line(line)?;
        if self.handshake_state == HandshakeState::AwaitingBridgeHello {
            let ProtocolMessage::BridgeHello(hello) = &message else {
                return Err(SessionError::ExpectedBridgeHello);
            };
            self.handshake_policy.validate(hello)?;
            self.handshake_state = HandshakeState::Ready;
            return Ok(message);
        }

        match &message {
            ProtocolMessage::BridgeHello(_) => return Err(SessionError::UnexpectedBridgeHello),
            ProtocolMessage::ToolRequest(request) => {
                let key = (request.run_id.clone(), request.tool_call_id.clone());
                let fingerprint: [u8; 32] = Sha256::digest(line.as_bytes()).into();
                if let Some(previous) = self.tool_request_fingerprints.get(&key) {
                    if previous != &fingerprint {
                        return Err(SessionError::ToolCallIdentityReused);
                    }
                } else {
                    self.tool_request_fingerprints.insert(key, fingerprint);
                }
            }
            _ => {}
        }
        Ok(message)
    }

    fn authorize_outbound_line(&self, line: &str) -> Result<ProtocolMessage, SessionError> {
        if self.handshake_state != HandshakeState::Ready {
            return Err(SessionError::HandshakeRequired);
        }
        let message = self.codec.parse_line(line)?;
        if matches!(message, ProtocolMessage::BridgeHello(_)) {
            return Err(SessionError::UnexpectedBridgeHello);
        }
        Ok(message)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
enum HandshakeError {
    #[error("Bridge Core Protocol version mismatch: expected {expected}, got {actual}")]
    ProtocolVersionMismatch { expected: u32, actual: u32 },
    #[error("Bridge Core Protocol schema mismatch")]
    CoreSchemaMismatch,
    #[error("Bridge is missing required Protocol Capability {name}")]
    MissingRequiredCapability { name: String },
}

struct HandshakePolicy {
    protocol_version: u32,
    core_schema_hash: String,
    required_capabilities: Vec<ProtocolCapability>,
}

impl HandshakePolicy {
    fn current() -> Self {
        let digest = Sha256::digest(CORE_SCHEMA);
        Self {
            protocol_version: CORE_PROTOCOL_VERSION,
            core_schema_hash: format!("sha256:{digest:x}"),
            required_capabilities: Vec::new(),
        }
    }

    #[cfg(test)]
    fn requiring_capability(mut self, capability: ProtocolCapability) -> Self {
        self.required_capabilities.push(capability);
        self
    }

    fn validate(&self, hello: &BridgeHello) -> Result<(), HandshakeError> {
        if hello.protocol_version != self.protocol_version {
            return Err(HandshakeError::ProtocolVersionMismatch {
                expected: self.protocol_version,
                actual: hello.protocol_version,
            });
        }
        if hello.core_schema_hash != self.core_schema_hash {
            return Err(HandshakeError::CoreSchemaMismatch);
        }
        for required in &self.required_capabilities {
            if !hello.capabilities.iter().any(|actual| actual == required) {
                return Err(HandshakeError::MissingRequiredCapability {
                    name: required.name.clone(),
                });
            }
        }
        Ok(())
    }
}

impl HandshakeResult {
    #[cfg(test)]
    fn max_frame_bytes(&self) -> Option<u32> {
        match self {
            Self::Accepted {
                max_frame_bytes, ..
            } => Some(*max_frame_bytes),
            Self::Rejected { .. } => None,
        }
    }

    #[cfg(test)]
    fn max_transcript_bytes(&self) -> Option<u32> {
        match self {
            Self::Accepted {
                max_transcript_bytes,
                ..
            } => Some(*max_transcript_bytes),
            Self::Rejected { .. } => None,
        }
    }

    #[cfg(test)]
    fn error_code(&self) -> Option<&'static str> {
        match self {
            Self::Accepted { .. } => None,
            Self::Rejected {
                error_code: HandshakeRejectionCode::ProtocolVersionMismatch,
                ..
            } => Some("protocol_version_mismatch"),
            Self::Rejected {
                error_code: HandshakeRejectionCode::CoreSchemaMismatch,
                ..
            } => Some("core_schema_mismatch"),
            Self::Rejected {
                error_code: HandshakeRejectionCode::MissingRequiredCapability,
                ..
            } => Some("missing_required_capability"),
        }
    }

    #[cfg(test)]
    fn diagnostic_id(&self) -> Option<&str> {
        match self {
            Self::Accepted { .. } => None,
            Self::Rejected { diagnostic_id, .. } => Some(diagnostic_id),
        }
    }
}

impl TranscriptSyncResult {
    #[cfg(test)]
    fn sync_id(&self) -> &str {
        match self {
            Self::Accepted { sync_id, .. } => sync_id,
        }
    }

    #[cfg(test)]
    fn accepted_revision(&self) -> Option<u64> {
        match self {
            Self::Accepted { revision, .. } => Some(*revision),
        }
    }
}

impl CommitResult {
    #[cfg(test)]
    fn commit_id(&self) -> &str {
        match self {
            Self::Committed { commit_id, .. } => commit_id,
        }
    }

    #[cfg(test)]
    fn committed_revision(&self) -> u64 {
        match self {
            Self::Committed { revision, .. } => *revision,
        }
    }
}

impl RunFinished {
    #[cfg(test)]
    fn is_completed(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }

    #[cfg(test)]
    fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled { .. })
    }

    #[cfg(test)]
    fn error_code(&self) -> Option<&'static str> {
        match self {
            Self::Completed { .. } | Self::Cancelled { .. } | Self::LimitReached { .. } => None,
            Self::Failed {
                error_code: RunFailureCode::BridgeProtocolError,
                ..
            } => Some("bridge_protocol_error"),
            Self::Failed {
                error_code: RunFailureCode::CommitTimeout,
                ..
            } => Some("commit_timeout"),
            Self::Failed {
                error_code: RunFailureCode::ProviderHttpError,
                ..
            } => Some("provider_http_error"),
            Self::Failed {
                error_code: RunFailureCode::ProviderProtocolError,
                ..
            } => Some("provider_protocol_error"),
            Self::Failed {
                error_code: RunFailureCode::ProviderTransportError,
                ..
            } => Some("provider_transport_error"),
            Self::Failed {
                error_code: RunFailureCode::RevisionConflict,
                ..
            } => Some("revision_conflict"),
            Self::Failed {
                error_code: RunFailureCode::RuntimeFailure,
                ..
            } => Some("runtime_failure"),
            Self::Failed {
                error_code: RunFailureCode::TranscriptSyncError,
                ..
            } => Some("transcript_sync_error"),
        }
    }

    #[cfg(test)]
    fn tool_request_limit(&self) -> Option<u32> {
        match self {
            Self::LimitReached {
                tool_request_limit, ..
            } => Some(*tool_request_limit),
            Self::Completed { .. } | Self::Cancelled { .. } | Self::Failed { .. } => None,
        }
    }
}

impl ToolResult {
    #[cfg(test)]
    fn is_denied(&self) -> bool {
        matches!(self, Self::Denied { .. })
    }

    #[cfg(test)]
    fn error_code(&self) -> Option<&'static str> {
        match self {
            Self::Success { .. } | Self::Denied { .. } => None,
            Self::Error {
                error_code: ToolErrorCode::InvalidToolRequest,
                ..
            } => Some("invalid_tool_request"),
            Self::Error {
                error_code: ToolErrorCode::ToolExecutionFailed,
                ..
            } => Some("tool_execution_failed"),
            Self::Error {
                error_code: ToolErrorCode::ToolRequestLimitExceeded,
                ..
            } => Some("tool_request_limit_exceeded"),
            Self::Error {
                error_code: ToolErrorCode::ToolOutcomeUnknown,
                ..
            } => Some("tool_outcome_unknown"),
        }
    }

    #[cfg(test)]
    fn may_have_executed(&self) -> Option<bool> {
        match self {
            Self::Success { .. } | Self::Denied { .. } => None,
            Self::Error {
                may_have_executed, ..
            } => Some(*may_have_executed),
        }
    }
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
