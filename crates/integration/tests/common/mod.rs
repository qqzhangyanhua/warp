use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::{env, fs};

use command::blocking::Command;
use warpui_core::integration::{RERUN_EXIT_CODE, TEST_ROOT_OUTPUT_FILE_ENV_VAR};

include!(concat!(env!("OUT_DIR"), "/cargo_target_tmpdir.rs"));

const MAX_TEST_RUNS: usize = 10;

/// Runs a single integration test.
///
/// This runs the `integration` binary from the `warp` crate, passing it the
/// name of the test to execute as the one positional argument.
pub fn run_integration_test(name: &str) -> Result<(), String> {
    let mut keep_going = true;
    let mut run_num = 0;
    while keep_going {
        let test_root_output = tempfile::NamedTempFile::new()
            .map_err(|err| format!("Failed to create test root output file: {err}"))?;
        let inherited_envs = env::vars_os().filter(|(k, _v)| {
            let k = k
                .to_str()
                .expect("environment variable keys should contain valid unicode");
            // Propagate the PATH to the integration test
            // process, otherwise the shell it spawns might not
            // be able to find the binaries it needs to execute.
            k == "PATH"
                // Propagate any Rust-related variables.
                || k.starts_with("RUST_")
                // Propagate any Warp-specific variables.
                || k.starts_with("WARP_")
                || k.starts_with("WARPUI_")
                // Propagate any wgpu-specific variables.
                || k.starts_with("WGPU_")
                // Make sure the test knows what X or Wayland server to use.
                || k == "DISPLAY"
                || k == "WAYLAND_DISPLAY"
                // Propagate XDG_RUNTIME_DIR, which is needed for tests to run.
                // We actively _do not_ want to propagate other XDG_ variables,
                // as they tend to encode the home directory, which we override
                // in tests to point to a per-test temporary directory.
                || k == "XDG_RUNTIME_DIR"
                // Propagate XAUTHORITY so we can run headless tests using xvfb.
                || k == "XAUTHORITY"
        });
        let status = Command::new(env!("CARGO_BIN_EXE_integration"))
            .arg(name)
            .env_clear()
            .envs(inherited_envs)
            .env("WARP_INTEGRATION", "1")
            .env(TEST_ROOT_OUTPUT_FILE_ENV_VAR, test_root_output.path())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();
        let status_result = should_rerun(name, &mut run_num, status);
        let cleanup_result = cleanup_test_root(test_root_output.path());
        keep_going = match (status_result, cleanup_result) {
            (Ok(keep_going), Ok(())) => keep_going,
            (Err(test_err), Ok(())) => return Err(test_err),
            (Ok(_), Err(cleanup_err)) => return Err(cleanup_err),
            (Err(test_err), Err(cleanup_err)) => {
                return Err(format!("{test_err}; additionally, {cleanup_err}"));
            }
        };
    }
    Ok(())
}

fn should_rerun(
    name: &str,
    run_num: &mut usize,
    status: std::io::Result<std::process::ExitStatus>,
) -> Result<bool, String> {
    let status = status.map_err(|err| format!("Test {name} failed with error {err:#}"))?;
    match status.code() {
        Some(0) => {
            println!("Test exited with success.");
            Ok(false)
        }
        Some(RERUN_EXIT_CODE) if *run_num < MAX_TEST_RUNS => {
            println!("Test exited with rerun code, trying again.");
            *run_num += 1;
            Ok(true)
        }
        Some(exit_code) => Err(format!("Test {name} failed with exit code {exit_code}")),
        None => {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                let signal = status
                    .signal()
                    .and_then(|signal| nix::sys::signal::Signal::try_from(signal).ok());
                if let Some(signal) = signal {
                    Err(format!(
                        "Test {name} failed due to signal {}",
                        signal.as_str()
                    ))
                } else {
                    Err(format!("Test {name} failed for unknown reason"))
                }
            }
            #[cfg(windows)]
            {
                Err(format!("Test {name} failed for unknown reason"))
            }
        }
    }
}

fn cleanup_test_root(output_file: &Path) -> Result<(), String> {
    let root = fs::read_to_string(output_file)
        .map_err(|err| format!("Failed to read integration test root: {err}"))?;
    let root = PathBuf::from(root);
    if !root.exists() {
        return Ok(());
    }

    let root = root
        .canonicalize()
        .map_err(|err| format!("Failed to resolve integration test root {root:?}: {err}"))?;
    let cargo_tmp = PathBuf::from(cargo_target_tmpdir::get())
        .canonicalize()
        .map_err(|err| format!("Failed to resolve Cargo test directory: {err}"))?;
    let system_tmp = env::temp_dir()
        .canonicalize()
        .map_err(|err| format!("Failed to resolve system temp directory: {err}"))?;
    let is_owned_root = (root.starts_with(&cargo_tmp) && root != cargo_tmp)
        || (root.starts_with(&system_tmp) && root != system_tmp);
    if !is_owned_root {
        return Err(format!(
            "Refusing to remove integration test root outside known temporary directories: {root:?}"
        ));
    }

    fs::remove_dir_all(&root)
        .map_err(|err| format!("Failed to clean up integration test root {root:?}: {err}"))
}

#[macro_export]
macro_rules! integration_tests {
    (   $(
            $(#[$args:meta])*
            $name:ident,
        )*
    ) => {
        $(
            $(#[$args])*
            // Ignore unused attributes, in case we're marking a test as
            // ignored twice, once via arguments passed to the macro and once
            // below.
            #[allow(unused_attributes)]
            // For right now, we only want to run integration tests on macOS
            // and Linux (iff the run_on_linux feature is enabled).
            #[cfg_attr(not(any(target_os = "macos", feature = "run_on_linux")), ignore)]
            #[test]
            fn $name() -> Result<(), String> {
                $crate::common::run_integration_test(stringify!($name))
            }
        )*
    }
}
