# Grok CLI 1.0.5 integration report for taurhaus

Date of verification: 2026-08-28 (Europe/Berlin)  
Host install: `~/.local/bin/grok`  
Resolved executable: `~/.grok/downloads/grok-linux-x86_64`  
Version: `grok 1.0.5 (5115b46bc9) [stable]`

## Scope, evidence, and safety

This report describes the installed binary, not an assumed earlier Grok release. Evidence came from the complete installed help tree, read-only inspection of `~/.grok`, safe binary string/file-format inspection, official SpaceXAI/xAI pages, and isolated probes under the requested scratch directory. The taurhaus repository was never written and no git write command was run. The real `~/.grok`, `~/.claude*`, and `~/.codex` configurations were not edited. Live Grok sessions used a disposable `GROK_HOME`, a copied credential store, the non-git `grok-probe` directory, and only the prompt `reply with the single word OK`. Disposable homes, raw leader logs, sockets, and lock files were removed after the probes.

The two requested retained captures are sanitized copies. Grok's raw debug stream logged a credential-bearing authentication object, so every scalar occurring in the copied `auth.json`, plus email-shaped strings, was replaced before retention. Final validation found no copied authentication scalar and no email-shaped string in either capture.

Official pages used for cross-checking include the [Grok Build overview](https://docs.x.ai/build/overview), [CLI reference](https://docs.x.ai/build/cli/reference), [headless/ACP guide](https://docs.x.ai/build/cli/headless-scripting), [settings](https://docs.x.ai/build/settings), [sessions](https://docs.x.ai/build/features/sessions), [hooks](https://docs.x.ai/build/features/hooks), [permissions](https://docs.x.ai/build/features/permissions), [skills/plugins](https://docs.x.ai/build/features/skills-plugins-marketplaces), [subagents](https://docs.x.ai/build/features/subagents), [MCP](https://docs.x.ai/build/features/mcp-servers), and the [Grok Build changelog](https://x.ai/build/changelog). Where those pages lag the installed 1.0.5 help or bundled documentation, this report says so and treats the installed binary as authoritative.

## 1. PROCESS SIGNATURE

### Facts

The command path is a direct symlink chain:

```text
~/.local/bin/grok
  -> ~/.grok/bin/grok
  -> ../downloads/grok-linux-x86_64
```

The resolved file is a stripped x86-64 ELF PIE, statically linked as `static-pie`, 166,854,368 bytes, with ELF build ID `df459c3cd090505e639a83d8a3a50d63add79245`. There is no shell/Node launcher. Linux resolves the symlinks and starts this ELF directly; `/proc/<pid>/exe` pointed to the downloaded file while `/proc/<pid>/cmdline` retained the invoked `~/.local/bin/grok` spelling.

The installed grammar is `grok [OPTIONS] [PROMPT] [COMMAND]`. A coordinator can classify argv as follows:

| Class | Verified argv shape | Important qualification |
|---|---|---|
| Interactive agent TUI | `grok [interactive flags]` or `grok [interactive flags] "initial prompt"` | A positional prompt is the first TUI turn; it is not print mode. |
| Headless/print | `grok -p PROMPT ...`, `grok --single PROMPT ...`, `grok --prompt-file FILE ...`, or `grok --prompt-json JSON ...` | `--output-format` alone does not select headless mode. |
| Agent protocol/service | `grok agent ... stdio`, `... headless`, `... serve`, or `... leader` | ACP stdio, WebSocket relay client, WebSocket server, or shared leader respectively; these are not the normal TUI. |
| Other subcommand | `grok <known-command> ...` | Some are TUI-like (`dashboard`); most are short CLI operations. Do not label every subcommand headless inference. |

The top-level commands advertised by this build are: `agent`, `completions`, `dashboard`, `doctor`, `du`, `export`, `inspect`, `leader`, `login`, `logout`, `mcp`, `memory`, `models`, `plugin`, `sessions`, `setup`, `trace`, `update`, `version`, `worktree`, and `wrap`.

Observed interactive process signature:

```text
comm: grok
argv: ~/.local/bin/grok --no-alt-screen --model grok-4.6 \
      --reasoning-effort low --disable-web-search
state while idle: SNsl+
threads: 50
children while idle: none
stdin/stdout: the tmux PTY
```

Observed leader signature:

```text
grok agent [common options] leader --no-exit-on-disconnect \
  --relay-on-demand --no-auto-update
threads: 35
children while idle: none
```

The processes held internal Unix socketpairs, inotify/eventpoll descriptors, log files, and a `/run/systemd/inhibit/*.ref` sleep-inhibitor FD. The interactive TUI also held a TLS TCP connection while idle. These descriptors persisted across working and idle states and are not a busy signal. Tool calls, MCP servers, shell commands, browser helpers, and subagents may create children on demand; “no children” is therefore an observed idle baseline, not an invariant.

The default leader endpoint is `$GROK_HOME/leader.sock` (`~/.grok/leader.sock` normally), accompanied by `leader.lock`. A manual leader had no wrapper or child process. A second leader using the same home failed with `Another leader already holds the lock`. A custom socket path is accepted through `--leader-socket`, but a long path inside the workspace failed with `path must be shorter than SUN_LEN`; use a short `/tmp/...sock` path.

At the final real-home check there was no live Grok process. `grok leader list --json` classified a concurrently-created real `~/.grok/leader.sock`/`leader.lock` pair as `Unreachable` with no live PID. This is stale state, not a running leader. It appeared during the study from activity outside the disposable probes; its timestamp predates this report's isolated leader tests, and it was not removed because the real Grok home was read-only.

### How verified

- `readlink -f`, `file`, `stat`, `/proc/<pid>/{exe,cmdline,fd}`, `ps`, `pgrep`, `ss`, and `lsof`.
- A tmux-hosted TUI was sampled before, during, and after the harmless prompt.
- Two short-lived leaders were started with disposable homes; `grok leader list/info --json` and an exclusivity attempt were run.
- Full `grok --help` and every advertised nested command help were executed.

### Unverified

- Child-process topology during every possible tool, MCP, browser, media, workflow, and sandbox operation was not exhaustively exercised.
- There is no verified promise that `comm`, thread counts, descriptor numbers, or the TLS connection remain the same in later releases.
- A future release may add commands, so a static argv classifier must be version-scoped.

### Recommendation for taurhaus

Resolve `/proc/<pid>/exe` to recognize the installed ELF, but retain the original NUL-delimited argv. First parse options that consume values, then classify the recognized command token. Mark print mode only when a single-prompt source (`-p`/`--single`, `--prompt-file`, or `--prompt-json`) is present. Treat a bare positional prompt as interactive. Version-gate the command list, and never infer busy/idle from process state, children, TCP connections, or FDs.

## 2. LAUNCH

### Facts

The exact high-value launch controls in 1.0.5 are:

| Need | Installed syntax and behavior |
|---|---|
| Model | `-m MODEL` / `--model MODEL`; interactive `/model`. |
| Reasoning | `--reasoning-effort LEVEL` / `--effort LEVEL`; interactive `/effort`. The live catalog, not a universal enum, decides what each model accepts. |
| Always approve | `--always-approve`; help says “Auto-approve all tool executions.” Bundled docs also accept hidden/compat alias `--yolo` and equivalence `--permission-mode bypassPermissions`. Deny rules, `PreToolUse` hooks, plan review, and managed locks still apply. |
| Persistent default | Current config form: `[ui] permission_mode = "always-approve"`. Legacy `[ui] yolo = true` still works, but `permission_mode` wins. The existing real config has legacy `yolo = false`. |
| Fresh | No `--resume`/`--continue`; every headless call is fresh by default. |
| Continue | `-c` / `--continue`: most recent session for the effective cwd. |
| Resume | `-r [ID_OR_TITLE]` / `--resume [ID_OR_TITLE]`; an omitted value means most recent for cwd. UUID-looking values are IDs; other values case-insensitively match titles in cwd and can be ambiguous. |
| New chosen ID | `-s UUID` / `--session-id UUID`: a valid, unused UUID for a new conversation. It never resumes. With resume/continue it is legal only with `--fork-session`, where it names the fork. |
| Resume as fork | Add `--fork-session`; optionally `--session-id NEW_UUID`. |
| Initial interactive prompt | Positional `PROMPT`, for example `grok "reply with the single word OK"`. |
| Headless prompt | `-p PROMPT` / `--single PROMPT`; alternatives `--prompt-file FILE` and `--prompt-json JSON_CONTENT_BLOCKS`. |
| Working directory | `--cwd PATH`; both session lookup and the agent workspace use this path. |
| Worktree | `-w[NAME]` / `--worktree[=NAME]`, optionally `--ref REF` / `--worktree-ref REF`. It creates a new git worktree for interactive/new sessions, based on current HEAD unless `--ref` is supplied. **The installed help explicitly says `-p` does not create a worktree from this flag.** |
| Remote resume code | Resume restores conversation. `--restore-code` restores the source snapshot; a remote session requires `--worktree` and never checks it out into the current directory. |

Use `--worktree=NAME` with `=` when there is also a positional prompt. Because the worktree name is optional, `grok -w "prompt text"` consumes the prompt as the worktree label.

`grok models` made a non-interactive authenticated request and listed exactly these currently available account models:

| Model | Context | Effort choices | Default |
|---|---:|---|---|
| `grok-4.6` | 500,000 | `xhigh`, `high`, `medium`, `low` | `high` |
| `grok-4.5` | 500,000 | `high`, `medium`, `low` | `high` |

`grok-4.6` was the catalog default. This is a live account/catalog result and can change independently of the CLI version. Models are listed by the `models` subcommand or the TUI `/model`, not a `/models`-only mechanism.

Headless output formats accepted by this binary are:

- `plain`: final human-readable text.
- `json`: one final JSON object.
- `streaming-json`: JSONL incremental events ending in an end/result event.
- `streaming-messages-json`: JSONL messages compatible with the installed Messages streaming schema; `--include-partial-messages` adds `stream_event` deltas.

The retained `streaming-messages-json` sample contains 29 lines: one `system`, 26 `stream_event`, one `assistant`, and one `result`; the result is successful and the text is `OK`. Its init message reports `permissionMode: "default"`. `--json-schema SCHEMA` constrains structured JSON and implies JSON output.

stdin is not implicitly appended to the prompt. The bundled headless guide says to use shell substitution or, preferably for a coordinator, `--prompt-file`; use `--prompt-json` for typed content blocks. `streaming-messages-json` stdout is read-only. Bidirectional prompt, cancel, and approval traffic uses ACP (`grok agent stdio`), not stdin mixed into this JSONL format. The official [headless guide](https://docs.x.ai/build/cli/headless-scripting) corroborates `-p` and ACP, but currently omits newer installed details such as `streaming-messages-json`, so installed help is the source of truth.

All advertised help paths checked:

```text
agent: stdio, headless, serve, leader
doctor: fix
leader: list, info, kill
mcp: list, add, remove, enable, disable, doctor
memory: clear
plugin: list, install, uninstall, update, enable, disable, details,
        validate, tag, marketplace
plugin marketplace: list, add, remove, update
sessions: list, search, delete
worktree: list, show, rm, gc, db
worktree db: rebuild, stats, path
leaf/top-level: completions, dashboard, du, export, inspect, login, logout,
                models, setup, trace, update, version, wrap
```

Notable service options: `agent stdio` is ACP on stdio; `agent headless` is a WebSocket relay client; `agent serve` listens on `127.0.0.1:2419` by default and supports `--secret`/`GROK_AGENT_SECRET`; `agent leader` owns the shared local leader and can use `--relay-on-demand`.

### How verified

- Complete help tree and bundled user guides.
- `grok models` with the disposable home and cached non-secret credentials.
- A real harmless `-p` call using `--model grok-4.6 --reasoning-effort low --tools read_file --no-subagents --disable-web-search --max-turns 1` and the requested streaming format.
- The official [CLI reference](https://docs.x.ai/build/cli/reference), [permissions page](https://docs.x.ai/build/features/permissions), and [worktree documentation](https://docs.x.ai/build/features/worktrees).

### Unverified

- Catalog availability, effort menus, and account entitlements are dynamic.
- The complete JSON schemas for all four output formats are not declared stable by the installed help.
- Headless `--worktree` was not run against a git repository because the permitted probe directory is intentionally non-git; its no-op semantics come directly from installed help.

### Recommendation for taurhaus

Use `grok --no-auto-update -p "$prompt" --cwd "$scratch" --output-format streaming-messages-json` for a one-shot capability slice and parse JSONL by `type`/`subtype`, ignoring unknown fields. Use `grok agent stdio` for any bidirectional slice. Read `grok models` at runtime and validate effort against that response. Default to ask mode; enable always-approve only through the slice policy, and still install deny rules/sandboxing. Never put `--worktree` into a headless recipe expecting isolation—create a worktree explicitly outside Grok or use interactive/ACP worktree support.

## 3. CONFIG + IDENTITY

### Facts

The default home is `~/.grok`; `GROK_HOME=/absolute/path` selects a different home per process. This was functionally verified: config, models, agent identity, logs, sessions, active-session registry, leader socket, and lock all moved into the disposable home. The official [settings page](https://docs.x.ai/build/settings) also identifies `$GROK_HOME/config.toml`. No `GROK_CONFIG_HOME` selector exists in the checked help/docs. XDG strings occur in linked libraries, but no XDG variable was verified as a Grok-home selector.

Current top-level real-home inventory (byte sizes are apparent bytes; directory byte totals are recursive content totals):

| Entry | Type/size | Format and role |
|---|---:|---|
| `.config-init.lock` | 0 B | Initialization lock. |
| `.metadata_version` | 5 B | Small text metadata schema marker. |
| `CHANGELOG.json`, `CHANGELOG.md` | 2,803 B; 1,657 B | Bundled 1.0.5 release notes, machine/human form. |
| `README.md` | 109,061 B | Full bundled reference. |
| `active_sessions.json`, `.lock` | 2 B; 0 B | JSON active-process registry and lock; currently `[]`. |
| `agent_id` | 36 B, mode 0600 | UUID-shaped installation/agent identity, separate from account identity. |
| `auth.json`, `.lock` | 1,751 B, mode 0600; 18 B | Credential/account map and coordination lock. Secret-bearing. |
| `config.toml` | 386 B | User configuration. |
| `leader.sock`, `leader.lock` | socket; 7 B | Current stale/unreachable leader endpoint and PID lock. |
| `managed_config.lock` | 0 B | Managed-config lock; no managed config file was present. |
| `models_cache.json` | 4,537 B | Catalog cache: `auth_method`, `etag`, `fetched_at`, `grok_version`, `models`, `origin`. |
| `slash-mru.json` | 68 B | JSON keys `by_command`, `by_prefix`. |
| `tip_cursor.json` | 13 B | JSON `cursor`. |
| `version.json` | 103 B | JSON `version`, `stable_version`, `checked_at`. |
| `worktrees.db` | 40,960 B | SQLite 3 worktree registry. |
| `bin/` | 60 B | Symlink entry for `grok`; no wrapper file. |
| `downloads/` | 166,854,368 B, 1 file | The installed ELF. |
| `bundled/` | 12,854,583 B, 416 files | Built-in agents, personas, roles, and skills. |
| `completions/` | 356,745 B, 2 files | Shell completion assets. |
| `docs/` | 459,742 B, 24 files | Installed user guide. |
| `marketplace-cache/` | 9,340,883 B, 541 files | Cached official marketplace git/content data. |
| `logs/` | 226,372 B, 2 files at snapshot | Plain/JSONL internal logs (`hooks.log`, `unified.jsonl`). Mutable and potentially sensitive. |
| `memtrace/` | 2,043 B, 3 files | JSONL memory/process traces. |
| `relocations/` | 10 zero-byte lock markers | Per-session relocation locks. |
| `sessions/` | 438,955 B at final snapshot | Two cwd groups, 11 session directories, plus JSONL/SQLite indexes. Sensitive transcripts. |

The installed user `config.toml` contains only these effective key names:

```text
[cli] installer
[marketplace] default_skills_installs_purged, official_marketplace_auto_installed
[[marketplace.sources]] name, type, repo
[ui] max_thoughts_width, fork_secondary_model, yolo, compact_mode
[privacy] privacy_banner_acked
```

No secret value occurs in that file. The auth store is a JSON object whose top-level record key has shape `https://auth.x.ai::<client-uuid>`. One record is present. Its observed field names and value shapes are:

```text
key: long opaque string (secret)
auth_mode: short enum string
create_time, expires_at: timestamp strings
user_id, principal_id, team_id: UUID strings
email: email string
first_name, last_name: strings
profile_image_asset_id: opaque string
principal_type: short enum string
coding_data_retention_opt_out: boolean
refresh_token: opaque secret string
oidc_issuer: URL string
oidc_client_id: UUID string
```

The signed-in account is identified by the OIDC issuer/client key plus `user_id`/`principal_id`, `email`, and `team_id`. None of their values is reproduced here. A non-interactive `grok models` succeeded and said the machine is logged in with grok.com; an ACP `authenticate` response reported `auth_mode: Oidc` and `subscription_tier: supergrok`. There was no interactive sign-in and no login flow was completed.

Config precedence in the installed guide is: CLI flags; direct environment settings; requirements/MDM; the `GROK_CONFIG`/`GROK_CONFIG_PATH` overlay; user `config.toml`; managed defaults; built-ins. `GROK_CONFIG` is inline JSON; `GROK_CONFIG_PATH` is an additional JSON/TOML overlay. They deep-merge only a security allowlist and do not replace the home. `GROK_HOME` is the home selector.

The following documented names were also found literally in the installed binary. This is a key-name inventory, not permission to expose their values:

```text
GROK_AGENT GROK_AGENT_DASHBOARD GROK_AGENT_SECRET GROK_APPEARANCE
GROK_ASK_USER_QUESTION_TIMEOUT_ENABLED GROK_ASK_USER_QUESTION_TIMEOUT_SECS
GROK_AUTH_EARLY_INVALIDATION_SECS GROK_AUTH_EXPIRED
GROK_AUTH_PROVIDER_ACCESS_TOKEN GROK_AUTH_PROVIDER_COMMAND
GROK_AUTH_PROVIDER_EXPIRES_AT GROK_AUTH_PROVIDER_LABEL
GROK_AUTH_PROVIDER_REFRESH_TOKEN GROK_AUTH_TOKEN_TTL
GROK_CLAUDE_MCPS_ENABLED GROK_CLAUDE_SKILLS_ENABLED
GROK_CLIPBOARD_NO_DATA_CONTROL GROK_CLIPBOARD_NO_OSC52
GROK_CLI_CHAT_PROXY_BASE_URL GROK_CODE_XAI_API_KEY
GROK_CONFIG GROK_CONFIG_PATH GROK_COPY_FILE
GROK_CURSOR_MCPS_ENABLED GROK_CURSOR_SKILLS_ENABLED
GROK_DEBUG_LOG GROK_DEFAULT_SELECTED_PERMISSION GROK_DEPLOYMENT_KEY
GROK_DISABLE_AUTOUPDATER GROK_EVENT GROK_EXIT_TIMEOUT_SECS
GROK_EXTERNAL_OTEL GROK_FEEDBACK_ENABLED GROK_FOLDER_TRUST GROK_HOME
GROK_HOOK_EVENT GROK_HOOK_NAME GROK_INVERT_SCROLL GROK_LOG_FILE
GROK_LSP_TOOLS GROK_MARKETPLACE_REQUIRE_SHA GROK_MAXIMUM_VERSION
GROK_MAX_MCP_OUTPUT_BYTES GROK_MAX_PARALLEL_IMAGE_GEN_CALLS
GROK_MAX_PARALLEL_VIDEO_GEN_CALLS GROK_MCP_STARTUP_TIMEOUT_SECS
GROK_MEMORY GROK_MESSAGE GROK_MINIMUM_VERSION GROK_MODELS_BASE_URL
GROK_MODELS_LIST_URL GROK_OIDC_CLIENT_ID GROK_OIDC_ISSUER
GROK_PLUGIN_DATA GROK_PLUGIN_ROOT GROK_REQUIRED_MAXIMUM_VERSION
GROK_REQUIRED_MINIMUM_VERSION GROK_RESPECT_GITIGNORE GROK_SANDBOX
GROK_SCROLL_LINES GROK_SCROLL_MODE GROK_SCROLL_SPEED GROK_SESSION_ID
GROK_SUBAGENTS GROK_TELEMETRY_BUILD_EVENTS_API_KEY
GROK_TELEMETRY_BUILD_EVENTS_URL GROK_TELEMETRY_BUILD_MIXPANEL_TOKEN
GROK_TELEMETRY_ENABLED GROK_TELEMETRY_EVENTS_API_KEY
GROK_TELEMETRY_EVENTS_URL GROK_TELEMETRY_MIXPANEL_ENABLED
GROK_TELEMETRY_MIXPANEL_TOKEN GROK_TELEMETRY_TRACE_UPLOAD
GROK_THEME GROK_VERSION GROK_VOICE_CAPTURE GROK_WEB_FETCH
GROK_WEB_FETCH_ALLOW_LOCAL GROK_WEB_FETCH_PROXY GROK_WEB_SEARCH_MODEL
GROK_WORKFLOWS GROK_WORKSPACE_ROOT XAI_API_KEY
```

Additional orchestration-relevant strings found in the ELF include `GROK_LEADER_SOCKET`, `GROK_WS_URL`, `GROK_CODE_WEB_URL`, `GROK_CODE_BACKEND_URL`, `GROK_XAI_API_BASE_URL`, `XAI_API_BASE_URL`, `GROK_FORCE_LOGIN_TEAM_ID`, `GROK_CHANNEL`, `GROK_SESSION_SEARCH`, `GROK_SESSION_REGISTRY`, `GROK_AUTO_UPDATE`, `GROK_EXTRA_CA_BUNDLE`, and `GROK_POOL_IDLE_TIMEOUT_SECS`. Several are internal/enterprise/test surfaces; string presence alone does not establish a public contract. `GROK_FORCE_LOGIN_TEAM_ID`, `GROK_CONFIG`, and `GROK_CONFIG_PATH` are independently corroborated by the installed 1.0.5 changelog.

### How verified

- Read-only `find`, `du`, `stat`, `file`, `jq` key-only queries, SQLite schema inspection, and literal string matching.
- Credential values were measured/classified in-process and never printed into the report.
- A disposable `GROK_HOME` was used for all authenticated live probes; real auth/config mtimes remained at their pre-probe values.
- Official [settings documentation](https://docs.x.ai/build/settings) corroborates `GROK_HOME` and config scopes.

### Unverified

- XDG variables were not proven to have no secondary effect; they are simply not a verified home selector.
- Semantics of strings-only/internal environment names are unverified unless separately documented above.
- The auth object can structurally contain multiple issuer/client records, but this build's behavior for selecting among multiple simultaneously stored accounts was not exercised.
- Real-home session/log/cache size changed during the investigation due to concurrent external activity. The report uses explicit snapshots and does not attribute those writes to Grok generally or to taurhaus.

### Recommendation for taurhaus

Set a private absolute `GROK_HOME` per capability slice or account and do not rely on XDG. Mount/copy only the minimum authentication material with mode 0600, never parse or log auth values, and avoid forwarding raw debug/unified logs. Use `GROK_CONFIG` for a permitted ephemeral soft-setting overlay, not for auth or permission escalation. Treat `sessions/`, logs, MCP credentials, and auth as secret-bearing even when their outer format is ordinary JSON/TOML/JSONL.

## 4. BUSY/IDLE + SESSION IDENTITY

### Facts

The strongest machine-readable state observed is the leader/ACP extension notification:

```json
{"method":"_x.ai/sessions/changed","params":{"upserted":[{
  "sessionId":"<uuid>", "activity":"working", "resident":true
}]}}
{"method":"_x.ai/sessions/changed","params":{"upserted":[{
  "sessionId":"<same uuid>", "activity":"idle", "resident":true
}]}}
```

This transition was captured around a real harmless ACP prompt. `_x.ai/queue/changed` also reported a queued entry, then `runningPromptId`/`runningText`, then an empty queue. `session/update` carried response chunks, `_x.ai/session/prompt_complete` ended the prompt, and `session/prompt` returned `stopReason: end_turn`. These are explicit state signals; unknown extensions must still be tolerated.

For a plain TUI without a shared leader:

- `$GROK_HOME/active_sessions.json` became an array containing `session_id`, `pid`, `cwd`, and `opened_at` after the first prompt.
- That entry remained present while the TUI was idle. It proves process/session residence, not busy state.
- It returned to `[]` after `/quit`.
- `events.jsonl` recorded `turn_started`, repeated `phase_changed` values such as `streaming_reasoning`/`streaming_text`, and `turn_ended` with `outcome: completed`.
- `summary.json` identifies the session and cwd; `signals.json` holds aggregate turn/tool/error/token/compaction/duration data.

Hook lifecycle provides a stable coordinator alternative. In installed 1.0.5, `UserPromptSubmit` means a prompt began; `Stop`, `StopFailure`, or `StopCancelled` means that prompt settled; `Notification` with `notificationType: idle_prompt` is a delayed state backstop. The installed hook guide warns that turn-end hooks can queue behind later activity, so correlate the newest `promptId` and ignore a late end for an earlier prompt. `subagentType` distinguishes child events.

`--debug-file` is append-mode diagnostic output. The isolated run included initialization, authentication, session creation, prompt queueing, first-token/response events, usage, and shutdown flush. It is rich enough for debugging but is not a safe or stable coordinator API: raw output contained a credential field and account metadata. The retained file is sanitized.

Requested artifacts:

| Artifact | Final shape | SHA-256 |
|---|---:|---|
| `grok-stream-sample.jsonl` | 29 lines, 8,903 bytes | `32b3992a0c404f4de017fa0a94edef29b8e56e713e5b4de108c60d905b7a1c87` |
| `grok-debug-sample.log` | 276 lines, 59,988 bytes | `3e4dc56f881ce693ca44fdbea1bf26b092655767ad06a26f3a41a114b5b87d0a` |

In tmux, `pane_current_command` and `pane_title` were both `grok` before, during, and after the turn. The title did not encode busy/idle. The TUI itself rendered `Grok 4.6 (low)`, context usage, spinners, “Worked for …”, and the empty composer; those are human UI, not a robust status channel. Grok supports terminal notification methods `auto`, `osc9`, `osc99`, `osc777`, `bel`, and `none`, plus title items and progress rendering. Those can alert a terminal, but no observed OSC/title change was a reliable state machine.

Process state stayed a sleeping multithreaded foreground process; socket and inhibitor FDs persisted. Neither `/proc` state nor fd/socket presence distinguished work from input wait.

### How verified

- Live ACP `session/new`/`session/prompt`, with the working and idle notifications captured.
- A separate tmux TUI sampled at 250 ms and after completion, with `active_sessions.json`, `events.jsonl`, process state, pane title, and FDs inspected.
- The installed hooks/notifications documentation and actual ACP initialization capability metadata.
- Full sanitized headless/debug artifacts and final hashes.

### Unverified

- The exact ACP extension vocabulary for every “waiting for approval,” “waiting for answer,” disconnected, and error state was not exhaustively triggered.
- `_x.ai/*` is vendor extension space and may change independently of standard ACP.
- Persisted `events.jsonl` flush latency and crash completeness are not guaranteed.
- No stable promise was found for a tmux pane-title state encoding.

### Recommendation for taurhaus

Preferred order: consume leader ACP `_x.ai/sessions/changed` and queue/prompt-complete notifications; also install versioned hooks as a lifecycle backstop; use `active_sessions.json` only for PID/session/cwd discovery; and use session files/debug logs only for recovery/diagnostics. Track `promptId` and session ID, not timestamps alone. Never use pane title, `ps` state, CPU, TCP sockets, or the existence of `active_sessions.json` as the busy bit.

## 5. TRANSCRIPTS

### Facts

Native conversations are stored automatically at:

```text
$GROK_HOME/sessions/<encoded-cwd>/<session-uuid>/
```

The normal cwd group is URL/percent encoded. The installed guide says a path too long for a directory component is converted to a slug plus hash and receives a `.cwd` marker containing the original path. No `.cwd` marker happened to exist in the current real store. `summary.json.info.id` and `.cwd` give the authoritative mapping rather than reverse-decoding the directory name alone. The official [sessions page](https://docs.x.ai/build/features/sessions) confirms sessions are keyed by working directory and shared across TUI, headless, and ACP.

Observed/permitted session members:

| File | Format and purpose |
|---|---|
| `summary.json` | Session metadata. Observed keys: `agent_name`, `chat_format_version`, `created_at`, `current_model_id`, `grok_home`, `info{id,cwd}`, `next_trace_turn`, message counts, `reasoning_effort`, `sandbox_profile`, `session_summary`, `updated_at`. |
| `updates.jsonl` | ACP-style replay/update stream. Installed docs call it the authoritative restore stream. |
| `chat_history.jsonl` | Raw model-wire conversation entries such as system/user/reasoning/assistant/tool content. |
| `events.jsonl` | Lightweight lifecycle events (`turn_started`, phase, first token, `turn_ended`). |
| `prompt_context.json` | Captured prompt/context metadata. |
| `rewind_points.jsonl` | Per-prompt file/chat rewind checkpoints. |
| `signals.json` | Aggregate counters, token/context usage, compaction count, tools/models, durations, and file/line stats. |
| `system_prompt.txt` | Resolved system prompt; sensitive project/rule context. |
| `title_refresh_idx` | Small title refresh cursor/index. |
| `*.lock` | Concurrency locks. |
| `plan.json`, `feedback.jsonl` | Documented optional files; absent from the inspected sessions. |
| `compaction_checkpoints/`, `subagents/` | Documented optional child/checkpoint directories; absent at snapshot. |

The root session search index is SQLite with a `session_docs(session_id,cwd,updated_at,title,content,content_hash)` table and FTS5 virtual/index tables. The worktree SQLite registry separately maps worktree ID/path/source repo/kind/git ref/head commit/session ID/creator PID/status/metadata.

Compaction is visible through several verified surfaces:

- `/compact [context]` and automatic threshold compaction.
- `PreCompact` and `PostCompact` hooks.
- `signals.json.compactionCount` and token-before/after/context fields.
- `chat_history.jsonl` compaction records and optional `compaction_checkpoints/` when produced.
- `streaming-messages-json` `system` compact-boundary messages, and debug lifecycle events for auto-compact start/completion/failure.

No actual compaction checkpoint existed in the inspected store, so the on-disk checkpoint payload shape was not sampled. `/context` and `/session-info` expose current context/compaction status. `grok export SESSION [OUTPUT]` renders a Markdown transcript; `grok sessions list/search/delete` provides management, with delete being destructive.

### How verified

- Key-only/file-name inspection of two cwd groups and 11 native session directories.
- JSON/JSONL outer schemas and SQLite `.schema`; no transcript text was copied into this report.
- Installed session/headless/hook guides and the official [sessions documentation](https://docs.x.ai/build/features/sessions).

### Unverified

- Optional compaction checkpoint and nested subagent payload schemas were not present locally.
- JSONL/update fields are not documented as a frozen public storage API.
- Remote/mirrored session reconciliation and moved-workspace mapping were not exercised.

### Recommendation for taurhaus

Use ACP and `grok sessions`/`grok export` for normal integration. For recovery/indexing, locate by `summary.json.info.id` and `info.cwd`, tail JSONL defensively, and ignore unknown records. Do not mutate the store. Treat `system_prompt.txt`, raw chat, prompt context, rewind snapshots, search content, and exported Markdown as sensitive. Detect compaction through hooks/ACP first and disk markers second.

## 6. HOOKS / NOTIFY / AGENTS / SKILLS / MCP

### Facts

Installed 1.0.5 hook events are:

```text
SessionStart SessionEnd UserPromptSubmit
PreToolUse PostToolUse PostToolUseFailure PermissionDenied
Stop StopFailure StopCancelled Notification
SubagentStart SubagentStop (SubagentEnd compatibility alias)
PreCompact PostCompact
```

`PreToolUse`, `Stop`, and `SubagentStop` are blocking in this build; ACP initialization advertised exactly those three under `_meta.x.ai/hooks.blockingEvents`. This is newer/more precise than the official hooks page's July text, which still describes only `PreToolUse` as blocking. Stop gates can continue the model with feedback and default to a 600-second timeout; passive hooks default to 5 seconds and fail open. A cancelled/refused/max-turn/no-progress turn fires `StopCancelled` instead of `Stop`. `SessionEnd` also fires during shutdown; a stop gate's decision is ignored when no turn remains.

Hook discovery:

- Personal: `~/.grok/hooks/*.json`, trusted.
- Project: `.grok/hooks/*.json`, requires folder trust.
- TOML hooks in config, managed/requirements hooks, plugin hooks.
- Claude/Cursor compatible hook configurations.

Event JSON is sent on stdin in camelCase. Common environment names include `GROK_HOOK_EVENT`, `GROK_HOOK_NAME`, `GROK_SESSION_ID`, `GROK_WORKSPACE_ROOT`, and `CLAUDE_PROJECT_DIR`; plugin hooks additionally receive `GROK_PLUGIN_ROOT` and `GROK_PLUGIN_DATA`. The official [hooks page](https://docs.x.ai/build/features/hooks) documents the JSON/command/HTTP contract; installed docs add the newer events and stop semantics.

Notifications can use terminal OSC 9/99/777, BEL, automatic selection, or none; conditions include always/unfocused/never and events such as turn complete, approval required, session ready, task complete, and agent error. Notification command hooks receive `GROK_EVENT`, `GROK_MESSAGE`, and `GROK_SESSION_ID`. TUI title composition and progress display are configurable. There is no verified arbitrary external “statusline command” protocol analogous to some other CLIs; Grok has its own status bar/title/progress plus hooks.

Agent/subagent inputs:

- `--agent NAME_OR_FILE` selects a discovered agent name or definition file; `GROK_AGENT` is the environment equivalent.
- `--agents JSON` supplies inline subagent definitions in headless mode.
- Agent Markdown files use YAML frontmatter followed by the system prompt body. Built-in examples contain `name`, `description`, `prompt_mode`, `permission_mode`, `agents_md`, and `model`.
- Definitions are discovered under project `.grok/agents/`, user `~/.grok/agents/`, built-ins, and plugins. The official [subagents page](https://docs.x.ai/build/features/subagents) identifies built-in `general-purpose`, `explore`, and `plan` roles.

Parser probes established that `--agents` requires a JSON object/map, not an array. `{"reviewer":{}}` parsed; map values accepted string `description` and `model`; `permissionMode` expected a string/map rather than an integer. Binary type strings name additional candidate fields including `promptMode`, `capabilityMode`, `permissionMode`, `skills`, `discoverSkills`, `inheritSkills`, `injectDefaultTools`, `tools`, `disallowedTools`, `effort`, `isolation`, `color`, `initialPrompt`, `mcpServers`, `mcpInheritance`, `hooks`, `memory`, `model`, `completionRequirement`, `toolOverrides`, and `userMessageTemplate`. Unknown JSON fields were tolerated in the parser probes, so presence in strings is not proof that every field is accepted by `--agents` in every context.

Skills are `SKILL.md` folders found in project/user/plugin/extra paths. User-invocable skills become slash commands. Installed bundled skills include build/review/design/implementation/workflow/media/document skills, and the runtime merged compatible Claude user skills as well. The official [skills/plugins page](https://docs.x.ai/build/features/skills-plugins-marketplaces) documents the discovery and frontmatter contract. The slash surface also includes native commands such as `/compact`, `/context`, `/session-info`, `/model`, `/effort`, `/always-approve`, `/resume`, `/fork`, `/rewind`, `/export`, `/hooks`, `/plugins`, `/skills`, `/mcps`, `/usage`, `/login`, `/logout`, `/quit`, and others; available commands can change with mode and installed skills.

MCP supports `stdio`, HTTP, and SSE transports. `grok mcp list/add/remove/enable/disable/doctor` manages user or project entries; `--scope project` writes `.grok/config.toml`, while user scope writes `$GROK_HOME/config.toml`. HTTP headers/env substitutions and OAuth are supported; OAuth credentials are documented under `$GROK_HOME/mcp_credentials.json`. `/mcps` manages runtime servers. The official [MCP page](https://docs.x.ai/build/features/mcp-servers) covers namespacing and configuration.

### How verified

- Installed hook, agent, subagent, skill, plugin, notification, and MCP guides.
- ACP initialize capability metadata and live available-command updates.
- Read-only built-in agent/skill frontmatter inspection.
- Isolated `--agents` parser probes stopped before model execution by using an invalid model after parsing.
- Complete MCP/plugin/subcommand help tree.

### Unverified

- A complete, versioned JSON Schema for `--agents` was not exposed; binary string names are not a public schema.
- No real hook command was installed because changing hook configuration was prohibited.
- OSC behavior across every terminal/tmux configuration was not exercised.
- MCP OAuth credential shape was absent locally and not inspected.

### Recommendation for taurhaus

Use hooks for lifecycle/notification fan-out and ACP for control. Ship agent definitions as validated files rather than large inline JSON; if inline definitions are necessary, validate them against the installed version with a no-run parser check. Treat project hooks/MCP/plugins as trust-requiring capability inputs. Keep a per-version allowlist for slash commands and MCP transports, and do not assume compatible-vendor skills are harmless merely because Grok discovers them.

## 7. DELIVERY

### Facts

There is no advertised `grok send` subcommand. Piping text to an already-running TUI's stdin is not a supported message API, and one-shot JSONL stdout is not bidirectional.

A supported external delivery route was verified end to end against the exact session displayed by a running TUI:

1. Start a shared leader (`grok agent leader`) and launch the TUI with `[cli] use_leader = true` in the same `GROK_HOME` (leader mode is off by default).
2. An external process starts `grok agent --leader stdio` with that home.
3. Send newline-delimited ACP JSON-RPC `initialize`; authenticate if the session is not already eagerly authenticated.
4. Send `session/load` with the running TUI's `sessionId`, `cwd`, and `mcpServers: []`.
5. Send `session/prompt` with that same ID and a standard ACP text content block.

The external client received the prior turn as replay updates, then sent the harmless prompt. The leader emitted `activity: working` then `idle`; the running tmux TUI visibly displayed the externally supplied second prompt and its `OK` response; the same session's message counts increased. Disconnecting the ACP client left the TUI resident. Thus this is not merely “create another session”—same-session injection works when both clients use the same leader.

The official [headless/ACP page](https://docs.x.ai/build/cli/headless-scripting) documents newline-delimited ACP `initialize`, `authenticate`, `session/new`, and `session/prompt`; the installed session guide also documents `session/load`. The dashboard is another supported leader-aware control surface: official product material shows dispatching new work and taking over sessions, but it is a TUI rather than a simple send command.

The raw leader Unix socket is **not** newline ACP. A connect-and-read attempt returned zero bytes. Sending a JSON-RPC line directly made the server interpret the first four bytes (`{"js`) as the decimal length 2,065,853,043 and reject it against a 67,108,864-byte maximum. This verifies a four-byte big-endian length prefix and mandatory registration before ordinary traffic. A supported `grok agent --leader stdio` client registered successfully with client type, version, mode, and yolo state; `grok leader info --json` reported `leader_protocol_version: 1`. The private registration/envelope schema is not documented.

tmux keystrokes remain a pragmatic fallback for a TUI that was not launched in leader mode. They are terminal input, not semantic delivery, and must handle focus, overlays, bracketed paste, and prompt state.

### How verified

- Same-session leader/TUI/ACP probe in a disposable home.
- ACP replay, prompt, completion, activity, and on-screen TUI observation.
- Raw Unix socket connect/read and deliberately invalid newline JSON probe.
- Leader debug registration events and `leader info` protocol metadata.

### Unverified

- Concurrent prompt arbitration if the TUI and external ACP client both submit at precisely the same time.
- Whether mid-turn external `session/prompt` always queues or steers under every `follow_up_behavior` setting.
- Private raw registration/envelope schema, compatibility negotiation beyond protocol version 1, and security assumptions for a process that can open the same user's socket.
- Delivery into a non-leader TUI by any method other than terminal input was not found.

### Recommendation for taurhaus

Make leader-backed ACP the semantic ingress: same `GROK_HOME`, check `leader info` version, initialize/authenticate, load the exact session, then prompt. Serialize writers per session and wait for prompt completion. Restrict socket/home filesystem access because local processes with access can control sessions. Do not implement the private framed socket protocol; use the installed Grok ACP adapter. Use tmux keystrokes only for legacy/non-leader sessions and clearly label that route as best effort.

## 8. STOP

### Facts

`/quit` (alias `/exit`) is the verified graceful TUI exit. In the tmux probe it terminated the Grok process, removed the session from `active_sessions.json`, and allowed the dedicated tmux server to exit normally.

Interactive key behavior in installed 1.0.5 is stateful:

- During a turn, `Esc` cancels immediately in minimal/default non-vim fullscreen mode; fullscreen vim scrollback swallows it, so use `Ctrl+C`.
- `Ctrl+C` with a non-empty draft clears the draft first; another press with an empty composer cancels the running turn.
- While cancellation is already pending, `Ctrl+C` can escalate toward quit.
- Global quit is `Ctrl+Q` (or `Ctrl+D` in VS Code-family terminals), requiring a double press within about one second. Older summary text calls this “with confirmation.”
- ACP `session/cancel` is the semantic turn cancel; cancellation fires `StopCancelled` with `reason: user_interrupt`.
- ACP `session/close` was tested and returned a close outcome while emitting a sessions-removed notification.

Headless installed documentation specifies exit 0 for normal completion, 1 for runtime/auth/network error, 130 for SIGINT, and 143 for SIGTERM. It says the session is saved through the last completed tool call and file changes are not rolled back. Foreground leader Ctrl-C exited on signal in the probes and removed the live process, though stale socket/lock files can remain and should be classified with `leader list` rather than assumed live.

### How verified

- Live `/quit`, ACP `session/close`, client EOF, leader signal, process liveness, and active registry checks.
- Installed keyboard, hook, headless, and session documentation.

### Unverified

- Every overlay-specific double-press/escalation path was not physically replayed.
- Crash behavior during an in-flight destructive tool was intentionally not tested.
- Signal-exit cleanup of socket files varied by launch wrapper; only process termination was consistent.

### Recommendation for taurhaus

Use ACP `session/cancel` for a turn and `session/close` for a leader-owned session. For an interactive tmux TUI, send `/quit` at an idle composer and wait for process exit; use signal escalation only on timeout. Never assume cancel rolls back files. After exit, verify PID liveness and use `grok leader list --json` to distinguish reachable versus stale socket/lock state.

## 9. USAGE / QUOTA + ACCOUNTS

### Facts

Usage surfaces in this build:

- `/usage` (alias `/cost`) opens usage; `/usage manage` links to credit usage/billing management.
- `/context` shows context-window use; `/session-info` includes auth method, model, turn/context details, and session identity.
- The TUI status rendered current tokens versus the 500K window.
- Headless JSON/JSONL and ACP completion metadata include input/output/total/cached/reasoning tokens, model calls, API duration, per-model usage, and `costUsdTicks` when supplied.
- `signals.json` persists aggregate token/context/latency counts.
- Rate-limit/capacity errors are classified for `StopFailure`; the UI/log strings include rate-limit messaging.

The live sanitized authenticate response identified the current signed-in tier as `supergrok`. This matches one of the launch-eligibility tiers described in the official [Grok Build announcement](https://x.ai/news/grok-build-cli), although current entitlements are dynamic and the product page now also advertises free trial access.

No top-level `grok usage` or `grok quota` subcommand exists in 1.0.5. Binary strings expose internal billing/usage vocabulary including `x.ai/session/usage`, a `/billing?format=credits` path, credit usage percent, billing period, monthly/on-demand/prepaid balances, and subscription tier. Those are implementation details, not a verified public endpoint for taurhaus. The official API [rate-limit page](https://docs.x.ai/developers/rate-limits) concerns API tiers and does not establish a stable Grok Build subscription-quota API.

Verified service/path strings, cross-checked where possible with cache/config/docs:

| Endpoint/path | Observed role |
|---|---|
| `https://auth.x.ai` | OIDC issuer/auth record prefix. |
| `https://cli-chat-proxy.grok.com/v1` | Account-backed model catalog/inference proxy; exact origin appears in `models_cache.json`. |
| `wss://code.grok.com/ws/code-agent` | Remote session/relay target named by leader tooling. |
| `https://code.grok.com` | Remote code/session service base string. |
| `https://api.x.ai` / `/v1` | API-key/custom-model API base. |
| `https://grok.com` and usage/supergrok paths | Account/usage management UI links. |
| `https://x.ai/cli/install.sh`, `install.ps1`, `/cli/changelogs` | Installer/update/changelog assets. |
| `https://storage.googleapis.com/grok-build-public-artifacts/cli...` | Fallback binary artifact storage. |

The official [enterprise guide](https://docs.x.ai/build/enterprise) independently identifies `cli-chat-proxy.grok.com`, `code.grok.com`, `assets.grok.com`, `x.ai`, and the Google storage fallback and says transport uses TLS.

Account storage is a map keyed by issuer/client identity and currently contains one account. `grok login` reauthenticates/switches the account and `grok logout` clears cached credentials. There is no advertised per-invocation `--account` selector. Structurally the JSON map could hold more than one issuer/client record, but simultaneous account-selection semantics were not verified.

### How verified

- Live headless/ACP usage payloads, TUI display, sanitized auth metadata, installed slash-command and monitoring guides.
- Top-level help proving no usage/quota subcommand.
- Key/path-only binary strings and models cache origin.
- Official announcement, enterprise, and rate-limit pages.

### Unverified

- A stable programmatic subscription quota/remaining-credit endpoint.
- Whether `/usage` always exposes rate-limit reset times for every plan.
- Supported simultaneous multi-account selection within one `GROK_HOME`.
- Internal billing method/path authorization and response schema.

### Recommendation for taurhaus

For per-turn accounting, consume the usage object from headless/ACP completion and store it with session/prompt IDs. Expose `/usage` as a human action, not an API dependency. Treat rate-limit errors as transient classified failures with backoff. For multiple accounts, use separate secured `GROK_HOME` instances and never copy auth values into logs or capability metadata; do not depend on the auth map's internal multi-record shape.

## 10. VERSIONING

### Facts

Programmatic version reads:

```text
$ grok --version
grok 1.0.5 (5115b46bc9) [stable]

$ grok version --json
{"currentVersion":"1.0.5 (5115b46bc9)","channel":"stable"}
```

`$GROK_HOME/version.json` separately stores `version`, `stable_version`, and `checked_at`. In a disposable home, the read-only update probe returned:

```json
{"currentVersion":"1.0.5","latestVersion":"1.0.5","updateAvailable":false,
 "installer":"internal","channel":"stable","autoUpdate":false,"error":null}
```

`grok update` exists. Its installed options are `--check`, `--json` (for check), `--force-reinstall`, `--version VERSION`, `--alpha`, and `--stable`. Help calls alpha faster/more experimental and stable the default weekly channel. A coordinator can pass `--no-auto-update` to headless/ACP runs; persistent equivalent is `[cli] auto_update = false`.

The official [Grok Build changelog](https://x.ai/build/changelog) identified 1.0.5, dated 2026-08-15, as latest at verification and matched the local `CHANGELOG.md`. Official usage/setup documentation is at [docs.x.ai/build](https://docs.x.ai/build/overview); the official product page is [x.ai/build](https://x.ai/build); the source page links `xai-org/grok-build` from [x.ai/open-source](https://x.ai/open-source).

### How verified

- Installed `--version`, `version --json`, `update --help`, local version/changelog files, and isolated `update --check --json`.
- Official changelog, product, docs, and open-source pages fetched on 2026-08-28.

### Unverified

- No update was installed, so rollback behavior, signature verification details, symlink replacement atomicity, and channel migration were not exercised.
- “Weekly” is the help's channel description, not a guaranteed release SLA.
- The online latest version can change after this report.

### Recommendation for taurhaus

Record both semantic version and commit in capability discovery using `grok version --json` (fall back to `--version`). Run `grok update --check --json` only in a disposable/read-write home or a dedicated update job because checking can refresh metadata. Pin/test capability slices per version; do not update during a coordinated session. Link users to the official changelog for drift and rerun the help/schema probes after upgrades.

## Compact JSON summary

```json
{
  "process_signature": "Direct symlink to a static-pie x86-64 ELF; interactive is no single-prompt flag/subcommand, headless has -p/--single/--prompt-file/--prompt-json, commands must be parsed from the installed command set.",
  "launch_flags": "-m/--model; --reasoning-effort/--effort; --always-approve; -c/--continue; -r/--resume; -s/--session-id is new-only; --fork-session; --cwd; -p; four output formats; -w is ineffective for headless creation.",
  "config_dir": "~/.grok by default; all per-process state relocates under $GROK_HOME.",
  "selector_env": "GROK_HOME",
  "identity": "Signed in through grok.com OIDC; auth.json map key is issuer::client-UUID; one account; tier supergrok; secret values omitted.",
  "busy_idle": "Best signal is leader ACP _x.ai/sessions/changed activity=working|idle plus queue/prompt-complete; hooks are the backstop; process/title/fds are not reliable.",
  "transcripts": "$GROK_HOME/sessions/<encoded-cwd>/<uuid>/ with summary JSON, replay/chat/event/rewind JSONL, signals, prompt context, system prompt, locks, and optional compaction/subagent state.",
  "hooks": "Session/prompt/tool/permission/stop/cancel/subagent/compaction/end hooks; PreToolUse, Stop, and SubagentStop block in installed 1.0.5; terminal OSC/BEL notifications supported.",
  "delivery": "Verified same-session injection: shared leader + grok agent --leader stdio, ACP initialize/authenticate, session/load, then session/prompt; no grok send command.",
  "stop": "ACP session/cancel or session/close; interactive /quit is graceful; Ctrl-C/Esc cancel with stateful semantics; headless SIGINT/SIGTERM documented as 130/143.",
  "usage": "Usage is available in /usage, /context, /session-info and headless/ACP usage objects; no stable external quota endpoint or top-level usage command verified.",
  "leader_socket": "$GROK_HOME/leader.sock plus leader.lock; one leader per socket/home; protocol v1 private 4-byte-BE-length-framed registration, so use Grok's ACP adapter rather than raw IPC.",
  "report_path": "/tmp/claude-1000/-home-mstie-projects-taurhaus/<uuid>/scratchpad/grok-report-codex.md",
  "unverified": [
    "private leader registration/envelope schema and future compatibility",
    "all ACP waiting-for-input extension states and simultaneous-writer arbitration",
    "complete --agents JSON schema",
    "stable programmatic subscription quota endpoint",
    "simultaneous multi-account selection in one GROK_HOME",
    "optional compaction/subagent checkpoint payload schemas"
  ]
}
```
