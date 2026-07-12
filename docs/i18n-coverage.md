# GUI i18n Coverage Checklist

Status of Chinese/English localization for Warp GUI copy.

## Baseline (already covered)

`app/src/i18n/` exposes `tr(ctx, Message::…)` with `Locale::En` and `Locale::ZhCn`.

| Item | Count / note |
|------|----------------|
| `Message` variants | **600** |
| English table (`en_text`) | **600** complete |
| Chinese table (`zh_cn_text`) | **600** complete |
| Call sites | Almost only `app/src/settings_view/*` |

### What the 157 messages cover

| Category | Approx. | Main surfaces |
|----------|---------|----------------|
| Settings sidebar sections | 26 | `settings_view/mod.rs` nav titles |
| In-page category titles | 25 | Appearance / Features / Code / Warpify |
| Appearance controls | 21 | Theme, icon, window opacity, etc. |
| Language widget | 6 | Locale picker itself |
| Features / related toggles | ~79 | Most Features titles/descriptions; a few Code / Warpify / AI labels |

### Relatively complete pages

- **Appearance** — categories + language + most appearance controls
- **Features** — most toggle titles/descriptions
- **Settings sidebar section names**

### Partially covered pages

- **Code** — some categories/labels; many constants still hardcoded
- **Warpify** — categories + a few SSH strings; subtitles/buttons still English
- **AI / Warp Agent** — almost entirely English

**Rough completion:** Settings user-visible copy maybe ~30–40% i18n’d; **whole GUI likely &lt;10%**.

---

## Gaps: principles

Items below are **hardcoded English not routed through `tr(Message)`**.

Filtered out of this list (do not translate, or not UI):

- Action / command IDs (`ToggleCopyOnSelect`, …)
- Telemetry event names and descriptions
- Internal keys, asset paths, protocol strings
- Pure debug / log messages (unless user-visible)

Keep as product names (usually untranslated): Warp, Oz, Warp Drive, Claude Code, Codex, Gemini, etc.

---

## P0 — Anonymous mode / first-run path (do first)

### Onboarding (`crates/onboarding`)

- [x] `intention_slide`: Welcome to Warp / How do you want to work? / Build faster with agents / Just use the terminal / No AI features
- [x] `intro_slide`: Already have an account? / A modern terminal with state of the art agents…
- [x] `ai_setup_slide`: Choose your AI setup / Use Warp Agent / Use third party agents / Access more models…
- [x] `ai_access_slide`: Get AI access / Configure AI (anonymous branch) / Subscription / Best value / Set up later / browser token prompts
- [x] `project_slide`: Open a project / Open local folder…
- [x] `theme_picker_slide`: Choose a theme / Sync light/dark… / privacy agreement copy
- [x] `customize_slide`: Customize your Warp / Tab styling / Tools panel / File explorer…
- [x] `agent_slide` / `third_party_slide`: Customize your Warp Agent / third party agents…
- [ ] Callouts: Meet the Warp input / Talk to the agent / terminal mode / agent mode

### Auth (`app/src/auth`)

- [x] Welcome to Warp! / Sign up for Warp / Skip for now / Using Warp Offline
- [x] Already have an account? / Don’t want to sign in right now?
- [x] Sign in on your browser… / Paste auth token… / Privacy Settings
- [x] Are you sure you want to skip login? / AI only for logged-in users…
- [x] New login detected / Export your data / This cannot be undone

### Anonymous-only additions

- [x] Toast: `This build only supports anonymous mode.` (`workspace/view.rs`)
- [x] Missing-provider error string (+ Open Settings emit already present)
- [x] Custom inference: HTTP plaintext warning, form labels, `+ Add model`, Cancel/Save/Add endpoint
- [x] Custom inference: placeholders
- [ ] Custom inference: Test connection copy

---

## P1 — Residuals on partially i18n’d Settings pages

### Appearance

- [ ] Input mode: Start/Pin Input at Top/Bottom / Toggle Input Mode
- [ ] Tab bar: Always show / Hide if fullscreen / Only show on hover
- [ ] Code review button show/hide command-palette descriptions
- [ ] Dual-track `Category::new("English", …).with_localized_title(…)` — clear English fallback names when localized title is enough

### Features

- [x] Left/Right Option|Alt key is Meta
- [x] Click to set global hotkey / Configure Global Hotkey / Press new keyboard shortcut / Change keybinding
- [x] Start Warp at login…
- [x] Tab key behavior
- [x] Width% Height% / When a command takes longer than… seconds
- [x] Toast notifications stay visible for… seconds
- [x] After all tabs / After current tab
- [x] Wayland-related notices
- [ ] Submodules: `external_editor` / `startup_shell` / `undo_close` / `working_directory`

### Code

- [x] Initialization Settings / Codebase indexing / Index new folders by default
- [x] Team admins have enabled/disabled… / AI Features must be enabled…
- [x] No folders have been initialized yet / Initialized / indexed folders
- [x] No index created / Syncing… / Indexing… / Codebase too large / Synced
- [x] Restart server / View logs / LSP SERVERS / Open project rules

### Warpify

- [ ] Subshells supported: bash, zsh, and fish
- [ ] Warpify your interactive SSH sessions
- [ ] Install SSH extension / Added commands / Denylisted commands / Learn more
- [ ] SSH install-policy descriptions

### AI / Warp Agent page (almost fully untranslated)

- [x] Active AI / Next Command / Rules / Suggested Rules
- [x] Warp Drive as agent context / Warp credit fallback
- [x] Show/Hide agent tips / Oz changelog / “Use Agent” footer
- [x] Agent decides / Always allow / Always ask (+ Ask on first write; coding read perms)
- [ ] Permission allow/deny list placeholders

### Shared settings chrome (`settings_page.rs`, search, footer)

- [x] Reset to default
- [x] Click to learn more in docs
- [x] This setting is not synced…
- [ ] This option is enforced by your organization…
- [x] No settings match your search…
- [ ] Open settings file

---

## P2 — Settings pages with no i18n wiring

### Account / `main_page`

- [x] Sign up / Upgrade Plan / Compare plans / Upgrade to Turbo|Lightspeed
- [x] Earn rewards / Refer a friend / Log out / Settings sync / updates

### Privacy

- [x] Secret redaction / Custom secret redaction
- [x] Help improve Warp / Manage your data / Visit the data management page
- [x] Privacy policy (page titles/links); crash reports residual
- [x] No enterprise regexes / Add regex modal

### Teams

- [ ] Team name / Leave team / Delete team / Create Team
- [ ] Promote/Demote admin / Remove from team / Transfer ownership / Cancel invite
- [ ] Domain/email validation and invite copy
- [ ] Your team is full / Payment past due / Free plan usage limits…

### Billing & usage

- [ ] Usage History / Monthly spend limit / Auto-reload / Buy credits / Buy more
- [ ] Cloud agent trial / No usage history / Last 30 days
- [ ] Contact team admin / Account Executive…
- [ ] Overall / Local / Cloud agent usage section titles

### Environments / Platform / MCP

- [ ] New environment / Quick setup / Use the agent / You haven’t set up any…
- [ ] Docker image / Auth with GitHub / Create|Save environment
- [ ] Oz Cloud API Keys / + Create API Key / No API Keys…
- [ ] Search MCP Servers / Once you add a MCP server… / No tools available

### Other settings pages

- [ ] Referrals (Invite a friend… / rewards / Copy link…)
- [ ] Warp Drive: Enable/Disable / To use Warp Drive, please create an account
- [ ] Keybindings conflict notices
- [ ] About / Scripting residual copy
- [ ] Custom router: Complexity-based / Prompt-based routing

---

## P2 progress note

- Privacy page high-frequency: **done**
- Account/main page high-frequency: **done**
- Workspace menus/toasts/tool panel labels: **started/mostly done**

## P3 — Main UI / Terminal high-frequency

### Workspace

- [ ] Tools panel: Project explorer / Global search / Warp Drive / Agent conversations
- [ ] Vertical tabs: No tabs open / No tabs match… / View as / Tab item / Additional metadata
- [ ] Conversation list: No conversations yet / New conversation / No matching conversations
- [ ] Search: Search tabs… / Search sessions, agents, files… / Search repos
- [ ] Toasts: Please sign in again… / Your app is out of date… / Failed to load conversation…
- [ ] Rename pane / Reset pane name
- [ ] Launch / feature-intro modals (deferrable; mostly marketing)

### Terminal input & agent

- [ ] Tell the agent what to build… / Kick off a cloud agent
- [ ] Run commands / Steer the running agent / Queue a follow up / Ask a follow up
- [ ] Choose an AI execution profile / Choose an agent model
- [ ] Model Specs / Reasoning level / Custom Model Router descriptions
- [ ] Preparing handoff… / Local skills cannot run on a remote machine…

### Terminal status / banners

- [ ] Loading session… / Starting shell… / Installing Warp SSH Extension… / Initializing…
- [ ] Out of credits / Monthly limit reached / Manage billing / Auto reload
- [ ] Login for AI / AI features unavailable for logged-out users / Sign Up
- [ ] Shell process exited… / Copy error / File issue
- [ ] Filter block output / Bookmark this block… / Save as Workflow
- [ ] Zero state: New terminal session / Don’t show again
- [ ] Share session modals and role-request copy
- [ ] Cloud agent loading: GitHub Authentication Required / Failed to start environment…

### Other terminal

- [ ] Available shell display names (Windows PowerShell / WSL…) — proper nouns may stay English
- [ ] Secret display: Always show secrets, etc.
- [ ] Working directory: Home directory / Previous session’s directory / Custom directory
- [ ] Warpify SSH install policy: Always ask / Always install / Never install

---

## P4 — Defer or do not translate

| Type | Guidance |
|------|----------|
| Product / brand names | Keep English (Warp, Oz, Warp Drive, Claude Code, Codex…) |
| Command Palette action IDs | Do not translate (internal) |
| Telemetry names / descriptions | Do not translate |
| Debug / log / internal errors | Low priority; translate only user-visible surfaces |
| One-shot launch-modal marketing | Defer |
| Settings-schema file descriptions | May stay English or track separately from UI |

---

## Volume estimate (for planning)

| Bucket | Status | Rough user-visible strings |
|--------|--------|----------------------------|
| Already i18n (157 `Message`s) | Done | ~150–200 |
| P0 first-run / login / anonymous | Open | ~80–120 |
| P1 Settings residuals | Partial | ~80–120 |
| P2 full Settings pages | Open | ~250–400 |
| P3 Workspace / Terminal | Open | ~300–500 |
| **Remaining** | | **~700–1100+** (somewhat less after dedupe) |

---

## Anonymous-only build: scope cut

Often **hidden or lower priority** in anonymous-only mode:

- Teams / Referrals / Billing
- Cloud Warp Drive capabilities / Oz Cloud API Keys
- Most cloud-environment marketing copy
- Account upgrade / sign-in CTAs (hide or replace with “unavailable”)

**Still required** for anonymous builds:

- P0
- Settings: Appearance / Features / AI (provider config) / local Privacy
- Terminal: basic input, status, empty states, high-frequency toasts

---

## Known engineering gaps

1. **`LocalePreference::System` → OS locale** — implemented via `sys-locale` in `app/src/i18n/mod.rs` (`system_locale` / `locale_from_system_tag`). Unsupported languages fall back to English; explicit traditional Chinese tags (`zh-TW` / `zh-HK` / `zh-Hant`) also fall back to English until a dedicated locale exists.
2. **Dual-track strings** — e.g. `Category::new("English", …).with_localized_title(Message::…)` still keep English constructors.
3. **`SettingsSection` `Display` / `FromStr`** still use English names; anything parsing English section titles may disagree with localized UI labels.
4. **Anonymous-mode high-frequency copy** — toast / missing-provider error / HTTP plaintext warning now use `Message`; broader onboarding/auth still open (P0).
5. **Guardrails to add/keep:**
   - Existing test: `all_messages_have_non_empty_text` in `table.rs`
   - Unit tests for `locale_from_system_tag` in `i18n/mod.rs`
   - Suggested: no duplicate match arms; English table must not contain CJK; new UI strings require a `Message` entry

---

## Suggested sprint order

1. ~~Fix **System locale** → follow OS language when possible~~ **Done**
2. ~~**P0** — Onboarding + Auth + anonymous toast / provider setup~~ **Mostly done** (Test connection residual)
3. ~~P1 settings high-frequency~~ done
4. ~~**P2** Privacy + Account + Workspace high-frequency~~ **started/mostly done**
4. **P2** — Privacy / Account (often visible) → other settings; skip Teams/Billing for anonymous-only if hidden
5. **P3** — Input placeholders, agent status, empty states, high-frequency toasts
6. Process: new user-visible strings must go through `Message` + En/Zh tables

---

## Implementation notes

- API: `crate::i18n::{tr, tr_cached, Message}` and `active_locale(ctx)`
- `tr_cached` is for deep settings chrome helpers without `AppContext`; refreshed by `tr`/`active_locale`
- Tables: `app/src/i18n/table.rs` (`en_text` / `zh_cn_text`)
- Enum: `app/src/i18n/message.rs`
- Locale setting: `app/src/settings/localization.rs` + Appearance language widget
- Prefer adding `Message` variants over scattering Chinese literals in views
- Prefer one source of display text (localized) over English constructor + localized override dual-track

---

## Related docs

- [CONTEXT.md](../CONTEXT.md) — domain vocabulary (Account Sign-in, Anonymous Session, OpenAI-compatible Provider)
- [docs/adr/0003-offer-an-anonymous-only-product-mode.md](adr/0003-offer-an-anonymous-only-product-mode.md)
- [docs/anonymous-only-mode-plan.md](anonymous-only-mode-plan.md)

---

_Last updated: 2026-07-12. Generated from a repo scan of `app/src/i18n`, `app/src/settings_view`, `crates/onboarding`, `app/src/auth`, `app/src/workspace`, and `app/src/terminal`. Re-scan after large UI string migrations._
