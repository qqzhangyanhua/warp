---
status: accepted
date: 2026-07-30
issue: 44
amends: [0009-adopt-the-permanent-zyh-local-product.md]
---

# Allow Explicit Personal Memory Embedding Requests

ADR-0009 permits app-initiated external traffic during an Agent Run only to
the selected Chat Completions Provider Origin and explicitly configured MCP
origins. Personal Memory adds an optional, separately configured Embedding
Provider. It also needs user-initiated connection-test and index-rebuild
actions in Settings, which are not Agent Runs. Without a narrow amendment,
those requests would violate the permanent ZYH Local Product network boundary.

## Decision

ZYH may contact a configured Embedding Provider Origin in exactly two scopes:

1. During an Agent Run whose current user input produced an explicit Memory
   Capability, and only when the authorized memory operation requires an
   embedding.
2. During an explicit Settings action that the user starts to test the
   Embedding Provider connection or rebuild the Personal Memory Index.

For the first scope, the run-scoped Endpoint Allowlist contains the selected
Chat Completions Provider Origin, the selected Embedding Provider Origin only
while servicing the Memory Capability, and any explicitly configured MCP
origins. The Agent Runtime receives neither the Embedding API Key nor direct
network authority; the ZYH-owned Personal Memory service performs the bounded
request.

For the second scope, the action-scoped allowlist contains only the configured
Embedding Provider Origin. Saving or opening Settings does not start a test or
rebuild. A rebuild remains a visible, cancelable consequence of the explicit
action; it is not persisted as background work and does not resume by itself
after restart.

Both scopes apply the existing Provider safety contract:

- The Base URL determines one Provider Origin. The API Key may be sent only to
  that origin, and redirects must remain same-origin.
- HTTPS is preferred. HTTP remains supported for self-hosted services only
  with the existing plaintext transport warning.
- The API Key is stored in operating-system secure storage and never enters
  SQLite, ordinary settings, exports, logs, diagnostics, Tool Result
  Projections, or Bridge messages.
- Requests, retries, redirects, response sizes, vector dimensions, item
  counts, and per-request content are bounded. A rebuild may issue multiple
  bounded requests, but never uploads the complete store as one payload.
- Errors expose only stable redacted categories. Personal Memory facts,
  values, topics, labels, queries, vectors, Provider response bodies, and API
  Keys are excluded from telemetry, logs, and diagnostics.

There is no startup, restoration, idle, timer, periodic, configuration-save,
or other background Embedding request. A bounded retry may remain inside the
same explicit operation. A pending index is retried only by a later explicit
memory operation or another explicit Rebuild Index action.

Exact and full-text Personal Memory operations remain local and available
without an Embedding Provider. Provider failure cannot roll back or delete a
canonical Personal Memory Record that was otherwise committed successfully.

## Consequences

ADR-0009's permanent local-product contract remains unchanged except for the
two explicit Embedding Provider request scopes above. Settings connection
tests and rebuilds are app-initiated network requests, but they are direct,
visible user actions with a single-origin allowlist rather than background
service behavior.

The Endpoint Allowlist implementation must represent the Embedding Provider
Origin separately from the Chat Completions Provider Origin. Tests must prove
that the origin is absent when no Memory Capability exists and outside an
active explicit Settings action.

Personal Memory indexing is operationally degraded when the Provider is
missing or unavailable. Canonical records and local exact retrieval remain
authoritative; the Personal Memory Index is disposable derived data.

## Alternatives Rejected

- Reusing the Chat Completions Provider configuration was rejected because a
  chat model does not imply an embeddings model or protocol.
- Giving the Agent Runtime the Embedding API Key was rejected because ZYH owns
  Provider Origin enforcement and Personal Memory effects.
- Startup indexing, timers, and automatic background retries were rejected
  because they violate the permanent ZYH network boundary.
- Requiring a bundled local embedding model was rejected because Personal
  Memory exact retrieval must work without model downloads or another runtime.

## Approval

The repository owner approved this ADR and the issue #44 high-risk plan on
2026-07-30. Production implementation under issues #45-#53 may rely only on
the explicit scopes and constraints recorded above.
