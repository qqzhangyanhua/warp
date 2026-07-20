# GUI i18n Coverage Checklist

Status of Chinese/English localization for ZYH GUI copy.

## Baseline (current)

`app/src/i18n/` exposes `tr(ctx, Message::…)` / `tr_cached(Message::…)` with `Locale::En` and `Locale::ZhCn`.

| Item | Count / note |
|------|----------------|
| `Message` variants | **2571** |
| English table (`en_text`) | **2571** complete |
| Chinese table (`zh_cn_text`) | **2571** complete |
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
| Command-palette / keybinding descriptions (central dual-track map) | Done |
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

### Known residual debt

| Item | Notes |
|------|-------|
| `SettingsSection` `Display` / `FromStr` | Still English for parse identity; deferred |
| Slash command descriptions/hints | **Migrated** to `Message` (`SlashDesc*` / `SlashHint*`) via identity-key map |
| Settings toggle binding descriptions | Dual-track map in `settings_view/mod.rs` `localized_toggle_binding_description` |
| Agent tips | **Migrated** to `Message` (`AgentTip*`) via identity-key map |
| Settings schema `description:` fields | TOML/settings metadata; many still English (lower user impact) |
| Rare empty states / marketing modals | Spot-check remaining hardcodes as features ship |
| Agent mode rotating hint examples | Prefix localized; example English kept |
| EditableBinding English identity strings on call sites | Intentional; Chinese applied at materialization |
| Teams / Billing long-tail | Lower priority for anonymous-only builds |
| HOA / feature-intro marketing copy | Long-form marketing strings |

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

**Rough completion (user-visible GUI):** high for Chinese daily use. Remaining dual-track: settings toggle binding descriptions.

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
- [docs/adr/0003-offer-an-anonymous-only-product-mode.md](adr/0003-offer-an-anonymous-only-product-mode.md)

---

_Last updated: 2026-07-20. Residual batch 2 (dual-track migrate + search/empty). Catalog **2571** variants; binding map 354 entries._
