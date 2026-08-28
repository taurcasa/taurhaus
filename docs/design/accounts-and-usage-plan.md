# Accounts and usage across CLIs — architecture and execution plan

Status: approved design, executed 2026-08-27 → 2026-08-28 (PRs 17a–17c, 18a, 18b, 19). 17d was cancelled and 17e was folded into the 0.7.0 release. Accounts and usage (daemon protocol 11) shipped in **0.7.0**; the Antigravity and Grok harnesses (daemon protocol 12 then 13) are merged to `main` and sit under `[Unreleased]`, targeted at **0.8.0** — the repo is still at 0.7.0 until that version bump and release land. Companion to [`harness-realignment-plan.md`](harness-realignment-plan.md) and [`../architecture/harness-model.md`](../architecture/harness-model.md).

## Goal

One flow for every CLI taurhaus launches:

1. **Detect** the accounts a tool is signed into (one config dir per account).
2. **Choose** the account per project — remembered per project, defaulting to the one the project used last, with a visible global default — and per launch from the sidebar's right-click menu, without a modal in the way.
3. **Resume** on the account that owns the session's history.
4. **Show usage** per account the way the tool's own status screen shows it (Claude Code `/usage`: session · week all models · week Fable; Codex `/status`: 5h · weekly, per model family; Antigravity `/usage`: weekly and 5-hour buckets per model group). A tool with no quota surface reports usage as unavailable and the registry carries the sentence the UI shows in a meter's place — Grok is the current case.

Adding a tool touches only its account/usage slice. Everything else — pins, resolution, launch rendering, resume derivation, chooser, chip, meter, context menu, settings — is tool-agnostic and data-driven from the registry.

## Verified facts (2026-08-27 for Claude/Codex, 2026-08-28 for Antigravity/Grok)

The Gemini CLI column that stood here is gone: Google refused that client for individual Code Assist accounts, 17d was cancelled, and the Google harness became the Antigravity CLI in 18a. The Antigravity and Grok columns are the providers implemented on `main` (`session_scanner/accounts/agy.rs`, `accounts/grok.rs`) — merged, not yet released, cross-checked against `docs/design/research/agy-report-{codex,opus}.md` and `grok-report-{codex,opus}.md`.

| | Claude Code 2.1.247 | Codex CLI 0.149.0 | Antigravity CLI 1.1.22 (`agy`) | Grok CLI 1.0.5 (`grok`) |
|---|---|---|---|---|
| Selector (env var, per process) | `CLAUDE_CONFIG_DIR` | `CODEX_HOME` (must exist; canonicalised) | **none** — one implicit account under the shared Google tooling root `~/.gemini`; `candidate_dirs` returns only the default dir | `GROK_HOME` — isolates credentials, config, sessions, the live registry and the leader socket |
| Credential file (key names only) | `.credentials.json` → `claudeAiOauth.{accessToken, expiresAt, subscriptionType, rateLimitTier}` | `auth.json` → `auth_mode`, `tokens.{access_token (JWT), id_token (JWT), refresh_token, account_id}`; expiry = access JWT `exp` | `<root>/antigravity-cli/antigravity-oauth-token` when present, else the shared `<root>/oauth_creds.json`; presence alone decides `logged_in` | `auth.json` (0600), a map keyed `<oidc_issuer>::<client_id>` → `{key, email, first_name, last_name, user_id, principal_id, expires_at}`; `key` is read only to test that a credential is present (trimmed, non-empty) — the value itself is never logged, persisted or sent anywhere (`accounts/grok.rs:85-91`) |
| Identity | `.claude.json` → `oauthAccount.{accountUuid, emailAddress, displayName, organizationName, seatTier}` | `id_token` claims: `email`, `https://api.openai.com/auth.{chatgpt_plan_type, chatgpt_account_id}` | `<root>/google_accounts.json` → `active` (the id *and* the label; no display name, org or plan) | the single `auth.json` record: id = `user_id` → `principal_id` → the canonical account directory; label = `email` → `user_id` → "Grok account"; `first_name`/`last_name` as the display name, `expires_at` as the expiry. A store holding several records is read as **no** account, because grok's own selection rule is unverified |
| Usage source | `GET https://api.anthropic.com/api/oauth/usage` — `Authorization: Bearer`, `anthropic-beta: oauth-2025-04-20` | `GET https://chatgpt.com/backend-api/wham/usage` — `Authorization: Bearer`, `ChatGPT-Account-ID: <tokens.account_id>` | **not HTTP**: `agy -p /usage --output-format json`, run in `<root>/antigravity-cli` with `AGY_CLI_DISABLE_AUTO_UPDATE=true` and a 10 s timeout, through the injectable command boundary (a test asserts the provider never reaches for HTTP) | **none** — no subscription quota endpoint; `/usage` is TUI-only and `grok usage` does not exist. `usage: false` in the registry, with the note "Grok shows credits in its own /usage" |
| Response → windows | `limits[]`: `session`, `weekly_all`, `weekly_scoped` (`scope.model.display_name`, e.g. "Fable"); `percent`, `resets_at` ISO, `severity` | `rate_limit.{primary,secondary}_window` + `additional_rate_limits[]` (`limit_name`, `metered_feature`, same window shape); window kind from `limit_window_seconds` (18000 = 5h, 604800 = weekly), never from primary/secondary | `command.data.groups[].buckets[]` → four fixed keys in provider order: `gemini-weekly`, `gemini-5h`, `3p-weekly`, `3p-5h`, titled *Gemini Models · Weekly/5h* and *Claude and GPT models · Weekly/5h*. `remaining_fraction` is inverted into `used_percentage`; the 5-hour pair is the compact form; an absent group keeps the windows that are there | — |
| External read triggers refresh? | No (GET with stored token; Claude Code refreshes on its own runs) | No (verified: `auth.json` unchanged after GET); 401 → no reactive refresh in 0.149 | No — a `-p` invocation is a headless print run and is not a session | — |
| No-subscription mode | API key → no `rate_limits` | API key → rate limits refused | a signed-out root reports `Unauthorized`; a sign-in diagnostic on a failed run is classified as `Unauthorized`, anything else as `Stale` | per-turn cost and tokens arrive in-band, not as a window |

Fixtures (real, redacted, in the repo — not the scratchpad): `src-tauri/src/daemon/fixtures/claude-oauth-usage-2.1.247.json`, `codex-wham-usage-0.149.json`, `agy-usage-1.1.22.json`. The Codex turn-complete fixture lives beside its consumer at `src-tauri/src/session_scanner/idle/fixtures/codex-agent-turn-complete-0.149.0.json`.

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
    fn account_provider(&self) -> Option<&'static dyn AccountProvider>;   // Some for every registered tool; independent of account_selector
    fn usage_provider(&self)   -> Option<&'static dyn UsageProvider>;     // Some iff usage
}

pub trait AccountProvider: Sync {
    fn default_dir(&self, home: &Path) -> PathBuf;                                  // ~/.claude, ~/.codex, ~/.gemini (agy), ~/.grok
    fn candidate_dirs(&self, home: &Path, live_selector_values: &[PathBuf]) -> Vec<PathBuf>;   // default + siblings (`<default>-*`) + live processes' selector values, canonical, deduped
    fn identify(&self, dir: &Path) -> Option<AccountIdentity>;                       // id, label(email), display_name, org, plan, logged_in, credential_expires_at
    fn session_dir(&self, transcript: &Path) -> Option<PathBuf>;                     // resume derivation; None = unknown
}
pub trait UsageProvider: Sync {
    fn fetch(&self, dir: &Path, http: &dyn HttpClient) -> UsageSnapshot;              // reads the credential file itself; classifies errors
}
pub struct UsageSnapshot { observed_at: DateTime<Utc>, status: UsageStatus /* Ok | Stale | Unauthorized | Unsupported */, windows: Vec<UsageWindow>, note: Option<String> }
pub struct UsageWindow { key: String, title: String, used_percentage: f64, resets_at: Option<i64>, severity: Severity /* Normal | Warning | Critical */, is_active: bool, compact: bool }
```

Floor: a tool without `account_selector` has one implicit account (no chooser, no chip, no submenu); without `usage` it has no meter. The registry's `match` is the only place tool identity fans out. `AccountSource` (the existing launch-provenance enum, whose wire strings are logged and shipped) is renamed `AccountOrigin` first — its strings stay.

## Generic core

- **Detection**: `accounts::detect(tool) -> Vec<Account { tool, id, dir, identity, is_default }>`, cached 60 s per tool; daemon method `list_accounts { tool }` (the Claude-only methods are removed; **protocol 11**, app + daemon shipped together in 0.7.0; 18a and 18b took the vocabulary to 12 and 13). Unsupported vs unavailable stay distinguishable (empty + `degraded:false` vs `degraded:true`).
- **Project memory**: side table `project_tool_accounts(project_id, tool, account_id, origin TEXT CHECK(origin IN ('pinned','last_used')), updated_at)`; migration 013 copies `projects.claude_account_id` as `('claude', id, 'pinned')` (column left in place, no longer read). `last_used` is upserted on every taurhaus launch (after resolution) and whenever the scanner binds a live session of that tool to a project (selector value read from the process environment, as the Claude registry already does) — throttled, only on change. `pinned` is written by the chooser's "remember", the chip, and the context-menu `Account` submenu; "Use default" deletes the row.
- **Resolution** (`resolve_launch_account`, tool-agnostic): explicit pick → session's dir (resume/continue with a known transcript) → `pinned` → `last_used` → global default (`settings.default_account_ids[tool]`) → selector already inside the user's base command (e.g. `claude2`) → tool default dir. The result carries `origin` so the UI can say *why* ("last used here", "from your launch command", "default dir"). Missing/logged-out targets fall back with `launch.account.fallback`.
- **Launch rendering**: `LaunchSpec.account_dir` rendered as `<SELECTOR>='<dir>' <command>` for any tool with a selector (hoisted out of the Claude arm, data-driven); base wins if it already sets the variable (`LaunchNote::SelectorIgnored`); goldens per tool.
- **Usage poller** (daemon on Windows because the config dirs live in WSL; app natively): per (tool, account); 60 s while that account has a live session, 10 min otherwise, immediately (5 s debounce) when a chooser/chip/context menu opens; one in-flight per account; backoff 60 s → 5 min on failures; `unauthorized` until the credential file's mtime changes. HTTP via `reqwest` (already in the graph through Tauri; match its TLS features; blocking client on the poller thread; 5 s timeout). Events `usage.fetched {tool, account_id, status, window_count}` (debug) / `usage.failed {tool, account_id, kind}` (warn once per state change) — never tokens, never URLs with query strings.
- **Frontend**: `accounts.svelte.js` keyed by tool (accounts, pins, usage, pending chooser); `AccountChooser`, `AccountChip`, `UsageMeter` (full: one bar per window with the tool's titles, `n% used`, `Resets <local time>`, severity tones; compact: the weekly buckets only, e.g. `All 28% · Fable 29%`), Settings → **Accounts** grouped by tool with the effective default and its origin made visible, sidebar context-menu submenus on every launch item of every tool with a selector **and at least two signed-in accounts** (`accountSubmenuApplies`, `src/lib/accountMenu.js:59-63`), session rows labelled with their account. Names/accents from `toolRegistry.js`.

## Per-tool providers

- **Claude**: `AccountProvider` = today's `claude_accounts.rs` moved behind the trait; `UsageProvider` = OAuth usage endpoint; windows from `limits[]` in Claude Code's order and titles (`Current session`, `Current week (all models)`, `Current week (Sonnet only)` on plans that report it, `Current week (<display_name>)` per `weekly_scoped`); severity from `severity`; `note` from a promo/notice field if the payload carries one.
- **Codex**: candidates `~/.codex`, `~/.codex-*`, live `CODEX_HOME`s; identity from the `id_token` payload (base64url JSON, unverified — display only), `auth_mode == "chatgpt"` ⇒ usage-capable, API-key mode ⇒ account without usage; `session_dir` from the rollout path (`<home>/sessions/…`); usage from `wham/usage`: windows `codex` primary/secondary (titled like the TUI: `5h limit`, `Weekly limit`, kind from `limit_window_seconds`) then one pair per `additional_rate_limits[]` titled `<limit_name> · 5h/weekly`; `credits`/`spend_control` → `note`. Expiry from the access JWT `exp`.
- ~~**Gemini CLI**~~ — superseded (2026-08-28): Gemini Code Assist for individuals rejects the client; the Google harness is the Antigravity CLI. The `GEMINI_CLI_HOME` registry data from 17b was removed with the Gemini entry in 18a.
- **Antigravity CLI** (`agy`, PR 18a, merged to `main`, unreleased): no selector, so `candidate_dirs` returns only `~/.gemini` and there is exactly one implicit account — no chooser, no submenus, no pin. `AccountProvider::identify` reads `google_accounts.json` → `active` for both the id and the label, and treats the presence of `antigravity-cli/antigravity-oauth-token` (or the shared `oauth_creds.json`) as signed in; `session_dir` returns `None`, so a resume derives no account. `UsageProvider` is the one command-backed provider: `agy -p /usage --output-format json` through the injectable command boundary, mapped to the four fixed buckets above.
- **Grok CLI** (`grok`, PR 18b): `GROK_HOME` selects a whole account — credentials, config, sessions, the live registry and the leader socket. `AccountProvider` reads the single record in `auth.json` (a map keyed `<oidc_issuer>::<client_id>`): `email` as the label (falling back to `user_id`, then "Grok account"), `first_name`/`last_name` as the display name, `user_id` as the id (falling back to `principal_id`, then the canonical account directory), `expires_at` as the credential expiry; the `key` is read only to test that a credential is present (trimmed, non-empty), never for its value, which is not logged or persisted. A record without a `key`, an expired one, or a store that has grown several records is not a launchable account. **No `UsageProvider`**: grok 1.0.5 exposes no subscription quota endpoint (`/usage` is TUI-only, `grok usage` does not exist) and reports cost and tokens per turn in-band, so `usage: false` and the registry carries the sentence Settings shows where a meter would be. Per-session context use is a later addition, not a window.

## PRs (lanes as always: one family implements, the other reviews; Fable writes the spec and makes the merge call)

| PR | Scope | Implementer / reviewers |
|---|---|---|
| 17a | Popup placement bug (reproduced with the new `just visual-shot` Edge-headless lane), `ContextMenu` submenus, account submenus on every Claude launch item + `Claude account` submenu, `requestClaudeLaunch({accountId})`; built on the existing store but with tool-parameterised menu building | Opus / Codex ×2 |
| 17b | Core generalisation: providers, generic detection/pins/last-used/resolution/launch/resume, Claude providers (OAuth usage), status-line bridge removal + one-shot uninstall, protocol 11, generic frontend store/components/settings, conformance + guards | Codex / Opus ×2, Fable boundary review |
| 17c | Codex provider (accounts + usage) | Codex / Opus ×2 |
| ~~17d~~ | **Cancelled 2026-08-28** — Gemini Code Assist for individuals now refuses the Gemini CLI client ("migrate to the Antigravity suite"); the Google harness becomes the Antigravity CLI (`agy`), see 18a | — |
| 18a | **Antigravity CLI (`agy`) integration, Gemini CLI removed everywhere**: registry entry + every capability slice (process signature, launch flags incl. `--dangerously-skip-permissions` as the auto-approve, model/effort, continue/resume-by-conversation, identity, busy/idle, delivery + wake, stop), account/usage provider, frontend descriptor/logo/accent, goldens + conformance; Gemini CLI deleted from registry, launch arm + golden, idle heuristic, catalog, adapters, frontend, fixtures. The compaction-hook, transcript-parser and transcript-compaction-signal slices were reviewed and **declared unsupported**, not implemented — `cli_tool.rs:386-390` sets `compaction_hook`, `transcript_parser` and `transcript_compaction_signals` to `false`, so agy compaction is not observed | Codex / Opus ×2 (research: Opus + Codex independently, 2026-08-28) |
| 18b | **Grok CLI (`grok`) integration** (new tool): same slice set as 18a — `--always-approve` auto-approve, `--model`/`--effort` with per-model validation, `--continue`/`--resume {session_id}`, `active_sessions.json` identity, `events.jsonl` activity, `/quit` stop, `GROK_HOME` accounts (no usage provider: no quota endpoint), compaction hooks with grok's camelCase envelope and a dedupe for the Claude registration it imports, Grok icon + graphite accent in the sidebar context menu, chips, mesh nodes, team builder and settings. ACP/leader delivery and usage windows are deliberately out of scope | Opus / Codex ×2 (research: Opus + Codex independently, 2026-08-28) |
| 19 | **Docs sweep**: every Gemini CLI reference removed or rewritten for `agy`, Grok added, accounts/usage documented (README, ARCHITECTURE, CLAUDE.md, CONTRIBUTING, `docs/**`, testing/visual guides, CHANGELOG, taureval role notes); Opus drift sweep, Codex claim verification, Fable narrative (harness-model slice table Google/xAI columns) | Opus + Codex / Fable |
| ~~17e~~ | **Folded into the 0.7.0 release** (app + daemon, protocol 11), shipped 2026-08-28 before 18a/18b. 18a and 18b carried protocol 12 and 13; the release that will ship them is 0.8.0, planned after 19 | Fable |

## Ledger

| PR | Implementer | Reviewers | Rounds | Majors found | Merged |
|---|---|---|---|---|---|
| 17a | Opus 5 | Codex ×2 | 4 (3 fix rounds; the last major — team-delegated Continue/Resume silently ignoring the pick — fixed by the orchestrator's pass) | 13 (round 1: 5 of 7 reported, both reviewers raising the pin-on-pick and the duplicate-label crash; round 2: 4; round 3: 3; round 4: 1) | #34 |
| 17b | Codex gpt-5.6 | Opus ×2 | 5 (4 fix rounds; final approve with one minor carried into 17c) | 12 incl. 1 blocker (round 1: 7 — duplicate `weekly_scoped` keys crashing the meter, fire-and-forget usage, `retire_once` on the startup path, SQLite in the scanner hot path, dead session label, superseded code kept; round 2: 4 — daemon DB ownership across drvfs, refresh RPC past the daemon timeout, unknown-project throttle, second TLS stack; round 3: 1 — usage-sync retry flood) | #35 |
| 17c | Codex gpt-5.6 | Opus ×2 | 3 (2 fix rounds; final approve with two minors fixed by the orchestrator's pass) | 2 (round 1 adversarial: `identify()` invented a logged-in "API key" account from any parseable `auth.json`; duplicate account ids from the `id_token` workspace claim crashed keyed lists) | #38 (merged with a red lint gate by orchestrator error — unused import fixed forward on `main`) |
| ~~17d~~ | — | — | cancelled | — | — |
| 18a | Codex gpt-5.6 (3 turns) | Opus ×2 | 4 (3 fix rounds) | 10 incl. 1 blocker (persisted `gemini` values aborting whole records on upgrade; protocol vocabulary without a bump; no recency bound on hook-fed activity; forced then missing `--dangerously-skip-permissions`; `{session_id}` never substituted; Windows hooks path; catalog id truncation; hook sink re-parsed per poll) | #39 |
| 18b | Opus 5 | Codex ×2 | 7 (3 fix rounds + 2 fix-only rounds + orchestrator fix) | 12 (compaction event value mismatch across two rounds; hook reconciliation on roster changes; resume under alternate `GROK_HOME`/after exit; runtime identity disabled; stop proof precedence; argv boundaries lost by the Linux inventory; trailing COMMAND after the prompt) | #40 |
| 19 | Opus 5 (drift sweep, 35 files) + Fable 5 (narrative) | Codex ×4 (claim verification) | 4 verification rounds; the orchestrator settled the last seven findings | 60+ raised over four rounds (wrong counts: 89→90 IPC commands, protocol 10/11→13, 27→28 daemon methods; release-status wording; PostCompact routing; Codex hooks; analyzer claims; per-CLI account/usage qualifications); 8 infographics remain `stale` pending regeneration | #41 |

## Release debt carried into 0.8.0

Known gaps that are accepted, not fixed, and must be re-read before the 0.8.0 release:

- **Grok compaction has no scripted end-to-end lane.** `just test-compaction` accepts `claude` and `codex` only (`justfile:126-140`), so grok's unique path — its own `PostCompact` event, stdout discarded, delivery through the mesh inbox, and the dedupe for the `~/.claude/settings.json` registration it imports — is verified by hand only (`docs/operations/compaction-testing.md:49-56`). Coverage is manual until a `test-compaction-grok` script exists.
- **Antigravity compaction is out of scope by declaration, not by omission** — see the 18a row above.

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
