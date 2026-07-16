#![allow(
    dead_code,
    reason = "Agent Runtime Selection intentionally remains disabled until GH11 Phase 7"
)]

#[cfg(not(target_family = "wasm"))]
mod bridge_process;
mod configuration;
mod protocol;
mod resources;
#[cfg(not(target_family = "wasm"))]
mod supervisor;
#[cfg(not(target_family = "wasm"))]
mod text_run;
mod tool_catalog;
mod tool_execution;
mod transcript;
mod transcript_sync;

#[cfg(not(target_family = "wasm"))]
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use bridge_process::BridgeLaunchConfig as AgentRuntimeLaunchConfig;
#[cfg(not(target_family = "wasm"))]
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use supervisor::{AgentRuntimeHandle, AgentRuntimeSupervisor, RuntimeError};
pub(crate) use tool_execution::ToolPermissionDecision;

#[cfg(all(test, not(target_family = "wasm")))]
#[path = "supervisor_tests.rs"]
mod supervisor_tests;

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod transcript_tests;

#[cfg(test)]
#[path = "resources_tests.rs"]
mod resources_tests;

#[cfg(test)]
#[path = "configuration_tests.rs"]
mod configuration_tests;

#[cfg(test)]
#[path = "transcript_sync_tests.rs"]
mod transcript_sync_tests;

#[cfg(all(test, not(target_family = "wasm"), feature = "local_fs"))]
#[path = "text_run_integration_tests.rs"]
mod text_run_integration_tests;

#[cfg(all(test, not(target_family = "wasm"), feature = "local_fs"))]
#[path = "text_run_sync_integration_tests.rs"]
mod text_run_sync_integration_tests;

#[cfg(all(test, not(target_family = "wasm"), feature = "local_fs"))]
#[path = "tool_run_integration_tests.rs"]
mod tool_run_integration_tests;
