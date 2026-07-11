# Dangerous Areas

## High-Risk Areas
- Auth and authentication:
  - `app/src/auth/`
  - `app/src/server/server_api/auth.rs`
  - `app/src/remote_server/auth_context.rs`
  - `app/src/remote_server/auth_provider.rs`
  - `crates/warp_server_auth/`
  - `crates/warp_server_client/src/auth/`
  - OAuth paths under `app/src/ai/`, `crates/ai/`, and `crates/mcp/`.
- Permissions:
  - `app/src/ai/blocklist/permissions.rs`
  - `app/src/local_control/permissions.rs`
  - `app/src/terminal/shared_session/permissions_manager.rs`
  - `crates/cloud_object_persistence/src/encoded_permissions.rs`
  - GraphQL object-permission mutations and queries under `crates/graphql/src/api/`.
- Billing:
  - `app/src/billing/`
  - `app/src/settings_view/billing_and_usage*`
  - `crates/graphql/src/api/billing.rs`
  - `crates/graphql/src/api/mutations/stripe_billing_portal.rs`
  - billing-related migrations.
- Secrets:
  - `app/src/external_secrets/`
  - `app/src/search/external_secrets/`
  - `app/src/server/server_api/managed_secrets.rs`
  - `app/src/server/telemetry/secret_redaction.rs`
  - `app/src/terminal/model/secrets.rs`
  - `crates/managed_secrets/`
  - `crates/managed_secrets_wasm/`
  - GraphQL managed-secret and task-secret APIs.
- Database migrations and persistence:
  - `crates/persistence/migrations/`
  - `crates/persistence/src/schema.rs`
  - `crates/persistence/schema.patch`
  - `app/src/persistence/`
- Public APIs, protocols, and generated clients:
  - `crates/graphql/`
  - `crates/warp_graphql_schema/`
  - `crates/warp_server_client/`
  - `crates/ipc/src/protocol.rs`
  - `crates/local_control/src/protocol.rs`
  - `app/src/terminal/local_tty/server/protocol.rs`
  - `app/src/remote_server/*_proto.rs`
- Release, deployment, and CI:
  - `.github/workflows/`
  - `.github/actions/`
  - `script/bundle`, platform bundle scripts, release scripts, and Sentry upload scripts.
- Terminal model locking:
  - Any path that calls or adds `TerminalModel::lock()` or equivalent `model.lock()` access on terminal state.

## Rules
- Do not edit generated files directly.
- Do not change auth, billing, permissions, migrations, infra, secrets, or public APIs without explicit risk notes and focused verification.
- Do not drop, rename, or rewrite persisted data without a compatibility plan.
- Do not change release or deployment scripts casually.
- Keep terminal model lock scopes short and verify callers are not already holding the same lock.
- When adding a toggleable setting, add matching Command Palette enable/disable entries and required context flags.

## Mechanical Checks To Add
- Recommended: a focused generated-client validation script for GraphQL/schema updates.
- Recommended: a migration validation wrapper that runs Diesel migration checks against a disposable database.
- Recommended: a lightweight focused-check script for common Rust unit-test and clippy subsets.
- Recommended: a lock-risk checklist for terminal model changes.
- Recommended: a CI or presubmit check that keeps this harness in sync after script or workflow changes.
