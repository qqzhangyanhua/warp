#[path = "installation/bundled_upload.rs"]
mod bundled_upload;

use std::path::Path;

use remote_server::bundled_artifact::runtime_http_download_allowed;
use remote_server::transport::{Error, InstallOutcome, InstallSource};

/// Runs the binary install sequence for the SSH transport.
///
/// Selects the matching verified bundled remote-daemon artifact on the
/// client, uploads it over SCP, and extracts it on the remote host. Never
/// calls a CDN, download endpoint, or other HTTP fallback.
pub(super) async fn install_binary(socket_path: &Path) -> InstallOutcome {
    debug_assert!(!runtime_http_download_allowed());

    let binary_path = remote_server::setup::remote_server_binary();
    log::info!("Installing remote server binary to {binary_path} from bundled artifact via SCP");

    let mut outcome = match bundled_upload::install(socket_path).await {
        Ok(()) => InstallOutcome {
            source: Some(InstallSource::Bundled),
            result: Ok(()),
        },
        Err(e) => InstallOutcome {
            source: Some(InstallSource::Bundled),
            result: Err(e),
        },
    };

    // Post-install verification: confirm the binary actually landed at the
    // expected path and is functional. This catches silent install failures
    // that would otherwise surface as a cryptic IPC handshake error.
    if outcome.result.is_ok() {
        log::info!("Running post-install verification for {binary_path}");
        let check_cmd = remote_server::setup::binary_check_command();
        let verify = remote_server::ssh::run_ssh_command(
            socket_path,
            &check_cmd,
            remote_server::setup::CHECK_TIMEOUT,
        )
        .await;
        match verify {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                let code = output.status.code().unwrap_or(-1);
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                outcome.result = Err(Error::Other(anyhow::anyhow!(
                    "Post-install verification failed: binary not found or not \
                     executable at {binary_path} (exit {code}): {stderr}"
                )));
            }
            Err(e) => {
                outcome.result = Err(Error::Other(anyhow::anyhow!(
                    "Post-install verification failed: {e}"
                )));
            }
        }
    }

    outcome
}
