# Local Provider Tool History Repair

## Goal

Keep the Local-only direct Provider path's `Conversation Record` and Chat Completions
projection complete and valid across tool turns and later user prompts.

The confirmed failure is an HTTP 400 from PackyAPI after a successful local tool loop.
The persisted task contains assistant `tool_calls` but no matching `tool` result messages, so
the next Provider Attempt sends an invalid OpenAI-compatible message sequence.

## Evidence

- The failing conversation `4e5fbaeb-8784-4062-8d87-74b4ac170ce3` contains one persisted
  user query, seven assistant outputs, and five tool calls, but zero tool-call results.
- Both configured PackyAPI models return 200 for a minimal request, a minimal function tool,
  and Warp's complete three-tool schema.
- A redacted request containing an assistant tool call followed by a user message, without the
  matching tool result, deterministically returns 400 with
  `insufficient tool messages following tool_calls message`.
- `local_task_from_inputs` persists user input only while creating the first task. Existing-task
  user queries and action results are not added to the `Conversation Record`.

## Change Check

1. This is a real production-path bug, not speculative design: the user's exact Provider status
   and the stored conversation shape were reproduced.
2. The smallest correct fix is to persist supported local inputs and produce a valid message
   projection. Retrying 400 responses, special-casing PackyAPI, or dropping all tools would hide
   the data-loss bug and change unrelated behavior.
3. Existing behavior at risk is message ordering, parallel tool-call grouping, failed-request
   persistence, and compatibility with conversations already written by the buggy build.

## Files Likely Involved

- `app/src/ai/agent/api/local_provider.rs`
- `app/src/ai/agent/api/local_provider/messages.rs`
- `app/src/ai/agent/api/local_provider_tests.rs`
- `app/src/ai/agent/api/local_provider/history_tests.rs`
- `app/src/ai/agent/api/local_provider/persistence_tests.rs`

No generated files, migrations, public API definitions, auth code, billing code, permission code,
or terminal model locks should change.

## Risks

- **Local-only Provider protocol:** Provider-visible message ordering changes. Validate the exact
  Chat Completions payload shape with focused transport tests and a PackyAPI manual check.
- **Conversation Record compatibility:** Existing records may contain unmatched tool calls whose
  results were never stored. Do not mutate persisted history or invent a successful result.
  Omit only unmatched legacy tool calls from the Provider projection, preserving text messages and
  the pre-tool-streaming behavior for those incomplete records.
- **Parallel tool calls:** Adjacent tool calls from one Provider response must be represented as one
  assistant message with multiple `tool_calls`, followed by the matching tool messages. A
  per-message conversion can still create an invalid sequence.
- **Failed Provider Attempts:** Preserve the existing rule that a rejected request does not create
  a new task or persist new input messages.
- **Tool execution:** Do not change permission decisions, execution, cancellation, tool-call
  limits, retries, Provider Origin, or API Key handling.

## Plan

1. Add a focused failing test with a validating fake Provider that rejects the confirmed pattern:
   an assistant tool call not followed by a matching tool result before the next user/assistant
   message.
2. Replace the per-message history conversion with a small sequence builder that:
   - groups tool calls belonging to the same Provider response into one assistant message;
   - emits their matching tool result messages in call order;
   - includes current action results when deciding whether task tool calls are resolved; and
   - omits unmatched legacy tool calls without synthesizing outcomes.
3. Convert supported current local inputs into `warp_multi_agent_api::Message` values. Reuse the
   existing typed action-result conversion so stored `ToolCallResult` values retain their real
   outcome and `tool_call_id`.
4. After the Provider accepts the request and before output actions are emitted:
   - include all supported input messages in `CreateTask` for a new task; or
   - emit `AddMessagesToTask` for an existing task.
   Keep non-success responses free of persistence actions.
5. Teach the Provider projection to consume stored `ToolCallResult` messages, so a later user turn
   reconstructs the same valid assistant-tool exchange from the `Conversation Record` alone.
6. Add regression coverage for sequential tools, parallel tools, later user prompts, legacy
   dangling tool calls, first-task creation, and rejected Provider Attempts.

## Validation

- Unit tests:
  - the confirmed dangling-tool history fails before the fix and passes after it;
  - current action results are persisted as typed tool-call-result messages;
  - a later request rebuilds `assistant.tool_calls -> tool` pairs from persisted history;
  - parallel tool calls are grouped into one assistant message and paired in order;
  - unmatched legacy calls are omitted without dropping adjacent assistant text;
  - an HTTP 400 emits no input-persistence client action.
- Integration tests: use the existing injected `LocalProviderTransport` seam; no GUI integration
  test is needed because the defect is entirely in request projection and response client actions.
- Manual checks:
  - start a new PackyAPI conversation, execute a local tool, then ask `你是谁`;
  - repeat in the existing affected conversation to verify compatibility projection;
  - confirm only the configured Provider Origin is contacted.
- Commands to run:
  - `cargo test --package warp --lib ai::agent::api::local_provider -- --nocapture`
  - `./script/format --check`
  - `./scripts/check.sh`

## Validation Results

- `cargo test --package warp --lib ai::agent::api::local_provider -- --nocapture` passed all
  36 focused tests, including reused tool-call IDs and the local tool-call-limit path.
- Scoped `rustfmt --check` passed for every changed Rust file.
- The repository-wide format check is blocked by existing rustfmt differences in unrelated GUI
  files; applying them would produce a large out-of-scope rewrite.
- The full `warp` library run completed with 5,581 passed, 318 failed, and 6 ignored tests. The
  failures are outside the Local Provider modules and are dominated by missing GUI test runtime
  registration, locale-dependent assertions, and local test-environment assumptions.
- The manual PackyAPI conversation checks remain for the installed application after rebuilding.

## Rollback

Revert the local input persistence and sequence-builder changes together. No schema or stored-data
rollback is required because the added `ToolCallResult` messages already use the existing
`Conversation Record` protobuf shape. Records written after the fix remain readable by older code,
although older code will continue to ignore their result messages in the direct Provider
projection.
