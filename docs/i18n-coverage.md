# GUI i18n Coverage Checklist

Status of Chinese/English localization for Warp GUI copy.

## Baseline (already covered)

`app/src/i18n/` exposes `tr(ctx, Message::…)` with `Locale::En` and `Locale::ZhCn`.

| Item | Count / note |
|------|----------------|
| `Message` variants | **1409** |
| English table (`en_text`) | **1409** complete |
| Chinese table (`zh_cn_text`) | **1409** complete |
| Call sites | ~1000+ across 75+ files |

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
- [x] Callouts: Meet the Warp input / Talk to the agent / terminal mode / agent mode

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
- [x] Custom inference: remove-endpoint confirmation, custom endpoint descriptions, default-model prompt
- [x] Custom inference: Test connection copy — wired via `CustomInferenceTestConnection` / testing + result Messages in `custom_inference_connection_test.rs`

---

## P1 — Residuals on partially i18n’d Settings pages

### Appearance

- [x] Input mode: Start/Pin Input at Top/Bottom / Toggle Input Mode
- [x] Tab bar: Always show / Hide if fullscreen / Only show on hover
- [x] Code review button show/hide command-palette descriptions
- [x] Dual-track `Category::new("English", …).with_localized_title(…)` — replaced with `Category::localized(Message, …)` (display-only title; English constructor gone for localized categories)

### Features

- [x] Left/Right Option|Alt key is Meta
- [x] Click to set global hotkey / Configure Global Hotkey / Press new keyboard shortcut / Change keybinding
- [x] Start Warp at login…
- [x] Tab key behavior
- [x] Width% Height% / When a command takes longer than… seconds
- [x] Toast notifications stay visible for… seconds
- [x] After all tabs / After current tab
- [x] Wayland-related notices
- [x] Submodules: `external_editor` / `startup_shell` / `undo_close` / `working_directory`

### Code

- [x] Initialization Settings / Codebase indexing / Index new folders by default
- [x] Team admins have enabled/disabled… / AI Features must be enabled…
- [x] No folders have been initialized yet / Initialized / indexed folders
- [x] No index created / Syncing… / Indexing… / Codebase too large / Synced
- [x] Restart server / View logs / LSP SERVERS / Open project rules

### Warpify

- [x] Subshells supported: bash, zsh, and fish
- [x] Warpify your interactive SSH sessions
- [x] Install SSH extension / Added commands / Denylisted commands / Learn more
- [x] SSH install-policy descriptions

### AI / Warp Agent page (almost fully untranslated)

- [x] Active AI / Next Command / Rules / Suggested Rules
- [x] Warp Drive as agent context / Warp credit fallback
- [x] Show/Hide agent tips / Oz changelog / “Use Agent” footer
- [x] Agent decides / Always allow / Always ask (+ Ask on first write; coding read perms)
- [x] Permission allow/deny list placeholders

### Shared settings chrome (`settings_page.rs`, search, footer)

- [x] Reset to default
- [x] Click to learn more in docs
- [x] This setting is not synced…
- [x] This option is enforced by your organization…
- [x] No settings match your search…
- [x] Open settings file

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

- [x] Team name / Leave team / Delete team / Create Team (page chrome already Message; confirm dialogs wired)
- [x] Promote/Demote admin / Remove from team / Transfer ownership / Cancel invite (+ transfer description + Cancel)
- [x] Domain/email validation and invite copy
- [x] Your team is full / Payment past due / Free plan usage limits…

### Billing & usage

- [x] Usage History / Monthly spend limit / Auto-reload / Buy credits / Buy more
- [x] Cloud agent trial / No usage history / Last 30 days
- [x] Contact team admin / high-frequency billing chrome (Manage billing / Compare plans / Open admin panel / Plan / Load more)
- [x] Account Executive / upgrade CTAs / usage section titles / overage modal / credit-type labels

### Environments / Platform / MCP

- [x] New environment / Quick setup / Use the agent / You haven’t set up any…
- [x] Docker image / Auth with GitHub / Create|Save environment (+ form residuals: Delete, Share with team, Setup commands, Description, Suggest image, GitHub empty/error helpers)
- [x] Oz Cloud API Keys / + Create API Key / No API Keys…
- [x] Search MCP Servers / Once you add a MCP server… / No tools available

### Other settings pages

- [x] Referrals (Invite a friend… / rewards / Copy link…)
- [x] Warp Drive: Enable/Disable / To use Warp Drive, please create an account
- [x] Keybindings conflict notices
- [x] About / Scripting residual copy
- [x] Custom router: Complexity-based / Prompt-based routing

---

## P2 progress note

- Privacy page high-frequency: **done**
- Account/main page high-frequency: **done**
- Workspace menus/toasts/tool panel labels: **started/mostly done**

## P3 — Main UI / Terminal high-frequency

### Workspace

- [x] Tools panel: Project explorer / Global search / Warp Drive / Agent conversations
- [x] Vertical tabs: No tabs open / No tabs match… / View as / Tab item / Additional metadata
- [x] Conversation list: No conversations yet / New conversation / No matching conversations
- [x] Search: Search tabs… / Search sessions, agents, files… / Search repos
- [x] Toasts: Please sign in again… / Your app is out of date… (wired; “Failed to load conversation” mostly log/internal)
- [x] Rename pane / Reset pane name (ActivePane branch aligned with Message)
- [ ] Launch / feature-intro modals (deferrable; mostly marketing)

### Terminal input & agent

- [x] Tell the agent what to build… / Kick off a cloud agent
- [x] Run commands / Steer the running agent / Queue a follow up / Ask a follow up
- [x] Named child-agent variants: Queue/Steer/Ask for `{}` agent (via Message + `replace`)
- [x] Choose an AI execution profile / Choose an agent model
- [x] Model Specs / Reasoning level / Custom Model Router descriptions (+ Cost / Intelligence / Speed / Auto mode description / Manage profiles / selected)
- [x] Agent-mode rotating placeholder prefix: ZYH anything → 让 ZYH 做任何事
- [x] Preparing handoff… / Local skills cannot run on a remote machine… / custom model / cloud conversation local block

### Terminal status / banners

- [x] Loading session… / Starting shell… / Installing Warp SSH Extension… / Initializing… (+ remote Checking/Installing/Updating)
- [x] Out of credits / Monthly limit reached / Manage billing / Auto reload
- [x] Login for AI / AI features unavailable for logged-out users / Sign Up
- [x] Shell process exited… / Copy error / File issue / More info (shell terminated banner + inline banner)
- [x] Filter block output / Bookmark this block… / Save as Workflow
- [x] Zero state: New terminal session / Don’t show again
- [x] Share session modals and role-request copy (headers, menus, Request edit access; role modal body already Message)
- [x] OSC52 clipboard banner / AWS Bedrock login banner / Use-agent footer (Dismiss / Don’t show again / resume tooltip)
- [x] Cloud agent loading: GitHub Authentication Required / Failed to start environment / cancelled / footer / first-time setup

### Other terminal

- [ ] Available shell display names (Windows PowerShell / WSL…) — proper nouns may stay English
- [x] Secret display: Always show secrets / Asterisks / Strikethrough
- [x] Working directory: Home directory / Previous session’s directory / Custom directory
- [x] Warpify SSH install policy: Always ask / Always install / Never install

### Conversation list / Tools panel / Search (P3c)

- [x] Convert conversation list empty states (`No conversations yet`, `No matching conversations`, `New conversation`) via Message
- [x] Convert Tools panel "Agent conversations" label via Message
- [x] Convert workspace search placeholder "Search sessions, agents, files..." via Message
- [ ] "Warp Drive" kept as product name (plan: untranslated)
- [x] Tab empty states (`No tabs open`, `No tabs match your search`, `View as`, `Tab item`) — via Message
- [x] Billing: `No usage history` — wired via existing `BillingNoUsageHistory` Message
- [x] Transfer ownership modal button — wired via `TeamsTransfer` Message

### Terminal context menu (B1)

- [x] Wire `context_menu.rs` through `terminal_menu_fields` bridge
- [x] Add missing arms: Copy output as Markdown, Save as prompt, Copy share link, Share conversation, Copy conversation text, Fork, Fork from here
- [x] Route dynamic `button_text`/`fork_label` through `terminal_menu_text`

### AI context menu categories (B2)

- [x] Add `localized_name()` to `AIContextMenuCategory`
- [x] Wire display call sites to use localized category names

### Agent tips (B4)

- [x] Add `localized_tip_description` match bridge (37 descriptions)
- [x] "Tip: " prefix → "提示：" in ZhCn

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
| Already i18n (1409 `Message`s) | Done | ~1400+ |
| P0 first-run / login / anonymous | Done | ~80–120 |
| P1 Settings residuals | Done | ~80–120 |
| P2 full Settings pages | Done (Teams confirm dialogs + Environments residual) | ~250–400 |
| P3 Workspace / Terminal | High-frequency done; shell display names + launch modals open | ~300–500 |
| Engineering debt (dual-track / Test connection) | Done | — |
| **Remaining** | | **~30–80** (shell proper nouns; launch/marketing modals; SettingsSection Display deferred; long-tail Settings) |

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
2. **Dual-track category titles** — ~~`Category::new("English").with_localized_title`~~ replaced by `Category::localized(Message, …)` on Appearance / Features / Code / Warpify. English `Category::new` remains only where there is no Message yet.
3. **`SettingsSection` `Display` / `FromStr`** still use English names; anything parsing English section titles may disagree with localized UI labels. **Deferred** (out of this checklist sprint; not user-visible chrome by itself).
4. **Anonymous-mode high-frequency copy** — toast / missing-provider error / HTTP plaintext warning use `Message`; onboarding/auth P0 done.
5. **Guardrails to add/keep:**
   - Existing test: `all_messages_have_non_empty_text` in `table.rs`
   - Unit tests for `locale_from_system_tag` in `i18n/mod.rs`
   - Suggested: no duplicate match arms; English table must not contain CJK; new UI strings require a `Message` entry

---

## Suggested sprint order

1. ~~Fix **System locale** → follow OS language when possible~~ **Done**
2. ~~**P0** — Onboarding + Auth + anonymous toast / provider setup~~ **Done** (incl. Test connection)
3. ~~P1 settings high-frequency~~ done (incl. dual-track Category cleanup)
4. ~~**P2** Privacy / Account / Environments residual / Teams~~ done
5. ~~**P3** high-frequency Terminal input / banners / share~~ done
6. ~~**P3 residual** secrets / AI dropdown values / appearance fonts / Additional metadata / Shared blocks / pane titles / Search repos~~ done
7. Process: new user-visible strings must go through `Message` + En/Zh tables

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

_Last updated: 2026-07-17. Batch 8: Appearance dropdown values, conversation list overflow menu, MCP action buttons/toasts, default-model modal, privacy Cancel, agent-assisted Add repo, execution profile Edit, AWS Refresh. Message catalog 1409 variants._
