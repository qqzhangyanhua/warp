# Agent Plan

## Goal
Fix `./script/run` on macOS when a Cargo bundle name differs from the historical
`WarpOss` or `WarpLocal` name. The script must operate on the bundle actually
produced by `cargo bundle`, including the current `ZYH.app` output.

## Files Likely Involved
- `script/macos/run`
- `app/Cargo.toml` (read-only source of bundle metadata; preserve current edits)

## Risks
- Release/deployment tooling: `script/macos/run` is adjacent to macOS bundle
  tooling and is classified as high risk by `docs/dangerous-areas.md`.
- Auth, billing, permissions, migrations, infra, public APIs, generated files,
  and terminal model locking are not involved.
- An incorrect path calculation could break debug, release, or custom-profile
  local launches for either the OSS or local channel.

## Plan
1. Read the selected binary's bundle name from Cargo package metadata, which is
   already queried by `script/macos/run` for the target directory.
2. Fail explicitly if the selected binary has no bundle name in Cargo metadata.
3. Build `WARP_APP_PATH` from the resolved name for debug, release, and custom
   profiles while preserving existing channel-specific URL schemes and binary
   names.
4. Do not modify the user's current bundle names or unrelated branding assets.

## Validation
- Unit tests: No existing unit-test seam covers this shell script; do not add a
  new framework for this narrow path-resolution fix.
- Integration tests: Run the real cached macOS bundle path with `--dont-open`.
- Manual checks: Confirm `ZYH.app/Contents/MacOS/warp-oss` receives the rpath,
  plist/resource/icon preparation, and code signing without referencing
  `WarpOss.app`.
- Commands to run:
  - `bash -n script/macos/run`
  - `TERM=xterm-256color ./script/run --features local_only_custom_provider_mode --dont-open`
  - `./scripts/check.sh` before PR-ready handoff; report any unrelated failures.

## Rollback
Revert only the `script/macos/run` path-resolution change. The current
`app/Cargo.toml` branding changes and bundle artifacts remain untouched.
