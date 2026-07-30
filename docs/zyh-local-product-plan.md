# Agent Plan

## Goal

Convert this fork into a permanent ZYH local product with no Warp account,
cloud, sync, telemetry, hosted AI, or background Warp network behavior.

The retained product consists of the desktop GUI, TUI, `zyh agent`, local
terminal and file features, user-initiated SSH/Git access, a Pi-backed local
Agent Runtime, explicitly configured OpenAI-compatible Providers, and explicitly
configured MCP servers.

This is a high-risk system change. Implementation must be split into reviewable,
buildable phases. This plan is awaiting human review; it does not authorize code
changes by itself.

## Fixed Decisions

- ZYH is permanently local. Remove the Local-only feature flag and all normal,
  anonymous, and local product-mode branches.
- Rename user-visible and external product contracts to ZYH. Do not bulk-rename
  unrelated internal Rust crates, modules, or types solely for branding.
- Production configuration lives under `~/.zyh/` on Unix and the equivalent
  home directory on Windows. Development uses `~/.zyh-dev/`; integration tests
  use isolated temporary homes.
- Keep domain-specific files instead of putting all state in `settings.toml`:
  `settings.toml`, `.mcp.json`, `workflows/*.yaml`, `themes/`,
  `keybindings.yaml`, `tab_configs/`, `launch_configurations/`, `skills/`,
  `plugins/`, `ssh_hosts.json`, local logs, and local SQLite all retain separate
  ownership.
- Global Agent rules use `~/.agents/AGENTS.md`. Project rules continue to use
  `AGENTS.md` or `WARP.md` as recognized cross-agent inputs.
- Project-owned ZYH configuration uses `<repo>/.zyh/`, not `<repo>/.warp/`.
- Old cloud Rules are discarded. They are not imported into `AGENTS.md`.
- Preserve terminal history, window and tab restoration, local Conversations,
  project metadata, and local run records. Remove cloud-only persisted data.
- Pi is the only Agent Runtime. Old Rust-bound Conversations remain viewable
  and can be explicitly forked into a new Pi Conversation.
- Support multiple named OpenAI-compatible Providers and Models with one
  explicit default. Do not probe, route, or fall back automatically.
- The Agent Tool Catalog contains only `run_shell_command`, `read_files`,
  `apply_file_diffs`, and locally configured MCP tools.
- Keep third-party credentials only when access is direct and user-initiated.
  Secrets remain in operating-system secure storage, never in project files.
- Remove Warp Drive rather than turning it into a local drive.
- Remove Environment Variable Collections. Users use shell configuration,
  `.env`, system environment variables, or a user-selected secret manager.
- Keep Workflows as local YAML and Notebooks as local Markdown files.
- Keep MCP as local file configuration. Remove Gallery, managed MCP, cloud
  installation objects, proxy tokens, and server-side resolution.
- Remove server-backed semantic indexing without adding a local vector index.
  Keep local file search, `rg`, file outlines, repository metadata, and remote
  SSH scanning.
- Remove automatic updates, Voice/transcription, WASM/Web, cloud Agents,
  sharing, teams, billing, telemetry, Sentry, feedback upload, and background
  skill/plugin downloads.
- Keep macOS, Linux, and Windows desktop builds. Bundle Linux/macOS arm64 and
  x86_64 remote daemon artifacts and upload them through SSH/SCP; never download
  the daemon at runtime.
- Rename local automation to `zyhctrl` and `zyh://`. Do not keep `warp://`,
  `warplocal://`, or old CLI aliases.
- Physically remove cloud-only code and workspace crates after all retained
  consumers have been migrated.

## Files Likely Involved

This list is intentionally grouped by ownership boundary and is not exhaustive.

- Product policy and startup: `app/src/lib.rs`, `app/src/root_view.rs`,
  `app/src/tui/`, `app/src/local_mode.rs`, `app/src/features.rs`,
  `crates/warp_features/`, menus, settings views, and search entry points.
- Paths, configuration, and branding: `crates/warp_core/src/paths.rs`, channel
  configuration, `app/src/user_config/`, `app/src/settings/`, config migration,
  managed-path watchers, local control, CLI, and platform bundle metadata.
- Local data features: Agent facts and project context, `app/src/workflows/`,
  MCP, Notebooks, EVC removal, SSH Center, and app/crate persistence modules.
- Agent Runtime and Provider: runtime, API, legacy SDK removal, typed tool
  execution, Conversation models, `crates/ai/`, `tools/warp-bridge/`, and Pi.
- SSH remote daemon: app and crate remote-server modules, terminal SSH and
  warpify code, deploy scripts, release bundles, and artifact manifests.
- Cloud deletion: app auth, billing, cloud object, Drive, cloud server, Agent,
  sharing, handoff, managed-secret, telemetry, GraphQL, cloud object, Warp
  server auth/client, and multi-agent client crates.
- Generated clients, hosted-service Git dependencies, and cloud-only managed
  secrets, Voice, and Computer Use crates are removed only after no retained
  consumer remains.

## Risks

- **Auth and startup:** Removing `AuthState`, `AuthManager`, `ServerApiProvider`,
  server experiments, and cloud singleton registration changes the application
  dependency graph. A hidden constructor dependency can prevent GUI, TUI, or
  CLI startup.
- **Provider credentials:** Provider and MCP secrets must not enter TOML, JSON,
  logs, protocol diagnostics, migration reports, or subprocess environments
  beyond the explicitly targeted process.
- **Persistence migration:** The existing SQLite database mixes local and cloud
  data. Incorrect filtering can delete local history or retain account, team,
  permissions, cloud object, or server task data.
- **Conversation compatibility:** Removing the Rust runtime must not reinterpret
  legacy tool activity or silently continue a Conversation under different
  execution semantics.
- **File conflicts:** Rules, Workflows, MCP, Notebooks, settings, and SSH hosts
  can be modified outside the application. Blind saves can destroy user edits.
- **Public contracts:** Renaming the executable, environment variables, URI
  scheme, local control protocol, paths, and remote daemon protocol is
  intentionally breaking and must be changed atomically across producers,
  consumers, packaging, tests, and docs.
- **SSH distribution:** Bundling four remote daemon targets increases bundle
  size and makes release correctness depend on artifact manifests, checksums,
  signing, SCP installation, and cross-version protocol checks.
- **Network boundary:** Removing visible cloud UI is insufficient. Startup,
  error reporting, update checks, skill restoration, remote daemon installation,
  model discovery, and stale restored panes can still initiate requests.
- **Workspace pruning:** Local types currently live in cloud crates. Deleting a
  crate before extracting the retained model can cause broad churn or duplicate
  serialization contracts.
- **Generated code:** GraphQL and generated client directories must not be
  hand-edited. Remove their consumers and generation entry points, then remove
  the owning crate or generated tree as a unit.
- **Release and CI:** WASM removal, bridge artifacts, daemon artifacts, package
  names, signing, and platform manifests affect release infrastructure.
- **Upstream compatibility:** This deliberately abandons upstream cloud product
  compatibility. Keep each phase narrow and buildable so future upstream merges
  fail in localized ownership boundaries rather than across a single giant diff.

## Plan

### 0. Record The Permanent Product Decision

1. Add an ADR that supersedes ADR-0004's build-gated mode and the rollout/fallback
   parts of ADR-0005. Preserve ADR-0005's Pi safety, transcript, commit barrier,
   tool authority, and Provider Origin decisions.
2. Update `CONTEXT.md` to make ZYH local-product vocabulary authoritative and
   remove Account Sign-in, Anonymous Session, Warp cloud, and dual-runtime terms
   from active product language.
3. Add a machine-readable inventory of app-initiated network clients, endpoint
   constants, cloud crates, cloud feature flags, user-visible cloud entry points,
   and persisted cloud tables. Treat this as the deletion checklist.
4. Add a failing baseline integration test that records all app-initiated
   outbound requests during GUI, TUI, and CLI startup.

**Gate:** Human review of the ADR, deletion inventory, migration classification,
and baseline network trace before changing startup or persistence.

### 1. Establish ZYH Paths And Non-Destructive Migration

1. Introduce one path API for production `~/.zyh/`, development
   `~/.zyh-dev/`, and hermetic integration-test homes. On Windows, resolve the
   same `.zyh` directory beneath the user's home.
2. Create a versioned migration manifest covering settings, keybindings, themes,
   Workflows, MCP, tab and launch configs, skills, plugins, SSH Center data,
   local SQLite, and local logs.
3. If `~/.zyh/` is absent, copy supported local data from the legacy root. Do
   not move or symlink it. Back up `settings.toml`, translate supported keys,
   omit cloud keys, and write an explicit report for unknown keys.
4. Detect `<repo>/.warp/` but never mutate a repository automatically. Offer an
   explicit migration that copies supported files into `<repo>/.zyh/` and
   reports conflicts.
5. Introduce shared atomic-write and conflict-detection helpers: capture a
   content hash, reject stale saves, write an owner-only temporary file, flush,
   atomically rename, and retain one last-known-good backup.
6. Keep credentials in platform secure storage. Rename secure-storage keys only
   through an explicit copy-and-verify migration; never write secret values to
   the migration report.

**Gate:** Path, permissions, conflict, backup, malformed-file, and idempotent
migration tests on macOS, Linux, and Windows before any consumer switches roots.

### 2. Make Startup Permanently Local

1. Replace the Local-only feature check with an unconditional local product
   policy, then remove `LocalOnlyCustomProviderMode`, `AnonymousOnlyMode`,
   remote experiments, and their override branches.
2. Remove Account, Anonymous Session, Local Identity, login/logout/whoami,
   `AuthState`, `AuthManager`, and identity refresh initialization.
3. Remove construction of Warp server, GraphQL, WebSocket, cloud object, sync,
   quota, telemetry, Sentry, update, referral, survey, and remote changelog
   services from GUI, TUI, CLI, and remote daemon startup.
4. Start directly in a usable terminal. If Agent is invoked without a valid
   Provider selection, open local Provider settings and report the exact missing
   field.
5. Remove login/account/team/billing/cloud settings pages, commands, menu items,
   deep links, restored-pane handlers, and onboarding branches.
6. Keep a local, redacted log pipeline under `~/.zyh/logs/`. Diagnostics export
   writes a local file only after an explicit user action.

**Gate:** GUI, TUI, and CLI reach usable local states without any Auth, Server
API, CloudModel, UpdateManager, SyncQueue, telemetry, Sentry, or update singleton.

### 3. Replace Cloud-Backed User Data With Local Sources

1. **Rules:** Replace `CloudAIFact` rows and `UpdateManager` writes with direct
   editing of `~/.agents/AGENTS.md`. Remove name, revision, owner, sync status,
   trash, and offline concepts. Project rows remain file-backed. Do not read or
   import cached cloud Rules.
2. **Workflows:** Extract the retained Workflow schema from cloud ownership,
   make create/edit/delete operate on `~/.zyh/workflows/*.yaml` or explicit
   project `.zyh/workflows/*.yaml`, reject collisions, and refresh from file
   watcher events. Remove Personal/Team collections, folders, sharing, and trash.
3. **MCP:** Make `~/.zyh/.mcp.json`, project files, and detected third-party
   local configs the only sources. Store ZYH-managed secret values in secure
   storage behind placeholders. Remove Gallery, managed resolution, server
   objects, proxy tokens, and cloud installation state.
4. **Notebooks:** Replace cloud Notebook IDs and persistence with local Markdown
   paths. New Notebooks choose a path on first save; subsequent saves are
   conflict-checked and atomic. Remove owner, sharing, cloud history, and cloud
   cache behavior.
5. **Environment variables:** Remove EVC models, panes, menus, invocation,
   persistence, tests, and Drive integration. Point remaining guidance to shell,
   `.env`, system environment, and user-selected secret manager workflows.
6. **Preferences:** Remove cloud preference objects and `CloudPreferencesSyncer`.
   Keep existing domain files, secure storage, local preferences, and SQLite
   responsibilities instead of forcing all data into `settings.toml`.
7. **SSH Center:** Move the versioned `ssh_hosts.json`, transaction journal, and
   backup beneath `~/.zyh/`; preserve system-storage-only password handling and
   fail-closed recovery from ADR-0006 through ADR-0008.

**Gate:** Each retained editor passes create/read/update/delete, external-edit
conflict, restart, malformed-file, permission, and backup-recovery tests without
cloud singletons registered.

### 4. Make Pi The Only Agent Runtime

1. Make Runtime Selection Policy unconditional for new Conversations and remove
   the Rust direct-provider fallback, rollout flag, runtime selector, and mixed
   Account/Anonymous/Cloud branches.
2. Keep the Conversation Record, Runtime Transcript, Runtime Binding, Resource
   Snapshot, Tool Execution Record, Commit Barrier, retry, cancellation,
   checkpoint invalidation, and indeterminate-execution guarantees from
   ADR-0005.
3. Mark legacy Rust-bound Conversations as view-only. Continuing them requires
   an explicit fork that creates a new Pi-bound Conversation without mutating
   the original record.
4. Route GUI, TUI, and `zyh agent` through the same Runtime Supervisor and Pi
   Bridge. Remove Agent SDK admin, server task, Ambient/Cloud Agent, handoff,
   orchestration, sharing, and conversation-sync execution paths.
5. Load multiple named Provider definitions from local settings, require one
   explicit default, snapshot the chosen Provider and Model per Agent Run, and
   keep API Keys in secure storage.
6. Retain Chat Completions only. Enforce same-Origin redirects, explicit errors,
   the bounded pre-output retry, and no Provider probing or fallback.
7. Reduce the Tool Catalog to shell, read files, apply diffs, and locally
   configured MCP. Remove Web Search/Fetch, semantic code search, Computer Use,
   Documents/Artifacts, upload, subagents, and cross-Agent messaging.
8. Keep Pi built-ins, extensions, autonomous resource discovery, and secret
   persistence disabled. Keep Bridge artifacts bundled and checksum-verified.

**Gate:** Protocol conformance, transcript reconstruction, crash-window,
idempotency, tool permission, Provider redirect, secret-redaction, GUI/TUI/CLI
parity, and legacy read-only/fork tests pass.

### 5. Remove Server Semantic Indexing And Cloud Search

1. Remove full-source embedding upload, GraphQL generation/retrieval, reranking,
   codebase quotas, sync state, speedbumps, and cloud index persistence.
2. Remove corresponding remote daemon protocol fields and handlers rather than
   retaining disabled messages.
3. Keep local repository detection, file outlines, file tree, `rg`, grep, local
   history search, and SSH-side scanning.
4. Remove cloud Notebook/Workflow embedding search and any index that consumes
   cloud objects.

**Gate:** Source and query fixtures prove that retained local search never calls
an embedding or Warp endpoint.

### 6. Make SSH Daemon Distribution Offline

1. Rename remote install paths and local protocol labels to ZYH. Replace account
   identity with a local connection or installation identifier that grants no
   cloud authority.
2. Build and bundle Linux/macOS arm64 and x86_64 daemon artifacts with a
   versioned manifest, byte size, SHA-256 digest, and protocol identity.
3. Select the matching artifact after the SSH preinstall check and upload it
   over the established SSH/SCP transport. Remove `/download/cli`, CDN retry,
   local download cache, and HTTP fallback code.
4. Fail release packaging when any required daemon artifact is absent or does
   not match the manifest. Development may use an explicit local artifact path;
   release builds reject it.
5. Remove remote crash reporting, semantic index sync, handoff snapshot upload,
   cloud preferences, and cloud auth from the daemon. Keep terminal, file,
   Git, repo metadata, and `rg` protocol capabilities.

**Gate:** All four artifact targets pass manifest verification and handshake
smoke tests; SSH integration succeeds with outbound HTTP denied.

### 7. Migrate And Clean Local Persistence

1. Copy the legacy SQLite database into the new ZYH root and retain a timestamped
   backup before opening it. Never clean the database in the legacy root.
2. Add forward-only transactional migrations. Do not edit historical migration
   files or generated Diesel schema directly.
3. Preserve terminal history, app/window/tab restoration, local Conversations,
   Pi sidecar records, local project metadata, and local run records.
4. Delete account credentials stored in SQLite, teams, permissions, cloud
   objects, refresh state, sync queues, object actions, cloud Workspaces,
   cloud Notebooks, old Rules, EVCs, server experiments, quota/billing data,
   cloud task metadata, and server conversation metadata from the copied DB.
5. Export supported local MCP installations into `.mcp.json` and secure storage
   before deleting their old persistence rows. Fail explicitly if a secret
   cannot be transferred.
6. Store a migration version and completion marker only after file and database
   migrations commit. Reruns must be idempotent and must not duplicate config,
   secrets, Workflows, or Conversation records.
7. Vacuum only after successful migration and a retained backup; never hide a
   failed cleanup behind a successful startup.

**Gate:** Fixture databases covering old local, anonymous, signed-in, malformed,
partial-migration, and current schemas preserve the allowed rows and delete the
forbidden rows exactly.

### 8. Delete Cloud Surfaces And Workspace Dependencies

1. Remove Warp Drive, sharing, teams, billing, referrals, managed secrets,
   cloud environments, Cloud/Ambient/Scheduled Agents, automatic updates,
   Voice/transcription, cloud-only onboarding, feedback upload, and remote docs
   entry points.
2. Extract any retained serialization or UI-neutral types from cloud crates into
   the owning local module before deleting the cloud crate.
3. Remove GraphQL, Warp server auth/client, cloud object, multi-agent client,
   cloud persistence, generated schema, and unused hosted-service dependencies
   from `Cargo.toml`, `Cargo.lock`, build scripts, test utilities, and CI.
4. Remove WASM members, target-specific dependencies, browser auth callbacks,
   Web entry points, and Web-only tests.
5. Remove cloud-only database model code after migrations no longer need runtime
   access to it. Keep migration SQL and compatibility fixtures needed to upgrade
   old databases.
6. Delete cloud source directories and tests as units. Do not leave permanent
   `cfg(false)`, disabled flags, inert singleton shims, or endpoint constants.

**Gate:** `cargo metadata` contains no cloud crate or hosted-service Git
dependency, and the retained workspace builds without cloud test helpers.

### 9. Complete External ZYH Contracts

1. Rename the application, executable, CLI help, environment variables, URI
   scheme, local control command, bundle IDs where owned by this fork, menu
   copy, file locations, and remote daemon labels.
2. Keep internal `warp_*` Rust names when they are implementation lineage rather
   than user-visible product contracts. Rename them only when already touched or
   when retaining the name would leak into an external contract.
3. Remove `warp://`, `warplocal://`, account/share/cloud deep links, Warp docs,
   Warp download links, server endpoints, and automatic browser jumps.
4. Keep only local `zyh://` actions. Validate and authorize local control through
   the existing owner-bound credential broker rather than introducing a network
   service.
5. Update local docs and examples. Do not add a replacement cloud documentation
   service.

**Gate:** User-visible snapshot tests and release artifact scans contain no
forbidden external Warp name, URI scheme, or endpoint. Internal crate names are
excluded from the branding assertion.

### 10. Enforce The Runtime Network Boundary

1. Centralize app-owned outbound HTTP creation so tests can classify every
   request by initiator and destination.
2. With no Provider or MCP configured, assert that GUI, TUI, CLI, remote daemon,
   settings, restored sessions, and shutdown produce no app-initiated external
   request.
3. During an Agent Run, permit only the selected Provider Origin and explicitly
   configured MCP origins. Preserve same-Origin redirect and API Key isolation.
4. Treat shell commands, Git, SSH, browser opens, and third-party CLIs as visible
   user-initiated subprocess effects, not app background network. They remain
   governed by their existing permission and UI paths.
5. Add static release scans for Warp service hosts, GraphQL endpoints,
   `warp://`, `warplocal://`, Sentry DSNs, update URLs, and daemon download URLs.
6. Remove test skips that would allow the network boundary to pass without
   running on all supported desktop platforms.

**Gate:** Network-deny integration tests and release scans are mandatory CI
checks, not optional manual verification.

## Validation

### Unit Tests

- ZYH path resolution and legacy-root classification on macOS, Linux, and
  Windows.
- Settings translation, unknown-key reports, backups, idempotency, and secret
  redaction.
- Atomic file writes, external-edit conflicts, permissions, malformed primary
  recovery, and last-known-good backups.
- Rule, Workflow, MCP, Notebook, SSH host, and project-config local CRUD.
- Provider selection, missing configuration, Origin restrictions, redirect
  rejection, bounded retry, and no fallback.
- Pi protocol handshake, transcripts, commit barriers, tool idempotency,
  cancellation, retry, recovery, and legacy Conversation forking.
- SQLite row classification, forward migration, partial failure, rollback,
  backup, idempotent rerun, and exact cloud-data deletion.
- Remote daemon artifact selection, checksum validation, install path,
  unsupported platform, SCP failure, and protocol mismatch.

### Integration Tests

- GUI, TUI, and `zyh agent` startup with an empty temporary home and all external
  network denied.
- Startup from copied signed-in and anonymous legacy fixtures without identity,
  refresh, cloud, telemetry, Sentry, or update requests.
- Local Provider Agent Run against a mock Chat Completions server, including
  shell, file, diff, and MCP tool loops.
- API Key never reaches a redirect target, MCP origin, log, diagnostic bundle,
  migration report, Bridge stderr, or Conversation record.
- Rules, Workflows, MCP, Notebooks, settings, and SSH hosts survive restart and
  reject concurrent external edits.
- SSH remote daemon install and use with HTTP denied for each supported remote
  platform fixture.
- Old Rust-bound Conversation remains read-only and explicit fork continues
  through Pi without changing the source record.
- Project `.warp/` migration requires confirmation and never silently dirties a
  Git worktree.

### Manual Checks

- Fresh install reaches a local terminal without onboarding or browser launch.
- Provider setup and Agent failure messages identify missing local fields.
- Rule editor edits `~/.agents/AGENTS.md` and reflects external changes.
- Workflow and Notebook editors show file paths and surface conflicts clearly.
- MCP secrets are absent from `.mcp.json` and visible only to the target process.
- SSH Center retains shortcuts locally and Remembered SSH Passwords remain in
  system storage.
- Settings, menus, command palette, URI handling, About, and CLI help expose no
  removed cloud actions or old external brand.
- App shutdown and restart issue no background network request.

### Commands To Run

Use focused package and module tests throughout each phase, then run the full
repository gate before every phase is considered complete:

```sh
./script/format --check
cargo nextest run -p warp --no-fail-fast
cargo nextest run -p warp_tui --no-fail-fast
cargo nextest run -p remote_server --no-fail-fast
cargo test --doc
./scripts/check.sh
```

Release validation must additionally build every supported desktop target,
verify the Pi Bridge and remote daemon manifests, run artifact string scans, and
execute the network-deny startup suite. Exact cross-build commands must be added
to the plan when the current release scripts have been reduced to ZYH-only
targets; do not claim cross-platform validation from a single local build.

## Completion Criteria

- No app startup or shutdown path registers Warp Auth, Server API, GraphQL,
  CloudModel, UpdateManager, SyncQueue, telemetry, Sentry, or update services.
- No Provider/MCP configuration means zero app-initiated external requests.
- Agent traffic reaches only the selected Provider Origin and configured MCP
  origins.
- No cloud crate, hosted-service client, cloud feature flag, cloud UI entry
  point, WASM target, daemon download path, or automatic updater remains in the
  retained workspace.
- Rules, Workflows, MCP, Notebooks, preferences, SSH Center, Conversations, and
  session restoration have explicit local sources of truth.
- The migrated ZYH database retains the approved local data and contains none
  of the classified cloud data.
- Release artifacts expose ZYH external contracts and contain no forbidden Warp
  endpoint or URI scheme.
- All focused tests, network-deny tests, artifact scans, and `./scripts/check.sh`
  pass without skipped failures.

## Rollback

- Implement each numbered phase as one or more small commits that keep the
  retained workspace buildable. Do not combine path migration, SQLite cleanup,
  Agent runtime replacement, and crate deletion in one commit.
- Migration copies legacy data into `~/.zyh/`; it never mutates or deletes the
  legacy root. A rolled-back binary can continue using the untouched legacy
  data.
- Before changing the copied SQLite database, create and verify a timestamped
  backup. Transaction failure leaves the pre-migration copy authoritative and
  does not write the completion marker.
- File migrations retain source files and one last-known-good destination
  backup. Project migration is user-confirmed and copy-only.
- Revert an incomplete phase rather than restoring removed cloud runtime shims.
  Once a release passes the permanent-local completion gates, cloud behavior is
  not a supported runtime rollback path.
- If a source deletion exposes an unclassified retained consumer, stop, restore
  the last buildable commit for that phase, update the dependency inventory, and
  request review before widening the plan.

## Review Gate

Human review is required before implementation because this plan changes auth,
secrets, migrations, public local protocols, release packaging, and the
Local-only network boundary. Review should explicitly approve:

- the retained-versus-deleted persistence table classification;
- the ZYH path and secure-storage migrations;
- legacy Conversation read-only and fork behavior;
- the Provider and MCP secret boundary;
- the remote daemon artifact matrix and release failure policy;
- the endpoint and artifact scan allowlist;
- the physical cloud crate deletion order.

No implementation should begin until this review gate is cleared.
