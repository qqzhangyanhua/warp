use std::collections::{HashMap, HashSet};
use std::io::BufRead;

use sha2::{Digest as _, Sha256};
use thiserror::Error;

mod lifecycle;
mod wire;

use lifecycle::map_run_finished;
pub(super) use lifecycle::{
    LifecycleMessage, RuntimeAssistantCommit, RuntimeFailureCode, RuntimeRunFinished,
    RuntimeRunOutcome, RuntimeRunStatus, RuntimeTextDelta, RuntimeToolRequest,
};
use wire::*;

const CORE_PROTOCOL_VERSION: u32 = 2;

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

    fn read_frame(&self, reader: &mut impl BufRead) -> Result<ProtocolMessage, ProtocolError> {
        let mut frame = Vec::with_capacity(self.max_frame_bytes.min(8 * 1024));
        std::io::Read::take(&mut *reader, (self.max_frame_bytes + 1) as u64)
            .read_until(b'\n', &mut frame)
            .map_err(|_| ProtocolError::InvalidMessage)?;
        if frame.last() == Some(&b'\n') {
            frame.pop();
            if frame.last() == Some(&b'\r') {
                frame.pop();
            }
        }
        if frame.len() > self.max_frame_bytes {
            return Err(ProtocolError::FrameTooLarge {
                max_frame_bytes: self.max_frame_bytes,
            });
        }
        let line = std::str::from_utf8(&frame).map_err(|_| ProtocolError::InvalidMessage)?;
        self.parse_line(line)
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
    #[error("Warp must send the handshake result before other protocol messages")]
    HandshakeResultRequired,
    #[error("Bridge hello is not valid after handshake completion")]
    UnexpectedBridgeHello,
    #[error("Tool call identity was reused for a different request")]
    ToolCallIdentityReused,
    #[error("Tool call identity was duplicated before its result")]
    ToolCallIdentityDuplicated,
    #[error("Tool result has no pending request")]
    UnexpectedToolResult,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HandshakeState {
    AwaitingBridgeHello,
    BridgeHelloValidated,
    Ready,
    Rejected,
}

pub(super) struct ProtocolSession {
    codec: ProtocolCodec,
    handshake_policy: HandshakePolicy,
    handshake_state: HandshakeState,
    tool_request_fingerprints: HashMap<(String, String), [u8; 32]>,
    pending_tool_requests: HashSet<(String, String)>,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("Bridge Protocol lifecycle validation failed")]
pub(super) struct LifecycleSessionError;

impl ProtocolSession {
    pub(super) fn new(max_frame_bytes: usize, handshake_policy: HandshakePolicy) -> Self {
        Self {
            codec: ProtocolCodec::new(max_frame_bytes),
            handshake_policy,
            handshake_state: HandshakeState::AwaitingBridgeHello,
            tool_request_fingerprints: HashMap::new(),
            pending_tool_requests: HashSet::new(),
        }
    }

    fn receive_inbound(&mut self, line: &str) -> Result<ProtocolMessage, SessionError> {
        let message = self.codec.parse_line(line)?;
        if self.handshake_state == HandshakeState::AwaitingBridgeHello {
            let ProtocolMessage::BridgeHello(hello) = &message else {
                return Err(SessionError::ExpectedBridgeHello);
            };
            self.handshake_policy.validate(hello)?;
            self.handshake_state = HandshakeState::BridgeHelloValidated;
            return Ok(message);
        }

        if self.handshake_state != HandshakeState::Ready {
            return Err(SessionError::HandshakeResultRequired);
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
                    if self.pending_tool_requests.contains(&key) {
                        return Err(SessionError::ToolCallIdentityDuplicated);
                    }
                }
                self.tool_request_fingerprints
                    .insert(key.clone(), fingerprint);
                self.pending_tool_requests.insert(key);
            }
            _ => {}
        }
        Ok(message)
    }

    fn authorize_outbound_line(&mut self, line: &str) -> Result<ProtocolMessage, SessionError> {
        let message = self.codec.parse_line(line)?;
        if self.handshake_state == HandshakeState::BridgeHelloValidated {
            let ProtocolMessage::HandshakeResult(result) = &message else {
                return Err(SessionError::HandshakeResultRequired);
            };
            self.handshake_state = match result {
                HandshakeResult::Accepted { .. } => HandshakeState::Ready,
                HandshakeResult::Rejected { .. } => HandshakeState::Rejected,
            };
            return Ok(message);
        }
        if self.handshake_state != HandshakeState::Ready {
            return Err(SessionError::HandshakeRequired);
        }
        if matches!(message, ProtocolMessage::BridgeHello(_)) {
            return Err(SessionError::UnexpectedBridgeHello);
        }
        if let ProtocolMessage::ToolResult(result) = &message {
            let (run_id, tool_call_id) = result.identity();
            let key = (run_id.to_string(), tool_call_id.to_string());
            if !self.pending_tool_requests.remove(&key) {
                return Err(SessionError::UnexpectedToolResult);
            }
        }
        Ok(message)
    }

    pub(super) fn authorize_lifecycle_outbound_line(
        &mut self,
        line: &str,
    ) -> Result<(), LifecycleSessionError> {
        self.authorize_outbound_line(line)
            .map(|_| ())
            .map_err(|_| LifecycleSessionError)
    }

    pub(super) fn receive_lifecycle_inbound(
        &mut self,
        line: &str,
    ) -> Result<LifecycleMessage, LifecycleSessionError> {
        let message = self
            .receive_inbound(line)
            .map_err(|_| LifecycleSessionError)?;
        Ok(match message {
            ProtocolMessage::BridgeHello(_) => LifecycleMessage::BridgeHello,
            ProtocolMessage::TranscriptSyncResult(TranscriptSyncResult::Accepted {
                sync_id,
                revision,
            }) => LifecycleMessage::TranscriptSyncAccepted { sync_id, revision },
            ProtocolMessage::RunStatus(status) => LifecycleMessage::RunStatus {
                conversation_id: status.conversation_id,
                run_id: status.run_id,
                status: match status.status {
                    RunState::Running => RuntimeRunStatus::Running,
                    RunState::WaitingForCommit => RuntimeRunStatus::WaitingForCommit,
                    RunState::WaitingForToolResult => RuntimeRunStatus::WaitingForToolResult,
                },
            },
            ProtocolMessage::TextDelta(delta) => LifecycleMessage::TextDelta(RuntimeTextDelta {
                conversation_id: delta.conversation_id,
                run_id: delta.run_id,
                event_id: delta.event_id,
                delta: delta.delta,
            }),
            ProtocolMessage::AssistantMessageCommit(commit) => {
                LifecycleMessage::AssistantMessageCommit(RuntimeAssistantCommit {
                    conversation_id: commit.conversation_id,
                    run_id: commit.run_id,
                    event_id: commit.event_id,
                    commit_id: commit.commit_id,
                    message_id: commit.message_id,
                    expected_revision: commit.expected_revision,
                    content: commit.content,
                })
            }
            ProtocolMessage::RunFinished(finished) => {
                LifecycleMessage::RunFinished(map_run_finished(finished))
            }
            ProtocolMessage::ToolRequest(request) => {
                LifecycleMessage::ToolRequest(RuntimeToolRequest {
                    frame_fingerprint: Sha256::digest(line.as_bytes()).into(),
                    conversation_id: request.conversation_id,
                    run_id: request.run_id,
                    tool_call_id: request.tool_call_id,
                    tool_id: request.tool_id,
                    tool_name: request.tool_name,
                    arguments: request.arguments,
                })
            }
            ProtocolMessage::HandshakeResult(_)
            | ProtocolMessage::TranscriptSyncBegin(_)
            | ProtocolMessage::TranscriptSyncItem(_)
            | ProtocolMessage::TranscriptSyncCommit(_)
            | ProtocolMessage::RunStart(_)
            | ProtocolMessage::CommitResult(_)
            | ProtocolMessage::RunCancel(_)
            | ProtocolMessage::ToolResult(_) => LifecycleMessage::Other,
        })
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

pub(super) struct HandshakePolicy {
    protocol_version: u32,
    core_schema_hash: String,
    required_capabilities: Vec<ProtocolCapability>,
}

impl HandshakePolicy {
    pub(super) fn current() -> Self {
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
                error_code: RunFailureCode::ProviderRedirectNotAllowed,
                ..
            } => Some("provider_redirect_not_allowed"),
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
