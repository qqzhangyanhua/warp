use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use tempfile::TempDir;
use warpui_core::r#async::executor::Background;
use warpui_core::r#async::Timer;

use super::bridge_process::{BridgeProcessError, BridgeStderrSummary};
use super::supervisor::RuntimeSupervisorConfig;
use super::{AgentRuntimeHandle, AgentRuntimeLaunchConfig, AgentRuntimeSupervisor, RuntimeError};

#[tokio::test]
async fn crate_runtime_interface_attaches_with_default_lifecycle_policy() {
    let observer_dir = TempDir::new().unwrap();
    let supervisor = AgentRuntimeSupervisor::new(
        test_launch_config("ready", &observer_dir),
        Arc::new(Background::default()),
    );

    let handle = supervisor.attach("conversation-1").await.unwrap();

    handle.start_run("run-1").await.unwrap();
    handle.finish_run("run-1").await.unwrap();
    supervisor.shutdown_all().await;
}

#[tokio::test]
async fn view_handles_share_one_process_and_one_active_run() {
    let observer_dir = TempDir::new().unwrap();
    let supervisor = test_supervisor("ready", &observer_dir);

    let first = supervisor.attach("conversation-1").await.unwrap();
    let second = supervisor.attach("conversation-1").await.unwrap();

    assert_eq!(
        first.process_id().await.unwrap(),
        second.process_id().await.unwrap()
    );
    first.start_run("run-1").await.unwrap();
    assert_eq!(
        second.start_run("run-2").await,
        Err(RuntimeError::RunAlreadyActive)
    );
    first.finish_run("run-1").await.unwrap();
    second.start_run("run-2").await.unwrap();
    second.finish_run("run-2").await.unwrap();

    supervisor.shutdown_all().await;
}

#[tokio::test]
async fn conversations_use_separate_private_processes_with_minimal_environments() {
    let observer_dir = TempDir::new().unwrap();
    let supervisor = test_supervisor("ready", &observer_dir);

    let first = supervisor.attach("conversation-1").await.unwrap();
    let second = supervisor.attach("conversation-2").await.unwrap();

    assert_ne!(
        first.process_id().await.unwrap(),
        second.process_id().await.unwrap()
    );
    let first_working_directory = first.working_directory().await.unwrap();
    let second_working_directory = second.working_directory().await.unwrap();
    assert_ne!(first_working_directory, second_working_directory);
    assert_private_directory(&first_working_directory);
    assert_private_directory(&second_working_directory);

    let observations = wait_for_launch_observations(&observer_dir, 2).await;
    assert_eq!(observations.len(), 2);
    let working_directories = [
        fs::canonicalize(&first_working_directory).unwrap(),
        fs::canonicalize(&second_working_directory).unwrap(),
    ];
    for observation in observations {
        assert!([
            first.process_id().await.unwrap(),
            second.process_id().await.unwrap()
        ]
        .contains(&observation.pid));
        assert!(working_directories.contains(&fs::canonicalize(observation.cwd).unwrap()));
        assert!(observation.environment_keys.iter().all(|key| {
            [
                "HOME",
                "USERPROFILE",
                "TMPDIR",
                "TMP",
                "TEMP",
                "SystemRoot",
                "WINDIR",
                "__CF_USER_TEXT_ENCODING",
            ]
            .contains(&key.as_str())
        }));
        assert!(!observation.environment_keys.iter().any(|key| key == "PATH"));
    }

    supervisor.shutdown_all().await;
}

#[tokio::test]
async fn hung_cancellation_terminates_the_process_and_releases_the_active_run() {
    let observer_dir = TempDir::new().unwrap();
    let supervisor = test_supervisor("hang-cancel", &observer_dir);
    let handle = supervisor.attach("conversation-1").await.unwrap();
    let first_process_id = handle.process_id().await.unwrap();
    handle.start_run("run-1").await.unwrap();

    assert_eq!(
        handle.cancel_run().await,
        Err(RuntimeError::Bridge(
            BridgeProcessError::CancellationTimedOut
        ))
    );

    let restarted = supervisor.attach("conversation-1").await.unwrap();
    assert_ne!(restarted.process_id().await.unwrap(), first_process_id);
    restarted.start_run("run-2").await.unwrap();
    restarted.finish_run("run-2").await.unwrap();
    supervisor.shutdown_all().await;
}

#[tokio::test]
async fn idle_processes_are_evicted_and_rebuilt_without_touching_active_runs() {
    let observer_dir = TempDir::new().unwrap();
    let supervisor =
        test_supervisor_with_idle_timeout("ready", &observer_dir, Duration::from_millis(20));
    let idle = supervisor.attach("idle-conversation").await.unwrap();
    let active = supervisor.attach("active-conversation").await.unwrap();
    let idle_process_id = idle.process_id().await.unwrap();
    let active_process_id = active.process_id().await.unwrap();
    active.start_run("run-1").await.unwrap();

    Timer::after(Duration::from_millis(30)).await;
    supervisor.evict_idle().await;

    assert_eq!(idle.process_id().await, Err(RuntimeError::StaleHandle));
    assert_eq!(active.process_id().await.unwrap(), active_process_id);
    let rebuilt = supervisor.attach("idle-conversation").await.unwrap();
    assert_ne!(rebuilt.process_id().await.unwrap(), idle_process_id);
    active.finish_run("run-1").await.unwrap();
    supervisor.shutdown_all().await;
}

#[tokio::test]
async fn scheduled_idle_eviction_releases_inactive_processes() {
    let observer_dir = TempDir::new().unwrap();
    let supervisor =
        test_supervisor_with_idle_timeout("ready", &observer_dir, Duration::from_millis(20));
    supervisor.start_idle_eviction();
    let idle = supervisor.attach("idle-conversation").await.unwrap();

    Timer::after(Duration::from_millis(50)).await;

    assert_eq!(idle.process_id().await, Err(RuntimeError::StaleHandle));
    supervisor.shutdown_all().await;
}

#[tokio::test]
async fn reattachment_cannot_be_evicted_while_a_restarted_process_handshakes() {
    let observer_dir = TempDir::new().unwrap();
    let supervisor = test_supervisor_with_idle_timeout(
        "exit-then-delay-next-handshake",
        &observer_dir,
        Duration::from_millis(100),
    );
    let original = supervisor.attach("conversation-1").await.unwrap();
    let original_process_id = original.process_id().await.unwrap();
    wait_for_process_exit(&original).await;
    Timer::after(Duration::from_millis(110)).await;

    let (reattached, ()) = futures::join!(supervisor.attach("conversation-1"), async {
        Timer::after(Duration::from_millis(20)).await;
        supervisor.evict_idle().await;
    });
    let reattached = reattached.unwrap();

    assert_ne!(reattached.process_id().await.unwrap(), original_process_id);
    reattached.start_run("run-1").await.unwrap();
    reattached.finish_run("run-1").await.unwrap();
    supervisor.shutdown_all().await;
}

#[tokio::test]
async fn checkpoint_invalidation_stales_handles_and_rebuilds_the_process() {
    let observer_dir = TempDir::new().unwrap();
    let supervisor = test_supervisor("ready", &observer_dir);
    let handle = supervisor.attach("conversation-1").await.unwrap();
    let first_process_id = handle.process_id().await.unwrap();

    supervisor.invalidate_checkpoint("conversation-1").await;

    assert_eq!(handle.process_id().await, Err(RuntimeError::StaleHandle));
    let rebuilt = supervisor.attach("conversation-1").await.unwrap();
    assert_ne!(rebuilt.process_id().await.unwrap(), first_process_id);
    supervisor.shutdown_all().await;
}

#[tokio::test]
async fn hung_handshake_times_out_without_delivering_protocol_messages() {
    let observer_dir = TempDir::new().unwrap();
    let supervisor = test_supervisor_with_config(
        "hang-handshake",
        &observer_dir,
        RuntimeSupervisorConfig {
            handshake_timeout: Duration::from_millis(50),
            ..test_config()
        },
    );

    let result = supervisor.attach("conversation-1").await;

    assert!(matches!(
        result,
        Err(RuntimeError::Bridge(BridgeProcessError::HandshakeTimedOut))
    ));
    assert!(!observer_dir.path().join("pre-handshake-stdin").exists());
    supervisor.shutdown_all().await;
}

#[tokio::test]
async fn successful_cancellation_releases_the_run_and_preserves_the_process() {
    let observer_dir = TempDir::new().unwrap();
    let supervisor = test_supervisor("ready", &observer_dir);
    let handle = supervisor.attach("conversation-1").await.unwrap();
    let process_id = handle.process_id().await.unwrap();
    handle.start_run("run-1").await.unwrap();

    handle.cancel_run().await.unwrap();

    assert_eq!(handle.process_id().await.unwrap(), process_id);
    handle.start_run("run-2").await.unwrap();
    handle.finish_run("run-2").await.unwrap();
    supervisor.shutdown_all().await;
}

#[tokio::test]
async fn unexpected_process_exit_is_restarted_on_the_next_attach() {
    let observer_dir = TempDir::new().unwrap();
    let supervisor = test_supervisor("exit-first-launch", &observer_dir);
    let handle = supervisor.attach("conversation-1").await.unwrap();
    let first_process_id = handle.process_id().await.unwrap();
    handle.start_run("run-before-exit").await.unwrap();
    wait_for_process_exit(&handle).await;

    let restarted = supervisor.attach("conversation-1").await.unwrap();

    assert_ne!(restarted.process_id().await.unwrap(), first_process_id);
    restarted.start_run("run-1").await.unwrap();
    restarted.finish_run("run-1").await.unwrap();
    supervisor.shutdown_all().await;
}

#[tokio::test]
async fn shutdown_all_stales_handles_and_allows_clean_rebuilds() {
    let observer_dir = TempDir::new().unwrap();
    let supervisor = test_supervisor("ready", &observer_dir);
    let handle = supervisor.attach("conversation-1").await.unwrap();
    let first_process_id = handle.process_id().await.unwrap();

    supervisor.shutdown_all().await;

    assert_eq!(handle.process_id().await, Err(RuntimeError::StaleHandle));
    let rebuilt = supervisor.attach("conversation-1").await.unwrap();
    assert_ne!(rebuilt.process_id().await.unwrap(), first_process_id);
    supervisor.shutdown_all().await;
}

#[tokio::test]
async fn stderr_observation_is_content_free_and_bounded() {
    let observer_dir = TempDir::new().unwrap();
    let supervisor = test_supervisor("stderr-burst", &observer_dir);
    let handle = supervisor.attach("conversation-1").await.unwrap();

    let summary = wait_for_stderr_summary(&handle).await;

    assert_eq!(summary.observed_bytes, 8 * 1024);
    assert!(summary.truncated);
    supervisor.shutdown_all().await;
}

#[derive(Debug, Deserialize)]
struct LaunchObservation {
    pid: u32,
    cwd: String,
    environment_keys: Vec<String>,
}

async fn wait_for_launch_observations(
    observer_dir: &TempDir,
    expected: usize,
) -> Vec<LaunchObservation> {
    let path = observer_dir.path().join("launches.jsonl");
    for _ in 0..100 {
        let observations = fs::read_to_string(&path)
            .ok()
            .map(|contents| {
                contents
                    .lines()
                    .map(|line| serde_json::from_str(line).unwrap())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if observations.len() >= expected {
            return observations;
        }
        Timer::after(Duration::from_millis(10)).await;
    }
    panic!("fake Bridge launch observations did not arrive");
}

async fn wait_for_process_exit(handle: &AgentRuntimeHandle) {
    for _ in 0..100 {
        if !handle.process_is_running().await.unwrap() {
            return;
        }
        Timer::after(Duration::from_millis(10)).await;
    }
    panic!("fake Bridge process did not exit");
}

async fn wait_for_stderr_summary(handle: &AgentRuntimeHandle) -> BridgeStderrSummary {
    for _ in 0..100 {
        let summary = handle.stderr_summary().await.unwrap();
        if summary.truncated {
            return summary;
        }
        Timer::after(Duration::from_millis(10)).await;
    }
    panic!("fake Bridge stderr was not observed");
}

fn assert_private_directory(path: &Path) {
    let metadata = path.metadata().unwrap();
    assert!(metadata.is_dir());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(metadata.permissions().mode() & 0o777, 0o700);
    }
}

fn test_supervisor(mode: &str, observer_dir: &TempDir) -> AgentRuntimeSupervisor {
    test_supervisor_with_idle_timeout(mode, observer_dir, Duration::from_secs(60))
}

fn test_supervisor_with_idle_timeout(
    mode: &str,
    observer_dir: &TempDir,
    idle_timeout: Duration,
) -> AgentRuntimeSupervisor {
    test_supervisor_with_config(
        mode,
        observer_dir,
        RuntimeSupervisorConfig {
            idle_timeout,
            ..test_config()
        },
    )
}

fn test_supervisor_with_config(
    mode: &str,
    observer_dir: &TempDir,
    config: RuntimeSupervisorConfig,
) -> AgentRuntimeSupervisor {
    AgentRuntimeSupervisor::new_with_config(
        test_launch_config(mode, observer_dir),
        config,
        Arc::new(Background::default()),
    )
}

fn test_launch_config(mode: &str, observer_dir: &TempDir) -> AgentRuntimeLaunchConfig {
    AgentRuntimeLaunchConfig::new(
        node_executable(),
        [
            OsString::from(fake_bridge_path()),
            OsString::from(mode),
            observer_dir.path().as_os_str().to_owned(),
        ],
    )
}

fn test_config() -> RuntimeSupervisorConfig {
    RuntimeSupervisorConfig {
        handshake_timeout: Duration::from_secs(2),
        cancellation_grace_period: Duration::from_millis(100),
        idle_timeout: Duration::from_secs(60),
    }
}

fn fake_bridge_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tools/warp-bridge/test/supervisor-fake-bridge.mjs")
}

fn node_executable() -> PathBuf {
    let executable = if cfg!(windows) { "node.exe" } else { "node" };
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(executable))
        .find(|candidate| is_executable_file(candidate))
        .expect("Node.js must be available for the fake Bridge tests")
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    true
}
