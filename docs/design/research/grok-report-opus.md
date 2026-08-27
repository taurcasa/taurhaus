# Grok CLI capability report for taurhaus (grok 1.0.5 stable)

Probed on 2026-08-28 (WSL2 Linux, user `mstie`). All facts below were verified by
command output, on-disk files, binary strings, or cited docs. Inferences are marked
**UNVERIFIED** with what would settle them.

**Binary**: `~/.local/bin/grok` → `~/.grok/bin/grok` →
`~/.grok/downloads/grok-linux-x86_64` (ELF 64-bit static-pie, stripped,
166,854,368 bytes, BuildID `df459c3cd090505e639a83d8a3a50d63add79245`).
`grok --version` → `grok 1.0.5 (5115b46bc9) [stable]`.
`~/.grok/bin/agent` is a second symlink to the same binary.

**Product name**: "Grok Build" (`grok --help` header: *Grok Build TUI*). The vendor
string inside the binary/system prompts is "SpaceXAI".

**Critical framing for taurhaus**: grok is *deliberately* Claude-Code-shaped. It reads
`~/.claude/settings.json` hooks, `~/.claude/skills/`, `~/.claude.json` MCP servers and
`.mcp.json`; it emits Anthropic Messages wire-format NDJSON; its Stop-hook decision
vocabulary is Claude-compatible. **grok is already executing taurhaus's Claude hooks on
this machine** — see slice 6.

---

## 1. PROCESS SIGNATURE

### Facts

**One process, no launcher indirection.** The symlink target *is* the agent binary.
`grok …` execs directly; there is no wrapper script, no node/python shim, no re-exec of
a second binary. Verified with `ps -eo pid,ppid,args`: an interactive launch under tmux
produced exactly one `grok` process (pid 1733435, stat `Ssl+`), and a headless launch
exactly one (pid 1731335, child only of my `timeout` wrapper). No child processes were
spawned in either mode for a no-tool prompt.

**argv shapes** (detect from `/proc/<pid>/cmdline`):

| Mode | argv signature |
|---|---|
| Interactive TUI | `grok` with **no** `-p`/`--single`/`--prompt-file`/`--prompt-json` and no subcommand. May carry `--cwd`, `--model`, `--resume`, a bare PROMPT positional, `--worktree`, `--always-approve`. |
| Headless / print | contains `-p` or `--single`, or `--prompt-file`, or `--prompt-json`. |
| ACP agent (stdio) | `grok agent … stdio` |
| ACP agent (WebSocket server) | `grok agent … serve` (default bind `127.0.0.1:2419`) |
| ACP agent (relay) | `grok agent … headless` |
| Shared backend | `grok agent … leader` |
| Management subcommands | first non-flag arg ∈ {`agent`,`completions`,`dashboard`,`doctor`,`du`,`export`,`inspect`,`leader`,`login`,`logout`,`mcp`,`memory`,`models`,`plugin`,`sessions`,`setup`,`trace`,`update`,`version`,`worktree`,`wrap`,`help`} |

Note the ambiguity trap: `grok "fix the bug"` is **interactive** (positional PROMPT =
seed prompt for a TUI session), while `grok -p "fix the bug"` is headless. Detect on the
flag, never on "has a positional".

**Process tree / fds.** The interactive process held open, verified via `/proc/<pid>/fd`:
`/dev/pts/N` on fd 0/1/17, `/dev/null` on fd 2, **`~/.grok/logs/unified.jsonl`
on fd 10 (write-only)**, an inotify fd, several epoll/eventfd/timerfd, an internal
socketpair, and 5 outbound TLS connections (Cloudflare + `35.186.241.51:443`). It also
holds `/run/systemd/inhibit/11.ref` (a sleep inhibitor). No listening socket in the
default configuration.

**The leader.** Off by default. `grok leader list` on a clean machine → `No leader
candidates found.`, and `~/.grok/leader.sock` did not exist. Leader mode is opt-in via
`--leader` or `[cli] use_leader = true` in `config.toml` (per `grok agent --help` and
`docs/user-guide/02-authentication.md:224`).

Starting one (`grok agent leader --relay-on-demand --no-auto-update
--no-exit-on-disconnect`) created:

- `~/.grok/leader.sock` — Unix **SOCK_STREAM**, `srwxr-xr-x`, LISTEN (confirmed via `ss -xl`)
- `~/.grok/leader.lock` — 7 bytes, holds the pid

`grok leader info --json` returned, verbatim:

```json
{"pid":1738109,"pidFromLock":1738109,"pidLive":1738109,"classification":"Reachable",
 "socketPath":"~/.grok/leader.sock","lockPath":"~/.grok/leader.lock",
 "wsUrlSuffix":"","clientId":3,
 "info":{"type":"leader_info","pid":1738109,"socket_path":"…","lock_path":"…",
 "ws_url_suffix":"","leader_protocol_version":1,"leader_binary_version":"1.0.5",
 "profiling_supported":true,"profiling_compiled_in":true,"cpu_profile_active":false,
 "cpu_profile_stopping":false,"profile_started_at":null,"profile_formats":[]}}
```

So: **one leader per user per socket path** (default `~/.grok/leader.sock`, overridable
per-process with `--leader-socket <PATH>` — every subcommand accepts it, so multiple
independent leaders are possible). Leader protocol version 1. It assigns an integer
`clientId` per connected client.

**An external process can talk to it — but not by guessing the framing.** I connected
from Python and sent newline-delimited `{"type":"leader_info"}` and a newline-delimited
JSON-RPC `initialize`; both got **zero bytes back and no banner on connect**. The
framing is therefore not bare NDJSON. `grok`'s own CLI talks to it successfully, so the
supported client is the binary itself.

**Sessions register with the leader implicitly** — a client attaches over the socket and
the leader hosts the ACP sessions. I verified an external ACP client can enumerate them:
`grok agent --leader stdio` + `session/list` returned every session for the cwd with
`sessionId`, `cwd`, `title`, `updatedAt` (full output in slice 7).

### How verified
`ps`, `/proc/<pid>/fd`, `/proc/<pid>/environ`, `ss -xl`/`ss -xp`/`ss -tnp`,
`grok leader list --json`, `grok leader info --json`, a Python `AF_UNIX` client,
`grok agent --leader stdio` handshake.

### Unverified
- **Leader socket wire framing.** Not NDJSON. Would be settled by `strace -f -e trace=write`
  on `grok leader info` while a leader runs, or by the published `grok-agent-sdk` source.
- Whether a leader is auto-spawned by an interactive TUI when `use_leader = true`
  (the `grok agent leader --relay-on-demand` help text says leaders *are* auto-spawned
  from interactive clients, but I did not enable `use_leader` since that means editing
  `config.toml`, which is out of scope).

### Recommendation for taurhaus
Detect grok sessions by argv on `/proc/*/cmdline`, keyed on the presence/absence of
`-p|--single|--prompt-file|--prompt-json` and on a subcommand token in argv[1]. **Do not
implement the leader socket protocol.** If taurhaus ever wants a non-tmux control plane,
shell out to `grok agent --leader stdio` and speak ACP JSON-RPC over its stdin/stdout —
that is a documented, stable surface and it worked first try.

---

## 2. LAUNCH

### Facts — flags verified against `grok --help` on 1.0.5

**Model**: `-m, --model <MODEL>`. Validated eagerly — `--model bogus-model` →
`Couldn't set model 'bogus-model': Invalid params: "unknown model id"`, exit non-zero.

**Model list**: `grok models` (a *subcommand*, not a slash command). Output:

```
You are logged in with grok.com.

Default model: grok-4.6

Available models:
  * grok-4.6 (default)
  - grok-4.5
```

Both models report `totalContextTokens: 500000` in the ACP `initialize` response.

**Reasoning effort**: `--reasoning-effort <EFFORT>` (alias `--effort`). Verified valid
values by feeding a bad one:
`unknown effort level 'bogus'; use one of: xhigh, high, medium, low`.
Per the ACP `initialize` payload: grok-4.6 supports `xhigh|high|medium|low` with
**`high` as default**; grok-4.5 supports `high|medium|low`, default `high`. Confirmed on
disk — every probe session's `summary.json` recorded `"reasoning_effort": "high"`, and
the TUI prompt border renders `Grok 4.6 (high)`. Runtime slash command: `/effort <level>`.

**Auto-approve**: `--always-approve` — "Auto-approve all tool executions". Documented
aliases and equivalences (`docs/user-guide/14-headless-mode.md:527`,
`22-permissions-and-safety.md:40,71-72`):

- `--always-approve` ≡ `--yolo` ≡ `--permission-mode bypassPermissions`
- Config equivalent: `[ui] permission_mode = "always-approve"` in `~/.grok/config.toml`
- Legacy/lockdown key: `[ui] yolo = false` (present in this machine's `config.toml`);
  in `requirements.toml` it *disables* always-approve org-wide
- **Deny rules, hooks, and admin locks still apply under always-approve.**
- Always-approve and `auto` are mutually exclusive; always-approve wins.

Full `--permission-mode` value set: `default`, `acceptEdits`, `auto`, `dontAsk`,
`bypassPermissions`, `plan`.

**Session lifecycle** (1.0.5 semantics — these changed recently, see slice 10):

| Intent | Flag |
|---|---|
| Fresh session | default; no flag |
| Fresh session with a **client-chosen UUID** | `-s, --session-id <UUID>` — **creates only**. Errors if not a valid UUID or if that ID already exists under the target session dir. It does **not** resume. |
| Continue most recent for cwd | `-c, --continue` |
| Resume by ID or title | `-r, --resume [<SESSION_ID_OR_TITLE>]` — errors if absent. Non-UUID values match titles case-insensitively for the cwd; UUID-shaped values always mean IDs. |
| Fork on resume | `--fork-session` (with `-r`/`-c`; `-s` then names the child UUID) |
| Restore repo snapshot on resume | `--restore-code` (remote sessions require `--worktree`) |

**Initial prompt**: bare positional `PROMPT` seeds an *interactive* session.
`-p, --single <PROMPT>` is single-turn headless. `--prompt-file <PATH>` and
`--prompt-json <JSON>` (content blocks) are the file/structured variants.
`--verbatim` sends the prompt exactly as given (no wrapping).

**Headless output**: `--output-format {plain|json|streaming-json|streaming-messages-json}`,
default `plain`. `--include-partial-messages` adds incremental `stream_event` lines and
**only affects `streaming-messages-json`**. `--json-schema <SCHEMA>` constrains output to
a JSON Schema and implies `--output-format json`.

**stdin**: headless mode **does not read piped stdin into the prompt**
(`docs/user-guide/14-headless-mode.md:395`). Use command substitution or `--prompt-file`.
Confirmed by the flag surface: there is no `-` prompt form.

**`--worktree [<WORKTREE>]`** (`-w`): starts the session in a new git worktree,
optionally named; `--worktree-ref`/`--ref` picks the base commit (defaults to source
HEAD). **`--worktree` is ignored in headless `-p` mode** — the help says explicitly
"Headless (`-p`) does not create a worktree from this flag". Worktrees are registered in
`~/.grok/worktrees.db` (SQLite, 40,960 bytes here) and managed with `grok worktree
list|show|rm|gc|db`.

**`--cwd <CWD>`**: sets the working directory. Project root is then discovered by walking
**upward from `--cwd` until a `.git` directory is found**
(`docs/user-guide/14-headless-mode.md:592`) — so pointing `--cwd` inside a monorepo makes
grok scope AGENTS.md/skills/git history to the whole monorepo and slows startup. Verified:
`grok inspect --json` in a non-git scratch dir reported `"projectRoot": null`.

**Other launch flags worth knowing**: `--tools`/`--disallowed-tools` (comma lists),
`--allow`/`--deny <RULE>` (aliases `--allowedTools`/`--disallowedTools`), `--max-turns <N>`,
`--rules <RULES>` (appended to system prompt), `--system-prompt-override`,
`--agent <NAME|file>`, `--agents <JSON>`, `--no-plan`, `--no-subagents`,
`--disable-web-search`, `--sandbox <PROFILE>` (env `GROK_SANDBOX`), `--no-alt-screen`,
`--minimal`, `--fullscreen`, `--oauth`, `--leader-socket <PATH>`, `--debug`,
`--debug-file <FILE>`.

**Exit codes** (documented, `14-headless-mode.md:559`): `0` success, `1` error,
`130` SIGINT, `143` SIGTERM.

### How verified
`grok --help` in full plus `--help` on all 20 subcommands and the nested
`agent stdio|headless|serve|leader`, `leader list|info|kill`, `sessions list|search|delete`,
`mcp add|list`, `plugin marketplace`, `worktree list`, `memory clear`. Live runs of
`grok models`, `grok -p …`, and deliberate bad-value probes.

### Unverified
- `--json-schema` behavior end-to-end (not exercised).
- Whether `--worktree` in interactive mode reuses `worktree_pool/`.

### Recommendation for taurhaus
Launch template for a mesh member:

```
grok --cwd <project> --model grok-4.6 --effort high --permission-mode <mode> \
     --session-id <uuid-you-minted> "<seed prompt>"
```

Mint the UUID yourself with `-s` so taurhaus owns the session identity from second zero —
this is the single biggest ergonomic win over Codex/Claude, because you never have to
scrape an ID back out. Then `-r <uuid>` for every subsequent headless interaction.
Do **not** pass `--worktree` in headless mode expecting isolation; it is silently ignored.

---

## 3. CONFIG + IDENTITY

### Facts — `~/.grok` contents (verified with `ls -la` and `grok du --json`)

Total `192,356,352` bytes. Top-level directories by size: `downloads` 166,858,752 ·
`bundled` 13,709,312 · `marketplace-cache` 10,604,544 · `docs` 512,000 ·
`completions` 360,448 · `sessions` 73,728 · `logs` 28,672 · `memtrace` 4,096 ·
`bin` 0 · `relocations` 0.

| Entry | Size | Format / purpose |
|---|---|---|
| `config.toml` | 386 B | TOML user config |
| `auth.json` | 1,751 B | **mode 0600** — OIDC/OAuth credentials |
| `auth.json.lock` | 18 B | `<pid>:<epoch>` lock (`1704643:1787872353`) |
| `agent_id` | 36 B | **mode 0600** — stable per-install agent UUID |
| `active_sessions.json` | 2 B → grows | **live interactive-session registry** (slice 4) |
| `active_sessions.lock` | 0 B | lock sentinel |
| `version.json` | 103 B | `{"version","stable_version","checked_at"}` |
| `.metadata_version` | 5 B | `1.0.5` |
| `models_cache.json` | 4,537 B | cached model catalog |
| `CHANGELOG.md` / `CHANGELOG.json` | 1,657 / 2,803 B | structured changelog (`category`,`description`,`breaking_change`) |
| `README.md` | **109,061 B** | the complete offline manual |
| `docs/user-guide/*.md` | 25 files | the full chaptered manual (see below) |
| `sessions/` | dir, 0700 | transcripts, grouped by URL-encoded cwd |
| `sessions/session_search.sqlite` | 36,864 B | FTS5 index over titles/prompts |
| `logs/unified.jsonl` | 25,126 B | **central structured log, all sessions** |
| `bundled/` | manifest.json 52,999 B + `skills/` `agents/` `roles/` `personas/` | shipped extensions |
| `worktrees.db` | 40,960 B | SQLite worktree registry |
| `slash-mru.json`, `tip_cursor.json` | 68 / 12 B | UI state |
| `marketplace-cache/` | 10.6 MB | git caches for plugin marketplaces |
| `memtrace/`, `relocations/` | | internal |
| `.config-init.lock`, `managed_config.lock` | 0 B | lock sentinels |
| `leader.sock`, `leader.lock` | | created only in leader mode |

The bundled manual chapters (`~/.grok/docs/user-guide/`) are authoritative for *this*
version and are **ahead of the website** (see slice 10):
`01-getting-started` `02-authentication` `03-keyboard-shortcuts` `04-slash-commands`
`05-configuration` (47 KB) `06-theming` `07-mcp-servers` `08-skills` `09-plugins`
`10-hooks` (43 KB) `11-custom-models` `12-project-rules` `13-memory` `14-headless-mode`
(41 KB) `15-agent-mode` `16-subagents` `17-sessions` `18-sandbox` `19-plan-mode`
`20-background-tasks` `21-terminal-support` `22-permissions-and-safety` (31 KB)
`23-dashboard` `24-monitoring-usage`.

**This machine's `config.toml`** (verbatim, no secrets in it):

```toml
[cli]
installer = "internal"

[marketplace]
default_skills_installs_purged = true
official_marketplace_auto_installed = true

[[marketplace.sources]]
name = "xAI Official"
git = "https://github.com/xai-org/plugin-marketplace.git"

[ui]
max_thoughts_width = 120
fork_secondary_model = "grok-4.6"
yolo = false
compact_mode = false

[privacy]
privacy_banner_acked = "2026-08-27T23:12:53Z"
```

(The debug log shows grok itself warns about `path=privacy` as an unrecognized config
key — `[privacy]` is written by the TUI but not in the parser's schema. Harmless.)

### Config-dir selection env var

**`GROK_HOME`** — "Override config directory (default: `~/.grok`)". Documented in both
`README.md` §Environment Variables and `14-headless-mode.md`. Confirmed present in the
binary's string table. There is **no XDG_CONFIG_HOME support** — I found no `XDG_` env
strings alongside the GROK_ set. Two related overrides exist:
`GROK_CONFIG` and `GROK_CONFIG_PATH` — per `CHANGELOG.json` these are new in 1.0.5:
*"**GROK_CONFIG** and **GROK_CONFIG_PATH** environment variables now let launchers
override selected config settings without editing config.toml."*

### Env vars found in the binary (`strings | grep -oE '\bGROK_[A-Z0-9_]+\b'`)

Complete deduplicated list of well-formed names (a handful of truncated artifacts like
`GROK_HOH`/`GROK_SESH1` are string-table collisions, not real vars):

`GROK_AGENT` `GROK_AGENT_DASHBOARD` `GROK_AGENT_SECRET` `GROK_ANNOUNCEMENTS_OVERRIDE`
`GROK_APPEARANCE` `GROK_ASKPASS` `GROK_ASK_USER_QUESTION_TIMEOUT_ENABLED`
`GROK_ASK_USER_QUESTION_TIMEOUT_SECS` `GROK_AUTH_EARLY_INVALIDATION_SECS`
`GROK_AUTH_EXPIRED` `GROK_AUTH_PROVIDER_ACCESS_TOKEN` `GROK_AUTH_PROVIDER_COMMAND`
`GROK_AUTH_PROVIDER_EXPIRES_AT` `GROK_AUTH_PROVIDER_LABEL` `GROK_AUTH_PROVIDER_REFRESH_TOKEN`
`GROK_AUTH_TOKEN_TTL` `GROK_CAMPAIGNS_OVERRIDE` `GROK_CHANNEL` `GROK_CLAUDE_MCPS_ENABLED`
`GROK_CLAUDE_SKILLS_ENABLED` `GROK_CLIENT_NAME` `GROK_CLIPBOARD_NO_DATA_CONTROL`
`GROK_CLIPBOARD_NO_OSC52` `GROK_CLI_CHAT_PROXY_BASE_URL` `GROK_CODE_XAI_API_KEY`
`GROK_COMPACTION_DETAIL` `GROK_COMPACTION_MODE` `GROK_CONFIG` `GROK_CONFIG_PATH`
`GROK_COPY_FILE` `GROK_CURSOR_MCPS_ENABLED` `GROK_CURSOR_SKILLS_ENABLED` `GROK_DEBUG_LOG`
`GROK_DEFAULT_SELECTED_PERMISSION` `GROK_DEPLOYMENT_KEY` `GROK_DISABLE_AUTOUPDATER`
`GROK_EVENT` `GROK_EXIT_TIMEOUT_SECS` `GROK_EXTERNAL_OTEL` `GROK_EXTRA_CA_BUNDLE`
`GROK_FEEDBACK_ENABLED` `GROK_FOLDER_TRUST` `GROK_FORCE_LEGACY_CONSOLE`
`GROK_FSNOTIFY_MAX_WATCHES` `GROK_HOME` `GROK_HOOK_EVENT` `GROK_HOOK_NAME`
`GROK_INTERNAL_OTLP_TRACES_ENDPOINT` `GROK_INVERT_SCROLL` `GROK_LOG_FILE` `GROK_LSP_TOOLS`
`GROK_MARKETPLACE_REQUIRE_SHA` `GROK_MAXIMUM_VERSION` `GROK_MAX_MCP_OUTPUT_BYTES`
`GROK_MAX_PARALLEL_IMAGE_GEN_CALLS` `GROK_MAX_PARALLEL_VIDEO_GEN_CALLS`
`GROK_MCP_STARTUP_TIMEOUT_SECS` `GROK_MEMORY` `GROK_MESSAGE` `GROK_MINIMUM_VERSION`
`GROK_MODELS_BASE_URL` `GROK_MODELS_LIST_URL` `GROK_OIDC_CLIENT_ID` `GROK_OIDC_ISSUER`
`GROK_OTEL_FILTER` `GROK_PLUGIN_DATA` `GROK_PLUGIN_ROOT` `GROK_REQUIRED_MAXIMUM_VERSION`
`GROK_REQUIRED_MINIMUM_VERSION` `GROK_RESPECT_GITIGNORE` `GROK_SANDBOX` `GROK_SCROLL_LINES`
`GROK_SCROLL_MODE` `GROK_SCROLL_SPEED` `GROK_SESSION_ID` `GROK_SLASH_COMMAND_TAGS`
`GROK_SQLITE_JOURNAL_MODE` `GROK_SUBAGENTS` `GROK_SUBAGENTS_MAX_DEPTH`
`GROK_TELEMETRY_BUILD_EVENTS_API_KEY` `GROK_TELEMETRY_BUILD_EVENTS_URL`
`GROK_TELEMETRY_BUILD_MIXPANEL_TOKEN` `GROK_TELEMETRY_ENABLED` `GROK_TELEMETRY_EVENTS_API_KEY`
`GROK_TELEMETRY_EVENTS_URL` `GROK_TELEMETRY_GCS_BUCKET` `GROK_TELEMETRY_MIXPANEL_ENABLED`
`GROK_TELEMETRY_MIXPANEL_TOKEN` `GROK_TELEMETRY_TRACE_UPLOAD` `GROK_THEME`
`GROK_TRACE_UPLOAD_BUCKET` `GROK_UPLOAD_QUEUE_AUTH_PROBE_SECS` `GROK_VERSION`
`GROK_VOICE_CAPTURE` `GROK_WEB_FETCH` `GROK_WEB_FETCH_ALLOW_LOCAL` `GROK_WEB_FETCH_PROXY`
`GROK_WEB_SEARCH_MODEL` `GROK_WORKFLOWS` `GROK_WORKSPACE` `GROK_WORKSPACE_COMMAND`
`GROK_WORKSPACE_MAX_ARCHIVE_BYTES` `GROK_WORKSPACE_ROOT`
Plus `GROK_PRIVACY_NOTICE_ROLLOUT`, `GROK_PRIVACY_BANNER_RESHOW_DAYS`, `GROK_PLUGIN_CTA`,
`GROK_SESSION_PICKER_GROUPED`, `GROK_CHANGELOG_OFFLINE` (found in adjacent string blobs).

xAI-namespaced: **`XAI_API_KEY`** only (plus the internal `GROK_CODE_XAI_API_KEY`).
No `XAI_BASE_URL`-style vars; base URLs are `GROK_*` (`GROK_MODELS_BASE_URL`,
`GROK_CLI_CHAT_PROXY_BASE_URL`) or CLI flags (`--xai-api-base-url`, `--grok-ws-url`).

### Signed-in identity

**The machine IS signed in.** `grok models` printed `You are logged in with grok.com.`
and no probe ever hit an auth prompt.

Identity lives in **`~/.grok/auth.json`** (mode 0600), a JSON object keyed by
`"<oidc_issuer>::<client_id>"` — here `https://auth.x.ai::b1a00492-…`. Key **names** and
value shapes only (no values reproduced):

| Key | Type / shape |
|---|---|
| `key` | string, 882 chars (the bearer/JWT) |
| `auth_mode` | `"oidc"` |
| `create_time` | ISO-8601 string (30 ch) |
| `user_id` | UUID (36 ch) |
| `email` | string (17 ch) |
| `first_name` / `last_name` | strings |
| `profile_image_asset_id` | string (80 ch) |
| `principal_type` | `"User"` |
| `principal_id` | UUID (36 ch) |
| `team_id` | UUID (36 ch) — **a team account** |
| `coding_data_retention_opt_out` | bool `true` |
| `refresh_token` | string (86 ch) |
| `expires_at` | ISO-8601 string |
| `oidc_issuer` | string (17 ch) |
| `oidc_client_id` | UUID |

Separately, `~/.grok/agent_id` (36 B, 0600) holds a stable install UUID that the ACP
`initialize` response echoes as `agentId` (`<uuid>`), next
to a per-process `agentInstanceId`.

Auth methods advertised over ACP: `cached_token` (default, "Cached token from
~/.grok/auth.json") and `grok.com` ("Sign in with Grok").

Login paths: `grok login` (browser OAuth), `grok login --oauth` (auth.x.ai),
`grok login --device-auth` / `--device-code` (headless), `XAI_API_KEY` env,
`GROK_AUTH_PROVIDER_COMMAND` (external binary that prints a token to stdout),
`GROK_OIDC_ISSUER` + `GROK_OIDC_CLIENT_ID` (customer SSO). Tokens auto-refresh 300 s
before expiry (`GROK_AUTH_EARLY_INVALIDATION_SECS`, default 300).

### How verified
`ls -la ~/.grok`, `grok du --json`, `cat config.toml`, a Python walk of `auth.json` that
printed **key paths and value types/lengths only**, `strings` over the binary,
`grok models`, `grok inspect --json`, ACP `initialize` response.

### Unverified
- Whether `GROK_HOME` fully isolates `active_sessions.json` and `leader.sock`
  (the leader default path is written as `~/.grok/leader.sock` in help text, but
  `leader_info` reported the resolved absolute path — likely `$GROK_HOME`-relative).
  Verify: `GROK_HOME=/tmp/x grok agent leader …` then check where the socket lands.
- Multi-account: `auth.json` is a **map keyed by issuer::client_id**, so the file
  structurally supports several credentials. Whether grok exposes an account *switcher*
  is unverified — see slice 9.

### Recommendation for taurhaus
`GROK_HOME` is the per-runtime isolation knob, exactly parallel to `TAURHAUS_DATA_DIR`
and `TAURHAUS_CLAUDE_DIR`. Route grok through `platform_paths.rs` with a `GROK_HOME`
override so mesh members can be isolated and so E2E runs never touch the developer's real
`~/.grok`. Never read `auth.json`; treat `~/.grok/agent_id` as the stable install id and
`auth.json`'s `team_id`/`user_id` as *existence* signals only.

---

## 4. BUSY / IDLE + SESSION IDENTITY

This is the strongest slice. grok gives taurhaus **four independent busy/idle signals**,
three of which need no configuration at all.

### Signal A — `~/.grok/active_sessions.json` (live registry) ★ best for identity

A JSON array, rewritten as sessions open and close. Captured live during an interactive
tmux session:

```json
[
  {
    "session_id": "<uuid>",
    "pid": 1733435,
    "cwd": "/tmp/.../scratchpad/grok-probe",
    "opened_at": "2026-08-27T23:22:06.993848110Z"
  }
]
```

Verified behaviors:

- **Interactive only.** During a `grok -p` run polled once per second for 12 s, the file
  stayed `[]` the entire time. Headless sessions never register.
- **Registers at session creation, not process start.** The TUI process ran for ~16 s
  showing `[]`; the entry appeared within 1 s of the first prompt (`opened_at` matches
  the `session created` log line to the millisecond).
- **Cleaned on graceful exit.** After `/quit`, the file was `[]`.
- **Goes STALE on crash.** I `kill -9`'d a registered TUI: the entry **persisted** with a
  dead pid. → taurhaus must liveness-check `pid` before trusting a row.
- **Self-heals.** The very next `grok -p` run pruned the stale entry back to `[]`.

This is the authoritative **session_id ↔ pid ↔ cwd** map. Nothing else gives you the pid.

### Signal B — tmux pane title (OSC 2) ★ best for busy/idle, zero config

grok sets the terminal title continuously. Read with
`tmux display-message -p -t <target> '#{pane_title}'`. Observed transitions, verbatim:

| State | `#{pane_title}` |
|---|---|
| Process up, no session yet | `grok` |
| Turn running (pre-first-token) | `⠦ - Waiting for response… - grok` |
| Turn running (model thinking) | `⠙ - Thinking - Reply with single word OK - grok` |
| Idle, awaiting input | `Reply with single word OK - grok` |

Shape: `[<spinner> - <status> - ]<session title> - grok`. **Busy iff the title starts
with a braille spinner glyph** (`U+2800`–`U+28FF`). The session title is grok's
auto-generated one (slice 5), so the pane title doubles as a live task label for the
mesh canvas. The TUI status strip corroborates: `Esc:cancel` appears only while busy.

### Signal C — per-session `events.jsonl` ★ best for a file watcher

`~/.grok/sessions/<encoded-cwd>/<session-id>/events.jsonl`, one JSON object per line,
written for **both** headless and interactive sessions. Verbatim from a probe:

```json
{"ts":"2026-08-27T23:19:11.067Z","type":"turn_started","session_id":"01a04585-…","turn_number":0,"model_id":"grok-4.6","yolo_mode":false,"conversation_message_count":3,"session_relationship":"primary","schema_version":"1.0"}
{"ts":"…","type":"loop_started","loop_index":0}
{"ts":"…","type":"phase_changed","phase":"waiting_for_model"}
{"ts":"…","type":"first_token"}
{"ts":"…","type":"phase_changed","phase":"streaming_reasoning"}
{"ts":"…","type":"phase_changed","phase":"streaming_text"}
{"ts":"…","type":"turn_ended","outcome":"completed"}
```

Event types seen: `turn_started`, `loop_started`, `phase_changed`, `first_token`,
`turn_ended`. Phases: `waiting_for_model`, `streaming_reasoning`, `streaming_text`.
`turn_ended.outcome` was `completed`. `schema_version: "1.0"` is declared on
`turn_started` — a real versioning contract. **Busy = last event is not `turn_ended`.**
Note `phase_changed` is emitted once per streamed chunk (20 identical
`streaming_reasoning` lines for a 2-word answer), so debounce.

### Signal D — `~/.grok/logs/unified.jsonl` (central log, all sessions, one file)

Held open on fd 10 by every running process. Schema per line:
`{ts, src, pid, ver, lvl, sid?, msg, ctx?}` where `src` ∈ {`shell`, `grok-pager`} and
`sid` is the session UUID. The turn lifecycle is fully legible:

```
session created · prompt received · shell.prompt.queued · shell.handle_prompt.start
shell.turn.tool_prep_done · shell.turn.inference_start · shell.turn.inference_done
shell.handle_prompt.done · turn.first_activity · turn.phase_transition
turn.end_reconcile.armed · agent response complete · turn.complete
```

Also carries `billing: fetched credits config` (slice 9), `model changed`,
`session.create.start/done`, `prompt.acp_send.start/done`. One file for **all** sessions
and pids, so a single tail covers the whole machine.

### Signal E — hooks (configuration required, most precise)

`10-hooks.md` ships a **prescriptive recipe for exactly this problem**, and it is worth
following verbatim rather than inventing one. Quoting the doc:

> A complete busy and idle indicator takes five registrations. `UserPromptSubmit` marks
> the session busy; `Stop`, `StopFailure`, and `StopCancelled` settle it however the turn
> ended; the `idle_prompt` `Notification` is the backstop for the turns that report none
> of the three. Registering only `StopCancelled` leaves the host busy after every normal turn.

The doc's own five rules for the scripts:

1. Track the newest `promptId`; ignore reports for older turns (a cancelled turn's report
   is dispatched off the command loop and can arrive *after* the next `UserPromptSubmit`).
2. Settle unconditionally when there is **no** `promptId` — that is grok reporting on the
   *session* (the `idle_prompt` ping, the session-end `Stop`).
3. Treat a `promptId` you never saw start as idle (interrupted bash-mode turns report
   with no preceding `UserPromptSubmit`).
4. **Exit early when `subagentType` is present** — a subagent's stop is not the session's.
   Critical for background subagents that outlive the parent turn.
5. Settle the host *before* recording the turn handled, so a hook killed mid-flight leaves
   the turn correctable.

Two documented traps: `Stop` is a **gate**, so it fires again on every continuation round
and `stopHookActive` is true for both the continuation and the final fire — "a UI gated on
`Stop` alone shows a false idle from the first continuation fire". And `idle_prompt` fires
"about a minute after the session settles", needs at least one turn to have ended, and is
cancelled if another message arrives first — so it is a backstop, not a primary.

`Stop` input also carries `backgroundTasks[]` (`id`, `type` ∈ {`shell`,`monitor`,`subagent`},
`status`, `command`/`description`/`agentType`) and `sessionCrons[]` (`id`, `schedule`,
`recurring`, `prompt`), letting a host distinguish "done" from "paused waiting on
background work".

### Session identity

- Session IDs are **UUIDv7** (`01a04585-2d53-7123-…` — the `7` in position 13, and the
  monotonic prefix). They sort chronologically as strings. Confirmed across 10 sessions.
- Exposed in: `active_sessions.json`, every `events.jsonl` `turn_started`, `unified.jsonl`
  `sid`, `summary.json.info.id`, the headless JSON `sessionId` field, every
  `streaming-messages-json` line's `session_id`, `GROK_SESSION_ID` in hook env,
  ACP `session/list`, and the directory name itself.
- taurhaus can **pre-assign** it with `-s <uuid>`.

### Streaming-messages-json (captured to `grok-stream-sample.jsonl`, 31 lines / 9,901 B)

Anthropic Messages wire format, essentially drop-in with Claude Code's stream-json:

- line 0: `{"type":"system","subtype":"init", session_id, apiKeySource:"oauth", model,
  cwd, permissionMode, tools[], slash_commands[], mcp_servers[], skills[], uuid}`
- lines 1–28: `{"type":"stream_event","event":{…},"parent_tool_use_id":null,session_id,uuid}`
  with inner `event.type` ∈ `message_start`, `content_block_start`, `content_block_delta`
  (`thinking_delta` then `text_delta`), `content_block_stop`, `message_delta`, `message_stop`
- line 29: `{"type":"assistant","message":{…full content blocks incl. signed thinking…}}`
- line 30: `{"type":"result","subtype":"success","is_error":false,"duration_ms":2817,
  "duration_api_ms":1635,"num_turns":1,"result":"OK","stop_reason":"end_turn",
  "total_cost_usd":0.00345576,"usage":{…},"modelUsage":{"grok-4.6-build":{…}},session_id,uuid}`

`parent_tool_use_id` is the subagent-nesting key. **Busy/idle from this stream is trivial:
`system/init` = start, `result` = done.**

The `--debug-file` capture (`grok-debug-sample.log`, 269 lines / 62,019 B) is
`tracing`-formatted text, not JSON: `<ISO8601Z>  <LEVEL> <crate::module>: <msg> k=v k=v`.
Crates seen: `xai_grok_shell`, `xai_grok_agent`, `xai_grok_pager`, `xai_acp_lib`,
`xai_grok_telemetry`, `xai_grok_workspace`. It includes `startup phase phase=… elapsed_ms=…`
lines (`managed_policy`, `bootstrap`, `model_catalog`, `spawn_worker`, `acp_initialize`,
`eager_auth`, `session_create`) and the full ACP `initialize` JSON. Useful for diagnosis,
**not** for machine parsing.

### Process fd/socket state as a signal
Weak. The interactive process keeps its 5 TLS connections open whether idle or busy, and
holds `unified.jsonl` open permanently. Do not infer busy from fd state.

### How verified
12-second 1 Hz poll of `active_sessions.json` + `ps` during a headless run; a full
interactive tmux lifecycle (launch → prompt → busy → idle → long prompt → Ctrl+C →
`/quit`) with `tmux display-message -p '#{pane_title}'` and `tmux capture-pane` at each
step; `kill -9` stale test; on-disk reads of `events.jsonl`, `signals.json`,
`updates.jsonl`, `unified.jsonl` before/after (347 → 391 lines across one turn); the two
captured probe artifacts.

### Unverified
- `turn_ended.outcome` values other than `completed` (expect `cancelled`/`failed`).
  Verify: interrupt a turn in a session and re-read its `events.jsonl`.
- Whether `events.jsonl` is fsynced per line (matters for a watcher's tail semantics).
- The exact `idle_prompt` delay ("about a minute" per docs; not timed).

### Recommendation for taurhaus
**Primary: pane title.** It costs one `tmux display-message` per poll, needs no config,
works identically for every member, and yields busy/idle *plus* a task label. Regex:
`^[\x{2800}-\x{28FF}]` ⇒ busy.
**Secondary: `active_sessions.json`** for session_id ↔ pid ↔ cwd, with a
`kill(pid, 0)` liveness check on every row — mirror the stale-pid handling taurhaus
already does for tmux focus hooks.
**Tertiary: watch `~/.grok/logs/unified.jsonl`** — one `notify` watch covers every grok
session on the box, and `sid` demultiplexes. This slots directly into the existing
`compaction_watcher.rs` / `session_scanner` pattern.
Reserve hooks for when you need *precision* (blocking gates, `backgroundTasks`). If you do
adopt them, implement the doc's five rules exactly, and register `SessionEnd` rather than
`Stop` if you ever also run a blocking `Stop` gate.

---

## 5. TRANSCRIPTS

### Facts — storage layout (verified on disk)

```
~/.grok/sessions/
  session_search.sqlite                     # FTS5 index over titles + prompts
  <URL-ENCODED-CWD>/                        # e.g. %2Fhome%2Fmstie%2Fprojects%2Flocalllms
    prompt_history.jsonl                    # GROUP level: every prompt for this cwd
    <session-uuid>/
      summary.json            (+ .lock)     # index entry / metadata
      events.jsonl                          # turn + phase lifecycle  (slice 4)
      updates.jsonl           (+ .lock)     # ACP session/update stream — authoritative
      chat_history.jsonl      (+ .lock)     # raw messages sent to the model
      system_prompt.txt                     # resolved system prompt (5,779 B here)
      prompt_context.json                   # how the prompt was assembled
      signals.json                          # per-session counters/metrics
      rewind_points.jsonl     (+ .lock)     # /rewind undo points
      title_refresh_idx                     # 1 byte
      plan.json / plan.md                   # plan-mode state (when used)
      compaction_checkpoints/               # saved compaction state (when used)
      feedback.jsonl                        # ratings (when used)
      subagents/                            # per-subagent meta.json (when used)
```

**Session ↔ cwd mapping is the directory name**: percent-encoded absolute cwd. When the
encoded name exceeds 255 bytes grok falls back to *slug + hash* and drops the real path
in a `.cwd` file inside the group. taurhaus must handle both forms — the encoded name is
not always decodable back to a path.

`summary.json` verbatim from a probe:

```json
{"info":{"id":"<uuid>","cwd":"/tmp/…/grok-probe"},
 "session_summary":"","created_at":"2026-08-27T23:18:55.652492630Z",
 "updated_at":"2026-08-27T23:18:57.301584586Z","num_messages":4,"num_chat_messages":6,
 "current_model_id":"grok-4.6","next_trace_turn":1,"chat_format_version":1,
 "request_id":"015230d2-…","grok_home":"~/.grok",
 "last_active_at":"2026-08-27T23:18:57.301584586Z","agent_name":"grok-build-plan",
 "sandbox_profile":"off","reasoning_effort":"high"}
```

Note it records `grok_home`, `agent_name`, `sandbox_profile` and `reasoning_effort` —
everything taurhaus needs to render a session card without launching anything. Docs
additionally list `generated_title`, `title_is_manual`, `parent_session_id` (fork/restore
lineage), `last_turn_summary`, `last_recap`.

**Doc-vs-reality discrepancy worth recording**: `17-sessions.md` describes `updates.jsonl`
as the ACP stream and does not mention `events.jsonl`; `14-headless-mode.md` describes
`sessions/` as "Session transcripts (SQLite)". On disk in 1.0.5 **both** `updates.jsonl`
and `events.jsonl` exist and are JSONL; SQLite is only the *search index*. Trust the disk.

`updates.jsonl` lines are ACP JSON-RPC notifications:
`{"timestamp":<unix>,"method":"session/update","params":{"sessionId","update":{"sessionUpdate":"user_message_chunk"|"agent_thought_chunk"|"agent_message_chunk"|"tool_call"|"tool_call_update"|"plan",…},"_meta":{…}}}`
with `_meta` carrying `eventId`, `agentTimestampMs`, `promptId`, `streamStartMs`,
`turnStartMs`, `updateType`, `chunkId`, `totalTokens`. The turn terminator is an
**x.ai extension** notification:
`{"method":"_x.ai/session/update","params":{"update":{"sessionUpdate":"turn_completed","prompt_id","stop_reason":"end_turn","usage":{inputTokens,outputTokens,totalTokens,cachedReadTokens,cacheCreationTokens,reasoningTokens,modelCalls,apiDurationMs,costUsdTicks,modelUsage}}}}`.

`prompt_history.jsonl` (group level, one per cwd) is a nice cheap index:
`{"timestamp","session_id","prompt","is_bash"}` — every prompt ever typed in that
directory, with its session id.

`signals.json` is a rich per-session metrics blob (~70 fields), including
`contextTokensUsed` / `contextWindowTokens` (4229 / **500000**), `turnCount`,
`toolCallCount`, `toolsUsed[]`, `compactionCount`, `cancellationCount`,
`agentLinesAdded`/`Removed`, `humanLinesAdded`/`Removed`, `agentFilesTouched`,
`gitCommitCount`, `prCreatedCount`/`prMergedCount`, `avgTimeToFirstTokenMs`,
`sessionDurationSeconds`, `peakRssBytes`, `inferenceIdleTimeoutConfiguredSecs` (3600).

### Reading transcripts out

- `grok sessions list [-n N]` — table: SESSION ID / CREATED / UPDATED / STATUS / SUMMARY,
  grouped by worktree label, **scoped to the current directory**. Verified.
- `grok sessions search <QUERY> [-n N]` — "combines a local SQLite index with remote results".
- `grok export <SESSION_ID> [OUTPUT]` (`-c` to clipboard) — clean Markdown. Verified:

  ```
  ## User

  reply with the single word OK

  ## Assistant

  OK
  ```
- `grok trace <SESSION_ID> [--local] [-o PATH] [--json]` — tar.gz trace bundle, default
  `$GROK_HOME/trace-exports/<session-id>.tar.gz`. **`--local` skips remote upload** —
  always pass it.
- ACP `session/list` over `grok agent stdio` — returns `sessionId`, `cwd`, `title`,
  `updatedAt`, plus `_meta["x.ai/session"].kind` (`"build"`) and facet counts. Verified.

### Titles
Auto-generated from the conversation, starting right after the first prompt, regenerated
at a couple of early turns then frozen. `/rename` sets a manual title and permanently
disables regeneration (`title_is_manual`); `/rename --auto` hands it back. Observed
generated titles: `"Reply with single word OK"`, `"Count 1-40 with short sentences"`,
`"User requests single-word OK reply"`.

### Compaction visibility
- Manual `/compact [context]`; the optional argument steers what to preserve.
- Auto-compact fires at `auto_compact_threshold_percent` (**default 85** % of the context
  window; `05-configuration.md:96`). `two_pass_compaction = false` is an opt-in variant.
- Hooks: **`PreCompact`** and **`PostCompact`**, whose `matcher` tests the trigger —
  `manual` or `auto`. This is a clean, first-class compaction signal.
- On disk: `compaction_checkpoints/` per session (absent in my short probes), and
  `signals.json.compactionCount` / `totalTokensBeforeCompaction`.
- Env knobs: `GROK_COMPACTION_MODE`, `GROK_COMPACTION_DETAIL`.
- Retention: sessions older than **30 days** are swept at startup —
  `SESSION_CLEANUP_START: … ttl_days=30` in the debug log.

### How verified
Directory walks, `python3 -m json.tool` on every state file, JSONL type surveys,
`grok sessions list`, `grok export`, ACP `session/list`, debug-log lines.

### Unverified
- The `.cwd` fallback file for >255-byte encoded paths (not triggered).
- `compaction_checkpoints/` contents and `PreCompact`/`PostCompact` payload fields beyond
  the trigger matcher. Verify: run a session past 85 % context, or `/compact` with a
  `PostCompact` hook registered.
- Whether `grok sessions search`'s remote half leaves the machine (it says "remote results").

### Recommendation for taurhaus
Index grok sessions by scanning `~/.grok/sessions/*/*/summary.json` — it is small, plain
JSON, carries cwd + model + effort + timestamps + counts, and needs no subprocess. Map to
projects via `info.cwd` through the existing normalization in
`provider/platform_paths.rs` (percent-decode the group dir, but fall back to `info.cwd`,
which is always authoritative). For content, prefer `grok export <id>` over parsing
`updates.jsonl` — it is a supported interface and already Markdown. Hook `PreCompact`/
`PostCompact` into the existing compaction pipeline; grok's `manual|auto` matcher is
strictly better signal than the Codex transcript tailing taurhaus does today.
**Never call `grok trace` without `--local`.**

---

## 6. HOOKS / NOTIFY / AGENTS / SKILLS / MCP

### Hooks — and the taurhaus-specific finding

**grok is already running taurhaus's Claude hooks on this machine.** `grok inspect --json`
in a neutral scratch directory reported:

```json
"hooks": [
  {"event":"session_start","hookType":"command",
   "target":"bash '~/.claude/hooks/taurhaus-session-start-compact.sh'",
   "source":{"type":"user","path":"~/.claude"},
   "matcher":"compact","vendor":"claude","compatibilityStatus":"enabled"},
  {"event":"(plugin)","hookType":"file",
   "target":"~/.claude/plugins/marketplaces/claude-plugins-official/plugins/ralph-loop/hooks/hooks.json",
   "source":{"type":"plugin","plugin_name":"ralph-loop",…}}
]
```

That is taurhaus's own `SessionStart(source=compact)` bridge
(`coordination/claude_hooks.rs`) firing inside grok. Whether that is desirable is a
design decision, not a bug — but taurhaus must know it happens.

**Hook discovery order** (all merged):

| Scope | Path | Trusted? |
|---|---|---|
| Global | `~/.grok/hooks/*.json` | always |
| Global | `~/.claude/settings.json`, `settings.local.json` | always (Claude compat) |
| Global | `~/.cursor/hooks.json` | always (Cursor compat) |
| Project | `<proj>/.grok/hooks/*.json` | **requires trust** |
| Project | `<proj>/.claude/settings.json`, `settings.local.json` | requires trust |
| Project | `<proj>/.cursor/hooks.json` | requires trust |
| Config | `~/.grok/config.toml` `[[hooks.<Event>]]` | always |
| Config | `managed_config.toml` (`$GROK_HOME`, `/etc/grok`), `requirements.toml` | always |
| Plugin | bundled in installed plugins | per-plugin |

Vendor scanning is disabled per-vendor with `[compat.<vendor>] hooks = false` (or
`GROK_CLAUDE_SKILLS_ENABLED` / `GROK_CLAUDE_MCPS_ENABLED` / `GROK_CURSOR_*` for the other
component types). Project trust is one unified gate for **hooks + MCP + LSP**, recorded in
`~/.grok/trusted_folders.toml`, granted by `/hooks-trust` or `--trust`, cascading to
subdirectories; `GROK_FOLDER_TRUST=0` or `[folder_trust] enabled = false` ungates everything.

**Events**: `SessionStart` (not for a subagent's own session), `UserPromptSubmit`,
`PreToolUse` (**can deny**), `PostToolUse`, `PostToolUseFailure`, `PermissionDenied`,
`Stop` (**can block**), `StopFailure`, `StopCancelled`, `Notification`, `SubagentStart`,
`SubagentStop` (**can block**; alias `SubagentEnd`), `PreCompact`, `PostCompact`,
`SessionEnd` (carries `subagentType` for a child).

**Format** — JSON file or the structurally identical TOML in config:

```json
{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[
  {"type":"command","command":"bin/safety-check.sh","timeout":10}]}]}}
```
```toml
[[hooks.PreToolUse]]
matcher = "Bash|Write|Edit"
hooks = [{ type = "command", command = "/opt/guard/pretooluse.sh", timeout = 10 }]
```

Handler fields: `type` (`command` | `http`), `command` | `url`, `timeout`, `env`.
HTTP hooks POST the full envelope as JSON.

**Stdin envelope is camelCase** (Claude uses snake_case) — the single most important
porting detail. Common fields on every event: `hookEventName`, `sessionId`, `cwd`,
`workspaceRoot`, `timestamp`, `permissionMode`, `promptId`. Tool events add `toolName`,
`toolInput`, `toolUseId`, `toolInputTruncated`; `PostToolUse` output is **`toolResult`**
(Claude's `tool_response`).

**Exit codes**: `0` allow · `2` deny (`PreToolUse`) or block-stop with stderr as feedback
(`Stop`/`SubagentStop`) · anything else fails **open**, recorded but non-blocking.

**Timeouts**: 5 s default; **600 s** for `Stop`/`SubagentStop` gates.

**Runner-injected env** (reserved — user values are stripped with a warning):
`GROK_HOOK_EVENT` (snake_case value, e.g. `pre_tool_use`), `GROK_HOOK_NAME`,
`GROK_SESSION_ID`, `GROK_WORKSPACE_ROOT`, and **`CLAUDE_PROJECT_DIR`** (a Claude-compat
alias for the workspace root, set on *every* hook). Plugin hooks additionally get
`GROK_PLUGIN_ROOT`, `GROK_PLUGIN_DATA`.

**Tool-name aliases in matchers** — Claude names are mapped: `Bash`→`run_terminal_command`,
`Read`→`read_file`, `Edit`/`Write`/`MultiEdit`→`search_replace`, `Grep`→`grep`,
`Glob`/`ListDir`→`list_dir`, `WebSearch`→`web_search`, `Task`→`spawn_subagent`. A matcher
keeps its original name too. MCP calls match the qualified `server__tool` name.

**Claude-compat gaps grok documents explicitly**: `UserPromptSubmit` is observe-only in
grok (exit code and stdout ignored) — an imported prompt-validation hook **silently stops
blocking**; use `PreToolUse` instead. `permission_mode` values are `default|auto|plan|
bypassPermissions` (no `acceptEdits`/`dontAsk` equivalent). `StopCancelled` is grok-only.

**TUI management**: `/hooks` modal (r reload, a add, x remove, Space enable/disable,
f filter), plus `/hooks-list`, `/hooks-trust`, `/hooks-add <path>`, `/hooks-remove <path>`,
`/hooks-untrust`.

### Notify / statusline
There is **no statusline hook**. The attention channel is the `Notification` event with
`matcher` on the notification type: `idle_prompt`, `permission_prompt`, `task_complete`, ….
The doc warns the `message` field is display text that changes between releases — match on
`notificationType`.

### Agents / subagents
- **Agent definition** = `.md` with YAML frontmatter, discovered from `.grok/agents/`
  (project), `~/.grok/agents/` (user), and built-ins. Verified frontmatter keys from
  `bundled/agents/explore.md`: `name`, `description`, `prompt_mode` (`full`),
  `permission_mode` (`plan`), `agents_md` (bool). Bodies support template expansion:
  `${{ tools.by_kind.execute }}`, `${{ tools.by_kind.search }}`, etc.
- Priority: `--agent-profile <PATH>` > `[agent]` in config.toml > `GROK_AGENT` env >
  default `grok-build`. `--agent <NAME|file>` is the top-level equivalent.
- **`--agents <JSON>` is a MAP, not an array.** Verified by error probing:
  `'[{"name":"x"}]'` → `invalid JSON: invalid type: sequence, expected a map at line 1 column 0`;
  `'{"bogus":1}'` → `failed to parse agent definition: invalid type: integer 1, expected struct AgentDefinition`.
  A map of name → AgentDefinition with `{"description":…,"prompt":…}` was accepted and ran
  successfully.
- Built-in subagent types for `spawn_subagent`: `general-purpose`, `explore` (read-only),
  `plan` (read-only).
- **Personas** = `.toml` behavioral overlays for subagents, from `.grok/personas/*.toml`,
  `~/.grok/personas/*.toml`, bundled (lowest), or inline `[subagents.personas.<name>]`.
  Fields: `instructions`, `instructions_file`, `description`, `inputs`/`outputs`
  (declared I/O contract with `name`/`io_type`/`required`/`description`), `model`,
  `reasoning_effort`, `default_isolation` (`none`|`worktree`).
- **Roles** = `.toml` capability/model defaults, from `.grok/roles/*.toml` or
  `[subagents.roles.<name>]`. Verified `bundled/roles/reviewer.toml` fields:
  `description`, `default_capability_mode` (`all`), `reasoning_effort`,
  `default_fork_context`. Also `model`, `prompt_file`.
- Missing persona ⇒ **spawn fails (fail-closed)**.
- Disable: `GROK_SUBAGENTS=0`, `[subagents] enabled = false`, or `--no-subagents`.
  Depth cap: `GROK_SUBAGENTS_MAX_DEPTH`.

### Skills / slash commands
- `SKILL.md` + YAML frontmatter. Verified keys from `bundled/skills/code-review/SKILL.md`:
  `name`, `description`, `disable-model-invocation`. Docs add `user-invocable`.
- Locations: `.grok/skills/`, `~/.grok/skills/`, plugins, bundled, **plus `~/.claude/skills/`**.
- Any enabled skill with `user-invocable: true` becomes `/<name>`; qualify collisions as
  `/local:<name>`, `/user:<name>`, `/<plugin>:<name>`. Built-ins always win the bare name.
- **Verified live**: the ACP init on this machine advertised
  `ralph-loop`, `cancel-ralph`, `ralph-loop:help` as slash commands — grok picked up the
  user's Claude `ralph-loop` plugin.
- ~57 built-in slash commands; the ones that matter here: `/context`, `/session-info`,
  `/usage` (alias `/cost`), `/compact`, `/effort`, `/model`, `/always-approve`, `/auto`,
  `/quit` (alias `/exit`), `/export`, `/resume`, `/new`, `/fork`, `/rename`, `/dashboard`,
  `/loop`, `/goal`, `/hooks`, `/mcps`, `/skills`, `/plugins`, `/doctor`, `/import-claude`.

### MCP
- `grok mcp add|remove|enable|disable|list|doctor`. `list --json` → `[]` here.
- Transports: `stdio`, `http`, `sse`. Scopes: `user` (`~/.grok/config.toml`) or
  `project` (`./.grok/config.toml`).
- `grok mcp add <NAME> -t http --header "…" -e KEY=val -- <cmd> <args>`
- **Claude-compat sources**: `~/.claude.json` and `.mcp.json` are read automatically.
- Logs at `~/.grok/logs/mcp/`. Knobs: `GROK_MAX_MCP_OUTPUT_BYTES`,
  `GROK_MCP_STARTUP_TIMEOUT_SECS`.
- Plugins/marketplaces: `grok plugin …`, `grok plugin marketplace …`; official source is
  `https://github.com/xai-org/plugin-marketplace.git`, cached in `~/.grok/marketplace-cache/`.

### How verified
Full read of `docs/user-guide/10-hooks.md` (43 KB), `15-agent-mode.md`, `16-subagents.md`;
`grok inspect --json`; `grok mcp list --json`; `cat` of bundled agent/role/skill files;
error-probing `--agents`; the ACP `initialize` and `system/init` payloads.

### Unverified
- `PreCompact`/`PostCompact` payload fields beyond the trigger matcher.
- Whether taurhaus's `SessionStart(compact)` script behaves correctly when invoked by grok
  (it will receive grok's camelCase envelope, not Claude's snake_case). **This is a live
  risk worth checking.**

### Recommendation for taurhaus
Two decisions to make deliberately:

1. **Decide whether grok should inherit `~/.claude/settings.json` hooks.** Today it does.
   If taurhaus's hooks assume Claude's snake_case stdin (`.hook_event_name`,
   `.session_id`), they will misparse grok's camelCase envelope and fail silently
   (hooks fail open). Either make the scripts accept both key styles, or set
   `[compat.claude] hooks = false` in the grok config used by mesh members.
2. **Prefer grok-native hook files at `~/.grok/hooks/*.json`** for anything grok-specific.
   They are always trusted, need no folder-trust grant, and keep the two runtimes' hook
   sets from drifting into each other.

The role/persona/agent model maps almost 1:1 onto taurhaus's role-template schema —
`focus_area`→`description`, `behavior_summary`→persona `instructions`,
`quality_gates`/`definition_of_done`→persona `inputs`/`outputs` contract,
`mode`→`default_capability_mode`, and `reasoning_effort` is a first-class field on both
roles and personas. Emitting `.grok/roles/*.toml` + `.grok/personas/*.toml` from
`templates/adapters.rs` is a small adapter, not a new subsystem.

---

## 7. DELIVERY — injecting into a running session

### Facts, ranked by how well they work

**A. tmux keystrokes — works, and is what taurhaus already does.** Verified end to end:
`tmux send-keys -t <target> "<text>"` then a separate `tmux send-keys -t <target> Enter`
delivered a prompt to a live TUI, which ran a turn and replied. The two-call split (text,
then Enter) matters — grok's input box handles them as distinct events.

Delivery *semantics* are documented and are richer than plain keystrokes suggest
(`03-keyboard-shortcuts.md:277`): plain `Enter` **queues** a message that the agent picks
up at the next turn boundary without stopping it, while the interject chord
(`Ctrl+Enter`/`Ctrl+I`, or `Ctrl+L` on VS Code-family terminals) is "send-now" and
"intentionally interruptive — it reads as *stop what you're doing and take this*".
That is a genuinely useful two-tier delivery API for a coordinator.

**B. ACP over stdio — the clean programmatic path.** `grok agent --no-leader stdio`
speaks JSON-RPC 2.0 on stdin/stdout. Verified with a real handshake: a single
`initialize` line returned a complete capability document (reproduced in slice 2/3),
advertising `sessionCapabilities: {list:{}, resume:{}, close:{}}` and `loadSession: true`.
Flow: `initialize` → `session/new` (or `session/load` with an existing `sessionId`) →
`session/prompt` → stream `session/update` notifications.
`session/new._meta` accepts `rules`, `systemPromptOverride`, `agentProfile`,
`yoloMode`, `autoMode`.

**C. Shared leader — attach a second client to one backend.** Verified: with a leader
running, `grok agent --leader stdio` + `session/list` returned every session for the cwd:

```json
{"sessions":[
 {"sessionId":"<uuid>","cwd":"/tmp/…/grok-probe",
  "title":"Reply with single word OK","updatedAt":"2026-08-27T23:22:09.932455324+00:00",
  "_meta":{"x.ai/session":{"kind":"build","facets":{"cwd":"…","kind":"build"}}}},
 …],
 "_meta":{"x.ai/facets":{…},"x.ai/partial":{"conversations":false}}}
```

The leader's whole purpose, per `grok agent --help`, is that it "allows multiple clients
to share one backend". So with `[cli] use_leader = true`, a TUI and an external ACP client
can both be attached, and the external client can `session/load` the same session.

**D. WebSocket server** — `grok agent serve --bind 127.0.0.1:2419 --secret <TOKEN>`
(`GROK_AGENT_SECRET`). Same ACP protocol over WS with token auth. Not probed.

**E. There is no `grok send`-like subcommand.** I enumerated all 20 subcommands; nothing
posts a message to a running session.

**F. Headless stdin is not a delivery channel** — headless does not read piped stdin into
the prompt (slice 2).

### How verified
Live `tmux send-keys` round trip with pane capture; ACP `initialize` over
`grok agent --no-leader stdio`; ACP `session/list` over `grok agent --leader stdio`
against a real leader; full subcommand enumeration.

### Unverified
- **`session/load` + `session/prompt` into a session that a live TUI currently owns.**
  This is the decisive question for a non-tmux delivery path and I could not test it,
  because enabling it requires `[cli] use_leader = true` in `~/.grok/config.toml`, which
  is out of scope for this probe. Verify with an isolated `GROK_HOME` containing
  `[cli] use_leader = true`: start a TUI, then from a second process
  `grok agent --leader stdio` → `session/load` that sessionId → `session/prompt`, and
  watch whether the TUI renders the injected turn.
- Whether concurrent prompts to one session are serialized or rejected.

### Recommendation for taurhaus
**Keep tmux `send-keys` as the delivery mechanism** — it is verified, it needs no config
change, and it is the same code path taurhaus already runs for Claude and Codex. Add one
grok-specific refinement: use plain `Enter` for *queued* handoffs (the message lands at
the next turn boundary without interrupting) and reserve the interject chord for genuine
"stop and take this" escalations. That maps neatly onto taurhaus's `INFO ONLY:` vs
`ACTION REQUIRED:` prefix convention — `INFO ONLY:` should queue, `ACTION REQUIRED:` may
interject.

Treat the leader as a **future** optimization, not a v1 dependency. If you pursue it,
resolve the `session/load`-into-live-TUI question first; if that works, grok becomes the
only one of the three CLIs where taurhaus can inject structured messages without
synthesizing keystrokes, which is worth a follow-up spike.

---

## 8. STOP

### Facts (all verified live except where noted)

| Gesture | Effect |
|---|---|
| `/quit` (alias `/exit`) | **Graceful.** Verified: process exited, tmux session ended, `active_sessions.json` returned to `[]`. |
| `Ctrl+Q` | Quit the application. **Double-press within 1000 ms** to confirm. |
| `Ctrl+D` | Quit on VS Code-family terminals, where `Ctrl+Q` is captured by the host. Double-press to confirm. Elsewhere `Ctrl+D` is *scroll down half page*. |
| `Ctrl+C` (turn running, empty prompt) | **Cancel the turn.** Verified: pane showed `Turn cancelled by user in 4.0s.` and the pane title dropped its spinner. |
| `Ctrl+C` (turn running, non-empty draft) | Clears the draft first; a **second** `Ctrl+C` cancels. |
| `Ctrl+C` (idle, non-empty draft) | Clears the draft in one press. |
| `Esc` (turn running) | Cancels immediately and **preserves the draft**. No-op in fullscreen vim mode. |
| `Esc` (while cancelling) | Re-sends cancel in every mode (retry if the first ack was lost). |
| `Ctrl+N` | New session. Double-press to confirm. |
| `/home` (alias `/welcome`) | Leave the session, keep the process. |
| SIGINT to a headless run | Exit **130**; state saved to the last completed tool call; file modifications **not** rolled back. |
| SIGTERM | Exit **143**. |
| SIGKILL | **Leaves a stale `active_sessions.json` entry** (verified) — pruned on the next grok run. |

Teardown budget (from `10-hooks.md`): queued turn-end hooks get **half a second**, then
the remainder are dropped and any still running are aborted; `SessionEnd` hooks then run
inside a **ten-second** session exit budget. `GROK_EXIT_TIMEOUT_SECS` tunes this.
`SessionEnd` fires with an end reason the matcher can test; a session-end `Stop` also
fires with `reason` `"channel_closed"` or `"shutdown"` (its decision output is parsed but
ignored).

Leader shutdown: `grok leader kill` stops **all** leaders ("Killed 1 leader process(es)").
**It leaves `leader.sock` and `leader.lock` behind** — verified: a post-kill
`grok leader list --json` reported `"pidLive":null,"classification":"Unreachable"` with
both paths still on disk.

### How verified
Live tmux session: long prompt → `Ctrl+C` → pane capture → `/quit` → registry check;
`kill -9` stale test; `grok leader kill` + `leader list --json`; keyboard-shortcuts and
hooks docs for the semantics I did not exercise.

### Unverified
- Ctrl+Q / Ctrl+D double-press timing (documented, not exercised).
- Whether a `SIGTERM` to an interactive TUI cleans `active_sessions.json` (only SIGKILL
  and `/quit` were tested). Verify: `kill -TERM` a registered TUI and re-read the file.

### Recommendation for taurhaus
Graceful stop = send `/quit` + `Enter` via tmux, then wait up to ~11 s (0.5 s hook drain +
10 s exit budget) before escalating to SIGTERM and then SIGKILL. Confirm the stop by
polling `active_sessions.json` for the row's disappearance rather than by watching the
pane. For mid-turn cancellation prefer `Esc` over `Ctrl+C` — it cancels in one press
regardless of draft state and preserves whatever the user had typed. Treat
`classification: "Unreachable"` from `grok leader list --json` as "stale files, safe to
ignore", not as an error.

---

## 9. USAGE / QUOTA + ACCOUNTS

### Facts

**Per-turn cost and tokens are reported in-band** — this is grok's standout feature versus
the other two CLIs. Every headless `--output-format json` response carries:

```json
{"text":"…","stopReason":"end_turn","sessionId":"…","requestId":"…","thought":"…",
 "usage":{"input_tokens":14415,"cache_read_input_tokens":0,"cache_creation_input_tokens":0,
          "output_tokens":743,"reasoning_tokens":65,"total_tokens":15158},
 "num_turns":1,"total_cost_usd":0.00565896,"total_cost_usd_ticks":56589600,
 "modelUsage":{"grok-4.6-build":{"inputTokens":14415,"outputTokens":743,
   "cacheReadInputTokens":0,"cacheCreationInputTokens":0,"modelCalls":1,"costUSD":0.00565896}}}
```

The `streaming-messages-json` `result` line carries the same plus `duration_ms`,
`duration_api_ms`, and `usage.server_tool_use.web_search_requests`. `costUsdTicks` is
integer micro-dollars ×10⁴ (34,557,600 ticks = $0.00345576) — use it to avoid float drift.
On disk, `updates.jsonl`'s `turn_completed` carries the same usage block with
`apiDurationMs` and `reasoningTokens`.

**Context usage** lives in `signals.json`: `contextTokensUsed` / `contextWindowTokens`
(500,000 for both models) and `contextWindowUsage`. Slash commands `/context` ("Show
context window usage and session stats") and `/session-info` ("Context window usage (used
and total tokens, with the percentage used)") surface it interactively.

**Credits / quota**: the slash command is **`/usage`** (alias **`/cost`**), documented as
"View credit usage or manage billing", with `/usage manage` opening billing. Corroborated
in the logs — `unified.jsonl` recorded `billing: fetched credits config` twice during a
single interactive session (once at session create, once after the turn). So grok does
poll a credits endpoint. There is **no `grok usage` subcommand** and no `--json` route to
it that I could find.

**Plan / subscription**: `grok models` prints `You are logged in with grok.com.` — a
consumer/SuperGrok-style login rather than a console.x.ai API key. `auth.json` carries a
`team_id`, so this is a **team account**; `22-permissions-and-safety.md` and
`04-slash-commands.md` note that on team accounts only an admin can change the privacy
setting, and admins can toggle Zero Data Retention. The binary contains upsell strings
`https://grok.com/supergrok?referrer=grok-build` and a usage deep link
`https://grok.com/?_s=usage`.

**Endpoints found in the binary** (verified via `strings`):

| URL | Role |
|---|---|
| `https://cli-chat-proxy.grok.com/v1` (+ `/chat/completions`) | default inference proxy (`GROK_CLI_CHAT_PROXY_BASE_URL`) |
| `wss://code.grok.com/ws/code-agent` | the relay WebSocket (`--grok-ws-url`) |
| `https://api.x.ai` / `https://api.x.ai/v1` | public xAI API (`--xai-api-base-url`) |
| `https://auth.x.ai`, `https://accounts.x.ai/sign-in` | OAuth / OIDC |
| `https://console.x.ai` | API key issuance |
| `https://grok.com`, `https://grok.com/?_s=usage`, `https://grok.com/supergrok?referrer=grok-build` | account / usage / upgrade |
| `https://docs.x.ai/build/overview`, `https://docs.x.ai/developers/rate-limits#rate-limit-tiers`, `https://docs.x.ai/developers/faq/security#how-to-enable-zdr`, `https://docs.x.ai/build/settings/zdr-video-storage` | docs |
| `https://x.ai/cli/install.sh`, `install.ps1`, `enterprise-install.sh`, `https://x.ai/cli/changelogs` | installers / changelog |
| `https://storage.googleapis.com/grok-build-public-artifacts/cli` | release artifacts |
| `https://github.com/xai-org/plugin-marketplace.git` | official marketplace |

**Rate limits**: grok classifies API errors into six types for the `StopFailure` hook
matcher — `rate_limit`, `authentication_failed`, `invalid_request`, `server_error`,
`max_output_tokens`, `unknown` — and **capacity errors (503/529) classify as
`rate_limit`**. That is a first-class, hookable rate-limit signal.

**Telemetry / OTEL**: `24-monitoring-usage.md` documents an external OpenTelemetry stream
under meter scope `ai.xai.grok_code`, enabled with `GROK_EXTERNAL_OTEL`, emitting metrics
(tokens by model and type, sessions per team per day, tool-permission denial ratio) and
OTLP log records. Config knobs `GROK_OTEL_FILTER`, `GROK_TELEMETRY_*`, plus PEM *paths*
for private-CA/mTLS. Also `[features] telemetry` and `trace_upload`.

**Privacy**: `auth.json.coding_data_retention_opt_out = true` on this machine —
already opted out of coding-data retention. `/privacy` manages it.

**Multi-account**: `auth.json` is a **map keyed by `<issuer>::<client_id>`**, which
structurally holds multiple credentials, and `11-custom-models.md` documents **per-model
auth providers**. `grok login` / `grok logout` have no `--account` or profile flag.

### How verified
`--output-format json` and `streaming-messages-json` captures; `updates.jsonl`
`turn_completed`; `signals.json`; `unified.jsonl` billing lines; `grok models`;
`strings` URL extraction; the slash-command, permissions, and monitoring docs; the
`auth.json` key-name walk.

### Unverified
- What `/usage` actually renders (credits remaining? a reset window? a quota bar?). It is
  TUI-only; I did not drive it. Verify: `tmux send-keys "/usage" Enter` then
  `capture-pane`.
- Whether any machine-readable quota endpoint exists. `billing: fetched credits config`
  proves one is called; its `ctx` payload would name it — capture with
  `--debug-file` during startup and grep for `billing`.
- Whether `GROK_HOME` isolation yields true multi-account support. Verify:
  `GROK_HOME=/tmp/acctB grok login` and check the two homes stay independent.

### Recommendation for taurhaus
Harvest cost and token usage from the `result` line / `turn_completed` record — grok is
the **only** one of the three CLIs handing taurhaus a per-turn `total_cost_usd` for free,
and `total_cost_usd_ticks` is exact integer arithmetic. Surface it per mesh member and
roll it up per project; store ticks, format dollars.

For quota, do not scrape `/usage`. Register a `StopFailure` hook with
`matcher: "rate_limit"` — that is a precise, structured "this member is throttled" signal
that taurhaus can render on the mesh canvas and use to pause dispatch. That is strictly
better than the log-scraping approach used elsewhere.

For multi-account, `GROK_HOME` per account is the only mechanism I would rely on today.

---

## 10. VERSIONING

### Facts

- `grok --version` → `grok 1.0.5 (5115b46bc9) [stable]`
- **`grok version --json` → `{"currentVersion":"1.0.5 (5115b46bc9)","channel":"stable"}`** —
  the programmatic read. Note `currentVersion` embeds the commit hash in parentheses;
  parse on the leading semver token.
- **`grok update --check --json`** →
  `{"currentVersion":"1.0.5","latestVersion":"1.0.5","updateAvailable":false,
    "installer":"internal","channel":"stable","autoUpdate":null,"error":null}`
  Here `currentVersion` is the bare semver. Cheap, structured, no side effects.
- `~/.grok/version.json` → `{"version":"1.0.5","stable_version":"1.0.5",
  "checked_at":"2026-08-27T23:12:14.018846896Z"}` — a pure file read, zero subprocess.
- `~/.grok/.metadata_version` → `1.0.5`.
- `grok update` subcommand flags: `--check`, `--json`, `--force-reinstall`,
  `--version <VERSION>` (e.g. `0.1.150` or `0.1.151-alpha.2`), `--alpha`, `--stable`.
- **Channels**: `stable` (default, *weekly* releases) and `alpha` (faster, may have bugs).
  `GROK_CHANNEL` exists in the binary.
- **Auto-update suppression**, four ways: `--no-auto-update` (session),
  `GROK_DISABLE_AUTOUPDATER=1` (process; falsy values `0|false|off|no|empty` count as
  unset), non-TTY stderr (automatic), `[cli] auto_update = false` (persistent).
  Update messages go to **stderr** so stdout stays clean for `--output-format json`.
  The leader has its own `--no-auto-update`.
- **Installer**: `[cli] installer = "internal"` here, i.e. grok manages its own binary in
  `~/.grok/downloads/` and repoints `~/.grok/bin/grok`. Artifacts come from
  `https://storage.googleapis.com/grok-build-public-artifacts/cli`.
- Version pinning for enterprises: `GROK_MINIMUM_VERSION`, `GROK_MAXIMUM_VERSION`,
  `GROK_REQUIRED_MINIMUM_VERSION`, `GROK_REQUIRED_MAXIMUM_VERSION`.
- **Changelog**: `~/.grok/CHANGELOG.md` (1,657 B) and `~/.grok/CHANGELOG.json` (2,803 B),
  the latter structured as `[{"category":"features"|"fixes","description":"…",
  "breaking_change":false}, …]`. Online at `https://x.ai/cli/changelogs`
  (`GROK_CHANGELOG_OFFLINE` forces the local copy). Slash command `/release-notes`.

### Documentation URLs (cited)

- Overview / install: <https://docs.x.ai/build/overview>
- Headless & scripting: <https://docs.x.ai/build/cli/headless-scripting>
- Modes & commands: <https://docs.x.ai/build/modes-and-commands>
- Skills / plugins / marketplaces: <https://docs.x.ai/build/features/skills-plugins-marketplaces>
- Permissions: <https://docs.x.ai/build/features/permissions>
- Plan mode: <https://docs.x.ai/build/features/plan-mode>
- Background tasks: <https://docs.x.ai/build/features/background-tasks>
- Dashboard: <https://docs.x.ai/build/features/dashboard>
- Keyboard shortcuts: <https://docs.x.ai/build/keyboard-shortcuts>
- Enterprise: <https://docs.x.ai/build/enterprise>
- Rate limit tiers: <https://docs.x.ai/developers/rate-limits#rate-limit-tiers>
- Announcement: <https://x.ai/news/grok-build-cli>
- Source/repo: <https://github.com/xai-org/grok-build>
- Changelogs: <https://x.ai/cli/changelogs>

**The website is behind the shipped binary.** Verified: `docs.x.ai/build/cli/headless-scripting`
documents `-s, --session-id` as "Create **or resume** a named headless session" and lists
only three `--output-format` values. The 1.0.5 binary's own `--help` and bundled
`docs/user-guide/14-headless-mode.md` say `-s` **creates only** ("Older hidden `-s`
upsert/resume behavior is gone") and list **four** formats including
`streaming-messages-json`.

### How verified
`grok --version`, `grok version --json`, `grok update --check --json`, `grok update --help`,
`cat version.json` / `.metadata_version` / `CHANGELOG.json`, `strings` for channel and
artifact URLs, WebSearch + WebFetch of the official docs.

### Unverified
- Whether `grok update --alpha` rewrites `[cli]` in `config.toml` (not run — it would
  modify config and possibly replace the binary).
- Actual release cadence beyond the docs' "weekly".

### Recommendation for taurhaus
Read `~/.grok/version.json` for the version — it is a plain file, needs no subprocess, and
already carries `stable_version` and `checked_at`. Fall back to `grok version --json` when
the file is missing, and parse only the leading semver of `currentVersion`. **Pin the
launch flags to `--no-auto-update`** (or set `GROK_DISABLE_AUTOUPDATER=1` in the mesh
member environment) so a background self-update never swaps the binary underneath a
running mesh. Because the website lags the binary, generate any taurhaus-side capability
matrix from `grok --help` and `~/.grok/docs/`, not from the web docs — and gate on
`1.0.5+` for the `-s`-creates-only semantics, since the older upsert behavior would
silently change what `--session-id` does.

---

## Artifacts produced

| Path | Contents |
|---|---|
| `/tmp/claude-1000/…/scratchpad/grok-stream-sample.jsonl` | 31 lines / 9,901 B — full `streaming-messages-json --include-partial-messages` capture |
| `/tmp/claude-1000/…/scratchpad/grok-debug-sample.log` | 269 lines / 62,019 B — full `--debug-file` capture |
| `/tmp/claude-1000/…/scratchpad/grok-long-probe.json` | `--output-format json` envelope with cost/usage |
| `/tmp/claude-1000/…/scratchpad/acp-out.jsonl` | ACP `initialize` response (`--no-leader stdio`) |
| `/tmp/claude-1000/…/scratchpad/acp-leader-out.jsonl` | ACP `session/list` response through the leader |
| `/tmp/claude-1000/…/scratchpad/leader-protocol-probe.txt` | raw leader-socket framing probe (negative result) |
| `/tmp/claude-1000/…/scratchpad/grok-probe/` | scratch project dir used for every live run |

## Safety compliance

- taurhaus repo: **read-only**; nothing read from it, nothing written, no git commands.
- `~/.grok`, `~/.claude*`, `~/.codex`: **no config modified**. The only writes were grok's
  own (session dirs under the scratch cwd, `logs/unified.jsonl`, `active_sessions.json`,
  and a leader socket/lock created by `grok agent leader` and removed-by-`grok leader kill`
  except for the two stale files grok itself leaves behind).
- Every live run used the scratch dir `…/scratchpad/grok-probe` with the harmless prompt
  *"reply with the single word OK"* (plus one benign counting prompt).
- **`--always-approve` was never used**, anywhere.
- No sign-in flow was started or completed; the machine was already authenticated.
- No tokens or secrets reproduced — `auth.json` was walked for **key names and value
  types/lengths only**.
- Stale registry entry created by the deliberate `kill -9` test was verified to
  self-prune on the next run; `active_sessions.json` was left `[]`.
