# Local-only Custom Provider Mode - Product Shell

GitHub: [#2](https://github.com/qqzhangyanhua/warp/issues/2)

## Summary

Local-only Mode is a build-gated product mode for users who configure their own
OpenAI-compatible Provider and do not use Warp accounts, Anonymous Sessions, or
Warp-hosted AI services. This increment establishes the product shell and
network boundary only: GUI and TUI must reach a usable local terminal without
Account Sign-in, CLI identity commands must be deterministic and local, and
cloud/account-only surfaces must not initialize in this mode.

Provider configuration and direct Chat Completions conversations are handled by
later issues.

## Behavior

1. When `LocalOnlyCustomProviderMode` is enabled, Warp starts without Account
   Sign-in and without creating an Anonymous Session.
2. The GUI root view opens the local terminal/workspace directly.
3. The TUI root view enters the terminal session directly and does not start
   device authorization or Anonymous Session creation.
4. Warp persists a random local identity in local preferences. The identity is
   stable across restarts and is not an account, token, or Anonymous Session.
5. `warp whoami` returns the local identity without refreshing credentials or
   fetching team/workspace metadata.
6. `warp login` and `warp logout` keep parsing but return a stable Local-only
   Mode error.
7. Background Warp account, cloud, telemetry, and Sentry initialization must be
   skipped in Local-only Mode where this increment owns the startup path.
8. When the build gate is disabled, existing Account Sign-in, Anonymous Session,
   and startup behavior remain unchanged.

## Out of Scope

- Direct OpenAI-compatible Provider transport.
- Local Agent engine and conversation history.
- Terminal/file/MCP/computer-use tool execution.
- Full settings UI cleanup for every account, billing, cloud, sharing, and
  handoff entry point.
- Deleting or migrating existing account tokens or cloud cache data.

## Acceptance Criteria

- Local-only GUI and TUI startup do not request or refresh Warp identity.
- Local identity is stable and `whoami` returns it locally.
- `login` and `logout` produce a stable Local-only Mode error.
- Startup code has a single mode policy used by GUI, TUI, CLI, telemetry, and
  cloud/network initialization.
- Focused tests cover flag-disabled behavior, Local-only identity output, and
  Local-only TUI/login/logout branches.
