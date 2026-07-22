# Tech Spec: Migrate Application State to the ZYH Home

**Issue:** [qqzhangyanhua/warp#24](https://github.com/qqzhangyanhua/warp/issues/24)

## Context

Application paths previously came from platform project directories and
home-relative `.warp*` roots. SQLite could use a separate macOS App Group
container, while macOS logs were written directly under `~/Library/Logs`.
This made a simple directory copy incomplete and unsafe.

Relevant implementation areas:

- `crates/warp_core/src/paths.rs` and `paths/zyh_home.rs`: current ZYH paths.
- `crates/warp_core/src/paths/legacy_roots.rs`: deterministic legacy roots.
- `crates/warpui_extras/src/owner_only_file.rs`: private atomic file writes.
- `app/src/zyh_home_migration.rs`: staging, locking, reporting, and publication.
- `app/src/zyh_home_migration/manifest.rs`: versioned migration inventory.
- `app/src/zyh_home_migration/sqlite.rs`: consistent database copy and cleanup.
- `app/src/zyh_home_migration/platform_secure_storage.rs`: retained secret copy.
- `crates/warpui_core/src/integration/mod.rs`: per-test environment isolation.

## Proposed Changes

### Path ownership

`AppHome` resolves one root by profile:

- production: `<home>/.zyh`;
- development: `<home>/.zyh-dev`;
- integration: required `ZYH_HOME`.

Config, data, state, and GUI SQLite live directly under this root. Cache, logs,
and TUI-owned settings/SQLite use stable subdirectories.

`LegacyRoots` remains separate and models the active legacy installation. Its
macOS paths include both normal Application Support and the App Group secure
state location. Its log root follows each platform's historical logger layout.

### Transactional migration

`migrate_legacy_home` performs this sequence:

1. Return immediately if the final destination exists.
2. Acquire an owner-only advisory lock beside the destination.
3. Create an owner-only sibling staging directory.
4. Process the versioned manifest without following source symlinks.
5. Translate settings and retain the original bytes in a private backup.
6. Back up, clean, migrate, and verify SQLite in staging.
7. Copy and read back retained secure-storage values.
8. Write the redacted report and completion marker last.
9. Atomically rename staging to the final ZYH home and sync its parent.

Temporary cleanup is scoped to the staging directory owned by the current
attempt. The legacy installation is read-only throughout.

### Manifest and logs

The manifest maps each supported file or directory from an explicit legacy
root to a destination-relative path. GUI SQLite uses `SecureState`. Logs use a
special entry kind: it accepts only the active channel's main, startup-rotated,
in-session-rotated, recovery, and temporary log names. It never recursively
copies the shared macOS `~/Library/Logs` directory.

### Settings and private files

Settings translation is table-driven with copy, rename, and omit-cloud rules.
Reports contain only key paths and enum statuses. Both migration output and
normal `settings.toml` persistence use the owner-only atomic primitive, which
creates `0700` directories and `0600` files on Unix, applies a protected
owner-only DACL on Windows, syncs temporary content before replacement, and
retains one `.bak` file.

### SQLite

The source database is opened read-only and copied using `sqlite3_backup` so
WAL-backed committed data is included. Existing migrations run only on the
destination. Classified account, cloud, team, sync, quota, and server-owned
rows are removed in a destination-only transaction. `integrity_check` and
`foreign_key_check` must pass before publication.

### Secure storage

The migration copies only `AiApiKeys` and `FileBasedMcpCredentials`. Missing
values are normal, identical destination values are idempotent, and differing
values are conflicts. SSH Center metadata is copied through `ssh_hosts.json`,
but no Remembered SSH Password key is enumerated until a stable contract exists.

On Windows, migration writes encrypted staging files through the same
owner-only atomic primitive. macOS uses Keychain and Linux uses Secret Service
with the existing legacy fallback only for reading.

### Startup and obsolete compatibility behavior

Production migration runs before normal initialization can create the final
home. The old Preview `.warp` to channel-directory symlink migration and its
test-only helper are removed. Integration setup always sets `ZYH_HOME` to a
child of the per-test root on every platform; Unix continues to isolate `HOME`
for shell behavior.

## Error Handling

- Unsupported source types and symlinks are skipped and redacted in the report.
- Filesystem, SQLite, or secure-storage failures abort publication.
- Concurrent lock contention returns an explicit in-progress outcome.
- Malformed settings are backed up and reported without creating active
  translated settings.
- Reports never include file contents, setting values, environment values, or
  secret material.

## Testing and Validation

- `warp_core` path tests cover profile and legacy root resolution.
- `warpui_extras` tests cover owner-only atomic replacement, backup rotation,
  stale-write rejection, and symlink refusal.
- App migration tests exercise manifest copy, settings translation, SQLite,
  secure storage, locking, retry, redaction, and platform-specific macOS roots.
- GUI integration tests exercise startup with the harness-provided ZYH home and
  settings persistence through normal application actions.
- Final validation runs focused tests, integration nextest coverage,
  `cargo check` for affected crates, formatting, and `./scripts/check.sh`.
- Windows ACL enforcement remains a required Windows CI/manual check because a
  macOS run cannot validate the resulting DACL.

## Rollback

Before release, revert the implementation. After a migrated build has run, an
older build must continue using the unchanged legacy roots; it must not be
pointed at `.zyh`. Rollback never moves files back or creates compatibility
symlinks.
