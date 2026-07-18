# GUI i18n Coverage Checklist

Status of Chinese/English localization for ZYH GUI copy.

## Baseline (current)

`app/src/i18n/` exposes `tr(ctx, Message::…)` / `tr_cached(Message::…)` with `Locale::En` and `Locale::ZhCn`.

| Item | Count / note |
|------|----------------|
| `Message` variants | **2108** |
| English table (`en_text`) | **2108** complete |
| Chinese table (`zh_cn_text`) | **2108** complete |
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
| Slash command descriptions/hints | Dual-track `match` in `static_commands/mod.rs` — migrate to `Message` |
| Settings toggle binding descriptions | Dual-track map in `settings_view/mod.rs` `localized_toggle_binding_description` |
| Agent tips | Dual-track `localized_tip_description` in `agent_tips.rs` |
| Rare empty states / marketing modals | Spot-check remaining hardcodes as features ship |
| Agent mode rotating hint examples | Prefix localized; example English kept |

**Rough completion (user-visible GUI):** high for Chinese daily use; residual dual-track maps above still work but should move to `Message` when touched.

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

_Last updated: 2026-07-18. WarpOss display → ZYH; app menus + context categories → Message; onboarding brand ZYH; residual dual-track listed. Catalog **2108** variants._
