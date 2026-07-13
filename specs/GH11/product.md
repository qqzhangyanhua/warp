# GH11: Local Pi Agent Runtime

GitHub: [#11](https://github.com/qqzhangyanhua/warp/issues/11)

## Summary

Local-only Mode uses Pi as the Agent Runtime for eligible new Interactive Agent Conversations while
Warp remains the canonical conversation owner, permission authority, tool executor, Provider
configuration owner, and user interface. The result must retain local Provider functionality without
restoring any dependency on Warp identity, hosted AI, quota, sync, telemetry, Sentry, or cloud
services.

## Problem

The direct Local Provider path is beginning to reproduce a second agent loop in Warp. A local runtime
also creates recovery hazards: retrying after a crash may duplicate a destructive tool effect,
partially committed output may be mistaken for complete context, and a child process with ambient
tools could bypass Warp's existing permission model.

## Goals

- Provide capable local Agent conversations through the configured OpenAI-compatible Provider.
- Keep Warp's existing permission and conversation behavior authoritative.
- Make process, application, and persistence failures recover conservatively and deterministically.
- Preserve existing conversations and all non-Local-only Agent paths.

## Non-goals

- Pi built-in tools, Extensions, credential discovery, Provider discovery, or resource discovery.
- Runtime selection as a general user preference or in-place conversion of existing conversations.
- Passive suggestions, CLI Agent, Ambient Agent, Cloud Agent, or Agent SDK execution through Pi.
- Parallel tool execution, durable Pi checkpoints, runtime downloads, or runtime self-updates.
- Claiming that the trusted bundled Bridge is contained by a cross-platform OS sandbox.

## Behavior

1. Only a newly created Interactive Agent Conversation in Local-only Mode may bind to Pi, and only
   when rollout policy enables it and the configured Provider supports Chat Completions.

2. A Conversation Record's Runtime Binding is immutable. Changing rollout configuration affects only
   later conversations and never moves a conversation between Pi and Rust.

3. Existing Conversation Records without a Runtime Binding remain readable and Rust-bound at
   revision zero. Account, Anonymous-only, CLI, Ambient, Cloud, Agent SDK, and passive-suggestion
   paths retain their current behavior.

4. A Pi-bound Conversation Record has at most one dedicated Agent Runtime process and one active
   Agent Run across all GUI and TUI views. Closing or moving a view does not create a second process.

5. User input submitted during an active Agent Run is either queued by Warp for a later run or
   explicitly cancels the active run before starting another; it never steers the active Pi run.

6. Text appears incrementally while the Provider responds. A complete assistant message becomes
   runtime context only after Warp durably commits it.

7. If a run stops during text streaming, visible partial text remains as Interrupted Output but is
   excluded from completed runtime context.

8. Automatic Provider retry is bounded to one additional attempt for approved pre-output failures.
   Once any model output is emitted, failure ends the run without automatic continuation.

9. Retry is always user-authorized. A Retry Run gets a new run identity and lineage, starts from the
   latest committed Runtime Transcript, and does not duplicate the original user message.

10. Cancelling a run stops pending approval and work that has not begun while preserving recoverable
    runtime state when possible. An unresponsive runtime is terminated after a bounded grace period.

11. Pi can request only tools in the immutable Tool Catalog supplied for that run. The initial
    catalog is limited to `run_shell_command`, `read_files`, `apply_file_diffs`, and MCP tools already
    configured by Warp.

12. Every tool request returns to Warp. Existing Warp permission surfaces make the live decision and
    Warp performs the effect; the Bridge never executes a Warp tool directly.

13. Tool Denial is a structured result, not cancellation. An unavailable tool or schema-invalid
    arguments become an Invalid Tool Request without permission or execution.

14. Every accepted request has a stable identity scoped to its conversation and run. Identical
    redelivery reuses the recorded outcome; reuse for a different request fails the run.

15. Warp durably records `pending` before permission and `executing` before invoking a tool effect.
    The complete outcome, fixed Tool Result Projection, task update, and revision commit atomically
    before Pi receives the result.

16. If Warp cannot prove whether an effect completed, recovery records an Indeterminate Tool
    Execution with `tool_outcome_unknown` and `may_have_executed: true`. It is visible to the user and
    the next Retry Run, ends the current run, and is never replayed automatically.

17. An Agent Run accepts at most 32 tool requests. Requests beyond the remaining budget do not
    execute, and the run ends with the distinct `limit_reached` outcome rather than Provider quota.

18. Warp remains the sole Provider configuration and API Key owner. No secret or conversation
    content is sent until the Bridge Core version, schema hash, and required capabilities match.

19. The API Key is sent only to the configured Provider Origin. Redirects are limited to three hops
    within the same scheme, host, and effective port. Existing HTTP Provider support retains its
    warning and is not presented as secure.

20. Run Configuration is immutable for one Agent Run and contains only that run's Provider settings,
    tool and resource catalogs, working directory, context limit, and reasoning settings. Permission
    policy is evaluated live and is not copied into it.

21. Only explicitly selected rules, attachments, project context, and Skill instructions are
    supplied as bounded Resource Snapshots. Historical runtime context uses the retained snapshot and
    never rereads a mutable source.

22. Runtime Content Blocks contain bounded inline text or image data. URLs, arbitrary binaries, and
    local path references are rejected rather than resolved by the Bridge.

23. Warp's Conversation Record is canonical. Runtime Checkpoints are disposable acceleration state
    and may be rebuilt from a provider-neutral Runtime Transcript of completed messages, Resource
    Snapshots, and paired tool activity.

24. Transcript Sync is bounded and transactional. An interrupted, out-of-order, incomplete, or
    invalid sync leaves the previously accepted revision active; a run cannot start until the complete
    revision is acknowledged.

25. Every Runtime Transcript mutation crosses a Commit Barrier. A stale expected revision fails
    without overwriting newer history and forces complete runtime reconstruction.

26. Re-delivery of a committed mutation identity returns its existing committed revision without
    duplicating conversation content or tool activity.

27. Deleting or evicting a Conversation Record removes its Agent Run Records and Tool Execution
    Records in the same transaction. A failed deletion leaves the conversation and sidecars
    consistent.

28. Bridge and Warp diagnostics use fixed error categories and diagnostic identities. They exclude
    prompts, output, tool arguments/results, commands, file content, credentials, raw Provider
    responses, stack traces, and unredacted endpoints.

29. Bridge absence, incompatibility, or startup failure keeps a Pi-bound conversation viewable and
    never silently runs it through Rust. Runtime conversion requires an explicit fork of completed
    history into a new conversation.

30. Local-only Mode continues to contact only its configured Provider Origin and explicitly
    configured MCP origins. It does not create or refresh Warp identity or use Warp AI, quota, cloud
    sync, telemetry, Sentry, or background Warp network services.

31. Release builds contain exactly one verified standalone Bridge artifact for their target and do
    not require a user-installed Node, Bun, or Pi. Installed applications never download or update
    the Bridge at runtime.

32. The supported artifact matrix is macOS arm64/x86_64, Linux arm64/x86_64, and Windows
    arm64/x86_64. A release build fails when its artifact, manifest identity, checksum, or size does
    not match.
