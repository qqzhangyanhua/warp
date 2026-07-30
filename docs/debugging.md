# Debugging

## Start Here
- Check [docs/testing.md](testing.md) for verified and candidate commands.
- Reproduce with the narrowest command before running `./scripts/check.sh`.
- Read area docs before debugging feature-specific behavior, especially `app/src/persistence/README.md`, `crates/integration/tests/INTEGRATION_TESTING.md`, and `tools/warp-bridge/README.md`.

## Local Logs
- Needs human confirmation: standard local log locations are not documented in the scanned repo docs.
- Sentry release/upload scripts exist under `script/`, but they are release/deployment tooling, not local log readers.
- ZYH does not upload feedback, crash reports, or remote diagnostics. Use a local diagnostics export when debug information must be shared.

## Common Failures

### OpenAI-compatible Provider failures

ZYH Agent Runs must use the selected OpenAI-compatible Provider through the Pi Agent Runtime. A Warp quota error on a new Conversation indicates an invalid legacy routing path; there is no hosted quota fallback in the permanent product.

Useful checks:

- In Settings, run the Provider connection test. It performs a bounded non-streaming Chat Completions request and reports authentication, missing-model, malformed-protocol, timeout, rate-limit, server, and transport failures inline and by toast.
- Confirm the configured Base URL, Model, and API Key are present and that the Provider implements the `/chat/completions` protocol.
- Bridge terminal outcomes use redacted categories: `provider_http_error`, `provider_protocol_error`, `provider_redirect_not_allowed`, and `provider_transport_error`. Raw Provider bodies and credentials must not be surfaced.
- `provider_redirect_not_allowed` means the Provider attempted to redirect outside the configured Provider Origin; this is a security boundary, not a retryable configuration fallback.

- `./script/presubmit` fails formatting: run `./script/format`, then retry.
- Inline Rust test modules fail presubmit: move tests into a separate `*_tests.rs` or `mod_test.rs` file and include it from the module under `#[cfg(test)]`.
- PowerShell lint is skipped locally when `pwsh` is absent; CI fails if `pwsh` is absent in GitHub Actions.
- Integration tests may require a real display. Use `WARPUI_USE_REAL_DISPLAY_IN_INTEGRATION_TESTS=1` when iterating on GUI integration tests.
- Diesel migrations are high risk because app startup upgrades the user's SQLite database in a transaction. Follow `app/src/persistence/README.md`.
- Permanent ZYH startup/network changes are high risk: verify they do not create or refresh Legacy Identity State, start cloud sync, send telemetry, initialize Sentry, or contact Warp background services.
- Generated schema files should not be manually edited except for the documented `crates/persistence/schema.patch` workflow.
- Bridge Protocol failures: run `(cd tools/warp-bridge && pnpm test)` and `(cd tools/warp-bridge && pnpm typecheck)`; the schema and valid/invalid fixtures are authoritative over Rust and TypeScript protocol types.

## Useful Commands
- `git status --short` - inspect workspace state.
- `rg <pattern>` - search code and docs.
- `./script/run` - run GUI app locally.
- `./script/run-tui` - run headless TUI locally.
- `./script/format --check` - check Rust formatting.
- `./script/presubmit` - full local gate.
- `cargo nextest run -p <crate>` - focused crate tests.
- `cargo test --package warp --lib -- <module_and_test> --exact --nocapture` - focused app unit test pattern from `.warp/workflows/run_unit_test.yaml`.
- `WARPUI_USE_REAL_DISPLAY_IN_INTEGRATION_TESTS=1 cargo test --package integration --test integration -- <test>` - focused integration test pattern from `.warp/workflows/run_integration_test.yaml`.

## Candidate Debug Commands
- `RUST_BACKTRACE=full ...` - documented in integration-test examples, useful when debugging Rust panics.
