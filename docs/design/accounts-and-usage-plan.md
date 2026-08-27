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
| 17a | Opus 5 | Codex ×2 | tbd | tbd | tbd (`feat/pr17a-accounts-menu`) |
| 17b | Codex gpt-5.6 | Opus ×2 | tbd | tbd | tbd |
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
