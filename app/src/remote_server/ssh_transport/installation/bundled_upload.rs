//! Install the remote daemon from a verified local bundled artifact over SCP.
//!
//! Never downloads over HTTP/CDN. The artifact is selected from the remote
//! OS/arch after preinstall, checksum-verified against the release manifest,
//! uploaded through the established SSH control socket, then extracted by the
//! install script.

use std::path::Path;

use remote_server::bundled_artifact::{
    default_artifact_root, resolve_verified_artifact, runtime_http_download_allowed,
};
use remote_server::setup::RemotePlatform;
use remote_server::transport::Error;

/// Installs the remote server by uploading a verified bundled tarball via SCP.
pub(super) async fn install(socket_path: &Path) -> Result<(), Error> {
    debug_assert!(
        !runtime_http_download_allowed(),
        "remote daemon install must never allow runtime HTTP download"
    );

    let platform = super::super::detect_remote_platform(socket_path).await?;
    install_for_platform(socket_path, &platform).await
}

async fn install_for_platform(
    socket_path: &Path,
    platform: &RemotePlatform,
) -> Result<(), Error> {
    let artifact_root = default_artifact_root().map_err(|e| {
        Error::Other(anyhow::anyhow!("Remote daemon artifact root unavailable: {e}"))
    })?;
    let verified = resolve_verified_artifact(&artifact_root, platform).map_err(|e| {
        Error::Other(anyhow::anyhow!(
            "Failed to resolve verified remote daemon artifact for {}/{}: {e}",
            platform.os.as_str(),
            platform.arch.as_str()
        ))
    })?;

    let timeout = remote_server::setup::SCP_INSTALL_TIMEOUT;
    let install_dir = remote_server::setup::remote_server_dir();
    let remote_tarball_name = format!("zyh-remote-daemon-upload-{}.tar.gz", uuid::Uuid::new_v4());
    let remote_tarball_path = format!("{install_dir}/{remote_tarball_name}");

    let mkdir_output = remote_server::ssh::run_ssh_command(
        socket_path,
        &format!("mkdir -p {install_dir}"),
        remote_server::setup::CHECK_TIMEOUT,
    )
    .await
    .map_err(Error::from)?;
    if !mkdir_output.status.success() {
        let code = mkdir_output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&mkdir_output.stderr).to_string();
        return Err(Error::ScriptFailed {
            exit_code: code,
            stderr,
        });
    }

    log::info!(
        "Uploading verified remote daemon artifact {} ({}) to {remote_tarball_path}",
        verified.target,
        verified.path.display()
    );
    remote_server::ssh::scp_upload(
        socket_path,
        &verified.path,
        &remote_tarball_path,
        timeout,
    )
    .await
    .map_err(Error::Other)?;

    log::info!("Running extract-only install script with tarball at {remote_tarball_path}");
    let script = remote_server::setup::install_script(Some(&remote_tarball_path));

    let output = remote_server::ssh::run_ssh_script(socket_path, &script, timeout)
        .await
        .map_err(Error::from)?;
    if output.status.success() {
        Ok(())
    } else {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(Error::ScriptFailed {
            exit_code: code,
            stderr,
        })
    }
}
