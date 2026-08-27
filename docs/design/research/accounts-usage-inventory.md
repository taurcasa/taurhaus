# Claude-specific ACCOUNTS & USAGE inventory — taurhaus

Repo: `~/projects/taurhaus`, branch `main` @ `557533e`. Read-only audit.
All rows below were read from source unless explicitly marked **UNVERIFIED**.

---

## 0. What is already generic (start here)

Three capability flags already exist in the registry and are already consumed as
predicates, not as `CliTool::Claude` comparisons. Generalisation is an
*extension* of an existing seam, not a new abstraction:

| Flag | Declared in | Claude | Codex | Gemini |
|---|---|---|---|---|
| `config_dir_env: Option<&'static str>` | `cli_tool.rs:110` | `Some("CLAUDE_CONFIG_DIR")` | `None` | `None` |
| `account_selection: bool` | `cli_tool.rs:112` | `true` | `false` | `false` |
| `usage_bridge: bool` | `cli_tool.rs:114` | `true` | `false` | `false` |

Call sites that already branch on capability rather than identity:

- `commands/command_center/launching.rs:112-126` — `spec(tool).capabilities.account_selection.then(|| …)` gates the whole account resolution for a launch.
- `session_scanner/claude_accounts.rs:600-613` and `:634-641` — live-session config-dir discovery and transcript sightings filter on `capabilities.account_selection`.
- `session_scanner/launch.rs:362-378` — the prefix uses `capabilities.config_dir_env` for the env-var *name*.
- `src/lib/claudeAccounts.svelte.js:261` — `if (!toolDescriptor(tool)?.capabilities.accountSelection) return run(null)`.

**The gap is not the predicate. It is that every type, function, file, IPC
command, daemon method, DB column, setting key and log event behind those
predicates is spelled `claude`, and that one renderer arm is still nested inside
`match self.tool { CliTool::Claude => … }`.**

---

## 1. Hard-coded-Claude symbol table

### 1a. Backend — accounts

| Kind | Current symbol / literal | File:line | Tool-generic name |
|---|---|---|---|
| module | `session_scanner::claude_accounts` | `src-tauri/src/session_scanner/claude_accounts.rs` | `session_scanner::accounts` |
| struct | `ClaudeAccount` | `claude_accounts.rs:76` | `ToolAccount` (+ `tool: CliTool`) |
| struct field | `ClaudeAccount.email: String` (**required**, detection returns `None` without it, `:354-357`) | `claude_accounts.rs:81` | `label: String` / `email: Option<String>` — Codex `auth.json` has no email |
| struct field | `ClaudeAccount.seat_tier` (from `seatTier`/`organizationType`) | `claude_accounts.rs:86` | `tier: Option<String>` |
| struct field | `ClaudeAccount.is_process_default` | `claude_accounts.rs:96` | keep; means "the dir the CLI reads with the selector unset" |
| struct | `ClaudeScan { config_dirs, accounts }` | `claude_accounts.rs:197` | `AccountScan` |
| **enum** | **`AccountSource`** = `Request \| Session \| Project \| GlobalDefault \| SignedIn \| DefaultConfigDir` | `claude_accounts.rs:132-146` | **NAME COLLISION** — this is launch *provenance*, not a trait. Rename to `AccountOrigin`/`AccountSelectionSource` before the proposed `AccountSource` trait can take the name. |
| struct | `AccountRequest<'a>` (already generic name) | `claude_accounts.rs:164` | keep |
| struct | `AccountResolution` (already generic name) | `claude_accounts.rs:174` | keep |
| fn | `detect_claude_accounts_in` / `_rooted` / `_cached` | `:207 / :227 / :468` | `detect_accounts_*` (take `tool: CliTool`) |
| fn | `scan_claude_config_cached` | `:453` | `scan_accounts_cached(tool)` |
| fn | `transcript_config_dirs()` | `:478` | `session_config_dirs(tool)` |
| fn | `newest_project_transcript(config_dirs, project_path)` | `:566` | keep name; the `projects/<slug>/*.jsonl` layout must come from `CliToolSpec::projects_subdir` + `session_extension`, not the local consts |
| fn | `record_claude_transcripts` / `remembered_claude_transcript` | `:634 / :713` | `record_session_transcripts` / `remembered_transcript(tool, …)` |
| fn | `resolve_launch_account` (already generic name) | `:731` | keep |
| fn | `configured_root_to_name` | `:542` | `configured_root_to_name(tool)` |
| fn | `to_launch_namespace` | `:553` | keep (tool-agnostic already) |
| const | `CONFIG_FILENAME = ".claude.json"` | `:28` | `CliToolSpec.account_config_filename` |
| const | `CREDENTIALS_FILENAME = ".credentials.json"` | `:31` | `CliToolSpec.credentials_filename` (Codex: `auth.json`) |
| const | `DEFAULT_CONFIG_DIRNAME = ".claude"` | `:34` | already exists as `CliToolSpec.base_dir_name` — duplicated here |
| const | `CONFIG_DIRNAME_PREFIX = ".claude-"` | `:37` | `CliToolSpec.sibling_dir_prefix` |
| const | `PROJECTS_SUBDIR = "projects"` | `:40` | already `CliToolSpec.projects_subdir` — duplicated |
| const | `TRANSCRIPT_EXTENSION = "jsonl"` | `:43` | already `CliToolSpec.session_extension` — duplicated |
| json keys | `oauthAccount.{accountUuid,emailAddress,displayName,organizationName,seatTier,organizationType}` | `:114-128` | per-tool identity adapter (the `AccountSource::identify` method) |
| enum | `CredentialStore::{File,Keychain}` + `host_credential_store()` | `:52-68` | keep, but make it a per-tool declaration: macOS-keychain-vs-file is a *tool's* auth model, not only the host's |
| statics | `DETECTION_OVERRIDE`, `DETECTION_OVERRIDE_LOCK`, `install_detection_override`, `install_scan_override` | `:404-449` | per-tool keyed override map |

### 1b. Backend — usage

| Kind | Current symbol / literal | File:line | Tool-generic name |
|---|---|---|---|
| module | `daemon::claude_usage` | `src-tauri/src/daemon/claude_usage.rs` | `daemon::tool_usage` |
| const | `CLAUDE_USAGE_FILENAME = "claude-usage.jsonl"` | `claude_usage.rs:47` | `<tool>-usage.jsonl` from spec |
| struct | `ClaudeUsageWindow { used_percentage, resets_at }` | `:70` | `UsageWindow` — already generic in shape |
| struct | `ClaudeAccountUsage { five_hour, seven_day, observed_at }` | `:82` | **`AccountUsage { windows: Vec<UsageWindow{id,label,…}>, observed_at }`** — `five_hour`/`seven_day` are Claude's product windows, not a universal shape |
| struct | `ClaudeUsageRecord { ts, config_dir, account_id, session_id, five_hour, seven_day }` | `:96` | `UsageRecord { ts, tool, config_dir, account_id, session_id, windows }` |
| struct | `ClaudeUsageAppendOutcome` | `:114` | `UsageAppendOutcome` |
| struct | `StatuslineInput { session_id, model_display, five_hour, seven_day }` | `:125` | Claude-transport-specific; belongs *inside* the Claude `UsageSource` impl |
| fn | `parse_statusline_input`, `render_status_line` | `:184 / :206` | stay Claude-private |
| fn | `append_usage_at`, `latest_usage_records`, `attach_usage_from`, `attach_usage` | `:228 / :521 / :584 / :636` | drop `claude` in path; take `tool` |
| fn | `account_id_for_config_dir` (re-reads `.claude.json`/`oauthAccount.accountUuid` inline, **duplicating** `read_account`) | `:644-654` | fold into `AccountSource::identify` |
| struct | `UsageSinkArgs { config_dir, sink, render }` | `:658` | add `tool` |
| fn | `parse_usage_sink_args`, `run_usage_sink` | `:668 / :707` | per-tool sink subcommand |
| CLI arg errors | `"unknown claude-usage-sink argument '{other}'"` | `:691` | parameterise |

### 1c. Backend — statusline bridge (the Claude usage *transport*)

Whole file `src-tauri/src/session_scanner/claude_statusline.rs` (2933 lines) is
Claude-transport-specific and should become the body of the Claude `UsageSource`
impl. Literals worth naming:

| Item | File:line |
|---|---|
| `SETTINGS_FILENAME = "settings.json"`, `HOOKS_DIRNAME = "hooks"` | `:82-83` |
| `SCRIPT_BASENAME = "taurhaus-statusline"`, `RECORD_FILENAME = "taurhaus-statusline.json"` | `:84-85` |
| `STATUS_LINE_KEY = "statusLine"` | `:86` |
| `USAGE_SINK_SUBCOMMAND = "claude-usage-sink"` (public; named in generated shell scripts on disk) | `:88` |
| `FALLBACK_LINE = "taurhaus · no usage yet"` | `:90` |
| `SINK_DEADLINE_SECONDS = 2`, `COMMIT_ATTEMPTS = 3`, `BRIDGE_PASS_INTERVAL = 60s` | `:94-102` |
| `ensure_statusline_installed_at`, `remove_statusline_at`, `statusline_is_installed_at`, `ensure_statusline_bridge{,_soon}` | `:158 / :339 / :428 / :471,:490` |
| `StatuslineInstall`, `StatuslineRecord` | `:108 / :119` |

### 1d. IPC commands (registered in `src-tauri/src/lib.rs:213-218`)

| IPC command | Params | Impl | Tool-generic |
|---|---|---|---|
| `list_claude_accounts` | none | `commands/claude_accounts/mod.rs:72` | `list_tool_accounts { tool }` |
| `set_project_claude_account` | `projectId`, `accountId?` | `claude_accounts/mod.rs:106` | `set_project_tool_account { projectId, tool, accountId? }` |
| `resolve_claude_launch_account` | `projectId`, `mode` | `command_center/mod.rs:109` | `resolve_launch_account { projectId, tool, mode }` |
| `launch_cli_session` param `claudeAccountId` | | `command_center/launching.rs:25`, `src/lib/ipc/sessions.js:86-93` | `accountId` |
| result struct | `commands::claude_accounts::ClaudeAccountsResult { accounts, source, degraded, error }` | `claude_accounts/mod.rs:41` | `ToolAccountsResult` |
| consts | `SOURCE_NATIVE="native"`, `SOURCE_DAEMON="daemon"` | `:32-34` | keep |
| docs | `docs/architecture/ipc-reference.md:83-89` "Claude account commands" | | |

### 1e. Daemon protocol (`src-tauri/src/daemon/protocol.rs`, `PROTOCOL_VERSION = 10` at `:30`)

| Kind | Symbol | Line | Notes |
|---|---|---|---|
| method const | `LIST_CLAUDE_ACCOUNTS = "list_claude_accounts"` | `:104` | doc-comment: *"Additive since protocol v10 (no version bump)"* |
| method const | `CLAUDE_PROJECT_TRANSCRIPT = "claude_project_transcript"` | `:108` | same additive contract |
| result | `protocol::ClaudeAccountsResult { accounts: Vec<ClaudeAccount> }` | `:147` | |
| params/result | `ClaudeProjectTranscriptParams { project_path }` / `Result { transcript }` | `:157 / :162` | |
| request ids | `"list-claude-accounts"`, `"claude-project-transcript"` | `claude_accounts/mod.rs:206,246` | free-form correlation strings |
| handlers | `handle_list_claude_accounts`, `handle_claude_project_transcript` | `daemon/handlers.rs:97, :115` | dispatch at `:83-86` |

### 1f. DB

| Item | Location | Tool-generic |
|---|---|---|
| migration `012_project_claude_account.sql` → `ALTER TABLE projects ADD COLUMN claude_account_id TEXT` | `src-tauri/src/db/migrations/012_project_claude_account.sql` | see §5 risk 3 |
| column read at ordinal **10** of a positional `row.get(10)` | `db/queries.rs:23`, column lists `:51,:68` | positional reads make adding columns brittle |
| model fields `Project.claude_account_id`, and 2 more structs | `models/mod.rs:95,110,154` (copied `:133`) | `tool_accounts: Map<CliTool,String>` or a side table |
| serialized key `claudeAccountId` | asserted `models/mod.rs:1103` | |
| fn | `queries::set_project_claude_account` | `db/queries.rs` | |

### 1g. Settings

| Item | Location |
|---|---|
| `TerminalSettings.claude_default_account_id: Option<String>` (+ `#[serde(alias="claude_default_account_id")]`) | `models/mod.rs:789-790`, default `:801` |
| frontend normalisation `terminal.claude_default_account_id ?? terminal.claudeDefaultAccountId` | `src/lib/ipc/system.js:269-270` |
| read on the launch path | `command_center/launching.rs:122, :650` |
| `CliVersions.claude_statusline_usage_supported` + `CLAUDE_STATUSLINE_USAGE_MIN_VERSION` gate | `models/mod.rs:304, :373-374`; consumed `claude_statusline.rs:680` |
| frontend mirror of that flag | `src/lib/ipc/system.js:42,137-140` |

### 1h. Log / event names

| Event | Emitted at |
|---|---|
| `launch.account.fallback` | `command_center/launching.rs:554, :572` |
| `launch.account.derived_from_session` | `launching.rs:592` |
| `launch.config_dir.ignored` (`LaunchNote::ConfigDirIgnored`) | `launch.rs:154` |
| `launch.command.rendered` field **`claude_account`** (carries the account **email**) | `launching.rs:148-155` → rename to `account` |
| `claude.usage.statusline.{installed,removed,skipped,failed}` | `claude_statusline.rs:585,605,618,647,659,708` |
| `LaunchCapability::ConfigDir` → wire string `"configDir"` | `launch.rs:127` |
| `AccountSource::as_str()` wire strings `request/session/project/global_default/signed_in/default_config_dir` | `claude_accounts.rs:150-159` |

### 1i. Paths

| Item | `src-tauri/src/provider/platform_paths.rs` |
|---|---|
| `CLAUDE_DIR_OVERRIDE_ENV = "TAURHAUS_CLAUDE_DIR"` | `:10` |
| `claude_usage_path()` | `:42` |
| `claude_dir()`, `claude_dir_override()` | `:47, :57` |
| `teams_dir()` = `claude_dir()/teams` | `:63` |
| `SessionRoot::AppManagedClaudeDir` arm | `:92-93` |
| `claude_hooks_dir()`, `claude_settings_path()` | `:118, :123` |
| `default_claude_dir()` (incl. Windows UNC `.claude`) | `:142-147` |
| Precedent for a second tool: `CODEX_HOME_ENV = "CODEX_HOME"`, `codex_dir()` | `:11, :66-68` |

### 1j. Frontend

| Item | File |
|---|---|
| store module + `claudeAccounts` `$state` | `src/lib/claudeAccounts.svelte.js` |
| exports `loggedInAccounts`, `resolveChooserAccounts`, `setGlobalClaudeAccount`, `effectiveClaudeAccountId`, `setProjectClaudeAccountChoice`, `refreshClaudeAccounts`, `refreshClaudeAccountUsage`, `requestClaudeLaunch`, `resetClaudeAccountsForTest` | same |
| `DETECTION_TTL_MS = 60_000`, `HISTORY_MODES = {resume,continue}` | `:112, :216` |
| IPC wrappers `listClaudeAccounts`, `normalizeClaudeAccount{,sResult}`, `normalizeClaudeAccountUsage` | `src/lib/ipc/system.js:372-432` |
| `setProjectClaudeAccount` | `src/lib/ipc/projects.js:140` |
| `resolveClaudeLaunchAccount`, `launchClaudeSession(…, claudeAccountId)`, `stopClaudeSession` | `src/lib/ipc/sessions.js:77, :86, :103` |
| components | `ClaudeAccountChooser.svelte`, `ClaudeAccountChip.svelte`, `ClaudeUsageMeter.svelte` |
| registry fallback fixture (byte-identical to Rust descriptors, asserted) | `src/lib/fixtures/tool-registry.json` |
| `TOOL_DISPLAY = { claude:'Claude', codex:'Codex', gemini:'Gemini' }` | `src/lib/Sidebar.svelte:379` |
| visual host | `src/visual-host/hosts/ClaudeAccountHost.svelte`, `registry.js`, `mockState.js`, `mocks/ipc.js` |

### 1k. Guard tests (what is currently pinned)

| Assertion | File:line |
|---|---|
| `claude.capabilities.config_dir_env == Some("CLAUDE_CONFIG_DIR")`; `usage_bridge` true | `src-tauri/tests/harness_conformance.rs:156-160` |
| `account_selection` tools **== exactly `vec![CliTool::Claude]`** | `harness_conformance.rs:199-204` |
| `usage_bridge` tools **== exactly `vec![CliTool::Claude]`** | `harness_conformance.rs:213-218` |
| `team_config_namespace` tools == `vec![Claude]`; `AppManagedClaudeDir` roots == `vec![Claude]` | `:192-211` |
| Rust `descriptors()` must equal `src/lib/fixtures/tool-registry.json` | `:141-148` |
| `CliTool::…` literal budget outside slice files: `EXPECTED_RUNTIME_LITERAL_COUNT = 59`, `ALLOWED_RUNTIME_FILES` list | `src-tauri/tests/module_boundary_assertions.rs:220-233` |

**These four `assert_eq!(…, vec![CliTool::Claude])` are the tripwires: adding a
second account/usage tool fails them by design and forces a deliberate edit.**

---

## 2. Current data flow (text diagram)

```
DETECTION
  Linux/macOS: claude_accounts_report()                    [claude_accounts/mod.rs:133]
    └─ detect_claude_accounts_cached()                     [claude_accounts.rs:468]
         └─ scan_claude_config_uncached()  (60 s TTL)      [:483]
              home     = detection_home_for(TAURHAUS_CLAUDE_DIR, $HOME)   [:517]
              extras   = config_dirs_of_live_sessions()  ← filtered by
                         capabilities.account_selection                    [:600]
              default  = PlatformPaths::claude_dir()
              procdflt = $HOME/.claude                                     [:529]
              candidates = [$HOME/.claude, default, $HOME/.claude-*, extras] dedup canonical  [:244]
              read_account(dir): .claude.json → oauthAccount{…}            [:335]
                                 logged_in = .credentials.json exists  (File)
                                           = true                      (Keychain/macOS)  [:380]
              sort: is_default desc, email asc
              → ClaudeScan { config_dirs, accounts }
    └─ claude_usage::attach_usage(&mut accounts)           [claude_usage.rs:636]
         └─ latest_usage_records(<app data>/claude-usage.jsonl)          [:521]
              shared flock, 500 ms bounded wait; None = "unknown" (≠ empty)
              match by canonical config_dir, else by account_id (newest ts) [:594-605]
  Windows: app → daemon RPC `list_claude_accounts`  [handlers.rs:97]
              (daemon runs the identical two steps inside WSL)
              UNKNOWN_METHOD → Unsupported → empty, degraded=false
              no reply       → Unavailable → empty, degraded=TRUE

USAGE INGEST (independent, write side)
  Claude Code TUI  ──stdin JSON (rate_limits.five_hour / .seven_day)──▶
    <config dir>/hooks/taurhaus-statusline(.sh)        [generated, 0700]
      └─ taurhaus-daemon claude-usage-sink --config-dir D [--sink S] [--render]
           parse_statusline_input → (optional) render_status_line → stdout = the row
           append_usage_at(S): sidecar .lock, try_lock (never waits),
             compact if >5 MB (tmp+rename), throttle 30 s/account, append 1 JSONL line
  Bridge install/reconcile: ensure_statusline_bridge_soon(daemon_exe)
    - after every list_claude_accounts (native, ≤1/60 s)  [claude_accounts/mod.rs:94-103]
    - inside the daemon handler on Windows                [handlers.rs:106-107]
    - gated by CliVersions.claude_statusline_usage_supported  [claude_statusline.rs:680]

RESOLUTION (per launch)
  launch_cli_session(projectId, mode, tool, claudeAccountId)
    └─ IF spec(tool).capabilities.account_selection      [launching.rs:112]
         resolve_claude_account():                        [:454]
           if mode ∈ {Continue,Resume} and no explicit id:
               transcript = claude_project_transcript(project)   (daemon on Windows)
                            ?? remembered_claude_transcript()    (in-proc sightings)
           accounts = claude_accounts_report()
           decide_launch_account() → resolve_launch_account()    [claude_accounts.rs:731]
             PRECEDENCE: Request → Session(transcript's config dir) → Project
                         → GlobalDefault → SignedIn → DefaultConfigDir
             needs_choice = nothing selected AND ≥2 signed-in
             fallback_from = the id that was asked for but unusable
       ELSE account = None

LAUNCH PREFIX
  LaunchSpec { tool, mode, base, model, team, claude_config_dir }.render()  [launch.rs:165]
    match tool { CliTool::Claude => { …model/effort/team flags…
        LAST:  if let Some(dir) = claude_config_dir
                 if let Some(env) = capabilities.config_dir_env
                    if base already contains env → note ConfigDirIgnored
                    else command = "ENV='<dir>' " + command      [:362-378]
                 else note CapabilityMissing{ConfigDir}
    } }
    config_dir is None when account.is_process_default → single-account users' command unchanged
    to_launch_namespace(): Windows→WSL path form                 [claude_accounts.rs:553]

RESUME DERIVATION
  transcripts at <config dir>/projects/<slug>/<id>.jsonl
  newest_project_transcript(config_dirs, path) → mtime max across ALL config dirs  [:566]
     ↳ uses scan.config_dirs (wider than scan.accounts: a mid-rewrite .claude.json
       names no account but its history is still on disk)                          [:472-480]
  config_dir_for_transcript(path) → the owning config dir → the account

UI
  Shell.svelte:323  refreshClaudeAccounts({force: reconnected})
  store claudeAccounts{accounts,degraded,defaultAccountId,projectChoices,pending}
  OverviewTab  → ClaudeAccountChip (visible iff accounts.length ≥ 2)
                  → ClaudeUsageMeter(compact) ; 30 s poll while menu open
  Shell        → ClaudeAccountChooser (iff claudeAccounts.pending)
                  requestClaudeLaunch: capabilities.accountSelection → ≥2 logged in
                    → no stored/global choice → !backendPlacesLaunch → open chooser
  Settings     → "Claude accounts" block (iff detected ≥ 2) → terminal.claude_default_account_id
  Sidebar      → context menu launches via requestClaudeLaunch
```

---

## 3. Seams for `AccountSource` / `UsageSource`

### 3.1 Naming precondition
`AccountSource` **is already taken** (`claude_accounts.rs:132`) as the launch-provenance
enum, and its `as_str()` values are on the wire and in the log. Rename that enum
(`AccountOrigin`) before introducing an `AccountSource` trait, or pick a different
trait name (`AccountProvider`). This is a mechanical but repo-wide rename.

### 3.2 Where the traits attach
Exactly like the existing slice traits, on `CliToolSpec` in `cli_tool.rs:477-581`,
beside `session_source()` / `activity_source()` / `compaction_signal_source()` /
`transcript_parser()` / `session_resolver()` — same `static` + `match self.tool`
shape, returning `Option<&'static dyn …>` gated on the capability flag (the
`compaction_signal_source` at `:529-547` is the exact template: `if !caps.X { return None }`).

```rust
// gated on capabilities.account_selection
pub fn account_source(&self) -> Option<&'static dyn AccountProvider>;
// gated on capabilities.usage_bridge
pub fn usage_source(&self) -> Option<&'static dyn UsageSource>;
```

### 3.3 `AccountProvider` — the four questions, and who answers them today

| Trait method | Claude impl today | Second-tool reality (verified for Codex) |
|---|---|---|
| `enumerate_config_dirs(home, extras, default) -> Vec<PathBuf>` | `config_dir_candidates` `:244` — `$HOME/.claude`, `default`, `$HOME/.claude-*`, live-session dirs | Codex: `$CODEX_HOME` or `~/.codex` (`platform_paths.rs:66-68`). **No sibling-dir convention observed** — `~/.codex` is a single dir on this host |
| `identify(dir) -> Option<ToolAccount>` | `read_account` `:335` reads `.claude.json` → `oauthAccount.*` | Codex `~/.codex/auth.json` keys: `auth_mode`, `OPENAI_API_KEY`, `tokens.{id_token,access_token,refresh_token,account_id}`, `last_refresh`. **`account_id` only — no email, no display name, no org.** Forces `email` to become optional/`label` |
| `logged_in(dir) -> bool` | `signed_in` `:380`: `.credentials.json` exists, or `true` on macOS keychain | Codex: presence of `auth.json` with a non-null `tokens` (UNVERIFIED that this is the CLI's own test — would be verified by reading Codex CLI source or observing `codex login`/`logout` toggling the file) |
| `selector() -> Selector` | `capabilities.config_dir_env = "CLAUDE_CONFIG_DIR"` | Codex = `CODEX_HOME` (also an env var). **No tool in the registry uses a flag selector today** — see risk 4 |

Additional per-tool data the trait (or extra `CliToolSpec` fields) must carry,
currently hard-coded as module consts in `claude_accounts.rs:28-43`:
`account_config_filename`, `credentials_filename`, `sibling_dir_prefix`, credential-store model.
Three of those consts (`DEFAULT_CONFIG_DIRNAME`, `PROJECTS_SUBDIR`, `TRANSCRIPT_EXTENSION`)
**already duplicate** `CliToolSpec.base_dir_name` / `.projects_subdir` / `.session_extension`
— deleting the duplicates is a free first step.

### 3.4 `UsageSource` — fetch + normalise

The read side is *already* generic in everything but its names; the write side is
entirely Claude-shaped and should live behind the trait:

```rust
trait UsageSource {
    fn ensure_bridge(&self, exe: &Path);      // ← claude_statusline.rs in full
    fn remove_bridge(&self, dir: &Path);
    fn sink_filename(&self) -> &'static str;  // ← CLAUDE_USAGE_FILENAME
    fn windows(&self) -> &'static [WindowSpec]; // ← five_hour / seven_day
}
```

The one type change that decides how far this generalises: **`ClaudeAccountUsage`
must stop having `five_hour` and `seven_day` as named fields** (`claude_usage.rs:82-91`)
and become a list of `(window_id, label, used_percentage, resets_at)`. Those two
names reach the DB-free JSONL records (`:107-109`), the IPC payload, the frontend
normaliser (`system.js:372`) and the meter's rendering.

### 3.5 Consumers that stay tool-agnostic (no change beyond renames)

- `resolve_launch_account` + `AccountRequest`/`AccountResolution` (`claude_accounts.rs:731-900`) — pure precedence over a slice of accounts; already tool-free.
- `decide_launch_account`, `log_account_resolution` (`launching.rs:497, :525`) — operate on the resolution.
- The whole usage *read/aggregate* path: `latest_usage_records`, `latest_per_account`, `attach_usage_from`, `newest_by_account_id`, the flock/compaction/throttle discipline (`claude_usage.rs:228-641`) — nothing in it is Claude-specific except the filename and the two window field names.
- `LaunchSpec` prefix mechanics: `command_contains_flag`, `shell_escape`, `LaunchNote::ConfigDirIgnored`, `LaunchCapability::ConfigDir` — already keyed off `config_dir_env`.
- `to_launch_namespace`, `canonical_key`, `newer`, `unix_now`.
- Frontend: `ClaudeUsageMeter` props are `{usage, dark, compact}` — **already tool-agnostic**; only the window field names inside it are Claude's. `ClaudeAccountChooser`/`Chip` props are `{accounts, selectedAccountId, defaultAccountId, degraded, dark, onSelect, onRequestUsage}` — **no tool identity in any prop**.
- `requestClaudeLaunch`'s capability gate (`claudeAccounts.svelte.js:261`).
- Daemon `DaemonAnswer` / `daemon_answer` UNKNOWN_METHOD-vs-unavailable discrimination (`claude_accounts/mod.rs:62-69, 254-291`) — reusable verbatim for any additive method.

---

## 4. Frontend surfaces to parameterise (+ test files)

| Surface | File | What must become tool-aware | Test file(s) |
|---|---|---|---|
| Account store | `src/lib/claudeAccounts.svelte.js` | `accounts`/`defaultAccountId`/`projectChoices` all become keyed by tool; `requestClaudeLaunch` already gates on `capabilities.accountSelection` | `src/lib/claudeAccounts.test.js` |
| Chooser | `src/lib/components/ClaudeAccountChooser.svelte` (mounted `src/Shell.svelte:624-637`) | copy strings ("which Claude subscription"), title, `is_default` label; props already generic | `ClaudeAccountChooser.test.js`; `src/Shell.meshFocus.test.js` |
| Chip | `src/lib/components/ClaudeAccountChip.svelte` (mounted `OverviewTab.svelte:168-176`) | visibility rule `accounts.length >= 2` becomes per-tool; one chip per account-capable tool | `ClaudeAccountChip.test.js` |
| Usage meter | `src/lib/components/ClaudeUsageMeter.svelte` | `five_hour`/`seven_day` → iterate declared windows; `STALE_MS`, reset-passed drop rule stay | `ClaudeUsageMeter.test.js` |
| Settings block | `src/lib/Settings.svelte:699-740` (`data-testid="settings-claude-accounts"`, radio `name="claude-default-account"`, `claude-account-row-{id}`, `claude-account-default-{id}`) | one block per account-capable tool; `terminal.claude_default_account_id` → per-tool key | `src/lib/settings.test.js` |
| Sidebar context menu | `src/lib/Sidebar.svelte:379-410` | `TOOL_DISPLAY` literal map + 7 hard-coded per-tool items → generated from `tools()`; item schema `{label, action, icon, separator?, danger?, keepOpen?}` (renderer `src/lib/ContextMenu.svelte`) | `src/lib/Sidebar.component.test.js`, `src/lib/contextMenu.test.js` |
| Session rows / tool badges | `src/lib/Sidebar.svelte`, `src/lib/OverviewTab.svelte:154` (`TOOLS = ['claude','codex','gemini']` literal) | drive from `toolRegistry.tools()` | `Sidebar.component.test.js` |
| IPC layer | `src/lib/ipc/system.js:372-432`, `projects.js:140`, `sessions.js:77-93` | normalisers + command names | `src/lib/ipc.test.js` |
| Team builder | `src/lib/components/MeshTeamBuilder.svelte:83-84` | reads `claudeAccounts.accounts` directly | — |
| Registry fallback | `src/lib/toolRegistry.js:25-116` + `src/lib/fixtures/tool-registry.json` | must stay byte-identical to Rust `descriptors()` | asserted by `harness_conformance.rs:141-148` |
| Visual lane | `src/visual-host/hosts/ClaudeAccountHost.svelte`, `registry.js`, `mockState.js`, `mocks/ipc.js`, `src/test/visual/ipcVisualMocks.js`, `src/test/visual/fixtures/claudeAccount.fixtures.js` | fixtures per tool | `src/test/visual/specs/claudeAccount.visual.test.js` |

---

## 5. Risks

1. **Daemon/IPC compatibility.** `PROTOCOL_VERSION = 10` and the app enforces an
   *exact* version match on every connect path (`harness-model.md:41`). The two
   account methods are documented additive-without-bump (`protocol.rs:102-108`);
   *renaming* them is a contract change and **does** bump the version, which per
   `harness-model.md:44` means app + `just install-daemon` must ship together.
   Mitigation that keeps the additive property: keep `list_claude_accounts` as an
   alias dispatching to the new `list_tool_accounts`, and rely on the existing
   `UNKNOWN_METHOD → Unsupported` path (`claude_accounts/mod.rs:276-282`) for the
   new one. Note the asymmetry that must be preserved: `Unsupported` → empty +
   `degraded:false`; `Unavailable` → empty + `degraded:true`. Collapsing them
   makes a dropped daemon look like "nobody is signed in".

2. **`AccountSource` name collision** (§3.1). The existing enum's `as_str()`
   values are logged and sent to the frontend; renaming the *enum* is safe, but
   changing those six strings is not.

3. **DB migration for a per-tool project pin.** `projects.claude_account_id` is
   read **positionally** (`db/queries.rs:23` `row.get(10)`) with hand-maintained
   column lists at `:51,:68`. Options: (a) `013_project_tool_account.sql` adding
   `codex_account_id`/… — cheap but unbounded in tool count and keeps positional
   reads growing; (b) a `project_tool_accounts(project_id, tool, account_id)`
   side table — correct, but needs a data migration copying `claude_account_id`
   into `(…, 'claude', …)` and a decision on whether to drop the old column
   (SQLite `DROP COLUMN` support is version-dependent — **UNVERIFIED** for the
   bundled rusqlite/SQLite here; verify with the `sqlite_version()` the crate
   links). Either way `Project.claude_account_id` is serialized as
   `claudeAccountId` and asserted in `models/mod.rs:1103`, and the frontend reads
   *both* spellings (`claudeAccounts.svelte.js:86`) — a rename needs a
   compatibility alias for stored payloads.

4. **Launch rendering for a flag-selected tool.** Today the prefix is built as
   `"{env}={escaped} {command}"` (`launch.rs:370`) — an env-var assignment, and
   it is emitted **only inside the `CliTool::Claude` match arm** (`:276-379`),
   so a tool declaring `config_dir_env` but not being Claude gets nothing
   (deliberately pinned by the test `a_config_dir_never_reaches_a_codex_launch`,
   `launch.rs:1254`). Two changes are needed: hoist the block out of the arm, and
   generalise `config_dir_env: Option<&str>` into a selector enum
   (`EnvVar{name} | Flag{flag}`) mirroring the existing `EffortFlag::{Argument,Config}`
   precedent (`cli_tool.rs:77-85`) — flags append (`append_flag`), env vars
   prepend, and ordering matters because the Claude arm prepends the team
   environment first (`:360-361`, pinned by `claude_render_puts_the_config_dir_in_front_of_the_team_environment`).
   **UNVERIFIED:** that any real CLI uses a flag rather than an env var — Codex
   uses `CODEX_HOME` (env), and Gemini CLI is not installed on this host, so its
   account-selection mechanism is unknown. Verify by `gemini --help` on a host
   that has it.

5. **Usage window shape.** `five_hour`/`seven_day` are Claude product windows
   baked into the record, the account payload, the IPC normaliser and the meter.
   A tool with different windows (or none) cannot be expressed without the
   `Vec<Window>` change in §3.4. Records already written to
   `claude-usage.jsonl` use the old field names — the reader needs a
   `#[serde(alias)]` or a one-way migration, and the file is append-only with a
   5 MB compaction rename, so a mixed-shape file is the normal state during a
   rollout.

6. **Conformance tripwires fail closed.** The four `assert_eq!(…, vec![CliTool::Claude])`
   (`harness_conformance.rs:192-218`) and `EXPECTED_RUNTIME_LITERAL_COUNT = 59`
   (`module_boundary_assertions.rs:233`) will fail the moment a second tool
   declares `account_selection`/`usage_bridge`. That is the intended design, but
   it means the generalisation PR *must* also rewrite those assertions —
   ideally into invariants ("every tool with `usage_bridge` has a `usage_source()`",
   "every tool with `account_selection` has a `config_dir_env` or a flag selector")
   rather than identity lists.

7. **Statusline bridge writes into a user-owned dir.** The Claude `UsageSource`
   installs a script and rewrites `settings.json` in the user's config dir with
   an elaborate wrap/restore/symlink/permissions contract
   (`claude_statusline.rs:1-68`). Any second tool's bridge inherits that whole
   hazard class, and the removal path refuses to act on a row it cannot prove is
   its own. Generalising must not turn "is this row ours?" into a per-tool
   guess — the `SCRIPT_BASENAME`/`RECORD_FILENAME`/exact-command match is the
   safety property.

8. **Secrets in logs.** `launch.command.rendered` carries a field literally named
   `claude_account` whose value is the account **email** (`launching.rs:148-155`),
   and rendered commands are redacted by `redact_command_for_logging`. A
   generalised record must keep the redaction and should prefer an opaque account
   id over an email where the UI does not need it.

9. **`TAURHAUS_CLAUDE_DIR` semantics do not generalise for free.** The
   `is_default` vs `is_process_default` distinction (`claude_accounts.rs:88-96`,
   `detection_home_for` `:517`, `process_default_config_dir` `:529`) exists
   because taurhaus's root can be moved while the CLI knows nothing about it —
   and it is what keeps a single-account user's command unprefixed. Every tool
   needs its own answer to "which dir does this CLI read with the selector
   unset", and the E2E isolation knob (`TAURHAUS_CLAUDE_DIR`) needs a per-tool
   equivalent or the isolated run scans the developer's real accounts.
