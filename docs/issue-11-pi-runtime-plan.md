# Agent Plan: Issue #11 Pi Agent Runtime

## Review Gate

Status: **Approved by the repository owner on 2026-07-13. Implementation may proceed by phase.**

This is a high-risk third-party runtime integration. The approval covers the database migration,
permission boundary, crash-recovery matrix, Bridge artifact supply chain, rollback behavior, and the
revised single-repository ownership model. Each phase still requires its focused validation before
the next high-risk boundary is enabled.

## Goal

Use Pi as the Agent Runtime for newly created Interactive Agent Conversations in Local-only Mode.
Warp remains the Runtime Supervisor, Conversation Record owner, Tool Execution Authority,
permission authority, Provider configuration owner, and UI. The implementation must preserve these
invariants:

1. The Conversation Record is canonical; a Runtime Checkpoint is disposable.
2. A Conversation Record has one immutable Runtime Binding and at most one active Agent Run.
3. Pi can request only tools in the immutable Tool Catalog; Warp alone approves and performs effects.
4. No model turn or tool effect may depend on an uncommitted Runtime Transcript mutation.
5. A tool-call identity that reached `executing` is never executed automatically again, even when
   Warp cannot prove whether its effect completed.
6. Existing Conversation Records remain Rust-bound and readable, with revision zero.
7. Disabling rollout never routes a Pi-bound Conversation Record through the Rust runtime.
8. Local-only Mode continues to contact only the configured Provider Origin and explicitly configured
   MCP servers, with no Warp identity, AI, quota, sync, telemetry, Sentry, or cloud traffic.

## Preconditions And Vendored Dependencies

- ADR-0005 must be committed and accepted before implementation code lands. It is currently an
  untracked workspace file and is the authoritative decision record for this plan.
- Warp owns `tools/warp-bridge`, including the versioned JSON Schema, valid and invalid JSONL
  conformance fixtures, fake/test Bridge, and release build definitions. This is the only Bridge
  Protocol source of truth.
- Exact upstream Pi release packages are vendored under `third_party/pi` with their source revision,
  package versions, SHA-256 hashes, and MIT License. The Bridge must import only this locked snapshot;
  it must not discover a user-installed Pi or fetch dependencies at application runtime.
- The Bridge version, Core Protocol schema hash, artifact SHA-256 hashes, sizes, standalone-binary
  toolchain, target support, and third-party license obligations must be available before packaging.
- The current worktree contains unrelated, unfinished direct Local Provider tool-loop changes.
  Implementation must start from an isolated branch/worktree or after the owner has committed those
  changes. Do not fold them into Issue #11 merely because they touch adjacent files.
- Verify the application package and all supported target builds can legally redistribute the Bridge
  before enabling the runtime Feature Flag.

## Files Likely Involved

### Decisions And Domain Vocabulary

- `CONTEXT.md`
- `docs/adr/0004-local-only-custom-provider-mode.md`
- `docs/adr/0005-use-pi-as-local-agent-runtime.md`
- `docs/issue-11-pi-runtime-plan.md`

### Runtime Selection And Conversation Integration

- `crates/warp_features/src/lib.rs`
- `app/src/features.rs`
- `app/src/local_mode.rs`
- `app/src/ai/agent/mod.rs`
- `app/src/ai/agent/api.rs`
- `app/src/ai/agent/api/impl.rs`
- `app/src/ai/agent/conversation.rs`
- `app/src/ai/blocklist/controller/response_stream.rs`
- `app/src/ai/blocklist/history_model.rs`
- `app/src/ai/conversation_status_ui.rs`
- `app/src/tui/mod.rs`

Proposed new runtime modules, split before any file approaches 500 lines:

- `app/src/ai/agent/runtime/mod.rs` - public Warp-side Agent Runtime interface.
- `app/src/ai/agent/runtime/supervisor.rs` - app-wide process/run ownership.
- `app/src/ai/agent/runtime/bridge_process.rs` - launch, handshake, JSONL IO, cancellation, exit.
- `app/src/ai/agent/runtime/protocol.rs` - strict native Rust protocol types and validation.
- `app/src/ai/agent/runtime/transcript.rs` - Runtime Transcript projection and bounded sync.
- `app/src/ai/agent/runtime/run_configuration.rs` - immutable Provider, Tool, and resource snapshot.
- `app/src/ai/agent/runtime/tool_execution.rs` - durable Tool Execution Authority adapter.
- `app/src/ai/agent/runtime/resources.rs` - bounded Resource Snapshot construction.
- Separate `*_tests.rs` files for each module.

Do not create a new general-purpose runtime crate until the first GUI/TUI call sites prove that the
app-owned module cannot be shared cleanly. The initial interface should be deeper than the process or
protocol details: start/cancel/retry a run, submit a queued prompt, subscribe to typed events, and
invalidate/rebuild one Conversation Record.

### Persistence And Migration

- `crates/persistence/migrations/<timestamp>_add_agent_runtime_records/up.sql`
- `crates/persistence/migrations/<timestamp>_add_agent_runtime_records/down.sql`
- `crates/persistence/src/schema.rs` - generated by the repository Diesel workflow, never hand-edited.
- `crates/persistence/schema.patch` - regenerated only through the documented patch workflow.
- `crates/persistence/src/model.rs`
- `crates/persistence/src/model_tests.rs`
- `app/src/persistence/mod.rs`
- `app/src/persistence/sqlite.rs`
- `app/src/persistence/sqlite_tests.rs`
- `app/src/persistence/agent.rs`
- `app/src/persistence/agent_tests.rs`

The additive migration should create exactly the two ADR-approved sidecar tables:

- `agent_runtime_runs`: integer primary key; `conversation_id`; `run_id`; optional
  `retry_of_run_id`; `starting_revision`; run state/terminal outcome; the last acknowledged
  `commit_id` and committed revision needed for barrier idempotency; timestamps; unique
  `(conversation_id, run_id)`.
- `agent_tool_execution_records`: integer primary key; `conversation_id`; `run_id`;
  `tool_call_id`; request fingerprint; durable state; versioned complete typed outcome; versioned
  fixed Tool Result Projection; timestamps; unique `(conversation_id, run_id, tool_call_id)`.

Both tables must reference the owning conversation/run and be deleted explicitly in the same SQLite
transaction as the Conversation Record. Do not rely only on implicit cascade behavior. Runtime
Binding and Conversation Record Revision remain optional, backward-compatible fields in
`AgentConversationData`; they are not duplicated as a second conversation source of truth.

### Existing Permission And Tool Execution Surfaces

- `app/src/ai/blocklist/permissions.rs`
- `app/src/ai/blocklist/action_model/mod.rs`
- `app/src/ai/blocklist/action_model/execute/mod.rs`
- `app/src/ai/blocklist/action_model/execute/shell_command.rs`
- `app/src/ai/blocklist/action_model/execute/read_files.rs`
- `app/src/ai/blocklist/action_model/execute/request_file_edits.rs`
- `app/src/ai/blocklist/action_model/execute/call_mcp_tool.rs`
- Existing focused executor and permission test files beside those modules.

The Bridge adapter must translate Tool Catalog entries into the existing typed action/executor
inputs. It must not call a shell, filesystem, or MCP client directly and must not duplicate
`BlocklistAIPermissions`. Permission policy is evaluated live for every Tool Request and is not
copied into Run Configuration.

### Bridge Artifacts And Packaging

- Proposed `tools/warp-bridge/` TypeScript package, protocol schema, fixtures, tests, and standalone
  build scripts.
- Proposed `third_party/pi/` verified upstream package archives plus provenance and license metadata.
- Proposed `resources/bundled/agent-runtime/bridge-manifest.json`.
- Proposed target-specific Bridge artifacts under the normal prepared-resource output, not checked
  in unless repository release policy explicitly requires it.
- `crates/warp_core/src/paths.rs`
- `script/macos/bundle`
- `script/linux/bundle`
- `script/windows/bundle.ps1`
- `script/windows/prepare_bundled_resources.ps1`
- Platform signing/notarization and third-party license manifests used by those scripts.

Development may accept an explicit local Bridge path. Release builds must reject that override,
must fail when the target artifact is absent, and must verify version, schema hash, digest, and size
before packaging.

### Highest-Level Test Seam

The primary seam is the Runtime Supervisor exercised with:

- a controllable fake Bridge process that speaks the real protocol fixtures;
- a mock OpenAI-compatible Provider;
- the real Tool Execution Authority with injectable permission decisions;
- a real disposable SQLite database with embedded migrations;
- the public conversation/event boundary used by GUI and TUI controllers.

This one seam must observe streamed UI events, Bridge traffic, permission decisions, durable rows,
and actual test effects. Lower-level protocol, migration, writer, and executor tests support it but do
not replace it. Existing integration-test prior art is in `crates/integration/src/test/agent_mode.rs`;
new end-to-end tests should be registered through the existing integration runner only after the
headless supervisor harness is stable.

## Risks

| Area | Risk | Required mitigation |
| --- | --- | --- |
| Database migration | Startup migration fails or old rows become unreadable | Additive tables and optional JSON fields only; migrate representative old fixtures; run migration twice; test redo on disposable DB |
| Production rollback | An old binary ignores Runtime Binding and may write a Pi-bound conversation as Rust-bound | Roll back with a new binary that leaves the schema in place and disables new Pi binding; do not promise binary downgrade after rollout |
| Conversation eviction/deletion | Sidecar rows are orphaned when the 200-row retention path or explicit deletion runs | Extend the single transactional deletion helper used by both explicit deletion and eviction; assert no sidecar rows remain |
| Permission bypass | Bridge process executes or reaches tools outside Warp policy | Disable Pi tools/Extensions; expose proxy schemas only; adapt exclusively through existing action executors and `BlocklistAIPermissions` |
| Duplicate side effects | Crash or acknowledgement loss replays a mutating tool | Persist `pending`, then `executing`, before the effect; never replay `executing` without a committed outcome; redeliver persisted outcomes only |
| False recovery result | Warp guesses success/failure after an uncertain effect | Persist and display `tool_outcome_unknown` with `may_have_executed: true`; terminate the run; require explicit Retry Run |
| Revision race | History edit or stale process overwrites newer Runtime Transcript state | Compare-and-swap expected revision inside the SQLite transaction; terminate the process and rebuild on conflict |
| Protocol drift | Warp and Bridge silently accept different wire semantics | Core major version plus schema hash; strict unknown-field rejection; shared valid/invalid fixtures; fail before secrets are sent |
| Secret/content leakage | API Keys, prompts, tool data, or Provider bodies reach logs or errors | Minimal environment, post-handshake secret delivery, typed redacted errors, content-free stderr tests, no raw protocol logging |
| Provider redirect | API Key follows a redirect to another origin/port/scheme | Enforce Provider Origin and three-hop limit in Bridge conformance tests; reject before forwarding credentials |
| Process ownership | Two views start two processes or runs for one conversation | App-wide supervisor keyed by Conversation Record identity; view controllers hold handles only; concurrency tests across GUI/TUI attachments |
| Cancellation | A hung Bridge blocks the conversation or cancellation kills an already completed effect ambiguously | Cancel pending approval/execution, request Bridge abort, wait bounded grace period, then terminate and reconstruct conservatively |
| Resource drift | Restart rereads changed rules/Skill files and changes historical model context | Persist normalized Resource Snapshot with initiating input and project it from the Conversation Record |
| Packaging | A release ships no Bridge, the wrong target, or an unverified binary | Target manifest validation, digest/size checks, release override rejection, platform signing, and artifact smoke tests |
| Scope collision | Current direct-provider work is accidentally rewritten or committed | Isolate worktree/branch; stage explicit paths; compare every commit against its phase; never bulk-add the dirty tree |

## Crash-Recovery Matrix

| Crash point | Durable state on restart | Required recovery | May execute tool automatically? |
| --- | --- | --- | --- |
| Before `pending` insert commits | No Tool Execution Record | Fail the request/run without an effect; require explicit retry | No |
| After `pending`, before permission decision | `pending` | Record an interrupted-before-execution error and end the run; do not recreate an approval prompt silently | No |
| After denial, before denial outcome commits | Usually `pending`, no effect | Treat as interrupted before execution; do not invent a denial result | No |
| After `executing` commits, before effect begins | `executing`, no outcome | Materialize Indeterminate Tool Execution conservatively | No |
| During effect | `executing`, no outcome | Materialize Indeterminate Tool Execution | No |
| Effect returns, before outcome transaction | `executing`, no outcome | Materialize Indeterminate Tool Execution | No |
| During outcome transaction | SQLite exposes either `executing` or complete outcome | Use the visible durable state; never partially reconstruct | No |
| Outcome transaction commits, before Bridge acknowledgement | Complete outcome and fixed projection | Redeliver the stored projection byte-for-byte | No |
| Assistant message commit succeeds, acknowledgement is lost | New revision plus committed message identity | Return the already committed revision for the same `commit_id` | No tool may start until acknowledged |
| Bridge exits during text streaming | No completed assistant message commit | Retain visible Interrupted Output; exclude it from Runtime Transcript | No |
| Transcript Sync stops before commit/ack | Prior accepted revision only | Discard the candidate sync and resend the complete transcript | No |
| Revision compare-and-swap fails | Newer Conversation Record revision wins | Cancel run, terminate process, invalidate checkpoint, rebuild from complete transcript | No |
| Cancellation grace period expires | Last committed Conversation Record/tool state | Terminate process and apply the same durable-state recovery rules | No |

Tests must inject failure immediately before and after every row in this matrix. A test-only fault
injector belongs at the persistence/effect boundaries, not in production call sites as ad hoc flags.

## Plan

### Phase 0: Contract And Workspace Gate

1. Land/approve ADR-0005 and this plan. Record Warp's Bridge ownership and the vendored Pi source
   revision.
2. Isolate Issue #11 from the existing dirty direct-provider changes.
3. Add verified upstream Pi package archives, provenance, checksums, and license under
   `third_party/pi`; do not copy the full Pi CLI or unrelated Provider integrations into the Bridge
   bundle.
4. Establish `tools/warp-bridge/protocol` as the authoritative Protocol Schema and conformance
   fixture location shared by TypeScript and Rust tests.
5. Confirm the standalone Bridge artifact/license/signing path for all release targets. Stop if any
   target or redistribution requirement is unresolved.

Validation: fixture provenance is reviewable; no runtime code or database migration has landed.

### Phase 1: Protocol Boundary, Test First

1. Add failing Rust conformance tests for handshake success, major/hash mismatch, required capability
   mismatch, unknown fields, malformed JSONL, missing/duplicate tool-call IDs, bounded frames, and
   pre-handshake secret exclusion.
2. Add strict native Rust protocol types and a framed JSONL codec that pass the shared fixtures.
3. Add a fake Bridge executable/harness driven by scripted fixture events. Keep it test-only.
4. Add typed redacted error categories; prove debug/display output cannot include supplied secrets or
   arbitrary Provider bodies.

Commit boundary: protocol types + fixture tests only; no Runtime Selection change.

### Phase 2: Additive Persistence Foundation, Test First

1. Add pre-migration fixtures containing legacy Conversation Records/tasks and migration tests that
   prove old data remains readable with Rust binding/revision-zero defaults.
2. Add the two sidecar-table migration, generate `schema.rs` through Diesel, and update
   `schema.patch` through the documented workflow.
3. Add typed persistence models and encoding versions for Agent Run Record, Tool Execution Record,
   complete outcome, and fixed Tool Result Projection.
4. Extend `AgentConversationData` with optional Runtime Binding and revision fields; prove unknown or
   missing fields remain backward compatible.
5. Extend the single deletion/eviction transaction to delete tool records, then run records, then
   tasks and the conversation. Test explicit deletion, retention eviction, rollback on failure, and
   no orphans.

Commit boundary: schema and read/write models are present but unused by runtime selection.

### Phase 3: Revisioned Commit Barrier, Test First

1. Add `CommitAgentRuntimeMutation` to the existing SQLite writer with `commit_id`, expected revision,
   complete task snapshot, optional sidecar mutation, and one-shot acknowledgement.
2. In one transaction, load/validate Runtime Binding, compare the expected revision, apply task and
   sidecar changes, advance the revision, and retain the last acknowledged commit identity.
3. Return the prior committed revision when the same barrier is redelivered after acknowledgement
   loss. Reject a reused identity with different content.
4. Keep Rust-bound `UpdateMultiAgentConversation` behavior unchanged.
5. Add writer tests for successful commit, stale revision, duplicate delivery, changed duplicate,
   transaction failure, acknowledgement loss, deletion race, and concurrent history edit.

Commit boundary: durable command exists behind tests; no Bridge process can execute tools yet.

### Phase 4: Runtime Supervisor And Bridge Lifecycle, Test First

1. Define the narrow Agent Runtime interface and add the app-wide Runtime Supervisor keyed by
   Conversation Record identity.
2. Launch the Bridge with a private working directory, minimal inherited environment, piped JSONL
   stdin/stdout, bounded redacted stderr, and kill-on-drop/explicit shutdown behavior.
3. Enforce one process and one active Agent Run per conversation across multiple view handles.
4. Implement handshake, bounded timeout, cancellation grace period, unexpected-exit handling, idle
   eviction, app shutdown, and checkpoint invalidation.
5. Test with the fake Bridge: duplicate views, concurrent conversations, hung handshake, hung cancel,
   process exit, restart, and no secret delivery before successful handshake.

Commit boundary: supervisor lifecycle works with fake Bridge; Runtime Selection remains disabled.

### Phase 5: Runtime Transcript, Run Configuration, And Text Runs

1. Project completed user/assistant messages, paired tool activity, and Resource Snapshots from the
   Conversation Record into a provider-neutral Runtime Transcript. Exclude UI-only state and
   Interrupted Output.
2. Implement bounded transactional Transcript Sync with begin/items/commit and revision
   acknowledgement. Discard interrupted candidates.
3. Build one immutable Run Configuration from the selected Chat Completions Provider, current model,
   Provider Origin, Tool Catalog, Agent Resource Catalog, working directory, context limit, and
   reasoning settings.
4. Materialize bounded Resource Snapshots from only explicitly selected rules, attachments, project
   context, and explicitly invoked Skills. Persist the snapshot before starting the run.
5. Stream text optimistically, then commit completed assistant messages through the revisioned
   barrier. Pi may not request a tool, start another model turn, or complete until acknowledgement.
6. Preserve partial text as Interrupted Output on failure and add explicit Retry Run lineage without
   duplicating the original user message.

Commit boundary: fake Bridge can complete/retry a text-only run with restart reconstruction; tools
remain unavailable.

### Phase 6: Durable Tool Execution Authority, Test First

1. Build the first immutable Tool Catalog for `run_shell_command`, `read_files`,
   `apply_file_diffs`, and already configured MCP tools, preserving stable Provider-visible names.
2. Validate tool name, arguments, `tool_call_id`, request fingerprint, and remaining Tool Request
   Limit before permission. Persist invalid requests as error outcomes without prompting/executing.
3. Insert `pending` before permission. Translate requests into existing typed action/executor inputs;
   do not bypass `BlocklistAIPermissions`.
4. Return Tool Denial as a structured persisted result. For approval, durably transition to
   `executing` before invoking the existing executor.
5. After the effect, atomically store complete outcome, fixed projection, task update, and revision;
   acknowledge only after commit. Execute sequentially and enforce the 32-request run limit.
6. Add the fault injector and prove every crash-matrix row, including outcome transaction failure,
   acknowledgement loss, and byte-for-byte result redelivery without re-execution.
7. Surface Indeterminate Tool Execution as `tool_outcome_unknown` with
   `may_have_executed: true`, fail the run, and carry it into an explicit Retry Run.

Commit boundary: full tool loop passes the primary supervisor/database/permission seam with fake
Bridge and no Runtime Selection change.

### Phase 7: Runtime Binding, GUI/TUI Integration, And Rollout

1. Add a product-level Pi Agent Runtime Feature Flag separate from Local-only Mode.
2. At new Conversation Record creation, bind to Pi only for eligible Local-only Interactive Agent
   Conversations with a valid Chat Completions Provider and enabled rollout flag. Treat missing
   bindings as Rust; existing binding always overrides the current flag.
3. Route view/controller start, cancel, queued prompt, Retry, history edit, fork, deletion, and
   restore operations through the Runtime Supervisor for Pi-bound records.
4. On history edits, cancel the run, terminate the process, invalidate its checkpoint, commit the new
   revision, and rebuild on the next run.
5. If Bridge startup fails, keep the conversation viewable and offer only an explicit fork of
   completed history into a new Rust-bound conversation. Never silently fall back.
6. Preserve Account, Anonymous-only, Ambient/Cloud Agent, Agent SDK/CLI, passive suggestion, and
   Rust-bound behavior. Disable passive suggestions only when Pi runtime selection owns the new
   Interactive Agent Conversation.
7. Add GUI and TUI regression tests against the same supervisor seam.

Commit boundary: rollout flag remains disabled by default; eligible test builds can opt in.

### Phase 8: Verified Artifact Packaging

1. Add the Bridge Artifact Manifest and a build-time verifier for version, Core Protocol schema hash,
   target, SHA-256 digest, and byte size.
2. Extend macOS, Linux, and Windows resource preparation to include exactly one target Bridge binary
   and required licenses. Apply normal signing/notarization.
3. Reject missing/mismatched artifacts and reject local-path overrides in release builds. Do not add
   application runtime download/update behavior.
4. Run each target artifact's handshake/conformance smoke test before packaging and a packaged-app
   smoke test after signing where CI supports it.

Commit boundary: release builds fail closed; rollout flag still controls new bindings.

### Phase 9: Release Gate And Review

1. Run the primary high-level seam through text, approved/denied/invalid tools, cancellation, Retry,
   process restart, checkpoint loss, history edit, deletion, limit exhaustion, and all crash points.
2. Run a network-deny test with Warp endpoints blocked and only mock Provider/configured MCP origins
   allowed.
3. Run focused migration, persistence, protocol, supervisor, permission, GUI, and TUI checks, then the
   repository standard check.
4. Use `/code-review` on the complete Issue #11 diff, with explicit review passes for persistence,
   permissions, protocol/secrets, and packaging. Resolve findings before enabling rollout.
5. Commit each phase independently with explicit paths. Do not stage unrelated direct-provider work.

## Validation

### Unit Tests

- Protocol fixture conformance and secret/redaction tests.
- `AgentConversationData` backward compatibility and Runtime Binding immutability.
- Runtime Transcript projection and Resource Snapshot stability after source mutation/deletion.
- Supervisor process/run uniqueness, cancellation, restart, and revision-conflict behavior.
- Tool Catalog validation, permission outcomes, Tool Request Limit, projection truncation, and typed
  denial/error outcomes.

### Persistence And Migration Tests

- Embedded migrations against a fresh database and representative legacy database fixtures.
- Migration rerun/idempotent startup and development-only migration redo on disposable files.
- Sidecar uniqueness/foreign keys and deletion/eviction without orphans.
- Revision compare-and-swap, duplicate commit acknowledgement, changed duplicate rejection, and full
  transaction rollback.
- Every crash-recovery matrix point with an effect counter proving at-most-once execution.

### Integration Tests

- Primary Runtime Supervisor/fake Bridge/mock Provider/real SQLite/real permission-adapter seam.
- GUI and TUI attach to the same Conversation Record without starting duplicate processes/runs.
- Complete text/tool conversation survives process and application restart.
- Pi-bound startup failure remains viewable and never enters Rust runtime implicitly.
- Local-only network-deny test observes no forbidden Warp traffic.
- Packaged Bridge handshake smoke test for every release target in CI.

### Manual Checks

- Approved and denied shell/file/MCP requests use existing permission UI and visible activity.
- Interrupted Output, outcome unknown, Retry, cancellation, limit reached, and explicit runtime fork
  have clear non-quota user-visible states.
- Changing rollout flag affects only newly created Conversation Records.
- Existing Rust-bound conversations remain unchanged in GUI and TUI.
- API Keys, prompts, file content, commands, and tool results do not appear in Bridge stderr or app
  diagnostics.

### Commands To Run During Implementation

Run focused checks after each red/green slice, adjusting exact test filters to the final module names:

```sh
cargo test -p persistence --lib
cargo test -p warp persistence::agent --lib
cargo test -p warp persistence::sqlite --lib
cargo test -p warp agent::runtime --lib --features local_only_custom_provider_mode
cargo test -p warp blocklist::permissions --lib --features local_only_custom_provider_mode
cargo test -p warp tui::tests --lib --features local_only_custom_provider_mode
cargo check -p warp --bin warp-oss --features local_only_custom_provider_mode
cargo fmt --check
git diff --check
```

Run the focused integration target through the repository's integration runner once registered. For
PR-ready handoff, run:

```sh
./scripts/check.sh
```

Cross-platform Bridge packaging/signing tests that cannot run locally must be run in CI and reported
explicitly; they may not be silently skipped.

## Rollback

1. Disable the Pi Agent Runtime rollout flag for **new** Conversation Records. Existing Pi-bound
   records remain Pi-bound and viewable; they never fall back implicitly.
2. Keep the additive sidecar tables and optional conversation metadata. Do not run a production down
   migration while any build may contain Pi-bound records.
3. Roll back by shipping a current-compatible binary with Pi selection disabled, not by downgrading
   to a binary that does not understand Runtime Binding. An old binary could erase unknown JSON
   fields on write or continue a Pi-bound conversation through Rust.
4. If a Bridge artifact is faulty, ship a corrected, manifest-pinned application build or disable new
   bindings. Do not hot-download a replacement at runtime.
5. A Pi-bound conversation that cannot continue remains readable. The only runtime conversion is an
   explicit user-authorized fork of completed history into a new Rust-bound Conversation Record.
6. Never repair an Indeterminate Tool Execution by replaying it or rewriting it as success/failure.
   Preserve the immutable unknown outcome through rollback and retry.

## Human Review Checklist

- [x] Warp repository ownership of the Bridge, protocol schema, and fixtures is accepted.
- [ ] ADR-0005 and this revised single-repository plan are committed.
- [ ] The two-table schema, encoding versions, indexes, foreign keys, deletion order, and retention
      interaction are approved.
- [ ] The no-binary-downgrade rollback constraint is accepted.
- [ ] The crash-recovery matrix is complete and the conservative `executing -> indeterminate` rule
      is accepted.
- [ ] The existing permission/action executors are confirmed as the only effect path.
- [ ] Provider Origin, secret delivery, diagnostic redaction, and minimal child environment are
      accepted.
- [ ] The phase boundaries and disabled-by-default rollout order are accepted.
- [ ] Vendored Pi provenance, license, checksums, update process, target matrix, signing, and Warp CI
      ownership are confirmed.
