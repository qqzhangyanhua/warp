# Agent Plan

## Goal

Add a build-gated `AnonymousOnlyMode` across the GUI, headless TUI, and CLI. In this mode Warp never offers Account Sign-in, automatically maintains a stable Anonymous Session, keeps local terminal and Warp Agent workflows, and routes AI through user-configured OpenAI-compatible Providers.

## Status Note

Local-only Mode is tracked separately in [ADR-0004](adr/0004-local-only-custom-provider-mode.md) and [specs/local-only-custom-provider](../specs/local-only-custom-provider/). Anonymous-only Mode keeps an Anonymous Session and may route supported service requests through Warp; Local-only Mode does not create or refresh Warp identity and routes Agent requests directly to the configured OpenAI-compatible Provider.

## Product Boundary

- Keep non-account onboarding, privacy controls, local terminal sessions, local settings, local conversation history, local workflows, third-party authentication, and interactive Warp Agent behavior.
- Require at least one valid OpenAI-compatible Provider before sending an AI request. A provider contains a name, Base URL, API Key, and one or more models.
- Support multiple providers and models. Preserve an explicit default model; otherwise select the first valid model deterministically.
- Store provider configuration in local secure storage. Never cloud-sync it or expose API Keys through logs, telemetry, UI errors, or test output.
- Route provider requests through the existing Warp Agent service using the Anonymous Session. Do not call providers directly from the client and do not fall back to Warp models or credits.
- Allow HTTP and HTTPS Base URLs. Show a non-blocking warning for HTTP because credentials and prompts may be transmitted in plaintext.
- Add an optional `Test connection` action that sends a minimal Chat Completions request through the normal Agent path, times out after 15 seconds, does not retry automatically, and cancels an older test when restarted.
- Hide account, billing, referrals, teams, sharing, cloud sync, cloud conversation storage, cloud memory, Warp-managed models, provider-specific key editors, Cloud Agent/Oz, remote tasks, handoff, and account upgrade/sign-in prompts.
- Retain CLI `login` and `logout` parsing but fail immediately with an Anonymous-only Mode error. Keep `whoami`, returning the anonymous principal and stable anonymous ID without team or email lookups.
- Reject browser auth callbacks, stale account deep links, Warp identity API Keys, and service-account authentication without mutating the Anonymous Session.
- If anonymous bootstrap is offline, enter the local workspace immediately and retry in the background. If credentials become irrecoverable, create a new Anonymous Session while preserving all local data.
- Hide cloud-only object types. Keep local Workflows, Notebooks, and Environment Variables where a real local persistence path exists.
- Preserve text, streaming, and standard tool calling. Hide provider-specific capabilities that cannot be supported by the selected custom model.

## Files Likely Involved

- Feature plumbing: `app/Cargo.toml`, `crates/warp_tui/Cargo.toml`, `crates/warp_features/src/lib.rs`, `app/src/features.rs`, `app/src/lib.rs`.
- Anonymous bootstrap and auth interception: `app/src/root_view.rs`, `app/src/auth/`, `crates/warp_server_auth/`, browser intent handling, and focused auth tests.
- TUI startup: `crates/warp_tui/src/session.rs`, `crates/warp_tui/src/root_view.rs`, TUI UI helpers, and TUI render tests.
- CLI compatibility: `app/src/ai/agent_sdk/admin.rs`, `app/src/ai/agent_sdk/mod.rs`, `crates/warp_cli/`, and their tests.
- Settings and account-only navigation: `app/src/settings_view/mod.rs`, settings page visibility helpers, account/team/billing/referral/Warp Drive pages, command search, and workspace entry points.
- AI policy and model catalog: `app/src/settings/ai.rs`, `app/src/ai/llms.rs`, `app/src/ai/agent/api.rs`, `app/src/workspaces/user_workspaces.rs`, model picker code, and request usage policy.
- Provider configuration and connectivity test: `crates/ai/src/api_keys.rs`, `app/src/settings_view/custom_inference_modal.rs`, `app/src/settings_view/ai_page.rs`, Agent API client code, and focused tests.
- Local/cloud separation: cloud preference initialization, Warp Drive visibility/actions, conversation persistence, remote agent and handoff entry points.

## Risks

- **Auth:** High risk. Anonymous credentials must remain refreshable while every account credential entry path is blocked.
- **Secrets:** High risk. Connection tests and provider errors must not leak API Keys or full sensitive response bodies.
- **AI protocol:** High risk. Anonymous server authorization and custom-provider forwarding must already be supported server-side; client changes cannot grant missing backend entitlements.
- **Compatibility:** The flag-disabled build must retain current login, cloud, billing, model, and CLI behavior.
- **Local/cloud data:** Cloud-only objects must not be mislabeled as local. No persisted cloud data is deleted or migrated.
- **TUI:** Startup currently blocks terminal creation on login, so the anonymous path must be tested independently from the GUI.
- **HTTP:** Remote HTTP is intentionally allowed and can expose credentials and prompts in transit, as recorded in ADR-0002.

## Plan

1. Add the shared `AnonymousOnlyMode` feature flag and a single policy helper used by GUI, TUI, and CLI. Keep it off in standard builds and enable it explicitly for the anonymous target build.
2. Add an anonymous-session bootstrap state machine that never blocks local workspace creation, persists the identity across restarts, retries transient failures, and replaces unrecoverable anonymous credentials without touching local data.
3. Bypass GUI and TUI account gates under the flag while preserving non-account onboarding and privacy settings. Intercept account callbacks and login-gated actions before they construct login UI.
4. Gate settings navigation, command search, palettes, banners, deep links, workspace actions, and cloud initializers so account-only and cloud-only surfaces do not exist in Anonymous-only Mode.
5. Update CLI dispatch so Warp identity login/logout/API-key paths return stable errors and `whoami` reports only the anonymous identity.
6. Reuse the existing Custom Inference data structures and secure storage. In Anonymous-only Mode, make custom endpoints always eligible, hide provider-specific keys and Warp models, and enforce deterministic custom-model selection without Warp-credit fallback.
7. Add the non-blocking HTTP warning, masked API Key behavior, missing-configuration redirect, and cancellable 15-second Test connection action using the normal Agent request path and sanitized errors.
8. Disable cloud conversation persistence, cloud preferences sync, Warp Drive sharing/team actions, remote agents, and handoff. Keep only paths proven to use local persistence.
9. Add focused unit and render tests for flag-on and flag-off behavior, then run targeted crate checks and the repository presubmit.

## Validation

- Unit tests: feature policy precedence; anonymous bootstrap success/offline/retry/replacement; AI enabled only with a valid custom model; deterministic default selection; HTTP warning; provider validation; connection-test success/auth failure/model failure/timeout/cancellation; secret redaction; CLI command behavior.
- GUI tests: no account-only settings/nav/search/palette/banner/deep-link entry; non-account onboarding reaches a terminal; missing AI config opens provider setup; custom model request has no Warp-credit fallback; local history remains enabled and cloud storage remains disabled.
- TUI tests: terminal renders before anonymous bootstrap completes; no login placeholder; AI configuration errors are actionable; flag-disabled login behavior is unchanged.
- Integration tests: persisted anonymous identity survives restart; offline startup reaches a usable terminal; provider configuration survives restart in secure storage; account auth callback cannot replace anonymous credentials.
- Manual checks: multiple custom providers/models, default model deletion fallback, HTTP warning, masked key toggle, Test connection progress/error states, third-party OAuth/SSH unaffected, local workflows available, all cloud/team/account surfaces absent.
- Commands: focused `cargo test` / `cargo nextest run` for touched crates, `./script/run-tui`, GUI manual verification, then `./scripts/check.sh` before PR-ready handoff.

## Rollback

Disable the `AnonymousOnlyMode` build feature. All existing account and cloud behavior remains in the flag-disabled branches, and this work performs no credential deletion, persisted-data migration, or generated-schema change.
