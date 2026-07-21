---
status: accepted
date: 2026-07-21
issue: 23
supersedes: [0001-route-anonymous-ai-through-warp-agent.md, 0003-offer-an-anonymous-only-product-mode.md, 0004-local-only-custom-provider-mode.md]
amends: [0005-use-pi-as-local-agent-runtime.md, 0007-keep-ssh-center-data-local.md]
---

# Adopt the Permanent ZYH Local Product

ZYH is permanently a local product. It has no Account Sign-in, Anonymous
Session, Warp-hosted AI, cloud sync, telemetry, Sentry, automatic update, or
background Warp network behavior. This is the only product contract, not a
build-gated mode or a fallback path.

The retained product includes the desktop GUI, TUI, `zyh agent`, local terminal
and file features, user-initiated SSH and Git access, explicitly configured
OpenAI-compatible Providers, explicitly configured MCP servers, and a Pi-backed
Agent Runtime. Production state belongs under `~/.zyh/`, development state
under `~/.zyh-dev/`, and tests under isolated temporary homes.

## Superseded Decisions

This ADR fully supersedes ADR-0001, ADR-0003, and ADR-0004. There is no
Local-only Mode feature flag, Local Identity, Account or Anonymous branch, or
non-local product behavior to preserve.

This ADR amends ADR-0007 only to replace its obsolete Anonymous-only and
Local-only mode consequence. Its decision to keep SSH Center data local remains
accepted as part of the permanent ZYH local product.

This ADR supersedes only ADR-0005's rollout, runtime-selection, and Rust-runtime
fallback decisions. It preserves ADR-0005's safety and recovery contract:

- The Conversation Record remains canonical and the Runtime Transcript remains
  derived.
- ZYH owns the Runtime Supervisor, Tool Execution Authority, Tool Catalog,
  Agent Resource Catalog, permission decisions, and tool effects.
- The Bridge Protocol remains versioned and fail-closed, with bounded content,
  complete Transcript Sync, Commit Barriers, and checksum-pinned artifacts.
- Tool execution remains durable and idempotent, with conservative
  Indeterminate Tool Execution recovery.
- Provider redirects remain same-Origin, API Keys remain isolated, and retries
  remain bounded to the pre-output window.

Pi is the only Agent Runtime for new Conversations. A legacy Rust-bound
Conversation is view-only and may be explicitly forked into a new Pi
Conversation; it is never silently rebound or continued under new semantics.

## Network Boundary

Without a configured Provider or MCP server, GUI, TUI, CLI, remote daemon,
startup, restoration, settings, shutdown, and background work make no
app-initiated external request.

During an Agent Run, app-initiated traffic is limited to the selected Provider
Origin and origins of explicitly configured MCP servers. Shell commands, Git,
SSH, browser opens, and third-party CLIs are visible user-initiated effects and
are not part of this allowlist. They remain governed by their own permission and
UI paths.

HTTP clients, WebSockets, GraphQL, Firebase, telemetry, Sentry, update checks,
remote-daemon downloads, hosted model discovery, and background skill or plugin
downloads that target Warp-owned services are deleted. Tests establish the
prefactoring HTTP, HTTPS, and WebSocket baseline through an external
deny-and-record proxy rather than private client hooks. The baseline covers the
inventoried proxy-aware clients; it does not turn the proxy into proof against
an unclassified client that opens a raw socket. Each deletion phase must audit
new network constructors and retain an independent socket-level check where the
target platform provides one.

## Persistence and Secrets

The ZYH migration copies supported local state from the legacy root and never
mutates or deletes the legacy source. It preserves terminal history, window and
tab restoration, local Conversations, Agent Run records, local project metadata,
and supported local configuration. Cloud-only rows are deleted from the copied
database according to the inventory linked below.

Rules, Workflows, MCP, Notebooks, settings, keybindings, themes, tab and launch
configurations, skills, plugins, SSH Center data, logs, and SQLite retain
domain-specific files instead of being collapsed into one settings file.
Project-owned ZYH configuration lives under `<repo>/.zyh/`; project migration is
explicit and never silently dirties a worktree.

Provider API Keys, MCP secrets, Remembered SSH Passwords, and retained direct
third-party credentials remain in operating-system secure storage. Migration
copies and verifies secure-storage entries before deleting legacy entries.
Secret values never enter TOML, JSON, project files, logs, diagnostics,
migration reports, Bridge messages beyond the targeted child process, or an
unrelated subprocess environment.

## SSH and Packaging

macOS, Linux, and Windows desktop builds remain. Release packages bundle remote
daemon artifacts for Linux and macOS on arm64 and x86_64. A versioned manifest
pins each artifact's platform, size, SHA-256 digest, and protocol identity.
After the SSH preinstall check, ZYH selects the matching bundled artifact and
uploads it over the established SSH/SCP connection. Runtime HTTP download and
fallback paths are deleted; a missing or invalid release artifact fails
packaging.

## Cloud Deletion Order

Cloud code is physically removed in dependency order:

1. Classify persisted state and migrate retained local data and secrets.
2. Extract retained local models and serialization contracts from cloud-owned
   crates into their owning local modules.
3. Move retained consumers to local files, Pi, Provider, MCP, and bundled SSH
   artifact paths.
4. Remove cloud UI, startup registration, and background jobs.
5. Remove GraphQL/generated clients, cloud clients, auth, sync, telemetry, and
   cloud persistence crates as complete units.
6. Remove workspace dependencies, generated inputs, hosted-service Git
   dependencies, feature flags, packaging hooks, and CI helpers after no
   retained consumer remains.

Generated files and historical migrations are not edited to simulate deletion.

## Approval Record

Issue #23 was marked `ready-for-agent`, and the repository owner requested its
implementation on 2026-07-21. That approval covers the following fixed
boundaries recorded by this ADR and the inventory:

| Boundary | Accepted decision |
| --- | --- |
| Persistence classification | Preserve local history, restoration, Conversations, run records, and project metadata; replace file-backed data; delete account, team, cloud, sync, quota, and server-task state. |
| ZYH and secure-storage migrations | Copy, verify, report, and remain idempotent; never mutate the legacy root or expose secret values. |
| Legacy Conversation behavior | Rust-bound Conversations are view-only; explicit fork creates a new Pi Conversation. |
| Provider and MCP secrets | Operating-system secure storage only; expose a secret solely to its selected target process and origin. |
| Remote daemon artifact matrix | Bundle Linux/macOS arm64/x86_64 artifacts and transfer them over SSH/SCP; no runtime download. |
| Endpoint allowlist | Only the selected Provider Origin and explicitly configured MCP origins are app-initiated external destinations. |
| Cloud crate deletion order | Migrate and extract retained ownership before deleting consumers, clients, generated trees, and manifests. |

The machine-readable classification is
[`docs/zyh-local-product-inventory.json`](../zyh-local-product-inventory.json).

## Consequences

Upstream cloud-product compatibility is intentionally abandoned. Renaming the
executable, paths, environment variables, URI scheme, local-control protocol,
bundle identifiers, and remote-daemon protocol is an intentional breaking
change and must be atomic across producers, consumers, packaging, tests, and
documentation.

Each implementation phase must remain buildable and must pass focused tests plus
`./scripts/check.sh`. No phase may claim the local network boundary while a
startup surface, restored state, or shutdown path can still initiate an
unclassified request.
