# Warp

Warp is a terminal and agentic development environment that can operate without a user account while retaining a non-account identity for supported services.

## Identity

**Account Sign-in**:
A user-initiated flow that associates Warp with a persistent personal or organization account.
_Avoid_: Login, authentication

**Anonymous Session**:
A non-account identity that lets a user access supported Warp features without Account Sign-in.
_Avoid_: Logged-out user, guest account

**Anonymous-only Mode**:
A product mode in which every user operates through an Anonymous Session and Account Sign-in is unavailable.
_Avoid_: Logged-out mode, temporary mode

**Local-only Mode**:
A build-gated product mode in which Warp uses a local identity only, does not create or refresh an Anonymous Session, and does not use Warp account, cloud sync, telemetry, Sentry, or background Warp network services. Local-only Mode is intended for OpenAI-compatible Provider users and is distinct from Anonymous-only Mode.
_Avoid_: Offline mode, logged-out mode, anonymous mode

**Local Identity**:
A stable UUID-backed identity stored in local preferences for Local-only Mode. It is not an account, token, or Anonymous Session.
_Avoid_: Anonymous ID, user account, auth token

## AI Providers

**OpenAI-compatible Provider**:
A user-configured AI provider identified by a Base URL, Model, and API Key and accessed through the OpenAI API protocol.
_Avoid_: Warp-managed model, account model

**Chat Completions Provider**:
An OpenAI-compatible Provider that supports streaming model output and function tools through the `/chat/completions` protocol.
_Avoid_: Responses API Provider, auto-detected Provider protocol

**Provider Origin**:
The scheme, host, and effective port of an OpenAI-compatible Provider Base URL. It is the only network origin to which that Provider's API Key may be sent.
_Avoid_: Provider path, model identity

## Agent Execution

**Agent Runtime**:
The local execution environment that owns model interaction, context evolution, and the agent loop while delegating tool effects to the Tool Execution Authority.
_Avoid_: AI backend, Warp Server

**Runtime Supervisor**:
The Warp-owned application service that enforces one Agent Runtime process and at most one active Agent Run per Conversation Record, independent of which UI surface displays it.
_Avoid_: View-owned process, controller-owned runtime

**Bridge Protocol**:
The versioned local contract connecting Warp to the Agent Runtime for run lifecycle, streamed output, tool requests and results, and cancellation. It transports decisions but grants no execution authority.
_Avoid_: Warp Server API, Pi RPC passthrough

**Protocol Schema**:
The versioned, machine-readable definition and conformance fixtures for the Bridge Protocol wire format. It is authoritative over either side's language-specific protocol types.
_Avoid_: Rust serialization types, TypeScript message types

**Protocol Capability**:
A separately versioned optional Bridge Protocol extension enabled only when its name, schema version, and schema hash match on both sides.
_Avoid_: Core minor version, silently accepted extension

**Bridge Artifact Manifest**:
The Warp-owned build input that pins the Bridge version and Core Protocol identity plus the checksum and size of each platform artifact.
_Avoid_: Latest Bridge lookup, runtime updater

**Agent Run**:
One Agent Runtime execution initiated by user input or an explicit Retry action that may span multiple model turns and tool requests before reaching a terminal outcome.
_Avoid_: Agent request, model call

**Provider Attempt**:
One request from an Agent Run to its configured Provider. A bounded automatic retry creates another Provider Attempt inside the same Agent Run rather than another run or conversation message.
_Avoid_: Agent Run, user retry

**Retry Run**:
A new, user-authorized Agent Run that retries a previous run from the latest committed Runtime Transcript without adding or duplicating a conversation message.
_Avoid_: Provider Attempt, automatic resume, Continue

**Agent Run Record**:
Warp's durable control metadata for an Agent Run, including its identity, retry lineage, starting transcript revision, and terminal outcome. It contains no duplicate conversation content.
_Avoid_: Conversation Record, Runtime Checkpoint

**Run Configuration**:
The immutable snapshot of Provider settings, catalogs, resources, and execution context supplied by Warp for one Agent Run. Later configuration changes apply only to later runs; live permission decisions are not part of this snapshot.
_Avoid_: Runtime Binding, live settings feed

**Interactive Agent Conversation**:
A user-initiated Agent conversation whose messages and tool activity are retained in the Conversation Record. Passive suggestions, CLI Agent runs, and Ambient or Cloud Agent runs are outside this concept.
_Avoid_: Passive suggestion, CLI Agent run, Ambient Agent run

**Agent Run Cancellation**:
A Warp-requested stop of the active Agent Run that preserves the Runtime process and its recoverable state. Process termination is only an escalation when cancellation cannot complete.
_Avoid_: Tool Denial, process shutdown

**Queued Prompt**:
User input held by Warp until the active Agent Run reaches a terminal outcome, after which it starts a new Agent Run. It never changes or joins the active run.
_Avoid_: Steering message, Pi follow-up

**Tool Execution Authority**:
The Warp-owned authority that decides whether a requested tool may run and returns its result. The Agent Runtime may request tool use but cannot perform the effect directly.
_Avoid_: Pi tool runner, backend permission layer

**Tool Catalog**:
The Warp-owned, explicitly scoped inventory of tools that the Agent Runtime may request, including each tool's stable identity and input contract. No tool or tool family is available implicitly outside this catalog.
_Avoid_: Pi tools, discovered tools

**Agent Resource Catalog**:
The Warp-owned, run-scoped inventory of explicitly selected Skill instructions and project context made available to the Agent Runtime. The runtime cannot discover, select, or activate additional resources on its own.
_Avoid_: Pi resources, auto-discovered Skills

**Resource Snapshot**:
The bounded, model-visible resource content selected for one Agent Run and retained with its initiating input so the same historical context can be reconstructed after the source changes.
_Avoid_: Live file reference, resource cache

**Agent Policy Prompt**:
The versioned Bridge-owned base instructions that define Agent behavior while describing only the tools and resources explicitly supplied by Warp. Permission enforcement is outside this prompt.
_Avoid_: Pi default prompt, Warp-supplied prompt blob

**Tool Denial**:
A Tool Execution Authority outcome indicating that a requested tool was not run because the user or policy refused it. It is returned to the Agent Runtime as a tool result and does not cancel the Agent Run.
_Avoid_: Agent cancellation, tool failure

**Invalid Tool Request**:
An Agent Runtime tool request whose named tool is unavailable or whose arguments violate the Tool Catalog contract. It is distinct from a Tool Denial because no permission decision or tool execution is attempted.
_Avoid_: Denied tool, failed tool execution

**Tool Request Limit**:
The Warp-owned maximum number of tool requests accepted within one Agent Run. Reaching it ends that run and requires explicit user input to begin another.
_Avoid_: Model turn limit, resumable pause

**Tool Execution Record**:
The Warp-owned durable association between an Agent Run-scoped stable tool-call identity, its request, execution state, and outcome when known. Its outcome may be delivered repeatedly to recover a runtime, but its tool effect occurs at most once.
_Avoid_: Pi tool result, acknowledgement cache

**Indeterminate Tool Execution**:
A Tool Execution Record for which Warp durably accepted the request but cannot prove whether the effect finished before the outcome was recorded. It becomes a model-visible unknown-outcome error and is never executed again automatically.
_Avoid_: Failed tool, pending approval, safe retry

**Tool Result Projection**:
The bounded, model-visible status and Runtime Content Blocks derived once from a Tool Execution Record and retained for deterministic delivery to the Agent Runtime. It does not replace the complete Warp-owned tool result.
_Avoid_: Stringified tool result, regenerated tool summary

**Conversation Record**:
The Warp-owned canonical record of user-visible conversation messages, their model-visible input context, and tool activity. It remains authoritative when runtime state is missing or inconsistent.
_Avoid_: Pi session, runtime transcript

**Conversation Record Revision**:
A monotonically increasing Warp-owned version advanced by each committed change that affects the Runtime Transcript. Agent Runtime work is valid only against the revision on which it started.
_Avoid_: UI render version, Runtime Checkpoint version

**Runtime Binding**:
The immutable association between a Conversation Record and the Agent Runtime implementation that owns all of its Agent Runs. Changing product configuration does not migrate or replace this association.
_Avoid_: Runtime fallback, per-request runtime selection

**Runtime Selection Policy**:
The Warp-owned rule that chooses a Runtime Binding when a new Conversation Record is created. It is rollout policy, not a per-request or general user preference.
_Avoid_: Runtime selector, Provider setting

**Runtime Transcript**:
A versioned, provider-neutral projection of a Conversation Record containing only the completed messages, Resource Snapshots, and paired tool activity needed to reconstruct Agent Runtime context. It is derived data and never replaces the Conversation Record.
_Avoid_: Database dump, Pi session file, generated summary

**Transcript Sync**:
The bounded, transactional transfer of a Runtime Transcript into an Agent Runtime. A sync becomes visible only after every item is validated and the complete Conversation Record revision is acknowledged.
_Avoid_: Partial transcript load, single unbounded frame

**Runtime Content Block**:
A bounded text or image value supplied by Warp as model-visible input or a Tool Outcome. It contains content directly and never grants the Agent Runtime authority to resolve a URL or local file path.
_Avoid_: File reference, arbitrary binary attachment

**Commit Barrier**:
A complete Agent Runtime message boundary at which Warp must confirm durable incorporation into the Conversation Record before the Agent Run may produce a tool effect, start another model turn, or complete.
_Avoid_: Text-delta acknowledgement, UI render acknowledgement

**Runtime Checkpoint**:
A disposable Pi-owned snapshot used to continue Agent Runtime work efficiently. It may be rebuilt from the Conversation Record and never replaces or rewrites that record.
_Avoid_: Conversation history, source of truth

**Checkpoint Invalidation**:
Discarding a Runtime Checkpoint when a Conversation Record change alters its Runtime Transcript. The next Agent Run starts from a complete projection of the new record rather than patching the old checkpoint.
_Avoid_: Incremental history patch, checkpoint migration

**Context Compaction**:
An Agent Runtime transformation that derives bounded model context from the complete Runtime Transcript projected by Warp. Its summary belongs to the Runtime Checkpoint and never replaces user-visible history.
_Avoid_: Conversation rewrite, history summarization

**Interrupted Output**:
Partial assistant output retained in the Conversation Record when an Agent Run ends before completing its message. It remains visible to the user but is not treated as a completed assistant message during runtime reconstruction.
_Avoid_: Completed response, discarded stream
