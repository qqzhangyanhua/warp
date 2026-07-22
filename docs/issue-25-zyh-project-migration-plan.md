# Agent Plan: Issue #25

## Goal

Implement the explicit, per-project migration described by GitHub issue #25
and ADR-0009:

- opening or using a repository with legacy `.warp/` project configuration
  never changes the worktree;
- a user-invoked command previews every supported source, destination, and
  conflict before confirmation;
- confirmation copies approved local project configuration into `.zyh/`
  without moving, deleting, symlinking, or overwriting legacy or destination
  files;
- cloud references and secret values are omitted through structured
  translation rather than copied as opaque bytes;
- the user sees a per-file result, including exact conflicts and failures,
  before deciding whether to commit the repository changes.

Issue #24, the declared blocker, was closed on 2026-07-22. This issue remains a
high-risk filesystem migration because it writes into repositories and handles
configuration that may contain credentials. The repository owner must confirm
this plan before product-code implementation begins.

## Scope Decisions

- Add an explicit GUI command for the repository containing the focused local
  terminal. Do not run migration during repository discovery, startup, terminal
  `cd`, workspace restoration, indexing, or watcher initialization.
- The command has two phases. `preview_project_migration` is read-only and
  renders the complete copy/conflict/skip plan. `execute_project_migration`
  requires the confirmed preview and rejects any source or destination whose
  content changed after preview.
- A non-repository directory, remote directory, missing `.warp/`, malformed
  supported file, or inaccessible path produces an explicit error. It does not
  fall back to another directory or create `.zyh/`.
- Define a versioned manifest instead of recursively copying `.warp/`. Version
  1 approves these relative paths:
  - `workflows/`
  - `launch_configurations/`
  - `tab_configs/`
  - `themes/`
  - `skills/`
  - `plugins/`
  - `.mcp.json`, only through the sanitizer below
- Unknown top-level entries are listed as unsupported and are not copied.
  Project rules outside `.warp/`, including `AGENTS.md` and `WARP.md`, are not
  moved or duplicated by this migration.
- A regular source file maps to the same relative path under `.zyh/`. Directories
  are walked without following symlinks. Symlinks, sockets, devices, and other
  non-regular entries are reported and skipped.
- Existing destination files are classified by content: identical regular
  files are `already_present`; different files and all destination symlinks are
  conflicts. Neither class is overwritten. Missing destinations are eligible
  to copy.
- Migrate `.mcp.json` with `serde_json`, using a narrow allowlisted schema.
  Preserve local server identity, transport, command, arguments, working
  directory, URL, and environment-variable references. Omit literal
  environment values, literal headers, bearer tokens, managed/cloud object
  identifiers, and unsupported fields. If a server cannot be represented
  without a secret or cloud reference, omit that server and report only its
  JSON path and reason, never its value.
- Use owner-only temporary files and same-directory atomic rename for each
  destination file. Create only missing destination directories. A failed file
  does not roll back successful independent copies; the result records the
  exact relative path and error, and rerunning is idempotent.
- Keep `WARP_CONFIG_DIR` as the legacy `.warp` name for compatibility and
  migration detection. Add a separate ZYH project-directory constant and
  switch retained ZYH consumers to `.zyh/`; do not silently dual-read `.warp/`
  after the migration feature is available.
- The result is displayed in the modal and is not persisted as another project
  file. The actual `.zyh/` diff remains the durable, user-inspectable result.
- No telemetry, network request, cloud API, secure-storage write, database
  migration, or public protocol change belongs in this issue.

## Pre-Agreed Test Seams

Implementation will use TDD only at these public module boundaries:

1. `preview_project_migration(repo_root)` observes a repository fixture and
   returns the complete manifest-derived list of copy candidates, identical
   destinations, conflicts, unsupported entries, symlinks, and sanitized MCP
   omissions without changing any filesystem metadata or content.
2. `execute_project_migration(confirmed_preview)` performs only the operations
   represented by that preview and returns per-file copied, skipped, conflict,
   and failed outcomes while preserving every legacy source.
3. The project path API resolves `.zyh/` for retained ZYH project consumers and
   `.warp/` only for legacy migration detection.
4. The command/modal boundary receives a preview, cannot execute before the
   confirm action, and renders the final result with exact relative paths.
5. A focused GUI integration test invokes the command from a local repository
   terminal and observes that decline leaves the worktree unchanged while
   confirmation creates only approved `.zyh/` files.

Approval of this plan also confirms these seams for the `tdd` workflow.

## Files Likely Involved

- Migration model and filesystem behavior:
  - `app/src/zyh_project_migration.rs` (new)
  - `app/src/zyh_project_migration/manifest.rs` (new)
  - `app/src/zyh_project_migration/mcp.rs` (new)
  - `app/src/zyh_project_migration_tests.rs` (new)
  - `app/src/zyh_project_migration_tests/mcp_tests.rs` (new if needed to keep
    files below 500 lines)
  - `app/src/lib.rs`
- Explicit GUI command and confirmation/result modal:
  - `app/src/zyh_project_migration/modal.rs` (new)
  - `app/src/terminal/view/action.rs`
  - `app/src/terminal/view/init.rs`
  - `app/src/terminal/view.rs`
  - `app/src/i18n/mod.rs`
  - `app/src/i18n/table.rs`
  - `app/src/i18n/binding_descriptions.rs`
- Project-directory ownership and retained consumers:
  - `crates/warp_core/src/paths.rs`
  - `crates/warp_core/src/paths_tests.rs`
  - `app/src/workflows/local_workflows.rs`
  - `app/src/workflows/local_workflows_tests.rs`
  - `crates/ai/src/skills/skill_provider.rs`
  - `crates/ai/src/skills/skill_provider_tests.rs`
  - `app/src/ai/mcp/mod.rs`
  - `app/src/ai/mcp/mod_tests.rs`
  - `app/src/ai/mcp/file_mcp_watcher.rs`
  - focused MCP watcher tests already colocated with that module
- GUI integration coverage:
  - `app/src/integration_testing/` only if a narrow test hook is required
  - `crates/integration/src/test/zyh_project_migration.rs` (new)
  - `crates/integration/tests/integration/ui_tests.rs`

The exact list may shrink during implementation search. Generated files,
historical migrations, user secrets, and unrelated `.warp` documentation are
out of scope.

## Risks

- **Repository mutation:** Merely opening, restoring, indexing, or watching a
  repository must remain read-only. Only the confirm action may call the
  execution function.
- **Time-of-check/time-of-use:** A source or destination can change while the
  preview modal is open. The preview records file type, size, and content hash;
  execution verifies them and reports a stale-preview failure instead of
  copying or overwriting.
- **Destination conflicts:** The migration never replaces, truncates, merges,
  or deletes an existing destination. Identical content is an idempotent no-op;
  differing content remains a visible conflict on every run.
- **Secret disclosure:** Source values, hashes of secret values, command
  environments, headers, and parsed MCP contents must not enter logs, modal
  errors, test snapshots, or migration results. Reports contain relative paths,
  server names, JSON field paths, and status only.
- **Cloud references:** Opaque cloud/managed identifiers must not be copied just
  because they appear in otherwise valid JSON. The MCP translator is
  allowlist-based; unknown fields are omitted and reported.
- **Filesystem traversal:** A source symlink or a path component replaced by a
  symlink must not escape `.warp/`. Validate with `symlink_metadata`, keep all
  relative paths manifest-derived, and reject destination symlinks.
- **Partial failure:** Per-file atomicity means a migration can be partially
  successful. The result must distinguish copied, already present, conflict,
  skipped, stale, and failed paths so rerun behavior is understandable.
- **Consumer compatibility:** Switching Workflows, Skills, and MCP from `.warp`
  to `.zyh` intentionally stops implicit legacy reads. Keep the old constant
  available for migration code and audit all current project-path consumers in
  the same implementation commit.
- **UI behavior:** Reuse existing `ActionButton` themes and modal/dialog
  components. Keep confirm disabled when preview has a fatal error, preserve
  cancel as the default safe action, and do not close the result view before the
  user can inspect failures.
- **Cross-platform behavior:** File identity, permissions, and rename semantics
  differ on Windows. Do not claim Windows coverage from a macOS-only run.

## Plan

1. Add separate legacy and current project-directory constants to
   `warp_core::paths`, with tests proving `.warp` is used only to locate legacy
   project configuration and `.zyh` is the retained ZYH project root.
2. Add a data-only migration manifest for the approved paths. Define explicit
   preview and result types whose statuses carry repository-relative paths and
   sanitized reasons, never source content.
3. Implement the read-only preview walker. Resolve the git worktree root,
   validate every manifest-derived path without following symlinks, hash
   regular files, classify destinations, and collect unsupported entries.
4. Implement the MCP translator with structured JSON parsing and an allowlisted
   output schema. Add one red test at a time for environment references,
   literal secrets, headers/tokens, cloud identifiers, mixed valid/invalid
   servers, malformed JSON, and output redaction.
5. Implement confirmed execution using the preview snapshots. Revalidate source
   and destination state, create missing `.zyh` directories, copy eligible
   files through owner-only temporary files and atomic rename, continue after
   independent failures, and return the complete per-file result.
6. Add the command-palette action for the focused local repository. The action
   performs preview only and opens a modal that lists source-to-destination
   mappings, conflicts, omissions, and unsupported entries before exposing the
   confirm command.
7. On confirmation, run execution off the UI thread, keep the modal in an
   in-progress state, then render the final copied/conflict/failed result. Reuse
   existing button themes and dialog primitives; add localized visible text and
   no feature-specific button theme.
8. Switch retained project consumers together: Workflows load from
   `.zyh/workflows`, the ZYH Skill provider uses `.zyh/skills`, and ZYH MCP
   discovery/classification watches `.zyh/.mcp.json`. Keep non-ZYH providers
   and global `~/.zyh` paths unchanged.
9. Add the focused GUI integration test for decline and confirmation, using a
   hermetic repository fixture. Observe behavior only through the command and
   resulting files; do not call migration internals from the integration test.
10. Run focused tests and affected-crate typechecks after each vertical slice,
    then run the full repository check. Review the committed diff against issue
    #25 and repository standards with the `code-review` skill, fix material
    findings, and commit the final implementation on the current branch.

## Validation

- Unit tests:
  - preview is metadata- and content-preserving for the entire repository;
  - decline performs no writes;
  - successful migration copies every approved path and leaves every legacy
    source unchanged;
  - identical destinations make repeated migration idempotent;
  - differing destinations and destination symlinks remain untouched and are
    reported as exact conflicts;
  - source symlinks, unknown entries, malformed MCP, literal MCP secrets, and
    cloud references are skipped without leaking values;
  - source or destination changes after preview fail as stale paths;
  - injected read, directory-create, temporary-write, sync, and rename failures
    identify the exact relative path while independent files still complete.
- Integration tests:
  - invoking the command in a fixture repository, inspecting preview, and
    declining leaves `git status --short` empty;
  - confirming creates only approved `.zyh/` files and displays the final
    result without changing `.warp/`;
  - opening/restoring the same fixture without invoking the command leaves the
    worktree unchanged.
- Manual checks:
  - inspect preview and result layout at narrow and standard window sizes;
  - verify cancel, confirm, in-progress, conflict, partial-failure, and repeated
    states remain legible and keyboard accessible;
  - inspect a migrated repository diff and verify no secret values or cloud
    identifiers appear under `.zyh/` or in application logs.
- Focused commands during implementation:
  - `cargo test --package warp --lib zyh_project_migration -- --nocapture`
  - `cargo test -p warp_core --features local_fs paths`
  - `cargo test -p ai skill_provider`
  - `cargo test --package warp --lib workflows::local_workflows -- --nocapture`
  - `cargo test --package warp --lib ai::mcp -- --nocapture`
  - the focused GUI integration command documented in `docs/testing.md`
  - `cargo check -p warp_core -p ai -p warp`
- Final command: `./scripts/check.sh`

No test or platform result will be claimed unless it is run in this workspace.
Windows filesystem behavior remains a PR-readiness risk until exercised on
Windows CI or a Windows development host.

## Rollback

- Before release, revert the implementation commit. The migration never alters
  `.warp/`, so the legacy source remains available.
- After a user has confirmed migration, rollback must not copy `.zyh/` content
  back into `.warp/`, delete `.zyh/`, or create a compatibility symlink. An
  older build may continue reading the untouched legacy project files.
- If only some files were copied before a failure, leave both roots intact. The
  user may inspect or remove the new `.zyh/` files explicitly; the application
  must not perform automatic cleanup inside a repository.
