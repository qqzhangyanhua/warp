use std::env;

use command::blocking::Command;
use integration::CLI_STARTUP_BASELINE_ENV;
use startup_request_recorder::RequestRecorder;

#[test]
fn cli_startup_respects_recorded_network_baseline() {
    let recorder = RequestRecorder::start().expect("startup request recorder should start");
    let home = tempfile::tempdir().expect("CLI startup HOME should be created");
    let inherited_envs = env::vars_os().filter(|(key, _)| {
        key == "PATH"
            || key.to_str().is_some_and(|key| {
                key.starts_with("RUST_")
                    || key.starts_with("WGPU_")
                    || matches!(key, "DISPLAY" | "WAYLAND_DISPLAY" | "XDG_RUNTIME_DIR")
            })
    });
    let mut command = Command::new(env!("CARGO_BIN_EXE_integration"));
    let zyh_home = home.path().join("zyh-home");
    command
        .args(["mcp", "list"])
        .env_clear()
        .envs(inherited_envs)
        .envs(recorder.proxy_environment())
        .env(CLI_STARTUP_BASELINE_ENV, "1")
        .env("ZYH_HOME", zyh_home)
        .env("HOME", home.path());

    let output = command
        .output()
        .expect("CLI startup process should complete");

    assert!(
        output.status.success(),
        "ZYH CLI did not complete a retained local command: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let requests = recorder
        .requests()
        .expect("CLI request recorder should synchronize");
    assert!(
        requests.is_empty(),
        "CLI startup made app-initiated requests: {requests:#?}"
    );
}
