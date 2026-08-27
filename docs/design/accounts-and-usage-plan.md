# Accounts and usage across CLIs — architecture and execution plan

Status: approved design, execution 2026-08-27 → (PRs 17a–17e, release 0.6.9). Companion to [`harness-realignment-plan.md`](harness-realignment-plan.md) and [`../architecture/harness-model.md`](../architecture/harness-model.md).

## Goal

One flow for every CLI taurhaus launches:

1. **Detect** the accounts a tool is signed into (one config dir per account).
2. **Choose** the account per project — remembered per project, defaulting to the one the project used last, with a visible global default — and per launch from the sidebar's right-click menu, without a modal in the way.
3. **Resume** on the account that owns the session's history.
4. **Show usage** per account the way the tool's own status screen shows it (Claude Code `/usage`: session · week all models · week Fable; Codex `/status`: 5h · weekly, per model family; Gemini `/stats`: per-model quota).

Adding a tool touches only its account/usage slice. Everything else — pins, resolution, launch rendering, resume derivation, chooser, chip, meter, context menu, settings — is tool-agnostic and data-driven from the registry.

## Verified facts (2026-08-27)

| | Claude Code 2.1.247 | Codex CLI 0.149.0 | Gemini CLI 0.57.0 (from source; not installed here) |
|---|---|---|---|
| Selector (env var, per process) | `CLAUDE_CONFIG_DIR` | `CODEX_HOME` (must exist; canonicalised) | `GEMINI_CLI_HOME` (config dir is `<home>/.gemini`) |
| Credential file (key names only) | `.credentials.json` → `claudeAiOauth.{accessToken, expiresAt, subscriptionType, rateLimitTier}` | `auth.json` → `auth_mode`, `tokens.{access_token (JWT), id_token (JWT), refresh_token, account_id}`; expiry = access JWT `exp` | `.gemini/oauth_creds.json` → `{access_token, refresh_token, id_token, expiry_date}` (plaintext unless `GEMINI_FORCE_ENCRYPTED_FILE_STORAGE`) |
| Identity | `.claude.json` → `oauthAccount.{accountUuid, emailAddress, displayName, organizationName, seatTier}` | `id_token` claims: `email`, `https://api.openai.com/auth.{chatgpt_plan_type, chatgpt_account_id}` | `.gemini/google_accounts.json` → `{active, old[]}`; `settings.json` → `security.auth.selectedType` |
| Usage endpoint | `GET https://api.anthropic.com/api/oauth/usage` — `Authorization: Bearer`, `anthropic-beta: oauth-2025-04-20` | `GET https://chatgpt.com/backend-api/wham/usage` — `Authorization: Bearer`, `ChatGPT-Account-ID: <tokens.account_id>` | `POST https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota` — `Authorization: Bearer`, body `{"project": "<id>"}` (project id from `GOOGLE_CLOUD_PROJECT` or `loadCodeAssist`) |
| Response → windows | `limits[]`: `session`, `weekly_all`, `weekly_scoped` (`scope.model.display_name`, e.g. "Fable"); `percent`, `resets_at` ISO, `severity` | `rate_limit.{primary,secondary}_window` + `additional_rate_limits[]` (`limit_name`, `metered_feature`, same window shape); window kind from `limit_window_seconds` (18000 = 5h, 604800 = weekly), never from primary/secondary | `buckets[]`: `remainingFraction`, `resetTime`, `modelId` (`remainingAmount` omitted when full) |
| External read triggers refresh? | No (GET with stored token; Claude Code refreshes on its own runs) | No (verified: `auth.json` unchanged after GET); 401 → no reactive refresh in 0.149 | No (plain POST); CLI rewrites creds on its own refresh |
| No-subscription mode | API key → no `rate_limits` | API key → rate limits refused | API key / Vertex → no Code Assist quota |

Fixtures (real, redacted): `scratchpad/oauth-usage-response.json` (Claude), `scratchpad/codex-usage-response.json` (Codex); Gemini shape from source only.

## Principles

- Harness-native: the tool's own credential file, its own usage endpoint, its own selector env var. No TUI scraping, no settings-file edits (the 0.6.8 status-line bridge is retired — it could never carry per-model buckets and it edited user config).
- Tokens belong to the tool: read at request time, memory only, never logged, never persisted, **never refreshed** by taurhaus. A 401 or an expired token marks the account `unauthorized`; polling resumes when the credential file changes (the tool refreshes on its next run).
- Usage is a normalised, ordered list of windows titled the way the tool titles them. The UI never knows what "Fable" or "weekly" means.
- Capabilities live in the registry; consumers branch on capability, never on tool identity. The conformance suite and literal guards cover the new slices.

## Capability slices

```rust
// registry (session_scanner/cli_tool.rs)
pub struct CliCapabilities { …, account_selector: Option<&'static str> /* env var */, usage: bool, … }
impl CliToolSpec {
    fn account_provider(&self) -> Option<&'static dyn AccountProvider>;   // Some iff account_selector.is_some()
    fn usage_provider(&self)   -> Option<&'static dyn UsageProvider>;     // Some iff usage
}

pub trait AccountProvider: Sync {
    fn default_dir(&self, home: &Path) -> PathBuf;                                  // ~/.claude, ~/.codex, ~ (Gemini: the HOME override)
    fn candidate_dirs(&self, home: &Path, live_selector_values: &[PathBuf]) -> Vec<PathBuf>;   // default + siblings (`<default>-*`) + live processes' selector values, canonical, deduped
    fn identify(&self, dir: &Path) -> Option<AccountIdentity>;                       // id, label(email), display_name, org, plan, logged_in, credential_expires_at
    fn session_dir(&self, transcript: &Path) -> Option<PathBuf>;                     // resume derivation; None = unknown
}
pub trait UsageProvider: Sync {
    fn fetch(&self, dir: &Path, http: &dyn HttpClient) -> UsageSnapshot;              // reads the credential file itself; classifies errors
}
pub struct UsageSnapshot { observed_at: DateTime<Utc>, status: UsageStatus /* Ok | Stale | Unauthorized | Unsupported */, windows: Vec<UsageWindow>, note: Option<String> }
pub struct UsageWindow { key: String, title: String, used_percentage: f64, resets_at: Option<i64>, severity: Severity /* Normal | Warning | Critical */, is_active: bool }
```

Floor: a tool without `account_selector` has one implicit account (no chooser, no chip, no submenu); without `usage` it has no meter. The registry's `match` is the only place tool identity fans out. `AccountSource` (the existing launch-provenance enum, whose wire strings are logged and shipped) is renamed `AccountOrigin` first — its strings stay.

## Generic core

- **Detection**: `accounts::detect(tool) -> Vec<Account { tool, id, dir, identity, is_default }>`, cached 60 s per tool; daemon method `list_accounts { tool }` (the Claude-only methods are removed; **protocol 11**, app + daemon ship together in 0.6.9). Unsupported vs unavailable stay distinguishable (empty + `degraded:false` vs `degraded:true`).
- **Project memory**: side table `project_tool_accounts(project_id, tool, account_id, origin TEXT CHECK(origin IN ('pinned','last_used')), updated_at)`; migration 013 copies `projects.claude_account_id` as `('claude', id, 'pinned')` (column left in place, no longer read). `last_used` is upserted on every taurhaus launch (after resolution) and whenever the scanner binds a live session of that tool to a project (selector value read from the process environment, as the Claude registry already does) — throttled, only on change. `pinned` is written by the chooser's "remember", the chip, and the context-menu `Account` submenu; "Use default" deletes the row.
- **Resolution** (`resolve_launch_account`, tool-agnostic): explicit pick → session's dir (resume/continue with a known transcript) → `pinned` → `last_used` → global default (`settings.default_account_ids[tool]`) → selector already inside the user's base command (e.g. `claude2`) → tool default dir. The result carries `origin` so the UI can say *why* ("last used here", "from your launch command", "default dir"). Missing/logged-out targets fall back with `launch.account.fallback`.
- **Launch rendering**: `LaunchSpec.account_dir` rendered as `<SELECTOR>='<dir>' <command>` for any tool with a selector (hoisted out of the Claude arm, data-driven); base wins if it already sets the variable (`LaunchNote::SelectorIgnored`); goldens per tool.
- **Usage poller** (daemon on Windows because the config dirs live in WSL; app natively): per (tool, account); 60 s while that account has a live session, 10 min otherwise, immediately (5 s debounce) when a chooser/chip/context menu opens; one in-flight per account; backoff 60 s → 5 min on failures; `unauthorized` until the credential file's mtime changes. HTTP via `reqwest` (already in the graph through Tauri; match its TLS features; blocking client on the poller thread; 5 s timeout). Events `usage.fetched {tool, account_id, status, window_count}` (debug) / `usage.failed {tool, account_id, kind}` (warn once per state change) — never tokens, never URLs with query strings.
- **Frontend**: `accounts.svelte.js` keyed by tool (accounts, pins, usage, pending chooser); `AccountChooser`, `AccountChip`, `UsageMeter` (full: one bar per window with the tool's titles, `n% used`, `Resets <local time>`, severity tones; compact: the weekly buckets only, e.g. `All 28% · Fable 29%`), Settings → **Accounts** grouped by tool with the effective default and its origin made visible, sidebar context-menu submenus on every launch item of every tool with a selector, session rows labelled with their account. Names/accents from `toolRegistry.js`.

## Per-tool providers

- **Claude**: `AccountProvider` = today's `claude_accounts.rs` moved behind the trait; `UsageProvider` = OAuth usage endpoint; windows from `limits[]` in Claude Code's order and titles (`Current session`, `Current week (all models)`, `Current week (Sonnet only)` on plans that report it, `Current week (<display_name>)` per `weekly_scoped`); severity from `severity`; `note` from a promo/notice field if the payload carries one.
- **Codex**: candidates `~/.codex`, `~/.codex-*`, live `CODEX_HOME`s; identity from the `id_token` payload (base64url JSON, unverified — display only), `auth_mode == "chatgpt"` ⇒ usage-capable, API-key mode ⇒ account without usage; `session_dir` from the rollout path (`<home>/sessions/…`); usage from `wham/usage`: windows `codex` primary/secondary (titled like the TUI: `5h limit`, `Weekly limit`, kind from `limit_window_seconds`) then one pair per `additional_rate_limits[]` titled `<limit_name> · 5h/weekly`; `credits`/`spend_control` → `note`. Expiry from the access JWT `exp`.
- **Gemini** (experimental, fixture-driven until a host with the CLI verifies it): candidates `~` (home) and `~/.gemini-homes/*`? — no: candidates are `GEMINI_CLI_HOME` values seen live plus the default home; identity from `google_accounts.json.active`; usage only when a project id is derivable (`GOOGLE_CLOUD_PROJECT`, else `Unsupported` with a note); windows per `buckets[]` (`<modelId>`, `used = 100 − remainingFraction·100`, `resetTime`).

## PRs (lanes as always: one family implements, the other reviews; Fable writes the spec and makes the merge call)

| PR | Scope | Implementer / reviewers |
|---|---|---|
| 17a | Popup placement bug (reproduced with the new `just visual-shot` Edge-headless lane), `ContextMenu` submenus, account submenus on every Claude launch item + `Claude account` submenu, `requestClaudeLaunch({accountId})`; built on the existing store but with tool-parameterised menu building | Opus / Codex ×2 |
| 17b | Core generalisation: providers, generic detection/pins/last-used/resolution/launch/resume, Claude providers (OAuth usage), status-line bridge removal + one-shot uninstall, protocol 11, generic frontend store/components/settings, conformance + guards | Codex / Opus ×2, Fable boundary review |
| 17c | Codex provider (accounts + usage) | Codex / Opus ×2 |
| 17d | Gemini provider (fixture-driven, experimental flag) | Opus / Codex ×2 |
| 17e | Docs (harness-model, CLAUDE.md rows, CHANGELOG) + release 0.6.9 (app + daemon, protocol 11) | Fable |

## Ledger

| PR | Implementer | Reviewers | Rounds | Majors found | Merged |
|---|---|---|---|---|---|
| 17a | Opus 5 | Codex ×2 | 4 (3 fix rounds; the last major — team-delegated Continue/Resume silently ignoring the pick — fixed by the orchestrator's pass) | 13 (round 1: 5 of 7 reported, both reviewers raising the pin-on-pick and the duplicate-label crash; round 2: 4; round 3: 3; round 4: 1) | #34 |
| 17b | Codex gpt-5.6 | Opus ×2 | 2 (1 fix round) | 10 (round 1: 6; round 2: 4 — daemon DB ownership, refresh RPC timeout, unknown-project throttle, TLS stack) | tbd |
| 17c | Codex gpt-5.6 | Opus ×2 | tbd | tbd | tbd |
| 17d | Opus 5 | Codex ×2 | tbd | tbd | tbd |

## 17a findings

The popup placement bug, reproduced with real renders through the new
`just visual-shot` lane (Edge headless at the three viewport presets, app frame
markup around the fixture):

- **Chooser** — `app.css` gives every direct child of `.shell-frame`
  `position: relative` unless it carries `data-shell-overlay`. The chooser
  overlay added in `c982822` did not, so `position: fixed` was overridden and
  the dialog became the last item of the frame's flex column: bottom of the
  window, half cut off, exactly as reported. `SearchOverlay` and
  `AddProjectModal` already opt out; the chooser now owns its own overlay root
  (with the attribute) so a caller cannot forget it again.
- **Chip menu** — positioned `absolute` inside the Overview header, so it was
  laid out against whatever ancestor happened to be positioned and clipped by
  the main panel's `overflow-hidden`. Now measured and clamped against the
  viewport like `ContextMenu` (flip above the chip, clamp both edges,
  reposition on scroll/resize).
- **Visual host** — `VisualHost.svelte` bumped a `renderVersion` counter inside
  its `{#key}` from an effect, mounting every fixture twice. A component that
  measures itself in an effect lost that measurement to the remount and
  rendered at 0,0 — which is why a viewport-anchored popup could not be shot at
  all. Mocks are now applied in a derived key, so a fixture mounts once.

### Review round 1

Five majors, all confirmed and fixed on the branch:

- **A launch row pinned the project.** `requestClaudeLaunch` defaulted to
  remembering an explicit account, so picking a subscription for one launch
  moved every later launch with it. A pin is written where this plan says it is
  — the chooser's remember, the chip, the `Claude account` submenu — and a
  launch row is now one launch.
- **Two accounts with the same display name crashed the submenu.** The flyout
  keyed its rows by the label, which the account rows derive from a
  non-unique display name; Svelte threw `each_key_duplicate` and no submenu
  rendered at all (reproduced in Edge, see the PR's before shot). Rows now carry
  their own key, and a repeated name is qualified by the email, or by the config
  dir when one subscription is signed into two of them.
- **The chooser fixture did not reproduce the mount point.** It kept a wrapper
  around a component that owns its overlay: two overlays, and the overlay no
  longer a direct child of `.shell-frame` — so the fixture could not have caught
  the bug it was written for. It now mounts exactly as `Shell.svelte` does, and
  a browser assertion holds the shape (one overlay, `fixed`, direct child).
- **`visual-shot` could succeed on the wrong page.** Any listener on the port
  was reused, an unknown fixture fell back to the first one, Edge's exit status
  was discarded and the process was unbounded. The lane now checks the host's
  identity, reads the rendered fixture back out of the same run's `--dump-dom`,
  honours the exit status, and runs under a wall clock — with tests driving the
  real script against a fake server and a fake browser.
- **The menu did not re-clamp after late rows.** Account detection starts when
  the menu opens and adds rows when it answers; the clamp ran once, against the
  size the menu had at the start. A `ResizeObserver` and the window's own resize
  now ask for it again.

Two minors: the tick is gone from `Continue`/`Resume`, whose account the backend
reads off the transcript rather than from the pin the frontend can see; and a
degraded detection is logged on the way into the outage instead of once per
right-click. Detection itself still retries on every opening — an existing
guard (`c982822`) requires a daemon that reconnects to restore the list on the
next right-click, and the call is one cheap daemon round trip.

### Review round 2

Four majors, all confirmed and fixed on the branch:

- **A repeated account id could not reach the dir it advertised.** Round 1's
  answer to the duplicate-label crash gave one subscription signed into two
  config dirs a row per dir, labelled by the dir. Both rows carry the same
  account uuid — the whole address a launch or a pin has — so the
  `.claude-account2` row launched `.claude` and the menu ticked both. Detection
  is now collapsed to one entry per id as it lands in the store, keeping the
  entry the backend resolves that id to (the first that can run), so the chip,
  the chooser and the submenus all offer exactly the choices a launch can
  express. Addressing a second config dir of one subscription needs a launch
  address that names the dir — 17b's resolution work, not a menu label.
- **The chip menu did not move after the usage it asked for arrived.** Opening
  it requests usage; the meters that answer make the menu taller and the chip
  wider, and only scroll and resize asked for the clamp again. A menu opened
  near an edge kept the coordinates its empty size earned. A `ResizeObserver`
  now watches both ends of the anchoring, as `ContextMenu` already did for its
  own rows.
- **The screenshot lane's wall clock did not insist.** Plain `timeout` asks with
  TERM; a hung renderer can ignore it and hold the lane indefinitely.
  `--kill-after` follows with KILL, both timeout statuses are honoured, and the
  fake browser in the test now traps TERM so the case is actually exercised.
- **A shot could be filed under a theme it did not render.** `theme` went into
  the URL unvalidated while the host silently falls back for one it does not
  know, and the rendered identity named only the component and the scenario. The
  script rejects anything but `light`/`dark`, the identity carries all four
  parts (component, scenario, viewport, theme), and the host reports an unknown
  theme or viewport as a fallback like any other.

One more found while re-shooting the fixtures for the above: the visual host
never set the `dark` class on the document that `Shell.svelte` and the
browser-mode lane both set, so every dark shot framed a dark popup in a light
panel. It sets it now — without which the new theme guard would have certified
exactly that.

### Review round 3

Three majors, all confirmed and fixed on the branch:

- **The late rows the earlier fix waited for were the flyout's.** Round 1 and 2
  put a `ResizeObserver` on the root menu, but the rows account detection adds
  are the submenu's: a flyout opened near the bottom edge grew past the edge and
  kept the top its empty size earned, which is the cut-off popup this PR exists
  to fix. The observer now watches both elements, and the flyout's placement
  also reruns when its children change or the root menu re-clamps under it.
- **A held ArrowRight launched on an account nobody chose.** ArrowRight opened
  the flyout and, inside an open one, was treated like Enter — so the key repeat
  of the press that opened it activated the first row. On a restart parent that
  stops a live session before relaunching it. Depth stops at one, so ArrowRight
  inside a flyout is now consumed and does nothing.
- **The screenshot lane did not check the screenshot.** It asked Edge for a
  window size and then only checked that a non-empty file existed, so a browser
  rendering at another size or device scale filed a PNG as evidence about a
  viewport it never showed — and the fake browser in the tests wrote three bytes
  that passed. The lane forces a device scale of 1 and reads the PNG's own IHDR
  back, and the fake browser writes real PNG headers so a wrong size is a red
  test.

One minor with it: the rendered-fixture check matched with a plain `grep`, so a
component or scenario carrying regex metacharacters matched whatever the host
had fallen back to. It matches as a fixed string now.
