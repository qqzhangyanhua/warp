# Testing

## Verified Commands
These commands are present in repo docs or existing wrapper scripts. Do not claim they passed unless run in this workspace.

- `./script/bootstrap` - platform setup and common skill restoration.
- `./script/run` - build and run the GUI desktop app.
- `./script/run-tui` - build and run the headless TUI front-end.
- `./script/format` - format Rust code.
- `./script/format --check` - check Rust formatting.
- `./script/presubmit` - full local PR gate.
- `cargo nextest run --no-fail-fast --workspace --exclude command-signatures-v2` - workspace tests used by presubmit.
- `cargo nextest run -p warp_completer --features v2` - completer v2 tests used by presubmit.
- `cargo test --doc` - Rust doc tests used by presubmit.
- `cargo test` - standard Rust tests for targeted packages.

## Candidate Commands
Detected but not run during harness generation:

- `cargo run` - build and run the GUI desktop app.
- `cargo bundle --bin warp` - bundle the main GUI app.
- `cargo clippy --workspace --all-targets --all-features --tests -- -D warnings` - documented broad clippy command.
- `./script/run-clang-format.py -r --extensions 'c,h,cpp,m' ./crates/warpui/src/ ./app/src/` - C/C++/Obj-C formatting.
- `find . -name "*.wgsl" -exec wgslfmt --check {} +` - WGSL shader formatting.
- `cargo test --package warp --lib -- <module_and_test> --exact --nocapture` - unit-test workflow template in `.warp/workflows/run_unit_test.yaml`.
- `cargo test --package warp --lib local_mode -- --nocapture` - focused Local-only Mode policy and identity tests.
- `cargo test --package warp --lib tui::tests -- --nocapture` - focused TUI startup mode tests, including Local-only/Anonymous-only branching when present.
- `WARPUI_USE_REAL_DISPLAY_IN_INTEGRATION_TESTS=1 cargo test --package integration --test integration -- <test>` - integration-test workflow template in `.warp/workflows/run_integration_test.yaml`.
- `crates/warp_graphql_schema/package.json`: `yarn generate` or equivalent package-manager invocation for the detected `generate` script. Needs human confirmation before using as a standard command.
- `crates/command-signatures-v2/js/package.json`: package scripts `build`, `clean`, and `watch`. Needs human confirmation before using as standard checks.

## Test Locations
- Rust unit tests commonly use separate files named `*_tests.rs` or `mod_test.rs`.
- GUI integration tests: `crates/integration/src/test/`, `crates/integration/tests/`.
- TUI tests: `crates/warp_tui/tests/` and render-to-lines unit tests near TUI modules.
- Settings value tests: `crates/settings_value/tests/`.
- WarpUI core tests and data: `crates/warpui_core/tests/`, `crates/warpui_core/test_data/`.
- App-level SSH tests: `app/tests/ssh/`.
- Fixtures/data: `crates/editor/test_fixtures/`, `crates/editor/test_data/`, `crates/warp_files/test_data/`.

## Standard Check
Run:

```sh
./scripts/check.sh
```

`./scripts/check.sh` calls the existing `./script/presubmit` wrapper.

## Notes
- Presubmit can be expensive. For narrow work, run focused tests first, then `./scripts/check.sh` before PR-ready handoff.
- GUI integration tests are GUI-only and may require a real display.
- TUI changes should prefer TUI render-to-lines tests and, when needed, terminal verification.
- Local-only Mode changes should include focused tests for both flag-enabled and flag-disabled behavior, plus a manual run that confirms startup does not contact Warp identity/cloud/telemetry endpoints.
- Do not add `pnpm`, `npm`, `pytest`, or `make` checks unless a repo wrapper or task-specific verification establishes them.
