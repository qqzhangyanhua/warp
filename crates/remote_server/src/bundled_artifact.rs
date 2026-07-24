//! Bundled remote-daemon artifacts for offline SSH install.
//!
//! ZYH ships Linux and macOS arm64/x86_64 daemon tarballs inside the desktop
//! package. After the SSH preinstall check, the client selects the matching
//! artifact, verifies size and SHA-256 against the manifest, and uploads it
//! over the established SSH/SCP connection. Runtime HTTP/CDN downloads are not
//! part of the product contract.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use warp_core::channel::ChannelState;

use crate::setup::{RemoteArch, RemoteOs, RemotePlatform};

/// Manifest format version. Bump only for breaking layout changes.
pub const REMOTE_DAEMON_MANIFEST_VERSION: u32 = 1;

/// Protocol identity pinned in every release manifest. Must match the
/// protobuf handshake expectations of the client that ships the artifact.
pub const REMOTE_DAEMON_PROTOCOL_IDENTITY: &str = "zyh-remote-daemon/1";

/// Relative directory under bundled resources where daemon artifacts live.
pub const REMOTE_DAEMON_BUNDLE_DIR: &str = "bundled/remote-daemon";

/// Manifest file name inside the artifact root.
pub const REMOTE_DAEMON_MANIFEST_FILE: &str = "remote-daemon-manifest.json";

/// Development-only override for the artifact root. Release packaging and
/// release-channel runtime both reject this override.
pub const REMOTE_DAEMON_ARTIFACT_ROOT_OVERRIDE_ENV: &str = "ZYH_REMOTE_DAEMON_ARTIFACT_ROOT";

/// The four platforms required in every release package.
pub const REQUIRED_REMOTE_DAEMON_TARGETS: &[&str] = &[
    "linux-x86_64",
    "linux-aarch64",
    "macos-x86_64",
    "macos-aarch64",
];

/// One platform entry in the remote-daemon artifact manifest.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RemoteDaemonArtifactEntry {
    /// Path relative to the artifact root (must stay within the root).
    pub relative_path: String,
    /// Exact byte length of the artifact file.
    pub size: u64,
    /// Lowercase hex SHA-256 digest of the artifact file.
    pub sha256: String,
}

/// Versioned manifest of all bundled remote-daemon artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RemoteDaemonArtifactManifest {
    pub manifest_version: u32,
    /// Daemon / release version string for this package.
    pub daemon_version: String,
    /// Protocol identity the artifacts speak (must match client expectations).
    pub protocol_identity: String,
    /// Map of target key → artifact entry. Keys use `{os}-{arch}`.
    pub artifacts: BTreeMap<String, RemoteDaemonArtifactEntry>,
}

/// A verified local artifact ready for SCP upload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedRemoteDaemonArtifact {
    pub target: String,
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
}

/// Errors while loading, selecting, or verifying a bundled daemon artifact.
#[derive(Debug, thiserror::Error)]
pub enum BundledArtifactError {
    #[error("remote daemon artifact root is not configured")]
    RootMissing,
    #[error("development remote daemon artifact root override is forbidden in release builds")]
    OverrideForbiddenInRelease,
    #[error("failed to read remote daemon manifest at {path}: {source}")]
    ManifestRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse remote daemon manifest: {0}")]
    ManifestParse(String),
    #[error("remote daemon manifest version mismatch: expected {expected}, found {found}")]
    ManifestVersion { expected: u32, found: u32 },
    #[error(
        "remote daemon protocol identity mismatch: expected {expected}, found {found}"
    )]
    ProtocolIdentity {
        expected: String,
        found: String,
    },
    #[error("remote daemon manifest is missing required target {target}")]
    MissingTarget { target: String },
    #[error("remote daemon manifest has unexpected targets: {targets}")]
    UnexpectedTargets { targets: String },
    #[error("unsupported remote host for bundled daemon: {os}/{arch}")]
    UnsupportedPlatform { os: String, arch: String },
    #[error("remote daemon artifact path escapes the artifact root: {path}")]
    PathEscapesRoot { path: String },
    #[error("remote daemon artifact missing at {path}")]
    ArtifactMissing { path: PathBuf },
    #[error(
        "remote daemon artifact size mismatch for {target}: expected {expected}, found {found}"
    )]
    SizeMismatch {
        target: String,
        expected: u64,
        found: u64,
    },
    #[error(
        "remote daemon artifact SHA-256 mismatch for {target}: expected {expected}, found {found}"
    )]
    DigestMismatch {
        target: String,
        expected: String,
        found: String,
    },
    #[error("invalid SHA-256 digest in remote daemon manifest for {target}: {digest}")]
    InvalidDigest { target: String, digest: String },
    #[error("failed to read remote daemon artifact at {path}: {source}")]
    ArtifactRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Target key used in the manifest for a detected remote platform.
pub fn target_key_for_platform(platform: &RemotePlatform) -> &'static str {
    match (&platform.os, &platform.arch) {
        (RemoteOs::Linux, RemoteArch::X86_64) => "linux-x86_64",
        (RemoteOs::Linux, RemoteArch::Aarch64) => "linux-aarch64",
        (RemoteOs::MacOs, RemoteArch::X86_64) => "macos-x86_64",
        (RemoteOs::MacOs, RemoteArch::Aarch64) => "macos-aarch64",
    }
}

/// Parse and validate a remote-daemon manifest from JSON bytes.
pub fn parse_manifest(bytes: &[u8]) -> Result<RemoteDaemonArtifactManifest, BundledArtifactError> {
    let manifest: RemoteDaemonArtifactManifest = serde_json::from_slice(bytes)
        .map_err(|e| BundledArtifactError::ManifestParse(e.to_string()))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// Validate structural and identity constraints of a loaded manifest.
pub fn validate_manifest(
    manifest: &RemoteDaemonArtifactManifest,
) -> Result<(), BundledArtifactError> {
    if manifest.manifest_version != REMOTE_DAEMON_MANIFEST_VERSION {
        return Err(BundledArtifactError::ManifestVersion {
            expected: REMOTE_DAEMON_MANIFEST_VERSION,
            found: manifest.manifest_version,
        });
    }
    if manifest.protocol_identity != REMOTE_DAEMON_PROTOCOL_IDENTITY {
        return Err(BundledArtifactError::ProtocolIdentity {
            expected: REMOTE_DAEMON_PROTOCOL_IDENTITY.to_string(),
            found: manifest.protocol_identity.clone(),
        });
    }
    if manifest.daemon_version.trim().is_empty() {
        return Err(BundledArtifactError::ManifestParse(
            "daemon_version must be non-empty".to_string(),
        ));
    }

    for target in REQUIRED_REMOTE_DAEMON_TARGETS {
        if !manifest.artifacts.contains_key(*target) {
            return Err(BundledArtifactError::MissingTarget {
                target: (*target).to_string(),
            });
        }
    }
    let unexpected: Vec<&str> = manifest
        .artifacts
        .keys()
        .map(String::as_str)
        .filter(|t| !REQUIRED_REMOTE_DAEMON_TARGETS.contains(t))
        .collect();
    if !unexpected.is_empty() {
        return Err(BundledArtifactError::UnexpectedTargets {
            targets: unexpected.join(", "),
        });
    }

    for target in REQUIRED_REMOTE_DAEMON_TARGETS {
        let entry = manifest
            .artifacts
            .get(*target)
            .ok_or_else(|| BundledArtifactError::MissingTarget {
                target: (*target).to_string(),
            })?;
        validate_entry(target, entry)?;
    }
    Ok(())
}

fn validate_entry(
    target: &str,
    entry: &RemoteDaemonArtifactEntry,
) -> Result<(), BundledArtifactError> {
    if entry.relative_path.is_empty() {
        return Err(BundledArtifactError::ManifestParse(format!(
            "artifact path for {target} must be non-empty"
        )));
    }
    if entry.relative_path.starts_with('/')
        || entry.relative_path.starts_with('\\')
        || path_escapes_root(&entry.relative_path)
    {
        return Err(BundledArtifactError::PathEscapesRoot {
            path: entry.relative_path.clone(),
        });
    }
    if entry.size == 0 {
        return Err(BundledArtifactError::ManifestParse(format!(
            "artifact size for {target} must be positive"
        )));
    }
    if !is_valid_sha256_hex(&entry.sha256) {
        return Err(BundledArtifactError::InvalidDigest {
            target: target.to_string(),
            digest: entry.sha256.clone(),
        });
    }
    Ok(())
}

fn path_escapes_root(relative: &str) -> bool {
    let mut depth = 0i32;
    for component in relative.split(['/', '\\']) {
        match component {
            "" | "." => {}
            ".." => {
                depth -= 1;
                if depth < 0 {
                    return true;
                }
            }
            _ => depth += 1,
        }
    }
    false
}

fn is_valid_sha256_hex(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Select the manifest entry for a remote platform.
pub fn select_artifact_entry<'a>(
    manifest: &'a RemoteDaemonArtifactManifest,
    platform: &RemotePlatform,
) -> Result<(&'static str, &'a RemoteDaemonArtifactEntry), BundledArtifactError> {
    let target = target_key_for_platform(platform);
    let entry = manifest.artifacts.get(target).ok_or_else(|| {
        BundledArtifactError::UnsupportedPlatform {
            os: platform.os.as_str().to_string(),
            arch: platform.arch.as_str().to_string(),
        }
    })?;
    Ok((target, entry))
}

/// SHA-256 hex digest of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Verify in-memory artifact bytes against a manifest entry.
pub fn verify_artifact_bytes(
    target: &str,
    entry: &RemoteDaemonArtifactEntry,
    bytes: &[u8],
) -> Result<(), BundledArtifactError> {
    let found_size = bytes.len() as u64;
    if found_size != entry.size {
        return Err(BundledArtifactError::SizeMismatch {
            target: target.to_string(),
            expected: entry.size,
            found: found_size,
        });
    }
    let found = sha256_hex(bytes);
    if !constant_time_eq_hex(&found, &entry.sha256) {
        return Err(BundledArtifactError::DigestMismatch {
            target: target.to_string(),
            expected: entry.sha256.clone(),
            found,
        });
    }
    Ok(())
}

fn constant_time_eq_hex(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x.to_ascii_lowercase() ^ y.to_ascii_lowercase();
    }
    diff == 0
}

/// Resolve the on-disk artifact root.
///
/// Order:
/// 1. Development override env (rejected when `release_build` is true)
/// 2. `{bundled_resources}/bundled/remote-daemon`
pub fn resolve_artifact_root(
    bundled_resources: Option<&Path>,
    override_root: Option<&Path>,
    release_build: bool,
) -> Result<PathBuf, BundledArtifactError> {
    if let Some(override_path) = override_root {
        if release_build {
            return Err(BundledArtifactError::OverrideForbiddenInRelease);
        }
        return Ok(override_path.to_path_buf());
    }
    let Some(resources) = bundled_resources else {
        return Err(BundledArtifactError::RootMissing);
    };
    Ok(resources.join(REMOTE_DAEMON_BUNDLE_DIR))
}

/// Whether the current process is treated as a release build for override policy.
///
/// Packaged clients bake `GIT_RELEASE_TAG` at compile time (via
/// [`ChannelState::app_version`]). End-user machines do not export that env var
/// at runtime, so checking only the process environment would allow overrides
/// on every release install. Packaging scripts may also set the env var.
pub fn is_release_build() -> bool {
    ChannelState::app_version().is_some_and(|v| !v.is_empty())
        || std::env::var_os("GIT_RELEASE_TAG").is_some_and(|v| !v.is_empty())
}

/// Load the manifest from an artifact root directory.
pub fn load_manifest_from_root(
    artifact_root: &Path,
) -> Result<RemoteDaemonArtifactManifest, BundledArtifactError> {
    let path = artifact_root.join(REMOTE_DAEMON_MANIFEST_FILE);
    let bytes = std::fs::read(&path).map_err(|source| BundledArtifactError::ManifestRead {
        path: path.clone(),
        source,
    })?;
    parse_manifest(&bytes)
}

/// Stream-hash a file; returns (byte length, lowercase hex SHA-256).
pub fn sha256_file(path: &Path) -> Result<(u64, String), BundledArtifactError> {
    let mut file = std::fs::File::open(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            BundledArtifactError::ArtifactMissing {
                path: path.to_path_buf(),
            }
        } else {
            BundledArtifactError::ArtifactRead {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buf).map_err(|source| BundledArtifactError::ArtifactRead {
            path: path.to_path_buf(),
            source,
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
    let digest = hasher.finalize();
    let hex = digest.iter().map(|b| format!("{b:02x}")).collect();
    Ok((size, hex))
}

/// Verify an on-disk artifact against a manifest entry without loading it whole.
pub fn verify_artifact_file(
    target: &str,
    entry: &RemoteDaemonArtifactEntry,
    path: &Path,
) -> Result<(), BundledArtifactError> {
    let (found_size, found) = sha256_file(path)?;
    if found_size != entry.size {
        return Err(BundledArtifactError::SizeMismatch {
            target: target.to_string(),
            expected: entry.size,
            found: found_size,
        });
    }
    if !constant_time_eq_hex(&found, &entry.sha256) {
        return Err(BundledArtifactError::DigestMismatch {
            target: target.to_string(),
            expected: entry.sha256.clone(),
            found,
        });
    }
    Ok(())
}

/// Load, select, and verify the local artifact for `platform`.
pub fn resolve_verified_artifact(
    artifact_root: &Path,
    platform: &RemotePlatform,
) -> Result<VerifiedRemoteDaemonArtifact, BundledArtifactError> {
    let manifest = load_manifest_from_root(artifact_root)?;
    let (target, entry) = select_artifact_entry(&manifest, platform)?;
    if path_escapes_root(&entry.relative_path) {
        return Err(BundledArtifactError::PathEscapesRoot {
            path: entry.relative_path.clone(),
        });
    }
    let path = artifact_root.join(&entry.relative_path);
    if !path.is_file() {
        return Err(BundledArtifactError::ArtifactMissing { path });
    }
    verify_artifact_file(target, entry, &path)?;
    Ok(VerifiedRemoteDaemonArtifact {
        target: target.to_string(),
        path,
        size: entry.size,
        sha256: entry.sha256.clone(),
    })
}

/// Resolve the default artifact root using bundled resources and the override env.
pub fn default_artifact_root() -> Result<PathBuf, BundledArtifactError> {
    let override_root = std::env::var_os(REMOTE_DAEMON_ARTIFACT_ROOT_OVERRIDE_ENV)
        .map(PathBuf::from);
    let bundled = warp_core::paths::bundled_resources_dir();
    resolve_artifact_root(
        bundled.as_deref(),
        override_root.as_deref(),
        is_release_build(),
    )
}

/// Product policy: remote daemon install must never initiate HTTP.
pub fn runtime_http_download_allowed() -> bool {
    false
}

#[cfg(test)]
#[path = "bundled_artifact_tests.rs"]
mod tests;
