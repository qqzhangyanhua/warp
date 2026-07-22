# Product Spec: Migrate Application State to the ZYH Home

**Issue:** [qqzhangyanhua/warp#24](https://github.com/qqzhangyanhua/warp/issues/24)

## Summary

ZYH stores production application state under `~/.zyh/`, development state
under `~/.zyh-dev/`, and integration-test state under an explicit temporary
home. On the first production launch, ZYH copies supported local state from the
active legacy installation without changing the legacy files or creating
compatibility links.

## Goals

- Give ZYH one predictable, private application home on every supported OS.
- Preserve supported local configuration and durable local state on first run.
- Make migration transactional, retryable, idempotent, and auditable without
  exposing setting values or secrets.
- Keep development and integration runs isolated from production and legacy
  user state.

## Non-goals

- Merging multiple Stable, Preview, OSS, Dev, Local, or Integration roots.
- Migrating project-owned `.warp/` directories; project migration is explicit
  and user-initiated.
- Moving or deleting legacy files, or creating compatibility symlinks.
- Inventing Remembered SSH Password keys before SSH Center defines a stable
  secure-storage key contract.
- Preserving account, team, cloud sync, hosted AI, quota, or telemetry state.

## Behavior Invariants

1. Production resolves its application home to `~/.zyh/` on macOS, Linux, and
   Windows. Development resolves to `~/.zyh-dev/`. Integration requires an
   explicit `ZYH_HOME` supplied by the test harness.
2. Automatic migration runs only for production and only when the destination
   is absent. Any existing destination, including an empty directory, wins and
   remains unchanged.
3. Migration considers only the active build's legacy installation. Other
   channel roots are not merged.
4. Supported settings, keybindings, themes, Workflows, MCP configuration, tab
   and launch configurations, Skills, Plugins, `ssh_hosts.json`, GUI/TUI
   SQLite, and application logs are copied when present.
5. Legacy settings are backed up, supported local keys are translated,
   cloud-only keys are omitted, malformed input is preserved but not activated,
   and unknown key paths are reported without their values.
6. The copied SQLite database is produced with SQLite's backup API, upgraded,
   stripped of classified cloud rows, and checked for integrity and foreign-key
   violations before publication.
7. Only retained, explicitly named secure-storage entries are copied and read
   back for verification. A differing destination value aborts migration.
8. Migration writes to an owner-only sibling staging directory and publishes
   it atomically only after all fatal work, the redacted report, and the
   completion marker succeed.
9. Normal private file writes use owner-only permissions, atomic replacement,
   and one last-known-good backup.
10. Source files, directories, timestamps, and secure-storage entries remain
    unchanged. Source symlinks are reported and never followed.
11. Concurrent launches cannot publish competing homes. A failed attempt leaves
    no final destination and a later run can retry.
12. On macOS, GUI SQLite is read from the legacy App Group secure-state path
    and only the active channel's log family is copied from `~/Library/Logs`.

## User-visible Outcomes

- A successful first launch creates the ZYH home, a redacted
  `migration-report.json`, and `migration-complete.json`.
- A malformed settings file does not block migration of unrelated supported
  data; its original bytes remain available in the private migration backup.
- A fatal filesystem, SQLite, or secure-storage error prevents publication and
  is reported as a startup error.
- A later launch sees the existing ZYH home and does not inspect or merge legacy
  state again.

## Validation

- Pure path tests cover production, development, integration, and legacy roots
  for macOS, Linux, and Windows syntax.
- Focused migration tests cover fresh state, existing destination, malformed
  settings, partial failure, rerun, concurrency, symlinks, SQLite cleanup,
  secure-storage conflicts, macOS secure state, and filtered logs.
- GUI integration tests confirm startup reads settings from the harness-provided
  ZYH home and normal settings writes retain a private backup.
- Windows ACL behavior requires Windows CI or manual verification and is not
  claimed from macOS validation.
