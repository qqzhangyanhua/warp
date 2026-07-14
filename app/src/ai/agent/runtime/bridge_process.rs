use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_process::{Child, ChildStderr, ChildStdin, ChildStdout};
use command::r#async::Command;
use command::Stdio;
use futures::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use parking_lot::Mutex;
use serde_json::json;
use tempfile::TempDir;
use thiserror::Error;
use warpui_core::r#async::executor::{Background, BackgroundTask};
use warpui_core::r#async::FutureExt as _;

use super::protocol::{HandshakePolicy, LifecycleMessage, LifecycleSessionError, ProtocolSession};

const MAX_HANDSHAKE_FRAME_BYTES: usize = 64 * 1024;
const MAX_FRAME_BYTES: usize = 1024 * 1024;
const MAX_TRANSCRIPT_BYTES: usize = 16 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 8 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct BridgeLaunchConfig {
    program: PathBuf,
    arguments: Vec<OsString>,
}

impl BridgeLaunchConfig {
    pub(crate) fn new<I, S>(program: impl Into<PathBuf>, arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        Self {
            program: program.into(),
            arguments: arguments.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct BridgeStderrSummary {
    pub observed_bytes: usize,
    pub truncated: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum BridgeProcessError {
    #[error("Bridge process could not be started")]
    SpawnFailed,
    #[error("Bridge process did not expose the required protocol pipes")]
    MissingProtocolPipe,
    #[error("Bridge handshake timed out")]
    HandshakeTimedOut,
    #[error("Bridge Protocol IO failed")]
    ProtocolIo,
    #[error("Bridge Protocol validation failed")]
    ProtocolViolation,
    #[error("Bridge process exited unexpectedly")]
    UnexpectedExit,
    #[error("Bridge cancellation timed out")]
    CancellationTimedOut,
    #[error("Bridge returned an unexpected cancellation response")]
    UnexpectedCancellationResponse,
}

impl From<LifecycleSessionError> for BridgeProcessError {
    fn from(_: LifecycleSessionError) -> Self {
        Self::ProtocolViolation
    }
}

pub(super) struct BridgeProcess {
    child: Child,
    input: BufWriter<ChildStdin>,
    output: BufReader<ChildStdout>,
    session: ProtocolSession,
    max_frame_bytes: usize,
    working_directory: TempDir,
    stderr_summary: Arc<Mutex<BridgeStderrSummary>>,
    stderr_task: Option<BackgroundTask>,
}

impl BridgeProcess {
    pub(super) async fn launch(
        launch_config: &BridgeLaunchConfig,
        handshake_timeout: Duration,
        executor: &Background,
    ) -> Result<Self, BridgeProcessError> {
        let working_directory = tempfile::Builder::new()
            .prefix("warp-agent-runtime-")
            .tempdir()
            .map_err(|_| BridgeProcessError::SpawnFailed)?;
        set_private_directory_permissions(working_directory.path())?;
        let mut command = Command::new(&launch_config.program);
        command
            .args(&launch_config.arguments)
            .current_dir(working_directory.path())
            .env_clear()
            .env("HOME", working_directory.path())
            .env("USERPROFILE", working_directory.path())
            .env("TMPDIR", working_directory.path())
            .env("TMP", working_directory.path())
            .env("TEMP", working_directory.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        preserve_required_windows_environment(&mut command);

        let mut child = command
            .spawn()
            .map_err(|_| BridgeProcessError::SpawnFailed)?;
        let input = child
            .stdin
            .take()
            .ok_or(BridgeProcessError::MissingProtocolPipe)?;
        let output = child
            .stdout
            .take()
            .ok_or(BridgeProcessError::MissingProtocolPipe)?;
        let stderr = child
            .stderr
            .take()
            .ok_or(BridgeProcessError::MissingProtocolPipe)?;

        let stderr_summary = Arc::new(Mutex::new(BridgeStderrSummary::default()));
        let stderr_task = executor.spawn(drain_stderr(stderr, stderr_summary.clone()));
        let mut process = Self {
            child,
            input: BufWriter::new(input),
            output: BufReader::new(output),
            session: ProtocolSession::new(MAX_HANDSHAKE_FRAME_BYTES, HandshakePolicy::current()),
            max_frame_bytes: MAX_HANDSHAKE_FRAME_BYTES,
            working_directory,
            stderr_summary,
            stderr_task: Some(stderr_task),
        };

        let hello = match process.read_message().with_timeout(handshake_timeout).await {
            Ok(message) => message?,
            Err(_) => return Err(BridgeProcessError::HandshakeTimedOut),
        };
        if !matches!(hello, LifecycleMessage::BridgeHello) {
            return Err(BridgeProcessError::ProtocolViolation);
        }
        let accepted = json!({
            "type": "handshake_result",
            "status": "accepted",
            "max_frame_bytes": MAX_FRAME_BYTES,
            "max_transcript_bytes": MAX_TRANSCRIPT_BYTES,
        })
        .to_string();
        process.write_message(&accepted).await?;
        process.max_frame_bytes = MAX_FRAME_BYTES;
        Ok(process)
    }

    pub(super) fn process_id(&self) -> u32 {
        self.child.id()
    }

    pub(super) fn is_running(&mut self) -> Result<bool, BridgeProcessError> {
        self.child
            .try_status()
            .map(|status| status.is_none())
            .map_err(|_| BridgeProcessError::ProtocolIo)
    }

    pub(super) async fn cancel_run(
        &mut self,
        conversation_id: &str,
        run_id: &str,
        grace_period: Duration,
    ) -> Result<(), BridgeProcessError> {
        let message = json!({
            "type": "run_cancel",
            "conversation_id": conversation_id,
            "run_id": run_id,
        })
        .to_string();
        self.write_message(&message).await?;

        let response = match self.read_message().with_timeout(grace_period).await {
            Ok(message) => message?,
            Err(_) => return Err(BridgeProcessError::CancellationTimedOut),
        };
        match response {
            LifecycleMessage::RunCancelled {
                conversation_id: cancelled_conversation_id,
                run_id: cancelled_run_id,
            } if cancelled_conversation_id == conversation_id && cancelled_run_id == run_id => {
                Ok(())
            }
            LifecycleMessage::BridgeHello
            | LifecycleMessage::RunCancelled { .. }
            | LifecycleMessage::Other => Err(BridgeProcessError::UnexpectedCancellationResponse),
        }
    }

    pub(super) async fn shutdown(mut self) {
        if self.child.try_status().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.status().await;
        if let Some(task) = self.stderr_task.take() {
            let _ = task.await;
        }
    }

    #[cfg(test)]
    pub(super) fn working_directory(&self) -> &Path {
        self.working_directory.path()
    }

    #[cfg(test)]
    pub(super) fn stderr_summary(&self) -> BridgeStderrSummary {
        *self.stderr_summary.lock()
    }

    async fn write_message(&mut self, line: &str) -> Result<(), BridgeProcessError> {
        self.session.authorize_lifecycle_outbound_line(line)?;
        self.input
            .write_all(line.as_bytes())
            .await
            .map_err(|_| BridgeProcessError::ProtocolIo)?;
        self.input
            .write_all(b"\n")
            .await
            .map_err(|_| BridgeProcessError::ProtocolIo)?;
        self.input
            .flush()
            .await
            .map_err(|_| BridgeProcessError::ProtocolIo)
    }

    async fn read_message(&mut self) -> Result<LifecycleMessage, BridgeProcessError> {
        let mut bytes = Vec::with_capacity(self.max_frame_bytes.min(8 * 1024));
        let mut bounded = (&mut self.output).take((self.max_frame_bytes + 1) as u64);
        bounded
            .read_until(b'\n', &mut bytes)
            .await
            .map_err(|_| BridgeProcessError::ProtocolIo)?;
        if bytes.is_empty() {
            return Err(BridgeProcessError::UnexpectedExit);
        }
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
        }
        if bytes.len() > self.max_frame_bytes {
            return Err(BridgeProcessError::ProtocolViolation);
        }
        let line =
            std::str::from_utf8(&bytes).map_err(|_| BridgeProcessError::ProtocolViolation)?;
        self.session
            .receive_lifecycle_inbound(line)
            .map_err(BridgeProcessError::from)
    }
}

async fn drain_stderr(mut stderr: ChildStderr, summary: Arc<Mutex<BridgeStderrSummary>>) {
    let mut buffer = [0; 1024];
    while let Ok(bytes_read) = stderr.read(&mut buffer).await {
        if bytes_read == 0 {
            break;
        }
        let mut summary = summary.lock();
        let remaining = MAX_STDERR_BYTES.saturating_sub(summary.observed_bytes);
        summary.observed_bytes += remaining.min(bytes_read);
        summary.truncated |= bytes_read > remaining;
    }
}

fn preserve_required_windows_environment(command: &mut Command) {
    #[cfg(windows)]
    for name in ["SystemRoot", "WINDIR"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }

    #[cfg(not(windows))]
    let _ = command;
}

fn set_private_directory_permissions(path: &Path) -> Result<(), BridgeProcessError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        let permissions = std::fs::Permissions::from_mode(0o700);
        std::fs::set_permissions(path, permissions).map_err(|_| BridgeProcessError::SpawnFailed)?;
    }

    #[cfg(not(unix))]
    let _ = path;

    Ok(())
}
