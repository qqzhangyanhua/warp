#[cfg(not(target_family = "wasm"))]
mod bridge_process;
mod protocol;
#[cfg(not(target_family = "wasm"))]
mod supervisor;

#[cfg(not(target_family = "wasm"))]
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use bridge_process::BridgeLaunchConfig as AgentRuntimeLaunchConfig;
#[cfg(not(target_family = "wasm"))]
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use supervisor::{AgentRuntimeHandle, AgentRuntimeSupervisor, RuntimeError};

#[cfg(all(test, not(target_family = "wasm")))]
#[path = "supervisor_tests.rs"]
mod supervisor_tests;
