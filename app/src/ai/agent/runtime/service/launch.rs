use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use crate::ai::agent::runtime::AgentRuntimeLaunchConfig;

pub(super) fn runtime_launch_config() -> Option<AgentRuntimeLaunchConfig> {
    let override_program = env::var_os("WARP_PI_BRIDGE_PROGRAM").map(PathBuf::from);
    let program = resolve_runtime_program(
        override_program.clone(),
        warp_core::paths::bundled_resources_dir(),
        cfg!(feature = "release_bundle"),
    )?;
    if !program.is_file() {
        return None;
    }
    let arguments = env::var_os("WARP_PI_BRIDGE_ARGS")
        .filter(|_| override_program.is_some())
        .map(|args| {
            env::split_paths(&args)
                .map(|arg| arg.as_os_str().to_owned())
                .collect::<Vec<OsString>>()
        })
        .unwrap_or_default();
    Some(AgentRuntimeLaunchConfig::new(program, arguments))
}

pub(super) fn resolve_runtime_program(
    override_program: Option<PathBuf>,
    bundled_resources: Option<PathBuf>,
    release_bundle: bool,
) -> Option<PathBuf> {
    if release_bundle && override_program.is_some() {
        return None;
    }
    override_program.or_else(|| {
        bundled_resources.map(|resources| {
            resources
                .join("bundled")
                .join("agent-runtime")
                .join(if cfg!(windows) {
                    "warp-bridge.exe"
                } else {
                    "warp-bridge"
                })
        })
    })
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
