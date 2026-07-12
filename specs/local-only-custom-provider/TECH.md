# Local-only Custom Provider Mode - Product Shell

See `PRODUCT.md` and GitHub issue #2.

## Context

- `crates/warp_features/src/lib.rs` defines high-level runtime flags.
- `app/src/features.rs` maps Cargo features to runtime flags.
- `app/src/lib.rs::initialize_app` builds `AuthState`, `ServerApiProvider`,
  telemetry, crash reporting, cloud models, and startup refresh behavior.
- `app/src/root_view.rs` decides whether the GUI shows auth/onboarding or opens
  a workspace, and currently creates an Anonymous Session in
  `AnonymousOnlyMode`.
- `app/src/tui/mod.rs` performs TUI login gating and currently creates an
  Anonymous Session in `AnonymousOnlyMode`.
- `app/src/ai/agent_sdk/admin.rs` implements CLI `login`, `logout`, and
  `whoami`.

## Proposed Changes

1. Add `FeatureFlag::LocalOnlyCustomProviderMode`, enabled by Cargo feature
   `local_only_custom_provider_mode` in both GUI and TUI crates.
2. Add `app/src/local_mode.rs` with a small policy API:
   - `is_local_only_custom_provider_mode()`
   - `account_sign_in_unavailable_message()`
   - `get_or_create_local_identity(ctx)`
   - `local_identity_for_test(ctx)` helpers gated for tests.
3. Persist local identity in private user preferences using a new key. Invalid
   stored values are ignored and replaced with a new UUID.
4. In `initialize_app`, derive `local_only` once and:
   - pass `anonymous_only && !local_only` to `AuthState::initialize`;
   - drop API-key auth when `local_only`;
   - skip auth refresh, logged-out telemetry, download reporting, cloud
     preference sync, server experiment refresh, crash reporting init,
     tracing auth refresh, and login-item registration in Local-only Mode.
5. In `RootView`, treat Local-only like a local terminal shell for initial state,
   but do not call `AuthManager::create_anonymous_user`.
6. In TUI init, treat Local-only as `LoggedIn` for session creation, but return
   before device auth or Anonymous Session creation.
7. In CLI admin commands, make `login`/`logout` return the Local-only error and
   make `whoami` return `local:<uuid>` without workspace metadata refresh.

## Tests

- `local_mode` unit tests:
  - persisted identity is reused;
  - malformed stored identity is replaced;
  - Local-only policy follows the feature flag.
- CLI admin tests:
  - `login` and `logout` reject in Local-only Mode;
  - `whoami` text output returns `local:<uuid>` and terminates.
- TUI tests:
  - Local-only initial phase is `LoggedIn`;
  - Anonymous-only still requests Anonymous Session, preserving existing behavior.

## Risks

- Auth/network startup is high risk. This increment avoids deleting stored
  credentials and keeps non-local behavior unchanged.
- Some cloud/account UI entry points may still be present until later cleanup
  increments. They must not be initialized or contacted by this startup path.
- This does not yet prove a full forbidden-network integration test. The first
  automated boundary is unit coverage around the owned startup branches.

## Validation

- Focused Rust unit tests for changed modules.
- `./script/format --check`
- `./scripts/check.sh` before PR-ready handoff if time and local dependencies
  permit.
