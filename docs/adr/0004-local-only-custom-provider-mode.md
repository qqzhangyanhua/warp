# Add Local-only Custom Provider Mode

Warp will provide a build-gated Local-only Mode for OpenAI-compatible Provider
users who do not use Account Sign-in, Anonymous Sessions, Warp-hosted AI,
cloud sync, telemetry, Sentry, or background Warp network services. The mode
uses a persistent local identity stored in local preferences and keeps account
credentials and cloud cache data untouched.

This mode is distinct from Anonymous-only Mode. Anonymous-only Mode keeps an
Anonymous Session and may route supported service requests through Warp. Local-
only Mode must not create or refresh a Warp identity and must eventually route AI
requests directly to the configured Provider.

## Consequences

GUI, TUI, and CLI need a shared policy for the mode boundary. Account, billing,
cloud, sharing, handoff, telemetry, and Sentry entry points are hidden or skipped
in this mode. Existing builds retain their current Account Sign-in and
Anonymous Session behavior when the build gate is disabled.
