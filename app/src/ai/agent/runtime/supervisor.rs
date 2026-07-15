use std::collections::HashMap;
#[cfg(test)]
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use futures::channel::{mpsc, oneshot};
use futures::lock::Mutex as AsyncMutex;
use parking_lot::Mutex;
use thiserror::Error;
use warpui_core::r#async::executor::Background;

use super::bridge_process::{BridgeLaunchConfig, BridgeProcess, BridgeProcessError};
use super::text_run;
pub(super) use super::text_run::{RuntimeEvent, TextRunRequest, TextRunResult};
use crate::persistence::{
    CommitAgentRuntimeMutationError, ModelEvent, PersistAgentRuntimeRunError,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct RuntimeSupervisorConfig {
    pub handshake_timeout: Duration,
    pub cancellation_grace_period: Duration,
    pub idle_timeout: Duration,
}

impl Default for RuntimeSupervisorConfig {
    fn default() -> Self {
        Self {
            handshake_timeout: Duration::from_secs(5),
            cancellation_grace_period: Duration::from_secs(2),
            idle_timeout: Duration::from_secs(5 * 60),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum RuntimeError {
    #[error(transparent)]
    Bridge(#[from] BridgeProcessError),
    #[error("Agent Runtime handle is no longer active")]
    StaleHandle,
    #[error("Conversation Record already has an active Agent Run")]
    RunAlreadyActive,
    #[error("Conversation Record has no active Agent Run")]
    NoActiveRun,
    #[error("Agent Run identity does not match the active run")]
    RunIdentityMismatch,
    #[error(transparent)]
    RunPersistence(#[from] PersistAgentRuntimeRunError),
    #[error(transparent)]
    CommitPersistence(#[from] CommitAgentRuntimeMutationError),
    #[error("Agent Runtime persistence writer is unavailable")]
    PersistenceUnavailable,
    #[error("Agent Runtime persistence acknowledgement was dropped")]
    PersistenceAcknowledgementDropped,
    #[error("Bridge assistant output is invalid for a text-only Agent Run")]
    InvalidAssistantOutput,
}

pub(super) struct RuntimeEntry {
    pub(super) conversation_id: String,
    pub(super) process: AsyncMutex<Option<BridgeProcess>>,
    pub(super) state: Mutex<RuntimeEntryState>,
}

pub(super) struct RuntimeEntryState {
    pub(super) active_run_id: Option<String>,
    pub(super) text_run_commands: Option<mpsc::UnboundedSender<TextRunCommand>>,
    pub(super) last_used: Instant,
    pub(super) evicted: bool,
}

pub(super) enum TextRunCommand {
    Cancel {
        grace_period: Duration,
        acknowledgement: oneshot::Sender<Result<(), BridgeProcessError>>,
    },
}

impl RuntimeEntry {
    fn new(conversation_id: String) -> Self {
        Self {
            conversation_id,
            process: AsyncMutex::new(None),
            state: Mutex::new(RuntimeEntryState {
                active_run_id: None,
                text_run_commands: None,
                last_used: Instant::now(),
                evicted: false,
            }),
        }
    }

    async fn shutdown_process(&self) {
        if let Some(process) = self.process.lock().await.take() {
            process.shutdown().await;
        }
    }
}

struct SupervisorInner {
    launch_config: BridgeLaunchConfig,
    config: RuntimeSupervisorConfig,
    executor: Arc<Background>,
    entries: AsyncMutex<HashMap<String, Arc<RuntimeEntry>>>,
}

#[derive(Clone)]
pub(crate) struct AgentRuntimeSupervisor {
    inner: Arc<SupervisorInner>,
}

impl AgentRuntimeSupervisor {
    pub(crate) fn new(launch_config: BridgeLaunchConfig, executor: Arc<Background>) -> Self {
        Self::new_with_config(launch_config, RuntimeSupervisorConfig::default(), executor)
    }

    pub(super) fn new_with_config(
        launch_config: BridgeLaunchConfig,
        config: RuntimeSupervisorConfig,
        executor: Arc<Background>,
    ) -> Self {
        Self {
            inner: Arc::new(SupervisorInner {
                launch_config,
                config,
                executor,
                entries: AsyncMutex::new(HashMap::new()),
            }),
        }
    }

    pub(crate) async fn attach(
        &self,
        conversation_id: impl Into<String>,
    ) -> Result<AgentRuntimeHandle, RuntimeError> {
        let conversation_id = conversation_id.into();
        let entry = {
            let mut entries = self.inner.entries.lock().await;
            let entry = entries
                .entry(conversation_id.clone())
                .or_insert_with(|| Arc::new(RuntimeEntry::new(conversation_id)))
                .clone();
            entry.state.lock().last_used = Instant::now();
            entry
        };

        let mut process = entry.process.lock().await;
        let should_launch = match process.as_mut() {
            Some(process) => !process.is_running()?,
            None => true,
        };
        if should_launch {
            let mut state = entry.state.lock();
            state.active_run_id = None;
            state.text_run_commands = None;
            drop(state);
            let launched = BridgeProcess::launch(
                &self.inner.launch_config,
                self.inner.config.handshake_timeout,
                &self.inner.executor,
            )
            .await?;
            *process = Some(launched);
        }
        drop(process);
        entry.state.lock().last_used = Instant::now();

        Ok(AgentRuntimeHandle {
            entry: Arc::downgrade(&entry),
            cancellation_grace_period: self.inner.config.cancellation_grace_period,
        })
    }

    pub(crate) async fn shutdown_all(&self) {
        let entries = {
            let mut entries = self.inner.entries.lock().await;
            entries.drain().map(|(_, entry)| entry).collect::<Vec<_>>()
        };
        for entry in entries {
            {
                let mut state = entry.state.lock();
                state.active_run_id = None;
                state.text_run_commands = None;
                state.evicted = true;
            }
            entry.shutdown_process().await;
        }
    }

    pub(crate) async fn evict_idle(&self) {
        let entries = {
            let mut entries = self.inner.entries.lock().await;
            let now = Instant::now();
            let idle_timeout = self.inner.config.idle_timeout;
            let mut evicted = Vec::new();
            entries.retain(|_, entry| {
                let mut state = entry.state.lock();
                let should_evict = state.active_run_id.is_none()
                    && now.duration_since(state.last_used) >= idle_timeout;
                if should_evict {
                    state.evicted = true;
                    evicted.push(entry.clone());
                }
                !should_evict
            });
            evicted
        };
        for entry in entries {
            entry.shutdown_process().await;
        }
    }

    pub(crate) async fn invalidate_checkpoint(&self, conversation_id: &str) {
        let entry = self.inner.entries.lock().await.remove(conversation_id);
        let Some(entry) = entry else {
            return;
        };
        {
            let mut state = entry.state.lock();
            state.active_run_id = None;
            state.text_run_commands = None;
            state.evicted = true;
        }
        entry.shutdown_process().await;
    }
}

pub(crate) struct AgentRuntimeHandle {
    entry: Weak<RuntimeEntry>,
    cancellation_grace_period: Duration,
}

impl AgentRuntimeHandle {
    pub(super) async fn run_text<F>(
        &self,
        persistence: &std::sync::mpsc::SyncSender<ModelEvent>,
        request: TextRunRequest,
        on_event: F,
    ) -> Result<TextRunResult, RuntimeError>
    where
        F: FnMut(RuntimeEvent),
    {
        let run_id = request.run_id().to_string();
        let (commands, command_receiver) = mpsc::unbounded();
        self.start_run_with_commands(run_id.clone(), Some(commands))
            .await?;
        let entry = self.entry.upgrade().ok_or(RuntimeError::StaleHandle)?;
        let result = text_run::execute(
            entry.clone(),
            persistence,
            request,
            command_receiver,
            on_event,
        )
        .await;
        if result.is_err() {
            if let Some(process) = entry.process.lock().await.take() {
                process.shutdown().await;
            }
        }
        let mut state = entry.state.lock();
        if state.active_run_id.as_deref() == Some(run_id.as_str()) {
            state.active_run_id = None;
            state.text_run_commands = None;
            state.last_used = Instant::now();
        }
        result
    }

    pub(crate) async fn start_run(&self, run_id: impl Into<String>) -> Result<(), RuntimeError> {
        self.start_run_with_commands(run_id, None).await
    }

    async fn start_run_with_commands(
        &self,
        run_id: impl Into<String>,
        text_run_commands: Option<mpsc::UnboundedSender<TextRunCommand>>,
    ) -> Result<(), RuntimeError> {
        let entry = self.entry.upgrade().ok_or(RuntimeError::StaleHandle)?;
        let mut process = entry.process.lock().await;
        let is_running = process
            .as_mut()
            .ok_or(BridgeProcessError::UnexpectedExit)?
            .is_running()?;
        if !is_running {
            return Err(BridgeProcessError::UnexpectedExit.into());
        }
        let mut state = entry.state.lock();
        if state.evicted {
            return Err(RuntimeError::StaleHandle);
        }
        if state.active_run_id.is_some() {
            return Err(RuntimeError::RunAlreadyActive);
        }
        state.active_run_id = Some(run_id.into());
        state.text_run_commands = text_run_commands;
        state.last_used = Instant::now();
        Ok(())
    }

    pub(crate) async fn finish_run(&self, run_id: &str) -> Result<(), RuntimeError> {
        let entry = self.entry.upgrade().ok_or(RuntimeError::StaleHandle)?;
        let mut state = entry.state.lock();
        if state.evicted {
            return Err(RuntimeError::StaleHandle);
        }
        match state.active_run_id.as_deref() {
            Some(active) if active == run_id => {
                state.active_run_id = None;
                state.text_run_commands = None;
                state.last_used = Instant::now();
                Ok(())
            }
            Some(_) => Err(RuntimeError::RunIdentityMismatch),
            None => Err(RuntimeError::NoActiveRun),
        }
    }

    pub(crate) async fn cancel_run(&self) -> Result<(), RuntimeError> {
        let entry = self.entry.upgrade().ok_or(RuntimeError::StaleHandle)?;
        let (run_id, text_run_commands) = {
            let state = entry.state.lock();
            if state.evicted {
                return Err(RuntimeError::StaleHandle);
            }
            (
                state
                    .active_run_id
                    .clone()
                    .ok_or(RuntimeError::NoActiveRun)?,
                state.text_run_commands.clone(),
            )
        };
        if let Some(commands) = text_run_commands {
            let (acknowledgement, acknowledged) = oneshot::channel();
            commands
                .unbounded_send(TextRunCommand::Cancel {
                    grace_period: self.cancellation_grace_period,
                    acknowledgement,
                })
                .map_err(|_| BridgeProcessError::UnexpectedExit)?;
            return acknowledged
                .await
                .map_err(|_| BridgeProcessError::UnexpectedExit)?
                .map_err(RuntimeError::from);
        }
        let mut process = entry.process.lock().await;
        let cancellation = process
            .as_mut()
            .ok_or(BridgeProcessError::UnexpectedExit)?
            .cancel_run(
                &entry.conversation_id,
                &run_id,
                self.cancellation_grace_period,
            )
            .await;
        if cancellation.is_err() {
            if let Some(process) = process.take() {
                process.shutdown().await;
            }
        }
        drop(process);
        self.finish_run(&run_id).await?;
        cancellation.map_err(RuntimeError::from)
    }

    #[cfg(test)]
    pub(super) async fn process_id(&self) -> Result<u32, RuntimeError> {
        let entry = self.entry.upgrade().ok_or(RuntimeError::StaleHandle)?;
        let process_id = entry
            .process
            .lock()
            .await
            .as_ref()
            .map(BridgeProcess::process_id)
            .ok_or_else(|| BridgeProcessError::UnexpectedExit.into());
        process_id
    }

    #[cfg(test)]
    pub(super) async fn process_is_running(&self) -> Result<bool, RuntimeError> {
        let entry = self.entry.upgrade().ok_or(RuntimeError::StaleHandle)?;
        let is_running = entry
            .process
            .lock()
            .await
            .as_mut()
            .ok_or(BridgeProcessError::UnexpectedExit)?
            .is_running()
            .map_err(RuntimeError::from);
        is_running
    }

    #[cfg(test)]
    pub(super) async fn stderr_summary(
        &self,
    ) -> Result<super::bridge_process::BridgeStderrSummary, RuntimeError> {
        let entry = self.entry.upgrade().ok_or(RuntimeError::StaleHandle)?;
        let summary = entry
            .process
            .lock()
            .await
            .as_ref()
            .map(BridgeProcess::stderr_summary)
            .ok_or_else(|| BridgeProcessError::UnexpectedExit.into());
        summary
    }

    #[cfg(test)]
    pub(super) async fn working_directory(&self) -> Result<PathBuf, RuntimeError> {
        let entry = self.entry.upgrade().ok_or(RuntimeError::StaleHandle)?;
        let working_directory = entry
            .process
            .lock()
            .await
            .as_ref()
            .map(|process| process.working_directory().to_owned())
            .ok_or_else(|| BridgeProcessError::UnexpectedExit.into());
        working_directory
    }
}
