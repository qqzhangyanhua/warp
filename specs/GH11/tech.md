# GH11: Local Pi Agent Runtime Tech Spec

GitHub: [#11](https://github.com/qqzhangyanhua/warp/issues/11)

## Context

The current Local-only direct Provider path owns model streaming and an emerging tool loop in Rust.
GH11 replaces that loop only for newly Pi-bound Interactive Agent Conversations while preserving all
other runtime bindings and Warp's Local-only network boundary. User-visible behavior is defined in
[product.md](product.md); architectural decisions are recorded in
[`docs/adr/0005-use-pi-as-local-agent-runtime.md`](../../docs/adr/0005-use-pi-as-local-agent-runtime.md),
and migration/crash/rollback sequencing is governed by
[`docs/issue-11-pi-runtime-plan.md`](../../docs/issue-11-pi-runtime-plan.md).

Relevant code at baseline `8942cae6ed74cc12e075aa3d53e61ab62026c70e`:

- `app/src/ai/agent/` owns Interactive Agent conversation execution.
- `app/src/ai/agent/conversation.rs` builds the existing conversation persistence event.
- `crates/persistence/src/model.rs` defines `AgentConversationData`, the backward-compatible JSON
  metadata stored with each conversation.
- `app/src/persistence/agent.rs` performs transactional conversation/task writes, tree-aware
  retention, and explicit deletion.
- `app/src/persistence/mod.rs` and `sqlite.rs` define and execute writer-thread commands.
- `crates/persistence/migrations/` and `crates/persistence/src/schema.rs` own SQLite upgrades and the
  generated Diesel schema.
- `app/src/ai/blocklist/action_model/execute/` and existing MCP execution paths are the only allowed
  tool-effect adapters.
- `tools/warp-bridge/` is the Warp-owned Bridge source, Protocol Schema, fixtures, and build root.
- `third_party/pi/` contains exact verified upstream Pi release archives and license provenance.

## Proposed Changes

### Protocol and dependency boundary

`tools/warp-bridge/protocol/core-v1.schema.json` is the only Core Protocol source of truth. Rust and
TypeScript parse against it, reject unknown fields, enforce negotiated frame/Transcript limits, and
share valid/invalid JSONL fixtures. Core covers handshake, transactional Transcript Sync, run start
and cancellation, streamed text, completed-message commits, tool requests/results, and terminal
outcomes. Optional evolution uses separately versioned and hashed capabilities.

The Bridge sends `bridge_hello` before receiving any sensitive data. Warp validates the Core major,
exact schema bytes, and every required capability, then sends an accepted or rejected handshake
result. Only the accepted result moves both sides to Ready.

Pi `0.80.6` is consumed only from `third_party/pi/packages/*.tgz`. The Bridge imports the agent core
and Chat Completions adapter but disables Pi credential/model files, built-in tools, Extensions,
resource discovery, session persistence, SDK retry, and unrelated Provider integrations.

### Runtime Supervisor and Bridge process

Add an app-wide Runtime Supervisor keyed by Conversation Record identity. Its public interface is
start, cancel, retry, queue prompt, subscribe to typed events, invalidate/rebuild, and delete. View
controllers hold handles only. The supervisor enforces one child process and one active Agent Run per
conversation.

The process adapter launches the bundled Bridge with piped JSONL, a private runtime directory,
kill-on-drop behavior, a minimal environment, bounded content-free stderr, a handshake timeout, and a
bounded cancellation grace period. It reads frames with a bounded reader before allocating an
unbounded line. Checkpoint invalidation or revision conflict terminates the process; idle eviction may
discard it because the Conversation Record can rebuild it.

### Runtime Transcript and Run Configuration

Project the canonical Conversation Record into completed user/assistant messages, retained Resource
Snapshots, and paired tool requests/results. Exclude UI-only state and Interrupted Output. Send a
bounded begin/items/commit transaction and activate it only after complete validation; acknowledge
the accepted Conversation Record revision before `run_start`.

Build one immutable Run Configuration per run. Normalize the existing Chat Completions Base URL and
Provider Origin in Warp, supply the API Key only after handshake, set SDK retries to zero, and allow
one Bridge-managed pre-output retry. Tool and resource catalogs contain only Warp-selected entries.

### Persistence and migration

Extend `AgentConversationData` with optional `runtime_binding` and `runtime_transcript_revision`
fields. Absence means Rust and revision zero; writes omit default values where possible to preserve
legacy JSON compatibility.

Add exactly two sidecar tables through an additive Diesel migration:

- `agent_runtime_runs`: conversation/run identity, retry lineage, starting revision, state/terminal
  outcome, last committed identity/revision, and timestamps; unique `(conversation_id, run_id)`.
- `agent_tool_execution_records`: composite tool identity, immutable request fingerprint, durable
  state, versioned complete outcome, fixed versioned projection, and timestamps; unique
  `(conversation_id, run_id, tool_call_id)` and linked to its run.

Explicitly delete tool records, then run records, then tasks and conversation rows in the existing
single transaction used by user deletion and retention eviction. Do not rely only on cascade.

Add `CommitAgentRuntimeMutation` to the SQLite writer. It carries a stable commit identity, expected
revision, complete task snapshot, optional sidecar mutation, and one-shot acknowledgement. The writer
uses a single transaction for revision compare-and-swap, task/conversation update, sidecar mutation,
and new revision. Redelivery of the last committed identity returns the existing revision.

### Tool Execution Authority

Validate catalog identity, Provider-visible name, arguments, stable tool-call identity, request
fingerprint, and remaining budget before permission. Invalid requests persist an error projection but
do not reach permission or execution.

Insert `pending`, ask the existing Warp permission surface, and persist denial before returning it.
For approved effects, durably mark `executing`, invoke only the existing typed action executor, then
atomically persist the complete outcome, fixed projection, task update, and revision before returning
to Pi. Recovery converts `executing` without outcome to immutable `tool_outcome_unknown`, ends the
run, and never invokes that identity again.

### Runtime selection and packaging

Add a high-level Pi Agent Runtime Feature Flag. At new Conversation Record creation, select Pi only
for eligible Local-only Interactive Agent Conversations. Persist the binding immediately; existing
binding always wins over current flag state. Startup failure offers no implicit fallback.

Build standalone Bridge artifacts from pinned source and dependencies for the six supported targets.
A Warp-owned manifest records Bridge/Pi versions, source revision, Core schema hash, target, SHA-256,
and byte size. Platform resource preparation verifies exactly one matching artifact and carries it
through existing signing/notarization. Release builds reject development path overrides; runtime
download/update is absent.

## Testing and Validation

- Shared TypeScript/Rust conformance fixtures verify Behavior 18, 22, 24, and 28, including unknown
  fields, malformed JSONL, mismatched schema/capabilities, frame/sync limits, redaction, and forbidden
  path or raw-detail fields.
- Runtime Supervisor tests use the spawnable fake Bridge to verify Behavior 4, 5, 9, 10, 18, 24, 25,
  and 29: duplicate views, concurrent conversations, handshake/cancel timeouts, restart, stale
  revision, checkpoint loss, and no pre-handshake secret delivery.
- Migration tests run all migrations against representative pre-GH11 conversation/task rows, verify
  Behavior 2, 3, and 27, rerun startup idempotently, and redo the migration on a disposable database.
- SQLite writer tests verify Behavior 15, 16, 25, and 26 with revision conflicts, repeated commit
  identities, acknowledgement loss, transaction rollback, constraint failure, and each injected
  crash boundary.
- Tool Authority tests use existing permission adapters and typed executors for Behavior 11-17,
  including approve, user/policy denial, malformed arguments, identity reuse, sequential batches,
  limit exhaustion, completed-result redelivery, and indeterminate recovery.
- Mock Provider tests verify Behavior 6-9 and 19: streaming, pre-output retry, no post-output retry,
  cancellation, same-origin redirects, cross-origin rejection, HTTP compatibility, and bounded
  content-free failures.
- Integration tests attach GUI and TUI views to one Conversation Record, restart the process and app,
  edit/fork/delete history, and verify Behavior 1-5, 7, 9, 21, 23, and 29.
- A network-deny run allows only the configured Provider and MCP origins and verifies Behavior 30.
- CI builds all six artifacts, runs handshake/conformance smoke tests, verifies manifest digest/size,
  and performs packaged-app smoke tests where signing infrastructure supports it (Behavior 31-32).
- Before PR-ready handoff run focused suites, `pnpm test`, `pnpm typecheck`, schema/provenance checks,
  migration redo on a disposable DB, and `./scripts/check.sh`. Report any platform check unavailable
  locally rather than claiming it passed.

## Parallelization

The migration/writer/tool-crash path remains sequential because each phase changes the next phase's
durability contract and the shared checkout already contains adjacent Local Provider work. Platform
artifact builds may fan out in CI by target after the Bridge source and manifest format stabilize;
each job owns only its target artifact and reports its digest to one manifest-generation merge step.
No parallel local implementation agents should edit the protocol, persistence schema, or Agent
Runtime modules concurrently.

## Risks and Mitigations

- **Migration or rollback data loss:** additive tables and optional JSON fields only; representative
  old DB tests; roll back with a schema-compatible binary that disables new bindings, never by binary
  downgrade.
- **Duplicate or invented tool outcome:** durable `pending -> executing -> outcome`; recovery never
  replays `executing` and never guesses success/failure.
- **Permission bypass:** no Pi tools or discovery; every request adapts through existing Warp
  permission and action surfaces.
- **Protocol or artifact drift:** exact Core hash, shared fixtures, manifest version/hash/size checks,
  and fail-closed packaging.
- **Secret/content leakage:** post-handshake delivery, minimal environment, fixed errors, bounded
  redacted stderr, same-origin redirect policy, and tests that inject recognizable secret content.
- **Runtime split-brain:** immutable binding, app-wide process ownership, monotonic revision CAS, and
  full reconstruction after invalidation.
