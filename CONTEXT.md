# ZYH

ZYH is a permanent local terminal and agentic development product. It has no
account identity, Anonymous Session, cloud sync, hosted AI, telemetry, Sentry,
automatic update, or background Warp service behavior.

## Product Boundary

**ZYH Local Product**:
The only supported product contract. It includes GUI, TUI, `zyh agent`, local
terminal and file features, user-initiated SSH and Git, explicitly configured
OpenAI-compatible Providers and MCP servers, and the Pi Agent Runtime.
_Avoid_: Local-only Mode, offline mode, accountless mode

**App-initiated Network Request**:
An outbound request created by ZYH or its owned runtime and background services.
Without an Agent Run it must not target an external origin. During an Agent Run
it is limited to the Endpoint Allowlist.
_Avoid_: User command traffic, all process traffic

**User-initiated Network Effect**:
Network activity performed by an explicitly invoked shell command, Git, SSH,
browser open, or third-party CLI. It remains visible and governed by the
invoking workflow but is not an App-initiated Network Request.
_Avoid_: Background request, endpoint exception

**Endpoint Allowlist**:
The selected Provider Origin plus origins of explicitly configured MCP servers
for the current Agent Run. It is empty during normal startup and local terminal
use.
_Avoid_: Warp endpoint list, implicit service discovery

**Legacy Identity State**:
Account, Anonymous Session, Local Identity, team, and permission data retained
only long enough to classify and delete it from a copied legacy database. These
are migration concepts, never active ZYH product states.
_Avoid_: Account Sign-in, Anonymous-only Mode, Local-only Mode

## Remote Access

**Remote Host Shortcut**:
An entry saved on the current device for starting an interactive terminal connection to a remote host. It identifies how to reach the host but does not contain authentication secrets.
_Avoid_: Cloud server login, server account, cloud account

**SSH Center**:
The account-independent settings area where users create, edit, delete, and connect through Remote Host Shortcuts on the current device.
_Avoid_: Shortcut bar, cloud account center

**Remembered SSH Password**:
A password retained locally for a Remote Host Shortcut and kept separate from the shortcut's connection details. It exists only while that shortcut uses password authentication and the operating system provides secure secret storage, and it is never displayed again after storage.
_Avoid_: Saved password, shortcut password, SSH Center password

**SSH Identity**:
An existing private key on the current device selected for a Remote Host Shortcut. SSH Center references its file path but does not own, copy, or retain the key or its passphrase.
_Avoid_: Imported key, SSH Center key, saved key

**SSH Authentication Mode**:
The single authentication strategy selected by a Remote Host Shortcut: system SSH, a Remembered SSH Password, or an SSH Identity. A shortcut never combines or silently falls back between modes.
_Avoid_: Authentication priority, credential fallback

**SSH Host Trust**:
The OpenSSH-managed association between a remote host and its public host key. Saving a Remote Host Shortcut does not establish trust; first use requires confirmation and a later key mismatch blocks connection.
_Avoid_: Trusted shortcut, automatic host acceptance

**SSH Session**:
A live terminal connection started from a snapshot of a Remote Host Shortcut. Once started, it is independent of later edits or deletion of that shortcut.
_Avoid_: Saved connection, active shortcut

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
The Pi process that owns model interaction, context evolution, and the agent loop while delegating tool effects to the Tool Execution Authority. Pi is the only runtime for new Conversations.
_Avoid_: AI backend, Warp Server, Rust runtime

**Runtime Supervisor**:
The ZYH-owned application service that enforces one Agent Runtime process and at most one active Agent Run per Conversation Record, independent of which UI surface displays it.
_Avoid_: View-owned process, controller-owned runtime

**Bridge Protocol**:
The versioned local contract connecting ZYH to Pi for run lifecycle, streamed output, tool requests and results, and cancellation. It transports decisions but grants no execution authority.
_Avoid_: Warp Server API, Pi RPC passthrough

**Protocol Schema**:
The versioned, machine-readable definition and conformance fixtures for the Bridge Protocol wire format. It is authoritative over either side's language-specific protocol types.
_Avoid_: Rust serialization types, TypeScript message types

**Protocol Capability**:
A separately versioned optional Bridge Protocol extension enabled only when its name, schema version, and schema hash match on both sides.
_Avoid_: Core minor version, silently accepted extension

**Bridge Artifact Manifest**:
The ZYH-owned build input that pins the Bridge version and Core Protocol identity plus the checksum and size of each platform artifact.
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
ZYH's durable control metadata for an Agent Run, including its identity, retry lineage, starting transcript revision, and terminal outcome. It contains no duplicate conversation content.
_Avoid_: Conversation Record, Runtime Checkpoint

**Run Configuration**:
The immutable snapshot of Provider settings, catalogs, resources, and execution context supplied by ZYH for one Agent Run. Later configuration changes apply only to later runs; live permission decisions are not part of this snapshot.
_Avoid_: Runtime Binding, live settings feed

**Interactive Agent Conversation**:
A user-initiated Agent conversation whose messages and tool activity are retained in the Conversation Record. Passive suggestions, CLI Agent runs, and Ambient or Cloud Agent runs are outside this concept.
_Avoid_: Passive suggestion, CLI Agent run, Ambient Agent run

**Agent Run Cancellation**:
A ZYH-requested stop of the active Agent Run that preserves the Runtime process and its recoverable state. Process termination is only an escalation when cancellation cannot complete.
_Avoid_: Tool Denial, process shutdown

**Queued Prompt**:
User input held by ZYH until the active Agent Run reaches a terminal outcome, after which it starts a new Agent Run. It never changes or joins the active run.
_Avoid_: Steering message, Pi follow-up

**Tool Execution Authority**:
The ZYH-owned authority that decides whether a requested tool may run and returns its result. The Agent Runtime may request tool use but cannot perform the effect directly.
_Avoid_: Pi tool runner, backend permission layer

**Tool Catalog**:
The ZYH-owned, explicitly scoped inventory of tools that the Agent Runtime may request, including each tool's stable identity and input contract. No tool or tool family is available implicitly outside this catalog.
_Avoid_: Pi tools, discovered tools

**Agent Resource Catalog**:
The ZYH-owned, run-scoped inventory of explicitly selected Skill instructions and project context made available to the Agent Runtime. The runtime cannot discover, select, or activate additional resources on its own.
_Avoid_: Pi resources, auto-discovered Skills

**Resource Snapshot**:
The bounded, model-visible resource content selected for one Agent Run and retained with its initiating input so the same historical context can be reconstructed after the source changes.
_Avoid_: Live file reference, resource cache

**Agent Policy Prompt**:
The versioned Bridge-owned base instructions that define Agent behavior while describing only the tools and resources explicitly supplied by ZYH. Permission enforcement is outside this prompt.
_Avoid_: Pi default prompt, Warp-supplied prompt blob

**Tool Denial**:
A Tool Execution Authority outcome indicating that a requested tool was not run because the user or policy refused it. It is returned to the Agent Runtime as a tool result and does not cancel the Agent Run.
_Avoid_: Agent cancellation, tool failure

**Invalid Tool Request**:
An Agent Runtime tool request whose named tool is unavailable or whose arguments violate the Tool Catalog contract. It is distinct from a Tool Denial because no permission decision or tool execution is attempted.
_Avoid_: Denied tool, failed tool execution

**Tool Request Limit**:
The ZYH-owned maximum number of tool requests accepted within one Agent Run. Reaching it ends that run and requires explicit user input to begin another.
_Avoid_: Model turn limit, resumable pause

**Tool Execution Record**:
The ZYH-owned durable association between an Agent Run-scoped stable tool-call identity, its request, execution state, and outcome when known. Its outcome may be delivered repeatedly to recover a runtime, but its tool effect occurs at most once.
_Avoid_: Pi tool result, acknowledgement cache

**Indeterminate Tool Execution**:
A Tool Execution Record for which ZYH durably accepted the request but cannot prove whether the effect finished before the outcome was recorded. It becomes a model-visible unknown-outcome error and is never executed again automatically.
_Avoid_: Failed tool, pending approval, safe retry

**Tool Result Projection**:
The bounded, model-visible status and Runtime Content Blocks derived once from a Tool Execution Record and retained for deterministic delivery to the Agent Runtime. It does not replace the complete ZYH-owned tool result.
_Avoid_: Stringified tool result, regenerated tool summary

**Conversation Record**:
The ZYH-owned canonical record of user-visible conversation messages, their model-visible input context, and tool activity. It remains authoritative when runtime state is missing or inconsistent.
_Avoid_: Pi session, runtime transcript

**Conversation Record Revision**:
A monotonically increasing ZYH-owned version advanced by each committed change that affects the Runtime Transcript. Agent Runtime work is valid only against the revision on which it started.
_Avoid_: UI render version, Runtime Checkpoint version

**Legacy Rust-bound Conversation**:
A historical Conversation Record created under the removed Rust runtime. It is
view-only. Continuing requires an explicit fork into a new Pi Conversation and
never mutates or rebinds the source record.
_Avoid_: Runtime Binding, runtime fallback, automatic migration

**Runtime Transcript**:
A versioned, provider-neutral projection of a Conversation Record containing only the completed messages, Resource Snapshots, and paired tool activity needed to reconstruct Agent Runtime context. It is derived data and never replaces the Conversation Record.
_Avoid_: Database dump, Pi session file, generated summary

**Transcript Sync**:
The bounded, transactional transfer of a Runtime Transcript into an Agent Runtime. A sync becomes visible only after every item is validated and the complete Conversation Record revision is acknowledged.
_Avoid_: Partial transcript load, single unbounded frame

**Runtime Content Block**:
A bounded text or image value supplied by ZYH as model-visible input or a Tool Outcome. It contains content directly and never grants the Agent Runtime authority to resolve a URL or local file path.
_Avoid_: File reference, arbitrary binary attachment

**Commit Barrier**:
A complete Agent Runtime message boundary at which ZYH must confirm durable incorporation into the Conversation Record before the Agent Run may produce a tool effect, start another model turn, or complete.
_Avoid_: Text-delta acknowledgement, UI render acknowledgement

**Runtime Checkpoint**:
A disposable Pi-owned snapshot used to continue Agent Runtime work efficiently. It may be rebuilt from the Conversation Record and never replaces or rewrites that record.
_Avoid_: Conversation history, source of truth

**Checkpoint Invalidation**:
Discarding a Runtime Checkpoint when a Conversation Record change alters its Runtime Transcript. The next Agent Run starts from a complete projection of the new record rather than patching the old checkpoint.
_Avoid_: Incremental history patch, checkpoint migration

**Context Compaction**:
An Agent Runtime transformation that derives bounded model context from the complete Runtime Transcript projected by ZYH. Its summary belongs to the Runtime Checkpoint and never replaces user-visible history.
_Avoid_: Conversation rewrite, history summarization

**Interrupted Output**:
Partial assistant output retained in the Conversation Record when an Agent Run ends before completing its message. It remains visible to the user but is not treated as a completed assistant message during runtime reconstruction.
_Avoid_: Completed response, discarded stream
