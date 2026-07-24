use super::*;
use crate::setup::{RemoteArch, RemoteOs, RemotePlatform};

fn valid_manifest_json(digest: &str, size: u64) -> String {
    format!(
        r#"{{
  "manifest_version": 1,
  "daemon_version": "test-1.0.0",
  "protocol_identity": "zyh-remote-daemon/1",
  "artifacts": {{
    "linux-aarch64": {{
      "relative_path": "linux-aarch64/zyh-remote-daemon.tar.gz",
      "size": {size},
      "sha256": "{digest}"
    }},
    "linux-x86_64": {{
      "relative_path": "linux-x86_64/zyh-remote-daemon.tar.gz",
      "size": {size},
      "sha256": "{digest}"
    }},
    "macos-aarch64": {{
      "relative_path": "macos-aarch64/zyh-remote-daemon.tar.gz",
      "size": {size},
      "sha256": "{digest}"
    }},
    "macos-x86_64": {{
      "relative_path": "macos-x86_64/zyh-remote-daemon.tar.gz",
      "size": {size},
      "sha256": "{digest}"
    }}
  }}
}}"#
    )
}

fn platform(os: RemoteOs, arch: RemoteArch) -> RemotePlatform {
    RemotePlatform { os, arch }
}

#[test]
fn target_keys_cover_four_required_platforms() {
    assert_eq!(
        target_key_for_platform(&platform(RemoteOs::Linux, RemoteArch::X86_64)),
        "linux-x86_64"
    );
    assert_eq!(
        target_key_for_platform(&platform(RemoteOs::Linux, RemoteArch::Aarch64)),
        "linux-aarch64"
    );
    assert_eq!(
        target_key_for_platform(&platform(RemoteOs::MacOs, RemoteArch::X86_64)),
        "macos-x86_64"
    );
    assert_eq!(
        target_key_for_platform(&platform(RemoteOs::MacOs, RemoteArch::Aarch64)),
        "macos-aarch64"
    );
    assert_eq!(REQUIRED_REMOTE_DAEMON_TARGETS.len(), 4);
}

#[test]
fn parse_and_validate_complete_manifest() {
    let bytes = b"payload";
    let digest = sha256_hex(bytes);
    let json = valid_manifest_json(&digest, bytes.len() as u64);
    let manifest = parse_manifest(json.as_bytes()).expect("manifest should parse");
    assert_eq!(manifest.daemon_version, "test-1.0.0");
    assert_eq!(manifest.protocol_identity, REMOTE_DAEMON_PROTOCOL_IDENTITY);
    assert_eq!(manifest.artifacts.len(), 4);
}

#[test]
fn reject_wrong_protocol_identity() {
    let json = valid_manifest_json(&sha256_hex(b"x"), 1).replace(
        REMOTE_DAEMON_PROTOCOL_IDENTITY,
        "warp-remote-daemon/1",
    );
    match parse_manifest(json.as_bytes()) {
        Err(BundledArtifactError::ProtocolIdentity { expected, found }) => {
            assert_eq!(expected, REMOTE_DAEMON_PROTOCOL_IDENTITY);
            assert_eq!(found, "warp-remote-daemon/1");
        }
        other => panic!("expected ProtocolIdentity, got {other:?}"),
    }
}

#[test]
fn reject_missing_required_target() {
    let json = r#"{
  "manifest_version": 1,
  "daemon_version": "1",
  "protocol_identity": "zyh-remote-daemon/1",
  "artifacts": {
    "linux-x86_64": {
      "relative_path": "linux-x86_64/a.tar.gz",
      "size": 1,
      "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    }
  }
}"#;
    match parse_manifest(json.as_bytes()) {
        Err(BundledArtifactError::MissingTarget { target }) => {
            assert!(REQUIRED_REMOTE_DAEMON_TARGETS.contains(&target.as_str()));
        }
        other => panic!("expected MissingTarget, got {other:?}"),
    }
}

#[test]
fn reject_path_escape() {
    let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let mut json = valid_manifest_json(digest, 1);
    json = json.replace(
        "linux-x86_64/zyh-remote-daemon.tar.gz",
        "../escape.tar.gz",
    );
    match parse_manifest(json.as_bytes()) {
        Err(BundledArtifactError::PathEscapesRoot { path }) => {
            assert!(path.contains(".."));
        }
        other => panic!("expected PathEscapesRoot, got {other:?}"),
    }
}

#[test]
fn select_entry_for_remote_platform() {
    let bytes = b"daemon-bytes";
    let digest = sha256_hex(bytes);
    let manifest = parse_manifest(valid_manifest_json(&digest, bytes.len() as u64).as_bytes())
        .unwrap();
    let (target, entry) =
        select_artifact_entry(&manifest, &platform(RemoteOs::Linux, RemoteArch::Aarch64)).unwrap();
    assert_eq!(target, "linux-aarch64");
    assert!(entry.relative_path.contains("linux-aarch64"));
}

#[test]
fn verify_bytes_accepts_matching_digest_and_size() {
    let bytes = b"daemon-bytes";
    let digest = sha256_hex(bytes);
    let manifest = parse_manifest(valid_manifest_json(&digest, bytes.len() as u64).as_bytes())
        .unwrap();
    let (target, entry) =
        select_artifact_entry(&manifest, &platform(RemoteOs::MacOs, RemoteArch::X86_64)).unwrap();
    verify_artifact_bytes(target, entry, bytes).unwrap();
}

#[test]
fn verify_bytes_rejects_digest_mismatch() {
    let bytes = b"daemon-bytes";
    let digest = sha256_hex(bytes);
    let manifest = parse_manifest(valid_manifest_json(&digest, bytes.len() as u64).as_bytes())
        .unwrap();
    let (target, entry) =
        select_artifact_entry(&manifest, &platform(RemoteOs::MacOs, RemoteArch::X86_64)).unwrap();
    match verify_artifact_bytes(target, entry, b"tampered") {
        Err(BundledArtifactError::SizeMismatch { .. } | BundledArtifactError::DigestMismatch { .. }) => {}
        other => panic!("expected size or digest mismatch, got {other:?}"),
    }
}

#[test]
fn release_build_rejects_local_override() {
    let err = resolve_artifact_root(
        Some(Path::new("/bundle/resources")),
        Some(Path::new("/tmp/dev-override")),
        true,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        BundledArtifactError::OverrideForbiddenInRelease
    ));
}

#[test]
fn development_allows_local_override() {
    let path = resolve_artifact_root(
        Some(Path::new("/bundle/resources")),
        Some(Path::new("/tmp/dev-override")),
        false,
    )
    .unwrap();
    assert_eq!(path, PathBuf::from("/tmp/dev-override"));
}

#[test]
fn default_root_uses_bundled_remote_daemon_dir() {
    let path = resolve_artifact_root(Some(Path::new("/app/resources")), None, false).unwrap();
    assert_eq!(
        path,
        PathBuf::from("/app/resources").join(REMOTE_DAEMON_BUNDLE_DIR)
    );
}

#[test]
fn runtime_http_download_is_denied() {
    assert!(!runtime_http_download_allowed());
}

#[cfg(unix)]
#[test]
fn resolve_verified_artifact_from_disk() {
    let tmp = tempfile_dir();
    let bytes = b"zyh-remote-daemon-tarball-contents";
    let digest = sha256_hex(bytes);
    let size = bytes.len() as u64;

    for target in REQUIRED_REMOTE_DAEMON_TARGETS {
        let dir = tmp.join(target);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("zyh-remote-daemon.tar.gz"), bytes).unwrap();
    }
    std::fs::write(
        tmp.join(REMOTE_DAEMON_MANIFEST_FILE),
        valid_manifest_json(&digest, size),
    )
    .unwrap();

    let verified = resolve_verified_artifact(
        &tmp,
        &platform(RemoteOs::Linux, RemoteArch::X86_64),
    )
    .expect("verified artifact");
    assert_eq!(verified.target, "linux-x86_64");
    assert_eq!(verified.size, size);
    assert_eq!(verified.sha256, digest);
    assert!(verified.path.exists());
}

#[cfg(unix)]
#[test]
fn resolve_verified_artifact_fails_on_digest_mismatch() {
    let tmp = tempfile_dir();
    let bytes = b"correct-bytes";
    let digest = sha256_hex(bytes);
    let size = bytes.len() as u64;
    for target in REQUIRED_REMOTE_DAEMON_TARGETS {
        let dir = tmp.join(target);
        std::fs::create_dir_all(&dir).unwrap();
        // Corrupt only one platform after writing a matching digest for all.
        let content = if *target == "macos-aarch64" {
            &b"wrong-bytes"[..]
        } else {
            &bytes[..]
        };
        // Keep size equal so we hit digest mismatch.
        let mut padded = content.to_vec();
        padded.resize(size as usize, b'x');
        std::fs::write(dir.join("zyh-remote-daemon.tar.gz"), padded).unwrap();
    }
    std::fs::write(
        tmp.join(REMOTE_DAEMON_MANIFEST_FILE),
        valid_manifest_json(&digest, size),
    )
    .unwrap();

    let err = resolve_verified_artifact(
        &tmp,
        &platform(RemoteOs::MacOs, RemoteArch::Aarch64),
    )
    .unwrap_err();
    assert!(matches!(err, BundledArtifactError::DigestMismatch { .. }));
}

#[cfg(unix)]
fn tempfile_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "zyh-remote-daemon-test-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
