# Agent Plan: Issue #24

## Goal

Implement the first complete ZYH configuration path described by GitHub issue
#24 and ADR-0009:

- production reads and writes one `~/.zyh/` home on macOS, Linux, and Windows;
- development uses `~/.zyh-dev/` and integration tests use an explicit temporary
  home;
- the first production launch copies supported local state from the active
  build's legacy roots without modifying, deleting, or symlinking the source;
- migration is transactional, owner-only, retryable, and produces a redacted
  report;
- existing ZYH homes always win and are never merged with legacy state.

This is a high-risk persistence and permissions change. The plan is awaiting
human review. It does not authorize production-code changes by itself.

## Scope Decisions

- Resolve the legacy source from the current build's channel and app ID. Do not
  merge Stable, Preview, OSS, Dev, or Local roots. Report other detected legacy
  roots as ignored conflicts.
- Run automatic legacy migration only for production. Development and
  integration tests must never inspect or copy a user's production or legacy
  roots.
- Keep GUI and TUI settings and SQLite isolated beneath the same ZYH home, using
  `tui/` for the TUI-owned files.
- Copy only regular files and directories declared in a versioned manifest.
  Never follow a source symlink. Record skipped symlinks and unsupported entries
  without copying their targets.
- Keep project `.warp/` to `.zyh/` migration out of this ticket. ADR-0009
  requires that migration to be explicit and user-initiated; issue #24 covers
  the application home.
- The branch has no SSH Center implementation or Remembered SSH Password key
  contract. Migrate `ssh_hosts.json` and its opaque credential references if the
  file exists, but do not invent or enumerate SSH password keys.
- Copy retained secure-storage entries (`AiApiKeys` and file-based MCP
  credentials) into the ZYH service namespace and verify the copy by reading it
  back. Leave the legacy entry unchanged. Do not copy Warp auth, hosted OAuth,
  quota, or managed-MCP entries.

## Pre-Agreed Test Seams

Implementation will use TDD only at these public module boundaries:

1. `AppHome::resolve(home, profile)` and `LegacyRoots::resolve(home, platform,
   channel/app-id)` for deterministic path behavior on every supported platform.
2. The owner-only atomic file API: expected content hash in, durable replacement
   plus one backup out, with an explicit stale-write error.
3. `translate_legacy_settings(source, rules)` for supported-key translation,
   cloud-key omission, malformed TOML, and unknown-key reporting.
4. `migrate_legacy_home(request)` for fresh migration, existing destination,
   injected partial failure, rerun, symlink rejection, secure-storage conflict,
   completion marker, and report redaction.
5. Full application startup in the integration harness, observing only files
   under the test-provided ZYH home.

Approval of this plan also confirms these seams for the `tdd` workflow.

## Files Likely Involved

- Path ownership:
  - `crates/warp_core/src/paths.rs`
  - `crates/warp_core/src/paths/zyh_home.rs` (new)
  - `crates/warp_core/src/paths_tests.rs`
  - `crates/warp_core/src/channel/state.rs`
- Durable local writes:
  - `crates/warpui_extras/src/owner_only_file.rs` (new)
  - `crates/warpui_extras/src/lib.rs`
  - `crates/warpui_extras/src/user_preferences/toml_backed.rs`
  - platform-specific tests under `crates/warpui_extras/src/`
- Migration orchestration:
  - `app/src/zyh_home_migration.rs` (new)
  - `app/src/zyh_home_migration_tests.rs` (new)
  - `app/src/zyh_home_migration/settings.rs` (new)
  - `app/src/zyh_home_migration/sqlite.rs` (new)
  - `app/src/lib.rs`
  - `app/src/preview_config_migration.rs` and its tests (remove the obsolete
    symlink migration after the ZYH migration is wired)
- Existing consumers switched through the shared path API:
  - `app/src/settings/mod.rs`
  - `app/src/keyboard.rs`
  - `app/src/user_config/mod.rs`
  - `app/src/warp_managed_paths_watcher.rs`
  - `app/src/persistence/sqlite.rs`
  - `crates/simple_logger/src/manager.rs`
  - `crates/ai/src/skills/skill_provider.rs`
  - MCP and plugin path helpers that currently call `warp_home_config_dir()`
- Test isolation:
  - `crates/warpui_core/src/integration/mod.rs`
  - focused integration tests under `crates/integration/src/test/`
- SQLite classification:
  - a new non-generated module under `crates/persistence/src/`
  - `docs/zyh-local-product-inventory.json` remains the reviewed classification
    source; generated schema and historical migrations are not edited.

The exact file list may shrink after implementation search. No unrelated path
or branding cleanup belongs in this ticket.

## Risks

- **Permissions:** Unix needs `0700` directories and `0600` files. Windows needs
  an explicit current-user-only DACL; relying on inherited AppData or home ACLs
  does not satisfy the issue. Migration must fail before publishing `~/.zyh/` if
  permissions cannot be enforced.
- **SQLite consistency:** Copying `warp.sqlite` while ignoring WAL/SHM can lose
  committed data. Use SQLite's backup API into staging, run existing migrations
  there, delete `replaced`/`deleted` tables in one destination-only transaction,
  then run integrity and foreign-key checks. Never edit the source database or
  historical migration files.
- **Settings secrecy:** Preserve the original settings file only in an
  owner-only migration backup. Reports contain key paths and status only, never
  values, hashes of values, environment values, credentials, or source file
  content.
- **Secure storage:** A destination value that already differs is a conflict,
  not permission to overwrite. A failed read-back aborts publication but may
  leave an already-copied destination secret; rerun must accept an identical
  value and remain idempotent.
- **Partial migration:** The application must not create a normal destination
  after a fatal migration failure. Populate a sibling staging directory, write
  the report and completion marker last, then atomically rename staging to the
  final absent destination.
- **Concurrent launches:** Acquire an owner-only migration lock next to the
  destination. A second process waits or returns an explicit in-progress result;
  it must not publish a competing home.
- **Existing destination:** Any existing final destination is a hard no-op,
  including an empty directory. This avoids silently merging or overwriting
  user-created ZYH state.
- **Behavioral compatibility:** Changing `data_dir`, `config_local_dir`,
  `state_dir`, `secure_state_dir`, and `cache_dir` affects settings, watchers,
  logs, SQLite, GUI/TUI restoration, and secure-storage fallback paths together.
  Consumer switching must be atomic within the commit.

## Plan

1. Add a pure `AppHome` resolver and legacy-root resolver. Map production to
   `<home>/.zyh`, development to `<home>/.zyh-dev`, and integration to a required
   harness-provided root. Derive config, data, state, logs, cache, GUI SQLite, and
   TUI paths from this value instead of platform project directories.
2. Add owner-only filesystem primitives for directory creation, regular-file
   creation, SHA-256 content snapshots, atomic same-directory replacement,
   parent-directory flush where supported, stale-write rejection, and one
   last-known-good `.bak`. Implement and test Unix modes and Windows DACLs.
3. Define migration manifest version 1 as data, not an `if/else` cascade. It
   maps the current legacy config/data/state roots to settings, keybindings,
   themes, Workflows, `.mcp.json`, tab configs, launch configs, Skills, Plugins,
   `ssh_hosts.json`, GUI/TUI SQLite, and logs. Each entry declares file/directory
   handling and whether failure is fatal.
4. Add an explicit settings rule table with `copy`, `rename`, and `omit-cloud`
   dispositions. Parse with `toml_edit`, preserve the original in an owner-only
   backup, write only supported local keys, and put unknown or omitted key paths
   in the redacted report. Malformed settings are backed up and reported; the
   active destination settings file is not created from malformed input.
5. Add destination-only SQLite migration. Take a consistent backup using the
   SQLite backup API, apply current migrations, clean tables according to the
   reviewed retained/replaced/deleted inventory, and verify `integrity_check`
   plus `foreign_key_check` before staging can complete.
6. Add secure-storage copy-and-verify behind a small trait so tests use an
   in-memory implementation. Copy only the retained explicit keys, treat missing
   keys as normal, reject differing destination values, and never place secret
   material in the report or logs.
7. Implement `migrate_legacy_home(request)` as a transaction over a sibling
   staging directory. It validates source entries with `symlink_metadata`,
   copies through owner-only writes, records sanitized per-entry outcomes,
   writes the versioned report and completion marker last, and atomically
   publishes only when every fatal step succeeds. Cleanup touches staging owned
   by this migration only; legacy roots remain byte-for-byte unchanged.
8. Call migration before secure-storage registration, settings construction,
   managed-path watcher creation, logging, or SQLite initialization can create
   the final ZYH home. Remove the Preview symlink migration and route existing
   path helpers through `AppHome` so consumers switch together.
9. Make the integration harness set an explicit per-test ZYH home on every
   platform, including Windows where changing `HOME` alone is insufficient. Add
   one startup-level test that seeds a legacy fixture and observes migrated data
   only through normal application paths.
10. Run focused tests after each vertical slice, typecheck affected crates
    regularly, then run `./scripts/check.sh`. After implementation, review the
    committed diff against issue #24 and repository standards using the
    `code-review` skill, fix material findings, and commit the final work on the
    current branch.

## Validation

- Unit tests:
  - production/development/integration path resolution for Unix and Windows path
    syntax through pure platform inputs;
  - owner-only permissions, atomic replacement, backup rotation, hash conflict,
    and injected write/rename failure;
  - settings copy/rename/omit/unknown/malformed behavior and report redaction;
  - fresh migration, absent source, existing destination, source symlink,
    partial failure, concurrent attempt, rerun, marker ordering, and unchanged
    legacy source;
  - secure-storage missing/equal/conflicting/read-back-failure cases;
  - SQLite retained data, deleted cloud data, malformed source, and integrity.
- Integration tests:
  - launch with a per-test ZYH home and representative legacy files;
  - launch with an existing destination and prove it remains unchanged;
  - GUI and TUI resolve separate settings/SQLite beneath the same test root.
- Manual checks:
  - macOS production-style first launch from a copied fixture, inspecting modes,
    report, backup, marker, and lack of symlinks;
  - Windows CI/manual ACL inspection for current-user-only access;
  - verify no legacy path timestamp, content, link count, or directory entry
    changes across migration.
- Focused commands during implementation:
  - `cargo test -p warp_core --features local_fs paths`
  - `cargo test -p warpui_extras owner_only_file`
  - `cargo test --package warp --lib zyh_home_migration -- --nocapture`
  - `cargo test --package warp --lib persistence::sqlite -- --nocapture`
  - the focused integration test command documented in `docs/testing.md`
  - `cargo check -p warp_core -p warpui_extras -p warp`
- Final command: `./scripts/check.sh`

Cross-platform ACL behavior cannot be claimed from a macOS-only run. Windows
validation remains required before the change is PR-ready.

## Rollback

- Before release, revert the implementation commit; the legacy installation is
  untouched and remains the source of truth.
- After a migrated build has run, rollback must not point an older binary at
  `~/.zyh/`. Run the older binary against its unchanged legacy roots. The ZYH
  home and copied secure-storage entries can remain; they are additive and do
  not alter the legacy files or secrets.
- Never roll back by moving files into a legacy root or creating compatibility
  symlinks.
