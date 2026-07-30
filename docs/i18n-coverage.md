# GUI i18n Coverage Checklist

Status of Chinese/English localization for ZYH GUI copy.

## Baseline (current)

`app/src/i18n/` exposes `tr(ctx, Message::…)` / `tr_cached(Message::…)` with `Locale::En` and `Locale::ZhCn`.

| Item | Count / note |
|------|----------------|
| `Message` variants | **3836** |
| English table (`en_text`) | **3836** complete |
| Chinese table (`zh_cn_text`) | **3836** complete |
| Call sites | ~1300+ across 120+ files |
| Guard test | `all_messages_have_non_empty_text` in `table.rs` |

### Covered surfaces (high confidence)

| Area | Status |
|------|--------|
| Settings sidebar + Appearance / Features / Privacy / Account / Teams residual | Done |
| Onboarding + Auth (incl. anonymous); brand copy ZYH | Done |
| Workspace / tab menus (via `workspace_menu_message` → `Message`) | Done |
| Terminal context menus (via `terminal_menu_message` → `Message`) | Done |
| Platform app menus (`app_menus.rs` via `app_menu_message` → `Message`) | Done |
| `@` AI context menu categories (via `CtxCat*` → `Message`) | Done |
| Agent input footer tooltips/toasts (via `footer_message` → `Message`) | Done |
| Terminal input search placeholders / a11y (via `input_message` → `Message`) | Done |
| Agent blocklist chrome, management filters, zero-state | Done |
| Notebooks, Drive chrome + cloud-object toast templates | Done |
| Code review comments / PR chrome | Done |
| Plugin install titles, steps, notes (Codex / Claude / Gemini / OpenCode) | Done |
| Local→cloud handoff toasts | Done |
| Launch modals (OpenWarp, Orchestration, Feature intro, Oz launch, HOA banner) | Done |
| Resource center sections + tips | Done |
| High-frequency toasts (clipboard, voice, images, export, MCP, skills, …) | Done |
| Privacy Safe Mode description | Done |
| Agent warping/status strings (Working… / Reading files… / etc.) | Done |
| Agent zero-state shortcuts (`/`, `@`, pause agent, …) | Done |
| Find bar tooltips/placeholders (shared + notebook + code editor) | Done |
| Conversation rename error toasts | Done |
| Prompt alert chrome (offline / credits / overages CTAs) | Done |
| Agent feedback tooltips (Good/Bad response, Resume) | Done |
| Voice provider errors + transcription chrome | Done |
| Unsaved-changes dialogs (env vars / workflows / Drive modal) | Done |
| Requested command + code-diff action labels | Done |
| Local agent task sync error messages | Done |
| Drive index chrome + payment banners | Done |
| Onboarding prompt-setup block | Done |
| Command search empty/credits/placeholders | Done |
| Legacy AI assistant panel / transcript / limit copy | Done |
| Command-palette / keybinding descriptions (central compatibility map) | Done |
| Secrets / empty trash / index speedbump / agent header / billing denied | Done |
| Free-AI modal / queued prompts / commit dialog / naming dialog / code review diffs | Done |

### Intentionally not translated

| Kind | Why |
|------|-----|
| Product / brand names | ZYH, Oz, Claude Code, Codex, Gemini, ZYH Drive, … |
| Technical IDs / paths | `dev.warp.WarpOss`, binary `warp-oss`, data dirs (compat) |
| Action / command IDs | Not user-facing chrome |
| Telemetry / feature-flag names | Internal |
| Pure `{err}` / backend error passthrough | Server or OS text |
| Debug-only toasts | Heap profile, IAP credential refresh (dev/dogfood) |
| Logs | Not UI |
| Removed hosted-product source | Account, billing, cloud environment, sharing, and Ambient Agent UI retained only while ADR-0009 deletion proceeds is not reachable in the ZYH Local Product. |

### Known residual debt

| Item | Notes |
|------|-------|
| `SettingsSection` `Display` / `FromStr` | Still English for parse identity; deferred |
| Slash command descriptions/hints | **Migrated** to `Message` (`SlashDesc*` / `SlashHint*`) via identity-key map |
| Settings toggle binding descriptions | **Migrated** to `Message` (`ToggleEnablePrefix` / `ToggleDisablePrefix` / `ToggleSuffix*`) |
| Agent tips | **Migrated** to `Message` (`AgentTip*`) via identity-key map |
| Settings schema `description:` fields | Consumed by the `generate_settings_schema` binary for external JSON Schema / Agent skill resources; no Settings GUI consumer in this workspace. User-visible permission `description()` consumers are localized separately |
| New GUI copy | Must be added through `Message`; spot-check hardcoded render arguments as features ship |
| Agent mode rotating hint examples | Prefix localized; example English kept |
| EditableBinding English identity strings on call sites | Intentional; Chinese applied at materialization |
| Removed hosted-product UI | Not translated while its source awaits deletion under ADR-0009 |

### 2026-07-20 residual batch 1

Wired through `Message` + `tr` / `tr_cached`:

- Welcome tips (`tip_view`) titles/descriptions + Close Welcome Tips
- Project entry buttons + tooltips
- Notifications discovery / error banners (buttons + titles + trigger copy)
- Alias expansion / Vim / AWS CLI / Open-in-ZYH banner chrome
- Code review file-nav tooltips + discard-disabled tooltips
- Command-palette navigation session hints (Running / Completed / Empty Session…)
- Left panel Drive + Agent conversations tooltips
- Vertical tabs “New session”
- Settings About update status (checking / downloading)
- Workspace reauth + autoupdate banner buttons/headings
- Agent status “Setting up environment”
- Terminal grid “Open in ZYH” tooltip

### 2026-07-20 residual batch 2

- Slash command descriptions/hints: dual-track Chinese removed; English identity keys map to `Message::SlashDesc*` / `SlashHint*`
- Agent tips: dual-track Chinese removed; identity keys map to `Message::AgentTip*`
- Search / empty-state chrome: command palette, global search, context chips, workflows, secrets, notebook embed, agent management filters, command search a11y, environments search, find-bar no-results, conversation “New/Fork” items

### 2026-07-20 residual batch 3

- Settings toggle binding descriptions: dual-track Chinese removed from `settings_view/mod.rs`
- Identity English suffixes map to `Message::ToggleSuffix*`; enable/disable prefixes use `ToggleEnablePrefix` / `ToggleDisablePrefix`
- Dynamic override composes prefix+suffix via `tr` for both locales

### 2026-07-28 permanent-local residual batch

- Global Rule list/editor titles, placeholders, conflicts, deletion confirmation, file errors, and actions
- Shared filterable-dropdown search and empty state
- Code editor go-to-line title, placeholder, and validation errors
- Local Notebook placeholders, link editor, file loading/conflicts/errors, Markdown controls, and insertion actions
- Local Workflow placeholders, arguments, aliases, environment variables, view state, and file errors
- Agent inline actions, requested-command choices, question input, follow-up tooltips, and code-review actions
- Workspace conversation search, tab-config/tool-bar labels, common modal copy, tips, and resource-center completion copy
- MCP local-product errors, SSH upload controls, `/init` copy, and retained SSH compatibility banners
- Local custom-model Router Editor and Execution Profile Editor fields, permissions, help text, validation, and placeholders

### 2026-07-29 retained GUI residual batch

- Appearance tools-panel visibility, project explorer, Agent conversations, Global Search, typography, vertical tabs, header layout, directory colors, padding, and zoom
- Global Search states and local/remote shell availability messages
- Code settings indexing and language-server statuses, Code Review and Project Explorer descriptions
- MCP settings page, server cards, edit/update dialogs, status labels, and actions
- Execution Profile summary labels, permission values, and allowlist/denylist headings
- AI settings retained paths: Profiles, Models, Base model, Input, MCP, AWS Bedrock, and Custom Routers
- High-frequency context chips, tab-config actions, command palette, notebook save actions, inline menu states, and Agent cancel tooltip
- Code footer language-server actions and statuses, new-worktree modal fields, and local `/init` indexing/LSP/AGENTS.md steps

### 2026-07-29 retained GUI residual batch 2

- Vertical-tabs groups, tab counts, pane-kind badges, detail metadata, and the complete display-settings popup
- Retained Agent task/queue states, requested-command and MCP response chrome, CLI subagent controls, file-search activity, code-suggestion controls, and AWS credential errors
- Local terminal prompt editor, startup/shell compatibility banners, SSH file-upload status, plugin links, and Open-in-ZYH accessibility copy
- Execution Profile editor models, context window, retained permissions, permission descriptions, and directory/command/MCP allowlists and denylists
- Privacy regex fields, summarization cancellation, Settings import actions, tab-config actions, Notebook link application, and local Workflow context/actions

### 2026-07-29 retained GUI residual batch 3

- Editor autosuggestion tooltips, Settings/Get Started pane titles, Privacy secret-display description, conversation-summary status, and file-access permission choices
- Command-palette files, repositories, tabs, sessions, launch configurations, sections, history, workflows, and project accessibility labels/help
- Local Agent context search labels for files, workflows, code symbols, rules, blocks, commands, notebooks, skills, and conversations
- Find result announcements, input-suggestion announcements, search loading/error announcements, theme-chooser accessibility copy, and block-filter accessibility copy

### 2026-07-29 retained GUI residual batch 4

- Global Search tool visibility description, search states, failure state, result accessibility copy, and local/remote shell availability
- Notebook editor actions, terminal block actions, generic menu state/help, input suggestions, retained search results, and workflow accessibility announcements
- Retained dynamic templates for history timestamps, copy actions, requested-command errors, tab-config parameters, shell descriptions, file errors, and notification labels
- Context-chip availability and Git tracking tooltips, including local-session/CLI requirements and ahead/behind/rebase states
- Local Agent question controls, Run Agents confirmation/spawn states, Web Search/Web Fetch states, task-stop summaries, orchestration labels, and conversation recovery states
- Terminal context availability reasons, history details, block-scroll tooltips, password notifications, debugging menu labels, and shell names/details
- Code/Notebook/Workflow fallback titles and errors, MCP deep-link/editor errors, Global Search failure, and retained context-search empty states

### 2026-07-29 retained GUI residual batch 5

- Global Search tools-panel description plus the second-layer scan for strings assigned to state, variables, errors, and shared render helpers before reaching GUI sinks
- Codebase Search query/repository states, result counts, Web Search URL counts, file-create/delete labels, and review-surface failure copy
- Context-chip localized titles for tooltips and copy menus without changing their English identity strings used by logs and compatibility paths
- Local Agent response controls, conversation-search fragments, file-read failures, Skill actions, recording states, CLI Agent waiting/plugin messages, and Execution Profile Web tools
- Mouse-reporting and synchronized-input toasts, undo hints, conversation details, pending-user labels, Remote Host fallbacks, and terminal file-reveal tooltips
- Local Notebook/Code/Project Explorer fallback titles, Worktree/tab-config names, conversation deletion/list controls, Code Review disabled-edit tooltips, and local CLI install toasts

### 2026-07-29 retained GUI residual batch 6

- Launch Configuration save-modal accessibility title/help and the Code Review toolbelt feature popup
- Local child Agent and local harness visible failures, including unsupported shells, missing/unsupported harness types, working-directory resolution, task creation, and hidden-pane creation
- Retained SSH/Warpify success-block title/descriptions, Learn More action, and the Never Warpify This Host action
- SSH Center audit: ADR-0007/0008 and the migration inventory define local `ssh_hosts.json` ownership, but this workspace currently contains no reachable SSH Center/Remote Host Shortcut GUI implementation to localize

### 2026-07-29 retained GUI residual batch 7

- Local multi-Agent orchestration send/start states, including success, failure, cancellation, and in-progress copy
- New-tab URI notification description and linked-worktree missing-path terminal message
- Pi Agent Runtime failure/tool-limit/startup errors; removed the obsolete instruction to continue with the retired Rust runtime
- Retained Agent Management search/new action, session status, source/harness/executor/run metadata, Runs heading, and loading states
- AI Settings and Execution Profile permission descriptions now share localized mappings instead of rendering protocol enum `description()` text directly
- Ask-question and local child-Agent permission labels/descriptions in the Execution Profile editor
- Agent thinking, child-message, prompt-submission, and long-running-command command-palette descriptions in the central binding compatibility map
- Retained Agent notification mailbox filters/header/empty state and CLI/local Agent completion, attention, cancellation, and error fallbacks
- Agent notification toast/management tooltips, Code Review loading state, command-search provider errors, and retained local/project Workflow sidebar labels and announcements
- Settings schema audit: `app/src/settings/**` `description:` values feed the generated JSON Schema (and Agent skill resource), with no GUI sink in this workspace; separate user-visible permission-description consumers were traced and localized

The retained-GUI hardcoded-string scan intentionally excludes unreachable hosted-product source covered by ADR-0009, including cloud environments, sharing, hosted Workflows/Drive, and the `/init` cloud-environment step.

**Rough completion (retained user-visible GUI):** high. Direct sinks and indirect state/template flows now leave technical identifiers, backend/OS error passthrough, or ADR-0009 excluded source. A no-hit result from either scan alone is not treated as proof of completion.

### Validation

- `Message`, both locale tables, and `ALL_MESSAGES` counts match at **3836** with no duplicate variants; `git diff --check` passes.
- Targeted `rustfmt --check` passes for the batch files except `workflows/categories.rs`, where it reports the pre-existing `load_cloud_workflows` `retain` formatting at line 490; the new i18n branches in that file are formatted.
- `TOOLCHAINS=com.apple.dt.toolchain.Metal.32023.918.1 CARGO_INCREMENTAL=0 cargo check --package warp --lib` reaches the `warp` app crate and is blocked only by the pre-existing inaccessible `feature_intro_modal::FEATURE_INTROS` reference in `app/src/workspace/view.rs`.
- `TOOLCHAINS=com.apple.dt.toolchain.Metal.32023.918.1 CARGO_INCREMENTAL=0 cargo test --package warp --lib i18n::table::tests -- --nocapture` is blocked by the same compile error before the i18n tests can run.

---

## Gaps: principles

Hardcoded English **not** routed through `tr(Message)` is a gap **unless** listed under intentionally-not-translated.

Prefer:

1. Add `Message` + `en_text` / `zh_cn_text` + `ALL_MESSAGES`
2. Call `tr` / `tr_cached` at the UI site
3. For menu/label bridges that must keep English as identity keys, map via `*_message(text) -> Option<Message>` (same pattern as `workspace_menu_message`)

Do **not** reintroduce dual-track `match text { "English" => "中文" }` tables.

---

## Brand display (OSS / local)

User-facing app name is **ZYH**:

| Surface | Value |
|---------|--------|
| `CFBundleDisplayName` / `CFBundleName` (oss + local) | `ZYH` |
| `package.metadata.bundle.bin.warp-oss` / `warp` `name` | `ZYH` |
| Bundle identifier / AppId application name | keep `WarpOss` (paths / install id compat) |
| Binary name | `warp-oss` |

---

## Implementation notes

- API: `crate::i18n::{tr, tr_cached, Message, active_locale}`
- Tables: `app/src/i18n/table.rs`
- Enum: `app/src/i18n/message.rs`
- Locale setting: Appearance language widget + `LocalizationSettings`
- Placeholders: prefer `{}` single-arg; named `{key}` / `{answered}` when multi-arg
- Plugin instruction steps use `Box::leak` for `'static` slices under `LazyLock`

---

## Related docs

- [CONTEXT.md](../CONTEXT.md)
- [docs/adr/0009-adopt-the-permanent-zyh-local-product.md](adr/0009-adopt-the-permanent-zyh-local-product.md)

---

_Last updated: 2026-07-29. Retained GUI residual batch 7. Catalog **3836** variants; binding map 366 entries._
