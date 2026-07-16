use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use crate::ai::agent::runtime::AgentRuntimeLaunchConfig;

pub(super) fn runtime_launch_config() -> Option<AgentRuntimeLaunchConfig> {
    let program = env::var_os("WARP_PI_BRIDGE_PROGRAM").map(PathBuf::from)?;
    let arguments = env::var_os("WARP_PI_BRIDGE_ARGS")
        .map(|args| {
            env::split_paths(&args)
                .map(|arg| arg.as_os_str().to_owned())
                .collect::<Vec<OsString>>()
        })
        .unwrap_or_default();
    Some(AgentRuntimeLaunchConfig::new(program, arguments))
}
