# Remote Daemon Artifacts

ZYH ships Linux and macOS arm64/x86_64 remote-daemon tarballs inside the desktop
package. Runtime install never downloads over HTTP/CDN; the client selects the
matching artifact, verifies size and SHA-256 against the manifest, and uploads
over SSH/SCP.

## Layout

Release packaging (`script/prepare_remote_daemon_artifacts`) copies:

```text
bundled/remote-daemon/
  remote-daemon-manifest.json
  linux-x86_64/zyh-remote-daemon.tar.gz
  linux-aarch64/zyh-remote-daemon.tar.gz
  macos-x86_64/zyh-remote-daemon.tar.gz
  macos-aarch64/zyh-remote-daemon.tar.gz
```

Source artifacts for packaging live under `dist/remote-daemon/` (or the
development-only `ZYH_REMOTE_DAEMON_ARTIFACT_ROOT` override). Release packaging
rejects the override and fails when any target is missing or mismatched.

## Manifest

```json
{
  "manifest_version": 1,
  "daemon_version": "<release version>",
  "protocol_identity": "zyh-remote-daemon/1",
  "artifacts": {
    "linux-x86_64": {
      "relative_path": "linux-x86_64/zyh-remote-daemon.tar.gz",
      "size": 12345,
      "sha256": "<64-char lowercase hex>"
    }
  }
}
```

All four targets are required. `protocol_identity` must be `zyh-remote-daemon/1`.

## Remote install path

Installed binaries live under ZYH home directories, for example:

- stable: `~/.zyh/remote-server`
- dev: `~/.zyh-dev/remote-server`
- local: `~/.zyh-local/remote-server`
