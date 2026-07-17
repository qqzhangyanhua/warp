# Agent Plan

## Goal

Allow the GUI to start in Local-only Mode without registering Warp Cloud update services, while preserving existing Account Sign-in and Anonymous-only behavior.

## Files Likely Involved

- `app/src/env_vars/manager.rs`
- `app/src/env_vars/manager_tests.rs`
- `app/src/workflows/manager.rs`
- `app/src/workflows/manager_tests.rs`

## Risks

- Local-only/Cloud network boundary: registering a real or inert `UpdateManager` could accidentally initialize Warp Cloud behavior, so the fix must leave it unregistered.
- Restored cloud-backed workflow or environment-variable panes may still reach Cloud-only actions after startup; focused startup verification must check for the next missing-singleton failure and forbidden Warp requests.
- Normal Account Sign-in and Anonymous-only modes must retain their existing `UpdateManager` subscriptions.
- No billing, permissions, migrations, public APIs, generated files, or terminal locking are changed.

## Plan

1. Add regression tests showing that `EnvVarCollectionManager` and `WorkflowManager` can be registered in Local-only Mode without an `UpdateManager` singleton.
2. Keep both managers registered, but skip only their `UpdateManager` event subscriptions in Local-only Mode. Do not register a placeholder Cloud manager.
3. Preserve the current subscription path unchanged outside Local-only Mode.
4. Audit the remaining Local-only startup registrations for constructors that synchronously require `UpdateManager`, so startup does not fail one singleton later.
5. Rebuild and rerun the original Local-only GUI startup reproduction with backtraces enabled.

## Validation

- Unit tests: new Local-only constructor tests for both managers.
- Integration tests: use the real Local-only GUI startup as the integration seam because no existing test covers the complete `initialize_app` singleton graph.
- Manual checks: confirm the workspace reaches a usable local terminal, no missing-singleton panic occurs, and startup logs show no Warp identity, Cloud sync, telemetry, or Sentry request.
- Commands to run:
  - `cargo test -p warp --lib env_vars::manager -- --nocapture`
  - `cargo test -p warp --lib workflows::manager -- --nocapture`
  - `cargo check -p warp --bin warp-oss --features local_only_custom_provider_mode`
  - `cargo fmt -- --check`
  - `./script/run --features local_only_custom_provider_mode`

## Rollback

Remove the two Local-only subscription guards and their regression tests. No persisted data or schema rollback is required.

## Implementation Outcome

- The real startup trace exposed additional eager Cloud dependencies beyond the two initial managers. Local-only Mode now skips those subscriptions and does not construct Cloud-only sharing, Warp Drive, import, team, billing, or preference surfaces.
- Local Agent conversations and AI documents remain registered for local persistence, while their Cloud polling, Warp Drive publication, and synchronization paths are disabled.
- Cloud credentials are cleared in memory after auth initialization in Local-only Mode, without changing persisted credentials used by normal launches.
- The bundled `WarpOss.app` opened a workspace, restored local Agent data, and bootstrapped local shells without a missing-singleton panic or credential refresh request.
