# Agent Plan

## Goal

Make GUI, TUI, and CLI startup follow the permanent ZYH local-product contract:
they reach their retained local surface without creating identity state or hosted
services, and without making an app-initiated external network request.

Agent entry points without a configured OpenAI-compatible Provider must direct
the user to local Provider settings. Startup keeps only redacted local logging;
diagnostic export remains an explicit user action.

This is a high-risk auth and startup change. This plan is awaiting human review
and does not authorize business-code changes by itself.

## Scope

- Replace the Local-only/Anonymous/Account startup branches with one immutable
  ZYH startup policy shared by GUI, TUI, and CLI dispatch.
- Remove Account, Anonymous Session, and Local Identity construction from
  startup, including the TUI device-authorization flow.
- Stop registering Auth, Warp server, GraphQL, cloud sync, quota, telemetry,
  Sentry, updater, and remote-experiment services in local startup.
- Remove `login`, `logout`, and `whoami` from the ZYH CLI grammar and dispatch.
- Produce an actionable local setup path for a missing Provider.
- Retain redacted local file logs and explicit local diagnostic export.
- Convert the existing GUI, TUI, and CLI request-recorder baselines from
  feature-flagged Local-only tests into permanent-product tests.

## Non-Goals

- Physically deleting every cloud crate or generated GraphQL tree. This issue
  removes startup consumers; dependency pruning follows the deletion order in
  ADR-0009 after retained consumers have moved.
- Pi runtime replacement, local data-source replacement, persistence cleanup,
  SSH daemon packaging, or product-wide branding. Those are separate phases in
  `docs/zyh-local-product-plan.md`.
- Changing migrations, secrets, permissions, billing, release/deployment, or
  public protocols.
- Adding Provider auto-detection, probing, routing, or fallback.

## TDD Seams Requiring Approval

Tests will be written only at these observable boundaries:

1. **CLI grammar and process startup**: `warp_cli::Args::try_parse_from` proves
   identity commands are absent; a retained local command process proves CLI
   startup completes with no recorded request.
2. **TUI process startup**: the PTY-based `warp-tui-oss` test proves terminal
   input is reached without a sign-in prompt, browser/device authorization, or
   recorded request.
3. **GUI integration startup**: the integration runner proves a focused local
   terminal and local settings are reachable, no forbidden singleton type is
   registered, and the recorder observes no request.
4. **Agent entry action**: the existing Agent controller/runtime public action
   path proves missing Base URL, Model, or API Key returns the corresponding
   local setup action and never a quota or Account error.
5. **Logging/diagnostic boundary**: the logging initialization API proves local
   file output uses the ZYH log root and redaction; the diagnostic export action
   proves no export occurs before explicit invocation and its output is local.

Each slice follows red -> green. Tests assert user-visible outcomes or the
application singleton registry, not private helper calls or mock call counts.

## Files Likely Involved

The exact set will be compile-driven; generated files and historical migrations
will not be edited.

- Shared startup and dispatch: `app/src/lib.rs`, a small startup-policy module
  under `app/src/`, and focused `*_tests.rs` files.
- GUI entry state: `app/src/root_view.rs`, retained terminal/workspace startup,
  and settings routing only where they still assume identity or cloud models.
- TUI entry state: `app/src/tui/mod.rs`, `app/src/tui/mod_tests.rs`,
  `crates/warp_tui/tests/local_only_startup.rs` (renamed to permanent-product
  vocabulary), and `crates/warp_tui/src/autoupdate.rs` if startup still calls it.
- Identity and feature policy: `app/src/local_mode.rs`, its tests,
  `crates/warp_features/src/lib.rs`, `app/src/features.rs`, and compile-reported
  consumers of `LocalOnlyCustomProviderMode` or `AnonymousOnlyMode`.
- CLI grammar and dispatch: `crates/warp_cli/src/lib.rs`,
  `crates/warp_cli/src/lib_tests.rs`, `app/src/ai/agent_sdk/mod.rs`, its tests,
  and identity-only admin code after its last consumer is removed.
- Provider setup outcome: `app/src/ai/agent/runtime/service/errors.rs`, the
  Agent action/controller boundary, `app/src/settings_view/ai_page.rs`, and
  focused tests beside those modules.
- Hosted startup services: construction sites in `app/src/lib.rs` plus retained
  consumers that currently require `AuthStateProvider`, `ServerApiProvider`,
  `ServerExperiments`, cloud object/sync, quota, telemetry, crash reporting, or
  update globals.
- Logging and diagnostics: `app/src/tracing/`, `app/src/crash_reporting/`,
  `crates/warp_logging/`, `app/src/workspace/action.rs`, and existing debug-info
  export code. Physical module deletion is limited to code made unreferenced by
  this issue.
- Higher-level validation: `crates/integration/src/bin/integration.rs`,
  `crates/integration/src/test/local_mode.rs` (renamed),
  `crates/integration/tests/integration/startup_request_recording.rs`, and the
  startup request recorder without changing its public behavior.
- Inventory/docs: `docs/zyh-local-product-inventory.json` only when a classified
  startup consumer is actually removed or replaced.

## Risks

- **Auth dependency graph:** retained views and models may read Auth or server
  globals even when they never send a request. Removing registrations must be
  paired with removing or relocating each retained consumer, not a null hosted
  service.
- **False network confidence:** the recorder covers proxy-aware HTTP, HTTPS,
  and WebSocket clients. The existing socket-level checks and constructor audit
  remain necessary for unclassified raw sockets.
- **Terminal regression:** deleting onboarding/auth root states can leave GUI or
  TUI without a mounted terminal, focused input, or restoration path.
- **Provider UX regression:** the current `MissingProvider` error is generic.
  Mapping it to settings must preserve missing-field specificity and must not
  start an Agent Run or leak API Keys.
- **Logging regression:** removing Sentry/telemetry must not disable local logs,
  put secrets in logs, or export diagnostics automatically.
- **Feature-flag fan-out:** permanent-product flags have many consumers. Delete
  obsolete branches in small compile-checked groups; do not replace them with
  a new permanent-true flag or hidden compatibility mode.
- **CLI compatibility:** removing identity commands is intentionally breaking
  per ADR-0009. Retained Provider, MCP, local Agent, and terminal commands must
  keep their argument and output contracts.
- **Current branch state:** `master` is already three commits ahead of
  `origin/master` for Issue #25, and `docs/zyh-local-product-plan.md` is an
  unrelated untracked user file. Neither will be rewritten or included in the
  Issue #26 commit.

## Plan

1. **Pin permanent startup expectations (red).**
   - Rename the existing Local-only startup tests to ZYH vocabulary.
   - Remove test feature overrides.
   - Change the CLI process baseline from `logout` to a retained local command.
   - Add assertions that the forbidden hosted-service singleton types are
     absent after GUI startup.
   - Run each affected test alone and record the expected failures.

2. **Introduce one startup policy and make TUI identity-free (green).**
   - Represent the permanent startup contract as one immutable policy passed to
     `initialize_app`; it is not a feature flag and has no hosted variant.
   - Make retained initialization explicit and group it by ownership boundary.
   - Remove TUI login phases, Auth subscriptions, anonymous-user creation, and
     device authorization; mount the terminal directly.
   - Run TUI unit tests, the PTY startup test, and `cargo check -p warp`.

3. **Remove identity from CLI grammar and dispatch.**
   - Delete `Login`, `Logout`, and `Whoami` variants, help text, tracing mapping,
     auth-requirement mapping, telemetry mapping, and dispatch arms.
   - Remove identity admin functions/modules only after `rg` proves no retained
     consumer.
   - Keep a retained local CLI command on the normal initialized path, but do
     not initialize hosted services for it.
   - Run `warp_cli` parser tests, Agent SDK focused tests, the CLI startup
     recorder test, and affected crate checks.

4. **Delete identity and hosted-service construction from shared startup.**
   - Remove API-key/anonymous/account startup selection, `AuthState`,
     `AuthManager`, Local Identity creation/refresh, and remote experiments.
   - Remove construction and registration of Warp REST/GraphQL/WebSocket,
     CloudModel/cloud sync, quota, telemetry, Sentry, updater, referral, survey,
     and remote changelog services.
   - For each compile failure, either remove a cloud-only consumer or move the
     retained local capability to a local dependency. Do not register inert
     hosted clients to satisfy constructors.
   - Keep persistence, settings, terminal, local Provider/MCP, local file
     features, and user-initiated SSH/Git construction.
   - Run `cargo check -p warp` after each service family and rerun GUI/TUI/CLI
     startup tests after every green slice.

5. **Remove obsolete product-mode flags and branches.**
   - Delete `LocalOnlyCustomProviderMode` and `AnonymousOnlyMode` only after
     startup no longer reads them.
   - Resolve remaining consumers by retaining the already-approved local branch
     or deleting the hosted branch; do not add replacement constants.
   - Delete `app/src/local_mode.rs` after Local Identity and the last mode helper
     consumer are gone.
   - Run focused tests and compile checks for every affected crate group.

6. **Make missing Provider configuration actionable.**
   - Extend the existing `MissingProvider` outcome to identify Base URL, Model,
     or API Key without including values.
   - Map the GUI/TUI action to local Provider settings and the CLI action to a
     concise `provider setup` instruction.
   - Assert no Agent Run/Provider Attempt starts and no quota or Account wording
     appears.
   - Run focused runtime/controller and CLI tests.

7. **Keep only local redacted logs and explicit diagnostics.**
   - Remove Sentry and telemetry startup/shutdown hooks from all three surfaces.
   - Initialize local file logging beneath the path API already established by
     Issue #24; preserve existing safe logging/redaction macros.
   - Retain or narrow the explicit debug-info export action so it writes only a
     local artifact after invocation. Remove automatic feedback/upload paths
     reached from startup.
   - Run logging/redaction/export tests and all startup recorder tests.

8. **Review, validate, and commit.**
   - Audit new network constructors and update the inventory dispositions that
     this issue actually changes.
   - Run formatting, focused checks, `./scripts/check.sh`, then the repository's
     full suite once.
   - Review the complete diff against both repository standards and Issue #26
     using the `code-review` skill's two parallel axes; fix findings and rerun
     affected checks.
   - Commit only Issue #26 files to the current branch with an Issue #26
     reference. Leave the unrelated untracked plan untouched.

## Validation

- Unit tests:
  - `cargo test -p warp_cli --lib`
  - Focused `cargo test --package warp --lib <module>` commands for startup,
    TUI, Agent runtime/controller, logging, and diagnostics.
- Higher-level tests:
  - `cargo test -p integration --test integration startup_request_recording`
  - The focused GUI startup integration test using the documented real-display
    environment when required.
  - `cargo test -p warp_tui --test local_only_startup` initially, renamed to the
    permanent ZYH test target during implementation.
- Type checking:
  - `cargo check -p warp_cli`
  - `cargo check -p warp_tui`
  - `cargo check -p warp`
- Final gates:
  - `./script/format --check`
  - `./scripts/check.sh`
  - `./script/presubmit` as the repository full suite once at the end.
- Manual checks:
  - Start GUI with an empty isolated ZYH home and type in a terminal.
  - Start TUI with an empty isolated ZYH home and type in shell mode.
  - Run retained local CLI help/Provider commands and confirm identity commands
    are absent.
  - Invoke Agent without Provider configuration and verify the local setup path.
  - Invoke diagnostic export explicitly and inspect the local redacted artifact.

Failures or skipped checks will be reported without being hidden. Cross-platform
startup coverage that cannot run on the current macOS host remains an explicit
CI residual risk.

## Rollback

Revert the Issue #26 commit as one unit. No schema, migration, secret, user-data,
or public-protocol mutation is planned, so rollback does not require data repair.
Do not restore individual hosted-service constructors without also restoring
their identity and lifecycle dependencies; partial rollback would recreate the
invalid mixed startup graph this change removes.
