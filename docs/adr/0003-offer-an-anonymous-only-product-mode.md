---
status: superseded
superseded-by: 0009-adopt-the-permanent-zyh-local-product.md
---

# Offer an anonymous-only product mode

Warp will provide a build-gated Anonymous-only Mode in which Account Sign-in and all account-only surfaces are unavailable while a stable Anonymous Session remains available for supported services. The mode keeps local terminal and agent workflows, uses user-supplied OpenAI-compatible Providers for AI, and excludes teams, billing, sharing, cloud sync, cloud conversation history, Warp-managed models, Warp identity API keys, and remote agents; the standard build retains its current behavior when the flag is disabled.

## Consequences

GUI, TUI, and CLI must share the same mode boundary. Local workflows remain available, but objects or features that only have cloud-backed storage are hidden rather than presented as partially functional.
