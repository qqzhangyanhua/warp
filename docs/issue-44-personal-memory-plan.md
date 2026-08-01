# Issue #44 Personal Memory High-Risk Implementation Plan

## Review Gate

Status: approved by the repository owner on 2026-07-30.

This plan and ADR-0010 were approved before production implementation for
issues #45-#53 began. Issue #44 changes documentation only.

## Goal

Implement the approved Personal Memory V1 contract from issue #43 in small,
dependency-ordered slices. Preserve exact user-authored facts in one global
local SQLite source of truth; expose only explicit, run-scoped memory tools;
and permit optional semantic retrieval without weakening ZYH's network,
secret-storage, recovery, or diagnostic boundaries.

## Scope Decisions

- Personal Memory Records are canonical. Exact/full-text keys and semantic
  vectors are disposable Personal Memory Index data.
- One application-owned Personal Memory service is the only interface used by
  Agent tools, Settings, and import/export. The Agent Runtime, Bridge, and UI
  do not access Personal Memory SQLite tables directly.
- Only explicit user requests create a Memory Capability. Ordinary prompts
  expose no Personal Memory tools, and a read capability never implies a
  mutation capability.
- Exact user-authored fact and value spans are copied from the initiating user
  message. Model-proposed text cannot silently replace an identifier.
- The store is global, local, plaintext, permanent until explicit deletion,
  and limited to 5,000 active records. There is no encryption, expiry, cloud
  sync, telemetry, secret classification, or automatic memory extraction.
- Exact and full-text operations work without an Embedding Provider. Semantic
  search uses only a separately configured OpenAI-compatible
  `/v1/embeddings` Provider.
- At most five selected records, under a separate content-size budget, enter a
  Tool Result Projection and may reach the Chat Completions Provider.
- Document RAG and command-history semantic search remain out of scope.

## Files Likely Involved

Issue #44 documentation:

- `CONTEXT.md`
- `docs/adr/0010-allow-explicit-personal-memory-embedding-requests.md`
- `docs/issue-44-personal-memory-plan.md`

Canonical persistence and service, beginning in #45:

- `crates/persistence/migrations/<timestamp>_create_personal_memory/{up,down}.sql`
- `crates/persistence/src/schema.rs` through Diesel generation, never by hand
- `crates/persistence/schema.patch` only through the documented schema workflow
- `app/src/persistence/sqlite.rs`
- `app/src/persistence/commands.rs`
- `app/src/persistence/mod.rs`
- new `app/src/persistence/personal_memory.rs` and separate test modules
- new `app/src/ai/personal_memory/` service, types, retrieval, and tests

Agent capability and effect boundary:

- `app/src/ai/agent/runtime/service.rs`
- `app/src/ai/agent/runtime/configuration.rs`
- `app/src/ai/agent/runtime/tool_catalog.rs`
- `app/src/ai/agent/runtime/tool_execution.rs`
- `app/src/ai/agent/runtime/tool_execution/request.rs`
- `app/src/ai/agent/runtime/tool_execution/blocklist_adapter.rs`
- `app/src/ai/agent/runtime/tool_execution/{types,recovery,projection}.rs`
- adjacent Runtime Supervisor, Tool Catalog, execution, crash, and recovery
  test modules

Provider configuration, transport, and Settings:

- `crates/ai/src/api_keys.rs` and `crates/ai/src/api_keys_tests.rs`
- a new ZYH-owned Embedding Provider transport module under
  `app/src/ai/personal_memory/`
- `app/src/settings_view/ai_page.rs`
- `app/src/settings_view/custom_inference_connection_test.rs` as the
  interaction and error-taxonomy pattern, not as a shared chat protocol
- new Personal Memory Settings page, modal, dialogs, and separate test modules
- the settings and local workspace-metadata owners identified in #48
- the `warp_features` registry re-exported by
  `crates/warp_core/src/features.rs`, plus Command Palette registrations

Integration and boundary validation:

- `app/src/ai/agent/runtime/tool_run_integration_tests.rs`
- `app/src/ai/agent/runtime/text_run_integration_tests.rs`
- `crates/integration/src/bin/integration.rs`
- `crates/integration/src/test/`
- `crates/integration/tests/integration/`
- existing startup request-recording and isolated `ZYH_HOME` test support

Before #45 edits the Tool Catalog path must be prefactored deliberately. Today
`ToolCatalog::resolve` produces a protobuf tool and `typed_action` converts it
to `AIAgentAction`. Prefer an internal resolved-tool enum with a Personal Memory
variant so the Tool Execution Authority can route memory effects directly to
the service without granting SQLite access to the Agent Runtime or expanding a
public protobuf solely for local dispatch. Confirm that persisted tool request
and Conversation Record reconstruction can retain the stable tool identity and
bounded projection before freezing this choice. If that cannot be done without
a protocol or public API change, stop and amend this plan for review.

## Risks

- Database migration: startup applies SQLite migrations to user data. The
  migration must be additive, transactional, reversible on a disposable copy,
  and preserve terminal and Conversation data. Generated schema files must use
  the documented Diesel flow.
- Persistence ownership: storing vectors or full-text rows as canonical would
  make Provider changes or partial rebuilds destructive. Foreign keys and
  transactions must make derived-state deletion complete.
- At-most-once effects: a memory mutation can outlive a lost acknowledgement.
  It must reuse Tool Execution Records and stable mutation identities rather
  than introduce a second replay mechanism.
- Permissions and capability: a malicious or mistaken Provider tool request
  must fail as an Invalid Tool Request before any store access, permission UI,
  or Embedding request.
- Network boundary: the Embedding Provider is a second origin and explicit
  Settings actions occur outside an Agent Run. ADR-0010, origin isolation,
  same-origin redirects, and no-background tests are mandatory.
- Secrets: the Embedding API Key must be committed to operating-system secure
  storage before non-secret configuration references it and must never enter
  SQLite, ordinary settings, Bridge data, exports, snapshots, or diagnostics.
- Privacy: facts, queries, labels, values, vectors, and Provider bodies are
  content. Logs and errors may contain only redacted categories and opaque
  diagnostic identities.
- Prompt injection: remembered content is untrusted tool data. It cannot alter
  the Agent Policy Prompt, Run Configuration, Tool Catalog, permissions, or
  subsequent tool arguments.
- GUI rollout: one high-level feature flag must hide both Settings and runtime
  tools. Disabling the user setting preserves data; clearing is a separate
  confirmed destructive action.
- Performance: ranking must remain bounded at 5,000 records. Remote Provider
  latency is measured separately from the local 300 ms p95 target.
- Public compatibility: avoid Bridge Protocol and public protobuf changes. Any
  unavoidable schema or public API change requires a reviewed plan amendment.

Auth and billing are not changed. Permissions, migrations, secure storage, the
Provider Origin boundary, and possibly persisted tool representations are
high-risk and receive focused tests before the standard repository check.

## Test Seams

The primary end-to-end seam is a GUI Interactive Agent Conversation launched
with an isolated `ZYH_HOME`, real disposable SQLite, controllable fake Bridge,
mock Chat Completions Provider, and mock Embeddings Provider. It must observe
user-visible receipts, verbatim recall, source affordances, Settings state,
record durability, Provider traffic, and deletion rather than private helper
calls.

Focused seams:

- Personal Memory service over real disposable SQLite for canonical records,
  derived indexes, conflicts, defaults, capacity, and transactions.
- Runtime Supervisor plus fake Bridge for capability exposure, strict tool
  schemas, Commit Barriers, acknowledgement loss, cancellation, and recovery.
- Mock HTTP Providers plus request recorder for URL normalization, bounded
  payloads, redirects, retries, response validation, redaction, and the
  Endpoint Allowlist.
- Secure-storage test doubles for API Key write/read/delete ordering and
  absence from all non-secret persistence and export surfaces.
- Settings view tests for enablement, management, rebuild state, destructive
  confirmations, workspace controls, and Command Palette parity.

Tests should assert durable state and externally visible behavior. They should
not lock in helper names, collection types, row ordering without a contract, or
incidental rendering structure.

## Crash And Acknowledgement Matrix

| Boundary | Durable state on recovery | Required behavior |
| --- | --- | --- |
| Before mutation record is accepted | No mutation identity or fact change | Return failure; a later explicit request may try again. |
| After pending Tool Execution Record, before `executing` | Pending request, no effect | Resume permission/execution only under the existing recovery contract. |
| After `executing`, before Personal Memory transaction | `executing`, no provable effect | Produce Indeterminate Tool Execution; never replay automatically. |
| After canonical mutation, before derived-index update in the same transaction | Transaction rolls back | No partial record/index state becomes visible. |
| After canonical record commits but Embedding request fails | Record committed, semantic state pending | Return a durable degraded receipt; exact retrieval works. |
| After memory effect, before Tool Outcome commit | Effect may exist, `executing` remains | Produce outcome unknown; direct the user to inspect state; never replay. |
| After Tool Outcome commit, before Bridge acknowledgement | Stable mutation identity and result committed | Redelivery returns the same result without a second effect. |
| Same identity with changed payload | Original fingerprint/result committed | Reject as an identity/protocol conflict; do not mutate. |
| During delete transaction | Canonical and every derived row commit together or roll back | Deleted content is either wholly present or wholly absent. |
| During rebuild before activation | Old identity invalid/unavailable; candidate index incomplete | Exact/full-text remain available; semantic search remains unavailable. |
| After complete rebuild, before activation acknowledgement | New index atomically active | Repeated acknowledgement reports the active identity; no rebuild replay. |
| Restart with pending index rows | Canonical records plus pending status | Make no request; wait for a later explicit operation or rebuild. |

## Phased Plan

1. #45, exact vertical slice: add the feature flag, additive canonical schema,
   application-owned service, explicit create/query Memory Capabilities,
   verbatim recall, committed source affordance, basic management surface, and
   restart coverage without any Embedding Provider.
2. #46, semantic vertical slice: add separate secure Embedding Provider
   configuration, `/v1/embeddings` transport, connection test, origin safety,
   one compatible derived index, and exact-before-semantic recall.
3. #47, fact model completeness: add duplicate handling, typed conflicts,
   replace-or-keep-both, labels, multiple accounts, explicit defaults, and
   management corrections without hidden history.
4. #48, availability controls: add global enable/disable, Command Palette
   parity, global-by-default behavior, local per-workspace read disablement,
   multi-window consistency, and worktree-clean persistence.
5. #49, index lifecycle: preserve canonical writes through Provider failure,
   expose pending state, invalidate incompatible identities, and implement a
   visible explicit atomic rebuild with no background requests.
6. #50, safe mutation: add conversational and Settings update/delete, choices
   for ambiguity, short undo, confirmed bulk deletion, permanent derived-state
   cleanup, and crash/acknowledgement-loss coverage.
7. #51, boundary hardening: prove least-capability tools, bounded top-five
   projection, untrusted-content handling, Provider payload restrictions,
   origin enforcement, no telemetry, content-free diagnostics, and network
   silence outside explicit operations.
8. #52, portability and capacity: add versioned plaintext JSON export/import,
   vector and secret exclusion, prevalidation and transaction rollback,
   explicit conflict handling, rebuild-required state, and the 5,000-record
   limit without eviction.
9. #53, final acceptance: run the 50-case bilingual evaluation, exact and
   no-match correctness gates, semantic quality gate, 5,000-record local p95
   benchmark, complete isolated GUI workflow, Provider traffic assertions, and
   all regression gates.

Each issue remains a reviewable tracer bullet and must preserve the completed
behavior of earlier issues. Do not combine phases merely to bypass a blocking
edge.

## Validation

For #44:

- `git diff --check`
- inspect Markdown headings, relative links, and ADR frontmatter
- compare the diff against every #44 acceptance criterion
- no production tests are required because #44 changes documentation only

For each later slice:

- run the focused Rust unit tests for every changed persistence, service,
  runtime, Settings, and secure-storage module
- run migration upgrade, rollback/redo, idempotent startup, constraint, and
  preservation tests against disposable databases whenever schema changes
- run the relevant Runtime Supervisor/fake Bridge integration tests whenever
  Tool Catalog, Tool Execution Authority, or recovery changes
- run Provider transport and request-recorder tests whenever Embedding or
  network behavior changes
- run the targeted GUI integration workflow whenever user-visible Settings or
  conversation behavior changes; report unavailable real-display checks
- run `./script/format --check`
- run `./scripts/check.sh` before every PR-ready handoff

For the final slice also run the fixed bilingual evaluation and documented
5,000-record benchmark, recording the controlled model and development
baseline. Remote Embedding latency is reported separately.

## Rollback

- The rollout feature flag disables every Personal Memory UI and Tool Catalog
  entry together. User disablement also stops reads and writes but does not
  delete canonical data.
- Before release, a failing slice may revert its code and its migration on a
  disposable development database. Never run a down migration against a
  user's database as an automated rollback.
- After a migration ships, preserve the additive tables and make later code
  tolerate them. Roll back behavior with the feature flag or a forward fix,
  not by deleting user records.
- Embedding Provider or index failures roll back only derived index activation.
  Keep canonical records and exact retrieval intact.
- A public protocol change is not part of this plan. If one becomes necessary,
  stop and obtain approval for a versioning and artifact rollback plan before
  editing the schema.
