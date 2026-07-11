# Debugging

## Start Here
- Check [docs/testing.md](testing.md) for verified and candidate commands.
- Reproduce with the narrowest command before running `./scripts/check.sh`.
- Read area docs before debugging feature-specific behavior, especially `app/src/env_vars/README.md`, `app/src/persistence/README.md`, and `crates/integration/tests/INTEGRATION_TESTING.md`.

## Local Logs
- Needs human confirmation: standard local log locations are not documented in the scanned repo docs.
- Sentry release/upload scripts exist under `script/`, but they are release/deployment tooling, not local log readers.
- For bug reports, `CONTRIBUTING.md` says Warp's `/feedback` command attaches relevant logs and environment details automatically.

## Common Failures
- `./script/presubmit` fails formatting: run `./script/format`, then retry.
- Inline Rust test modules fail presubmit: move tests into a separate `*_tests.rs` or `mod_test.rs` file and include it from the module under `#[cfg(test)]`.
- PowerShell lint is skipped locally when `pwsh` is absent; CI fails if `pwsh` is absent in GitHub Actions.
- Integration tests may require a real display. Use `WARPUI_USE_REAL_DISPLAY_IN_INTEGRATION_TESTS=1` when iterating on GUI integration tests.
- Diesel migrations are high risk because app startup upgrades the user's SQLite database in a transaction. Follow `app/src/persistence/README.md`.
- Generated schema files should not be manually edited except for the documented `crates/persistence/schema.patch` workflow.

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
- `WITH_LOCAL_SERVER=1 ./script/run` - run GUI app against local warp-server on default port 8080.
- `WITH_LOCAL_SERVER=1 SERVER_ROOT_URL=http://localhost:8082 WS_SERVER_URL=ws://localhost:8082/graphql/v2 ./script/run` - run GUI app against a local warp-server on a custom port.
- `RUST_BACKTRACE=full ...` - documented in integration-test examples, useful when debugging Rust panics.
