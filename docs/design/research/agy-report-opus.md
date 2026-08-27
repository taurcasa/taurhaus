# Antigravity CLI (`agy`) — Capability Report for taurhaus

**Binary:** `~/.local/bin/agy` · **Version:** 1.1.22 · **Probed:** 2026-08-28
**Machine:** WSL2 (Linux 6.6.87.2-microsoft-standard-WSL2), signed in as a personal Google account (Google AI Pro tier).

All probes ran in `/tmp/claude-1000/-home-mstie-projects-taurhaus/f3286b16-ffc7-4d16-915d-046705823a3d/scratchpad/agy-probe`.
The taurhaus repo was never written to; no agy/antigravity/claude/codex config was modified (verified by mtime + content compare at the end).
`agy install` and `agy update` were never run. No login flow was completed.

**Headline for taurhaus:** `agy` is far more coordinator-friendly than the old Gemini CLI. It has (a) a real
NDJSON event stream in *both* directions (`--input-format stream-json` keeps one conversation open across turns
— a genuine non-tmux delivery channel), (b) a first-party lifecycle hook system whose `Stop` event carries
`fullyIdle`, (c) an flock-based `presence/` registry that marks live sessions, and (d) tool subprocesses that
inherit `ANTIGRAVITY_CONVERSATION_ID` + a local agent-API address. The one significant trap is **workspace
trust**: an untrusted workspace silently disables workspace customizations *and* relocates the agent's cwd.

---

## 1. PROCESS SIGNATURE

### Facts

**Single static-ish executable, no helper processes.** 199 MB (`208,429,312` bytes),
`ELF 64-bit LSB pie executable, x86-64, dynamically linked, stripped`. Go binary: Go symbol names are retained
in the string table under `google3/third_party/jetski/...` (internal codename **jetski**; also `cortex` for the
agent loop, `codeium`/`cascade`/`windsurf` proto lineage).

`ps --ppid <agy_pid>` during a live print-mode run returned **no children**. The "language server" that the log
announces (`server.go:1487 Starting language server process with pid …`) is in-process goroutines, not a fork —
it binds two localhost ports (see §4/§7). The only bundled sidecar binary on disk is
`~/.gemini/antigravity-cli/bin/webm_encoder` (17 MB, for `/voice` and screen recording); it is not spawned for
normal turns. `ripgrep` is embedded in the binary (changelog 1.1.21) rather than shelled out.

**argv shapes to discriminate modes:**

| Mode | argv signature | Notes |
|---|---|---|
| Interactive TUI | `agy` with **no** `--print`/`-p` and no subcommand (flags allowed: `--model`, `--sandbox`, `--log-file`, `-i`, …) | verified: `agy --log-file <path>` and `agy --sandbox --log-file <path>` both ran as TUIs |
| Print / headless | argv contains `-p`, `--print`, or `--prompt` | `--print` is a **string** flag; a bare `--print` errors |
| Subcommand | argv[1] ∈ `agent`, `agents`, `changelog`, `help`, `install`, `mcp`, `mic-serve`, `models`, `plugin`, `plugins`, `update` | short-lived, exits on its own |

Under tmux, `#{pane_current_command}` is `agy` for an interactive session. `#{pane_title}` is **not** set by agy
(stayed at the hostname) — see §4.

### How verified
`file`, `du`, `ps -eo pid,ppid,etimes,rss,args`, `ps --ppid`, `ls -l /proc/<pid>/fd`, `strings` symbol
extraction (45,872 distinct `google3/third_party/jetski/...` symbols), `agy --help`, live tmux session.

### Unverified
- Whether `mic-serve` or `/voice` forks `webm_encoder` (not exercised — would need a microphone).
- Whether the Windows/macOS builds spawn helpers.

### Recommendation for taurhaus
Detect an interactive agy session by `comm == "agy"` **and** absence of `-p`/`--print`/`--prompt` and of a
known subcommand in argv. Treat it as a single-process runtime — no child-process tree to walk, unlike the
node-based CLIs. Do not rely on a stable install path; resolve via `PATH` or `~/.local/bin/agy`.

---

## 2. LAUNCH

### Facts — full flag surface (from `agy --help`, v1.1.22)

| Flag | Meaning |
|---|---|
| `--model <id>` | model for the session; ids from `agy models` |
| `--effort low\|medium\|high` | reasoning effort. **Some models require it**: `--model %s requires --effort (available: %s)` |
| `--agent <name>` | custom agent for the session |
| `--mode accept-edits\|plan` | execution mode |
| `--dangerously-skip-permissions` | auto-approve all tool permission requests |
| `--sandbox` | run with terminal restrictions enabled |
| `--print` / `-p` / `--prompt` | single non-interactive prompt (string-valued) |
| `--prompt-interactive` / `-i` | run an initial prompt **then stay interactive** |
| `--output-format text\|json\|stream-json` | print-mode output |
| `--input-format text\|stream-json` | print-mode stdin; requires `--output-format stream-json` |
| `--json-schema <str\|path>` | enforce structured output (final `result` only for stream-json) |
| `--print-timeout <dur>` | default `5m0s` |
| `--continue` / `-c` | continue most recent conversation **for this cwd** |
| `--conversation <id>` | resume a specific conversation by UUID |
| `--project <id\|name>` | project for the session |
| `--new-project` | create a new project for the session |
| `--add-dir <path>` (repeatable) | add a directory to the workspace |
| `--disable-slash-commands` | disable slash/skill expansion in print mode |
| `--log-file <path>` | override CLI log file path |
| `--remote-control` | **undocumented in `--help`** but accepted (see §7) |
| `--version` | prints `1.1.22` and exits |

**Flag-order trap (verified).** `--print` takes a value, so `agy -p --input-format stream-json` fails with
`-p took "--input-format" as its prompt`. For stdin-driven runs use `--print=` (empty value) with the other
flags **before** it:
```
agy --input-format stream-json --output-format stream-json --print= < msgs.ndjson
```

**Auto-approve semantics (verified).** Without `--dangerously-skip-permissions`, headless mode **soft-denies**
any tool needing confirmation and the run still exits 0 with an empty response plus this stderr notice:
> `jetski: no output produced — a tool required the "command" permission that headless mode cannot prompt for, so it was auto-denied. Add an allow-rule under permissions.allow in settings.json (e.g. command(<target>)). Alternatively, re-run with --dangerously-skip-permissions to auto-approve all tools.`

So a headless run can *silently do nothing*. There **is** a config-level equivalent: `toolPermission` in
`settings.json` (`request-review` default, `proceed-in-sandbox`, `always-proceed`, `strict`) plus a
`permissions.allow` list of CEL-ish rules such as `command(npm test)`. `always-proceed` also auto-approves MCP
calls and page reads (changelog 1.1.21). Note the binary states *"Admin escalation permissions cannot be
auto-approved and will always prompt the user."*

**Fresh vs continue vs resume (verified live).** `--continue` and `--conversation <id>` both resumed the
conversation created by an earlier *interactive* TUI session and returned its real prior content. Both report
cumulative `num_turns` and a `duration_seconds` covering the whole conversation lifetime (e.g. `num_turns: 6`,
`duration_seconds: 385.19`), not just this invocation — **do not treat `duration_seconds` as turn latency on a
resumed conversation.**

**Projects (verified).** A "project" is a named, cwd-independent grouping stored as
`~/.gemini/config/projects/<id>.json` (e.g. `default-cli-project.json` → `{"id","name","projectResources"}`),
with the default id in `~/.gemini/antigravity-cli/cache/default_project_id.txt` (`default-cli-project`). A
separate, older map `~/.gemini/projects.json` records `{absolute_path: project_name}` (currently the user's
`taurhaus` and `localllms` checkouts). `--project` accepts an id **or** a name (changelog 1.1.18). An unknown
project name did **not** error and did **not** create a file — it silently fell back to the default project.

**`agy models` (v1.1.22, this account):**
```
gemini-3.7-flash-{high,medium,low}   gemini-3.6-flash-{high,medium,low}
gemini-3.5-flash-{high,medium,low}   gemini-3.1-pro-{high,low}
claude-sonnet-4-6                    claude-opus-4-6-thinking
gpt-oss-120b-medium
```
It requires sign-in: unauthenticated it prints *"Please sign in to view available models. Launch the CLI
without arguments to sign in."* **`agy agents` returned empty** (exit 0, no output) — no custom agents defined.
`--output-format json` is **not** accepted by `models`/`agents` in 1.1.22 (`flags provided but not defined:
-output-format`) despite a changelog line advertising it — that line is for a build newer than this one.

### How verified
`agy --help`, `agy --version`, `agy models`, `agy agents`, live `-p` runs with and without
`--dangerously-skip-permissions`, `--continue` / `--conversation` round-trip against a real prior TUI
conversation, `--project` with valid and invalid names, inspection of `~/.gemini/config/projects/`.

### Unverified
- `--mode plan` / `accept-edits` behavioural differences (not exercised).
- `--sandbox` containment strength (launched but no escape test).
- `--new-project` (would have written a new project file to user config).

### Recommendation for taurhaus
Launch as: `agy --model <id> --effort <e> [--mode accept-edits] --add-dir <repo>` for interactive lanes.
For headless lanes **always pass `--dangerously-skip-permissions` or pre-seed `permissions.allow`** — otherwise
silent no-ops. Treat a `SUCCESS` result with an empty `response` plus the `headlessDenialNotice` on stderr as a
*failure* in taurhaus's own status model. Prefer `--conversation <id>` over `--continue` for determinism, since
`--continue` is keyed on cwd.

---

## 3. CONFIG + IDENTITY

### Facts — the layout

```
~/.gemini/                                   # shared Google-tooling root
├── settings.json                            # {"security":{"auth":{"selectedType":"oauth-personal"}}}
├── oauth_creds.json                         # keys: access_token, refresh_token, scope, token_type,
│                                            #       id_token, expiry_date   (values NOT read/printed)
├── google_accounts.json                     # {"active":"<email>","old":[]}   <-- signed-in identity
├── installation_id                          # 36-byte UUID
├── projects.json                            # {path: project_name}
├── trustedFolders.json                      # {path: "TRUST_FOLDER"|"DO_NOT_TRUST"}
├── config/
│   ├── config.json                          # {"userSettings":{"remoteControlHostname":"…"}}
│   ├── mcp_config.json                      # global MCP servers (currently empty)
│   ├── hooks.json                           # global hooks (not present here)
│   └── projects/<project-id>.json
└── antigravity-cli/                         # THE CLI's app data dir
    ├── settings.json                        # CLI settings incl. trustedWorkspaces[]
    ├── antigravity-oauth-token              # {"token":{…},"auth_method":"…"}  (values NOT read/printed)
    ├── installation_id, jetski_state.pbtxt  # installation_uuid, migrations, post_onboarding
    ├── cli.log -> log/cli-<YYYYMMDD_HHMMSS>.log     # symlink to CURRENT run's log
    ├── log/cli-*.log                        # one file per process launch
    ├── conversations/<uuid>.db(+ -wal,-shm) # per-conversation SQLite trajectory
    ├── conversation_summaries.db            # summaries index (see §4 caveat)
    ├── presence/<uuid>.lock                 # flock'd while a session holds the conversation
    ├── annotations/<uuid>.pbtxt             # e.g. title:"Request For Single Word"
    ├── brain/<uuid>/{scratch,.user_uploaded,.system_generated/logs/transcript.jsonl}
    ├── implicit/<uuid>.pb                   # implicit trajectory
    ├── cache/{default_project_id.txt, onboarding.json,
    │          conversation_metadata.json, last_conversations.json}
    ├── knowledge/, crashes/, updater/, builtin/, bin/webm_encoder
    └── scratch/                             # fallback cwd for UNTRUSTED workspaces (see §3 trust)
```

**Signed-in identity (verified).** `~/.gemini/google_accounts.json` → `{"active": "<user's gmail address>"}`.
The TUI banner independently prints the account and tier: `<email> (Google AI Pro)`. Auth type is in
`~/.gemini/settings.json` → `security.auth.selectedType = "oauth-personal"`. Credentials live in
`~/.gemini/oauth_creds.json` and `~/.gemini/antigravity-cli/antigravity-oauth-token`. **I read only key names
and value lengths; no token material was printed or copied.** The log notes
`composite_token_storage.go:123 Using file-based token storage because WSL environment detected` — on non-WSL
Linux it would prefer the OS keyring, so file locations are not portable.

**Machine is currently signed in — verified non-interactively** by `agy models` returning the model list and by
a full print-mode turn succeeding.

**Logs.** Default is `~/.gemini/antigravity-cli/log/cli-<timestamp>.log`, with
`~/.gemini/antigravity-cli/cli.log` repointed as a symlink to the current run. `--log-file <path>` overrides it
(verified). Format is **glog**, one line per record, each prefixed with the literal noise
`ERROR: logging before google.Init: ` then `I|W|E<MMDD> <HH:MM:SS.uuuuuu> <goroutine> <file>:<line>] <msg>`.
It is verbose and contains no secrets I observed, but does contain the account state and workspace paths.

**Per-process config-dir selection (multi-account): NOT AVAILABLE.**
- `JETSKI_APP_DATA_DIR` exists as a string in the binary but setting it had **no effect**: the run still logged
  `common.go:172 CLI app data directory: ~/.gemini/antigravity-cli` and my alternate directory stayed
  empty.
- There is a `GeminiDir` concept (`local.SetGeminiDir`, `entrypoints.resolveGeminiDirPath`) and the log shows
  `launchsteps.go:84 Failed to resolve GeminiDir ".gemini": .gemini must be an absolute path … falling back to
  default` — so an absolute override is *plausible*, but **no CLI flag exposes it** (`--gemini-dir` →
  `flags provided but not defined`) and I did not find the env var name.
- `google_accounts.json` has an `old: []` array implying account switching exists, but only one account can be
  active at a time.

**Workspace trust is a real gate (verified, important).** `trustedFolders.json` marks `…/projects/taurhaus` as
`DO_NOT_TRUST` and `…/projects/localllms` as `TRUST_FOLDER`; `antigravity-cli/settings.json` carries
`trustedWorkspaces: ["~/projects/localllms"]`. In my **untrusted** scratch workspace, a `run_command`
tool call executed with `"Cwd":"~/.gemini/antigravity-cli/scratch"` — **not** the workspace — and
workspace customizations were not loaded (§6). The TUI asks *"Do you trust the contents of this project?"*.

**Other env vars found in the binary:** `AGY_CLI_HIDE_LOGO`, `AGY_CLI_DISABLE_LATEX`,
`AGY_CLI_DISABLE_ESCAPE_SEQUENCE_OPTIMIZATION(S)`, `AGY_CLI_FORCE_OSC8`, `AGY_ADC_AUTH`, `GEMINI_API_KEY`,
`ANTIGRAVITY_SIDECAR_WEB_PORT`, plus the injected-into-children set in §7.

**Endpoints (from `strings` + live log):** `https://daily-cloudcode-pa.googleapis.com/v1internal:loadCodeAssist`
(observed live), `https://antigravity.google/oauth-callback` (OAuth redirect),
`https://accounts.google.com/o/oauth2/auth` (OAuth), `https://www.googleapis.com/oauth2/v3/tokeninfo`,
`https://antigravity.google/g1-activity?…` (AI-credits activity page),
`https://www.gstatic.com/antigravity/web/dev/tailwindcss.min.js`.

### How verified
Directory walk of `~/.gemini`, `cat` of every non-secret config file, Python key-shape dump (names + lengths
only) for the two credential files, `agy models` as a sign-in probe, live `--log-file` override, a controlled
`JETSKI_APP_DATA_DIR` experiment, `agy --gemini-dir` rejection, `run_command` cwd observation, `strings` URL
extraction.

### Unverified
- The exact env var (if any) that relocates `GeminiDir` — **would be verified** by disassembling
  `entrypoints.resolveGeminiDirPath`, or by `ltrace`/`strace -e getenv` on startup.
- Whether `HOME` relocation works end-to-end (would require re-authenticating a second account — out of scope).
- Whether keyring storage changes file layout on native Linux/macOS.

### Recommendation for taurhaus
Point the agy adapter at `~/.gemini/antigravity-cli/` as the app-data root and `~/.gemini/` as the identity
root. Read the account from `google_accounts.json:active`. **Do not plan on per-process multi-account
isolation** the way `TAURHAUS_DATA_DIR`/`CLAUDE_CONFIG_DIR` work — as of 1.1.22 agy is single-account per
machine; model this as a hard constraint in the mesh (one agy lane per host) and re-test on each version bump.
**Add a trust preflight**: before launching an agy lane for a project, check `trustedFolders.json` /
`trustedWorkspaces` and surface an explicit "workspace not trusted — agy will run in a scratch cwd and ignore
`.agents/`" warning rather than letting it fail silently.

---

## 4. BUSY/IDLE + SESSION IDENTITY

This is the slice with the best news and one sharp caveat.

### Facts

**(a) `presence/<conversation-id>.lock` is a live-session registry — verified.**
Zero-byte files under `~/.gemini/antigravity-cli/presence/`. `/proc/locks` showed
`FLOCK ADVISORY WRITE <agy_pid> …` on the lock inode belonging to the running interactive session, and
`flock -n` on that file failed while the session lived and succeeded after `/exit`. After all sessions exited,
**none** of the 31 lock files were held. Symbols confirm the design:
`store.acquirePresenceLock`, `store.releasePresenceLock`, `store.presenceDirPath`,
`store.(*Manager).acquireConversationPresence`, `store.(*Manager).setPresence`.

So: **lock held ⇒ that conversation is open in a live process. Lock free ⇒ no process owns it.**
Crucially the lock is held for the **whole session lifetime**, *not* per turn — I polled `flock -n` at ~3 Hz
across a full turn and it never flipped. It answers "alive?", **not** "busy?".

**(b) `conversation_summaries.db` is NOT a usable live signal — verified caveat.**
Schema is tantalising:
```sql
CREATE TABLE conversation_summaries(
  conversation_id text PRIMARY KEY, title text, preview text, step_count integer,
  last_modified_time datetime, workspace_uris text, status text, source text,
  project_id text, agent_name text, parent_conversation_id text, nesting_depth integer,
  battle_id text, winning_conversation_id text,
  not_fully_idle numeric,          -- <-- exactly what a coordinator wants
  killed numeric, last_user_input_time datetime,
  last_user_input_step_index integer, app_data_dir text)
```
**But in practice it is nearly unwritten.** After 31 conversations across print and interactive modes, the
table still held exactly **one** row (the very first probe), `not_fully_idle = 0`, `status = ''`,
`workspace_uris = ''`. It never updated during or after any later run. `cache/conversation_metadata.json` is
stale the same way. Do not build on these in 1.1.22.

**(c) `cache/last_conversations.json` — verified live and useful.**
`{ "<absolute cwd>": "<conversation-uuid>" }`, rewritten promptly as conversations are created. This is the
clean **cwd → current conversation id** mapping.

**(d) `--output-format stream-json` — the best machine signal. Verified end to end.**
Sample captured at `…/scratchpad/agy-stream-json-sample.jsonl`. Closed vocabulary, three event types:

```jsonc
{"event":"init","conversation_id":"<uuid>",
 "init":{"cwd":"<abs path>","tools":[ …57 tool names… ],"permission_mode":"request-review"}}

{"event":"step_update","step_update":{
 "conversation_id":"<uuid>","step_index":1,"state":"DONE","step_type":"agent_response",
 "text_delta":"OK\n","duration_seconds":1.72,
 "usage":{"input_tokens":13875,"output_tokens":104,"thinking_tokens":103,
          "cache_read_tokens":0,"total_tokens":13979}}}

{"event":"result","result":{
 "conversation_id":"<uuid>","status":"SUCCESS","response":"OK\n",
 "duration_seconds":1.79,"num_turns":1,"usage":{…},
 "error":"…"                       // present when status=="ERROR"
 // "command":{"name":"usage","data":{…}} for CLI-answered slash commands
}}
```
`step_type` observed: `user_input`, `agent_response`. The underlying enum is large
(`CORTEX_STEP_TYPE_*`: `RUN_COMMAND`, `VIEW_FILE`, `GREP_SEARCH`, `FILE_CHANGE`, `CHECKPOINT`,
`EPHEMERAL_MESSAGE`, `BRAIN_UPDATE`, `FINISH`, …). `permission_mode` in `init` reflects the effective mode.

**(e) Lifecycle hooks carry an explicit idle flag — the intended coordinator signal.**
The `Stop` hook receives `{"executionNum", "terminationReason", "error", "fullyIdle", …}` where
`terminationReason ∈ {model_stop, max_steps_exceeded, error}` and **`fullyIdle` is true only when all
background tasks and subagents are done**. This mirrors internal symbols
`agent_state_component.(*AgentState).IsFullyIdle`, `.SubagentsAndTasksIdle`, `.DependentsFullyIdle`,
`WithFullyIdleCallback`, and `backend.(*ServerBackend).WaitForConversationFullyIdle`. Full contract in §6.

**(f) TUI scraping — verified and reliable as a fallback.**
The footer right-hand hint is an unambiguous discriminator in the captured pane:
- **working:** spinner + phase label (e.g. `⣷  Begin Essay Development…`) and footer `esc to cancel`
- **idle:** footer `? for shortcuts`

**(g) tmux pane title / OSC: NOT emitted by default.** `#{pane_title}` stayed at the hostname through startup,
a full turn, and completion. `/title` exists but is a *configurable command*, not an automatic status feed —
running it returned `Error: No title command configured in settings.json. Please add a "title" block.` and in
print mode it reports `/title is not available in print mode (it configures the terminal window title)`.
(The host tmux also has `set-titles off`.)

**(h) Session identity is exported to child processes — verified.** See §7: every tool subprocess sees
`ANTIGRAVITY_CONVERSATION_ID`, `ANTIGRAVITY_TRAJECTORY_ID`, `ANTIGRAVITY_PROJECT_ID`.

**(i) fd/socket state is not a useful signal**: an idle TUI held only 2 socket/pipe fds and process state
oscillated `S`/`R` purely with rendering.

### How verified
Live tmux TUI session driven with `tmux send-keys`; ~3 Hz polling of `flock -n`, `/proc/locks`, `ps -o stat=`,
pane capture, and SQLite reads across several full turns; before/after SQLite dumps; captured stream-json
samples; `agy -p /title`; symbol extraction.

### Unverified
- Whether `not_fully_idle` is written by the Antigravity **IDE/2.0** surfaces (which share this DB via
  `app_data_dir`) even though the CLI does not. **Would be verified** by running the IDE against the same
  `~/.gemini` and re-reading the table.
- Whether a `Stop` hook actually fires in this install (blocked by the trust gate — see §6).

### Recommendation for taurhaus
Use a **layered** busy/idle model, in this priority order:

1. **Headless lanes:** consume `--output-format stream-json` directly. `result` ⇒ idle; `step_update` ⇒ busy.
   This is exact and needs no polling.
2. **Interactive lanes:** install a `Stop` + `PreInvocation` hook pair that writes
   `{conversationId, state, ts}` into a taurhaus-owned file (hooks get `conversationId` on stdin **and**
   `ANTIGRAVITY_CONVERSATION_ID` in env). `Stop.fullyIdle` is precisely taurhaus's "idle" notion including
   subagents. **Gate this on the workspace being trusted.**
3. **Liveness (always):** scan `~/.gemini/antigravity-cli/presence/*.lock` with a non-blocking `flock`; held ⇒
   session alive. Combine with `cache/last_conversations.json` for cwd → conversation id.
4. **Fallback only:** pane-footer scrape (`esc to cancel` vs `? for shortcuts`).

**Do not** build on `conversation_summaries.not_fully_idle` or on tmux pane titles in 1.1.22. Add a version
assertion so the adapter fails loudly if a future agy changes the stream-json vocabulary.

---

## 5. TRANSCRIPTS

### Facts

**Storage is SQLite, one database per conversation** — not JSONL:
`~/.gemini/antigravity-cli/conversations/<conversation-uuid>.db` (+ `-wal`, `-shm`; WAL mode, so read with
`?mode=ro` and expect uncommitted tail in `-wal`).

Schema (verified):
```sql
trajectory_meta(trajectory_id TEXT PK, cascade_id TEXT, trajectory_type INT, source INT)
steps(idx INT PK, step_type INT, status INT, has_subtrajectory NUM, metadata BLOB,
      error_details BLOB, permissions BLOB, task_details BLOB, render_info BLOB,
      step_payload BLOB, step_format INT)
gen_metadata(idx, data BLOB, size INT)
executor_metadata(idx, data BLOB)
parent_references(idx, data BLOB)
trajectory_metadata_blob(id TEXT DEFAULT "main", data BLOB)
battle_mode_infos(idx, data BLOB)
```
`step_payload` and the other BLOBs are **binary protobuf**, not JSON — but user and assistant text is plainly
recoverable as embedded UTF-8. Observed `step_type` integers: `14` = user input, `15` = agent response,
`101` = system/inbox message. `status = 3` = done.

**Mapping conversation → project/cwd (verified, two independent ways):**
1. `trajectory_metadata_blob.data` embeds the workspace URI and project id, e.g.
   `file:///tmp/.../agy-probe` … `default-cli-project`, alongside `cascade_id` and `trajectory_id`.
2. `~/.gemini/antigravity-cli/cache/last_conversations.json` gives `{cwd: conversation_id}` for the *latest*
   conversation per cwd (fast path, but only the newest).

Titles live separately as protobuf-text: `annotations/<uuid>.pbtxt` → `title:"Request For Single Word"`
(auto-generated at conversation creation since 1.1.21).

**A JSONL transcript does exist, but it is hook-facing.** Both the shipped hooks doc and the public docs name
`transcriptPath` = `<appDataDir>/brain/<conversationId>/.system_generated/logs/transcript.jsonl`
(`antigravity-cli/` for the CLI, `antigravity/` for Antigravity 2.0, `antigravity-ide/` for the IDE).
`brain/<uuid>/` directories were created for every conversation, containing `scratch/` and `.user_uploaded/`;
I did **not** observe `.system_generated/logs/transcript.jsonl` materialise for these short runs.

**Compaction is tracked and is visible in principle.** Symbols:
`jetski_cortex_go_proto.(*CompactionInfo).GetCompactedAtStepIndices`, `AgentStateUpdate.GetCompactionInfo`,
`store.(*Manager).applyCompactionInfo`, `agent_state_component.(*AgentState).reconstructCompactedIndices`,
`render.isCompactionBoundaryStep`, `render.renderCompactionMarker`. So compaction is represented as a set of
compacted step indices on the agent-state update and rendered as a boundary marker in the TUI.
**It is not surfaced as a `stream-json` event type** — the vocabulary is only `init`/`step_update`/`result`.

### How verified
`sqlite3`/Python read-only dumps of `conversations/<id>.db` and `conversation_summaries.db`; printable-string
extraction from `step_payload` and `trajectory_metadata_blob`; directory walks of `brain/` and `annotations/`;
shipped `hooks.md` and the public hooks page; symbol extraction.

### Unverified
- Exact `.proto` for `step_payload` — **would be verified** by extracting the embedded
  `google.protobuf.FileDescriptorProto` set from the binary (the descriptors are present) and decoding properly
  instead of scraping strings.
- The precise trigger that writes `.system_generated/logs/transcript.jsonl` — **would be verified** by running
  a long conversation with a hook installed in a trusted workspace and watching the path.
- The full `step_type` integer → `CORTEX_STEP_TYPE_*` mapping (only 14/15/101 observed).

### Recommendation for taurhaus
Read transcripts via **read-only SQLite** on `conversations/<id>.db`, not by tailing files; open with
`file:…?mode=ro` and tolerate WAL lag. Map to a project via `trajectory_metadata_blob` (authoritative) with
`last_conversations.json` as the fast path. **Do not** invest in a JSONL tailer like the Codex compaction
extractor until `transcriptPath` is confirmed to be written for CLI sessions — and even then, prefer the hook
payload's `transcriptPath` over guessing the location, since it differs per surface. Treat compaction as
**not observable** from outside in 1.1.22 and design the mesh's compaction bridge to degrade gracefully for
the agy lane.

---

## 6. HOOKS / NOTIFY / SKILLS / PLUGINS

### Facts

**agy ships a full first-party hook system** — documented both publicly
(https://antigravity.google/docs/hooks) and offline inside the binary's builtin skill at
`~/.gemini/antigravity-cli/builtin/skills/agy-customizations/docs/hooks.md`.

**Five events.** `PreToolUse`, `PostToolUse` (both take a regex `matcher` on the tool name and wrap handlers in
a `{matcher, hooks[]}` group), and `PreInvocation`, `PostInvocation`, `Stop` (flat handler lists, matcher
ignored). Tool names are the lowercased `CORTEX_STEP_TYPE_` suffix, e.g. `run_command`, `view_file`.

**Config shape** (`hooks.json`, top-level keys are hook *names*, so multiple sources merge):
```json
{ "lint-checker": {
    "PostToolUse":[{"matcher":"run_command",
      "hooks":[{"type":"command","command":"./scripts/lint.sh","timeout":10}]}] },
  "safety-gate": { "enabled": false, "PreToolUse":[…] } }
```
`type` only supports `"command"` (run via `sh -c`, `~` expanded, **cwd = the directory containing
hooks.json**), `timeout` defaults to 30 s. Hooks run **synchronously and block the agent loop**.

**I/O contract:** JSON on stdin, JSON on stdout, **camelCase** keys (protojson). Common fields on every event:
`conversationId`, `workspacePaths[]`, `transcriptPath`, `artifactDirectoryPath`, `modelName`.

| Event | Extra input | Output |
|---|---|---|
| `PreToolUse` | `toolCall{name,args}`, `stepIdx` | `decision` ∈ `allow`/`deny`/`ask`/`force_ask`; `reason`; `permissionOverrides[]`; `overwrite{}` (shallow merge into tool args) |
| `PostToolUse` | `stepIdx`, `error?` | `{}` |
| `PreInvocation` | `invocationNum`, `initialNumSteps` | `injectSteps[]` of `{toolCall}` / `{userMessage}` / `{ephemeralMessage}` |
| `PostInvocation` | same as Pre | `injectSteps[]`, `terminationBehavior` ∈ `force_continue`/`terminate`/`""` |
| `Stop` | `executionNum`, `terminationReason`, `error`, **`fullyIdle`** | `decision:"continue"` blocks the stop; `reason` injected as a system message |

**Discovery locations for `hooks.json`** (strings in the binary): `~/.gemini/config/hooks.json` (global),
`~/.gemini/antigravity-cli/hooks.json` (CLI-global; `store.(*Manager).GetDefaultHooksPath` + `SaveHooks`
suggest `/hooks` can write here), and `<workspace>/.agents/hooks.json`. The customization roots are
`.agents/`, `.agent/`, `_agents/`, `_agent/`, discovered by walking **cwd → repository root**.

**⚠ Verified negative: workspace hooks did NOT load in my probe.** I wrote a valid
`.agents/hooks.json` (JSON validated) with all five events at a `git init`-ed workspace root; every run logged
`hooks_manager.go:53 loaded 0 named hooks from 0 hooks.json file(s)`, `agy -p /hooks` returned `{"hooks":[]}`,
and no hook script ever executed. The most probable cause is the **workspace trust gate** — symbols
`store.(*Store).workspaceTrusted` and `types.(*CliSetting).IsTrustedWorkspace` exist, and independently I
observed that in this untrusted workspace agy relocated `run_command`'s cwd to
`~/.gemini/antigravity-cli/scratch`. I did **not** confirm this, because accepting the trust prompt or editing
`trustedWorkspaces` would have modified the user's agy config, which was out of scope.

**Notify.** There is **no notify hook**. `settings.json` has a boolean `notifications` (default `false`) for
desktop/bell notifications. Strings mention *"notification on tool confirmation and agent completion"* and
*"notification when the timer fires or cron triggers"*. No session-start / session-end / compaction hook exists
(confirmed against the public hooks page).

**Statusline is a second, richer integration point.** `settings.json` has a `statusLine` block holding an
external **command**; `store.(*StatusLineRunner).Run/Output/Stop/ErrorHint` executes it and renders stdout.
`/statusline` supports `on`/`off`/`set`/`delete`/`help`. Changelog 1.1.21 added a `cost` field to *"the status
line data model"*, so the command receives structured session data. There is an analogous `title` block for the
terminal window title.

**Skills / slash commands / plugins / MCP.**
- **Skills:** `skills/<name>/SKILL.md` with YAML frontmatter (`name`, `description`, optional `metadata.icon`)
  plus optional `scripts/`, `references/`, `examples/`, `resources/`. Progressive disclosure — only name +
  description enter context until activated. `agy -p /skills` listed the 5 builtins
  (`agy-customizations`, `antigravity-guide`, `generative_ui`, `migrate-workflows`, `permissioned-github`).
- **Rules:** `GEMINI.md` / `AGENTS.md` anywhere in the tree (walked cwd → repo root), plus `.agents/rules/*.md`.
- **Plugins:** `plugins/<name>/plugin.json` bundling `skills/`, `rules/`, `hooks.json`, `mcp_config.json`.
  Enable/disable state lives in `config.json` under a `plugins` map keyed by directory name; CLI subcommands
  `agy plugin list|import|install|uninstall|enable|disable|validate|link`. `agy plugin list` → *"No imported
  plugins."* Notably `agy plugin import [gemini|claude]` can **import Claude Code plugins**.
- **MCP:** `~/.gemini/config/mcp_config.json` (global) or per-plugin; stdio (`command`/`args`/`env`) or SSE
  (`serverUrl`). `agy mcp add|remove|list|enable|disable`. `agy mcp list` → *"No MCP servers configured."*
- **Slash commands** (from Go `*Command` types): `agents artifact btw changelog clear config context copy
  credits diff effort exit feedback fork help hooks keybindings logout mcp model open permissions rename resume
  rewind search skills statusline tasks title usage voice` + generic/workflow-defined ones. Legacy "workflows"
  (`~/.gemini/config/workflows/*.md`, `global_workflows/*.md`, `workflows.json`) are being migrated to skills by
  the builtin `migrate-workflows` skill.

### How verified
Read the four shipped docs (`hooks.md`, `skills.md`, `plugins.md`, `mcp_servers.md`, `json_configs.md`) and
`SKILL.md`; cross-checked against https://antigravity.google/docs/hooks; wrote and validated a real
`.agents/hooks.json` + executable handler and ran turns against it; `agy -p /hooks`, `/skills`, `/config`;
`agy mcp list`, `agy plugin list`; log inspection; symbol extraction.

### Unverified
- **That hooks fire at all on this machine** — blocked by the trust gate. **Would be verified** by trusting a
  scratch workspace (writes `trustedFolders.json`) or placing `hooks.json` at
  `~/.gemini/antigravity-cli/hooks.json` (writes user config) — both deliberately avoided.
- The exact JSON schema the `statusLine` command receives (field list beyond `cost`). **Would be verified** by
  setting a `statusLine` command that dumps stdin — again a user-config write.
- Whether `plugin import claude` faithfully translates Claude Code hooks.

### Recommendation for taurhaus
This is agy's **strongest** coordination surface and the right place to build the mesh bridge — it is a much
better fit than the Claude `SessionEnd` hook pattern because `Stop.fullyIdle` already encodes
"including subagents".

Concretely: ship a taurhaus plugin (`plugins/taurhaus/` with `hooks.json` + `plugin.json`) rather than raw
`.agents/hooks.json`, so it is one enable/disable unit and can also carry MCP tools later. Wire
`PreInvocation` → busy, `Stop` → idle (recording `fullyIdle` and `terminationReason`), and `PostToolUse` for
activity breadcrumbs. Key it on `conversationId` from stdin.

Two hard requirements to design around: **(1)** hooks block the agent loop, so handlers must be a few
milliseconds — write a line to a file, never call back into taurhaus synchronously; **(2)** installation
requires the workspace to be trusted, and trusting is a user action. Make the taurhaus setup flow do this
explicitly and verify with `agy -p /hooks` (which returns machine-readable `{"hooks":[…]}` under
`--output-format json`) before declaring the lane healthy.

---

## 7. DELIVERY (injecting a message without tmux keystrokes)

### Facts

**(a) `--input-format stream-json` is a genuine, verified delivery channel.**
It reads one NDJSON message per line from stdin and **runs a turn for each, in a single conversation, keeping
the process alive between turns**. I brute-forced the (undocumented) schema; the accepted shape is:

```json
{"event":"user","message":{"content":"your text here"}}
```
`message.content` also accepts Anthropic-style content blocks: `[{"type":"text","text":"…"}]`.
Rejected forms and their exact errors (useful for a strict adapter):
- missing `event` → `stream input message is missing the "event" field`
- `event` ∈ anything else (`user_message`, `user_input`, `input`, `message`, `prompt`, `turn`) →
  stderr `warning: ignoring unsupported stream input message event "<x>"` and the line is **silently skipped**
- `{"event":"user"}` → `stream input "user" message is missing the "message" field`
- `message` as a bare string → `cannot unmarshal string into Go struct field streamInputMessage.message of type printmode.streamInputUserMessage`
- `message:{"text":…}` → `stream input "user" message has no content`

Two-message run verified: same `conversation_id` across both turns, `step_index` advancing 0→3, and the second
`result` reporting `num_turns: 2` with cumulative usage. Requires `--output-format stream-json`.
CLI-answered slash commands are **not** available on this channel:
*"/%s is answered by the CLI itself and is unavailable with --input-format stream-json; run it as its own
--print /%s invocation"*.

**(b) A local agent API exists, and every tool subprocess is handed its address — verified.**
Each agy process starts an in-process language server on two random localhost ports, e.g.
`Language server listening on random port at 45687 for HTTPS (gRPC)` / `at 46167 for HTTP`. I confirmed by
having agy run a command that dumped its own environment (names first, then non-secret values) that **tool
subprocesses — and therefore hooks — inherit:**

| Variable | Observed value / shape |
|---|---|
| `ANTIGRAVITY_LS_ADDRESS` | `localhost:46537` (random per process) |
| `ANTIGRAVITY_AGENTAPI_EXE` | `~/.local/bin/agy` (the CLI re-invokes itself as the API client) |
| `ANTIGRAVITY_CSRF_TOKEN` | **present but empty** (length 0) in CLI mode |
| `ANTIGRAVITY_CONVERSATION_ID` | live conversation UUID |
| `ANTIGRAVITY_TRAJECTORY_ID` | trajectory UUID |
| `ANTIGRAVITY_PROJECT_ID` | `default-cli-project` |
| `ANTIGRAVITY_AGENT` | `1` |
| `ANTIGRAVITY_LS_VERSION` | `cli-1.1.22` |
| `ANTIGRAVITY_SOURCE_METADATA` | JSON: `{"tool":{"conversationId","stepIndex","toolCall":{"id","name","argumentsJson","thinkingSignature"}}}` |

Matching handlers exist: `agentapi.(*newConversationHandler)`, `(*sendMessageHandler)`,
`(*getConversationMetadataHandler)`, plus `store.(*Manager).sendMessageOrSteer` — i.e. the API can **steer an
in-flight conversation**, not merely queue a new turn.

**(c) An in-conversation inbox exists.** Tools `send_message` and `manage_inbox`
(`handlers.(*ManageInboxSubHandler).handleList/handleRead`, `tools.(*SendMessageTool)` with
`recipient`/`recipientName`), and messages appear in the trajectory as step type `101`, e.g.
`[Message] timestamp=… sender=system priority=MESSAGE_PRIORITY_LOW content=[Notice] …`. Priorities:
`MESSAGE_PRIORITY_{LOW,NORMAL,HIGH,UNSPECIFIED}`. This is agent↔subagent messaging, reachable from inside the
session.

**(d) `--remote-control` exists but is a separate, unauthenticated-here path.** Accepted as a flag (not
rejected), and `config.json` already holds `userSettings.remoteControlHostname`. Invoking it printed
`No valid authentication found (). Starting login...` and emitted a Google OAuth URL — **I stopped immediately
and did not complete any login**, per the task constraints. Changelog and strings indicate it is a headless
daemon with a WebRTC/DataChannel + SSH transport (`RemoteControlDetails_Transport`,
`Remote control connection shutdown timed out`, `%s Failed to unmarshal SDP offer payload`) and that its
`Open in your browser: http://localhost:<port>` banner *"was never a supported way to connect"*.

**(e) There is no Unix-domain socket or PID-file IPC.** The running TUI held only 2 socket/pipe fds; nothing in
the app-data dir resembles a control socket.

### How verified
Systematic stdin-schema brute force with per-attempt stderr/stdout capture; a two-message NDJSON run;
`run_command`-driven environment dump (names, then non-secret values only); `/proc/<pid>/fd` inspection; log
port lines; symbol extraction; a single `--remote-control` invocation aborted at the sign-in prompt.

### Unverified
- The `agentapi` wire protocol, and whether it is reachable from an unrelated process (the empty CSRF token
  suggests it may be, but it is bound to `localhost` on a random port and I did not probe it). **Would be
  verified** by `agy`-as-client experiments using `ANTIGRAVITY_AGENTAPI_EXE` from inside a session, or by
  gRPC reflection against `exa.language_server_pb.LanguageServerService`.
- Everything about `--remote-control` beyond its existence — deliberately not pursued.

### Recommendation for taurhaus
**Make `--input-format stream-json` the primary agy delivery path** and reserve tmux keystrokes for the
attached-TUI case only. It gives taurhaus a long-lived conversation with structured request *and* response
framing over plain pipes — strictly better than what the Claude/Codex lanes get today. Implement it as: spawn
`agy --input-format stream-json --output-format stream-json --print=` with a held-open stdin, write one
`{"event":"user","message":{"content":…}}` line per delivered message, and parse `result` events for
completion. **Validate the exact event vocabulary at startup** and fail loudly on the
`ignoring unsupported stream input message event` warning, because unknown events are silently dropped — a
schema drift would otherwise look like a hung agent.

Do not build on `--remote-control` (separate auth, undocumented) or on the local agent API (random port,
undocumented protocol) until Google documents them.

---

## 8. STOP

### Facts (all verified live in tmux)

| Gesture | Effect |
|---|---|
| `Ctrl+C` **during a turn** | **Interrupts the turn only.** Pane shows `⎿ Interrupted · What should Antigravity CLI do instead?` and returns to the prompt. **Process stays alive.** |
| `Ctrl+C` at an idle prompt | Terminates the session (per docs, with confirmation if the agent is working) |
| `Ctrl+D` | Exits when the prompt is empty; docs describe `Ctrl+D Ctrl+D` |
| `Esc` | Cancels the in-flight stream (footer literally reads `esc to cancel` while working) |
| `/exit`, `/quit` | **Clean shutdown — verified:** process gone, tmux session ended |
| Print mode | Exits on its own; `--print-timeout` (default `5m`) bounds the wait |

Exit codes were fixed in 1.1.20 so that benign tool errors and permission denials no longer produce non-zero;
only cascade-level failures do. A dropped agent-state stream now exits non-zero (1.1.18).
`printmode.ExitCode` is the mapping symbol.

### How verified
Drove a live TUI: started a long generation, sent `C-c`, captured the pane and confirmed the PID survived; then
sent `/exit` and confirmed both the process and the tmux session were gone.

### Unverified
- Behaviour of `Ctrl+C` at an idle prompt (not exercised, to avoid ambiguity with the interrupt case).
- Whether `SIGTERM` is handled gracefully (would risk leaving a stale presence lock).

### Recommendation for taurhaus
Graceful stop = send `/exit` + `Enter` to the pane, then wait for process exit. **Never use `Ctrl+C` as a stop**
— it is an interrupt and will leave a live, idle session that taurhaus may then mis-attribute. Use `Esc` (or a
single `Ctrl+C`) as the "cancel current turn" primitive, which taurhaus currently lacks a clean equivalent for
in other lanes. After stop, confirm by checking that the session's `presence/<id>.lock` is no longer flock-held.

---

## 9. USAGE / QUOTA + ACCOUNTS

### Facts

**`agy -p /usage --output-format json` returns a fully structured quota object — verified.** This is the
cleanest quota surface of any CLI taurhaus integrates. Shape:

```jsonc
{"conversation_id":"","status":"SUCCESS",
 "response":"Gemini Models\tWeekly Limit Remaining\t100%\t2026-09-03T23:11:28Z\n…",
 "command":{"name":"usage","data":{
   "description":"Within each group, models share a weekly limit and a 5-hour limit…",
   "groups":[
     {"name":"Gemini Models","description":"Models within this group: Gemini Flash, Gemini Pro",
      "buckets":[
        {"id":"gemini-weekly","name":"Weekly Limit Remaining","window":"weekly",
         "remaining_fraction":0.9988006949424744,"reset_time":"2026-09-03T23:11:28Z",
         "description":"You have used some of your weekly limit, it will fully refresh in 6 days, 23 hours."},
        {"id":"gemini-5h","window":"5h","remaining_fraction":0.9928041100502014,
         "reset_time":"2026-08-28T04:11:28Z", …}]},
     {"name":"Claude and GPT models","description":"Models within this group: Claude Opus, Claude Sonnet, GPT-OSS",
      "buckets":[{"id":"3p-weekly",…},{"id":"3p-5h",…}]}]}}}
```
Two model groups (`Gemini Models`; `Claude and GPT models`), each with a `weekly` and a `5h` bucket, each
carrying `remaining_fraction` (0–1 float) and an absolute `reset_time`. Quota is consumed **proportionally to
token cost**, not by request count. Note `/usage` costs nothing (`num_turns: 0`, zero tokens) — safe to poll.

**Credits.** `/credits` is a separate G1-credits surface; it returned
`Error: Eligibility check failed: … UNAVAILABLE (code 503)` at probe time (a transient service error, not a
config problem). `settings.json` has `useG1Credits` (default `false`) — *"Use personal credits when quotas
exhausted"* — and `AI Credits not enabled (enable in /settings)`. The activity page is
`https://antigravity.google/g1-activity?…`. The TUI banner shows the tier (`Google AI Pro`).

**Exhaustion string:** `You have exhausted your quota on this model.` A `quota_manager.go doRefreshQuota` loop
refreshes periodically (`quotaRefresh`, `sendQuota`, `QuotaInfo` protos).

**Multi-account: not supported per-process.** As established in §3, there is a single active account
(`google_accounts.json:active`) with an `old: []` array; no env var or flag selects a config home. `/logout`
exists. Enterprise/Vertex paths exist (`AGY_ADC_AUTH`, `GEMINI_API_KEY`, "Gemini Enterprise Agent Platform
mode", `SetEnableBusinessLogin`) but were not exercised.

**Statusline `cost`.** Changelog 1.1.21 added *"a `cost` field to the status line data model, exposing the
unrounded estimated cost of the current session"* — a per-session spend signal available to a statusline
command.

### How verified
`agy -p /usage` in text and `--output-format json`; `agy -p /credits`; `agy -p /config` (both text and JSON,
showing `useG1Credits`, `statusLine`, `toolPermission`, `trustedWorkspaces`); TUI banner; `strings`; changelog.

### Unverified
- `/credits` payload shape (blocked by the upstream 503). **Would be verified** by re-running when the service
  is available.
- Enterprise/API-key auth modes and whether they change the quota model.

### Recommendation for taurhaus
Poll `agy -p /usage --output-format json` for the agy lane's quota tile — it is free, structured, and gives
both a fraction and an absolute reset timestamp, so taurhaus can render a real countdown rather than the
heuristics used for other CLIs. Key the display on the two groups so a user picking `claude-opus-4-6-thinking`
inside agy sees the right bucket. Surface `remaining_fraction < ~0.1` as a lane warning, and treat the
`exhausted your quota` string as a hard lane-stop condition. Model accounts as **one per machine**.

---

## 10. VERSIONING

### Facts

- **Programmatic version:** `agy --version` → `1.1.22` (bare string, exit 0). Also exposed to child processes
  as `ANTIGRAVITY_LS_VERSION=cli-1.1.22`, and logged at startup as `Language server version: 1.1.22`.
- **`agy changelog`** prints reverse-chronological release notes, newest first, as `X.Y.Z:` followed by `·`
  bullets. Head of output (1.1.22): a `/model <name>` argument that switches and saves the default in one step;
  `/effort` hint improvement; artifact-rescan coalescing; fixes for Gemini 3.1 Pro / 3.5 Flash effort under API
  key, continuous redraw pegging CPU near 32%, a frozen subagent timer, HTTP 502 not being retried, `self`
  subagent config drift, Windows file-deletion sharing violations, the headless daemon's misleading
  `Open in your browser` banner, and POSIX assumptions in `migrate-workflows`.
- **Update channel:** self-updating via `agy update` (**not run**). State in
  `~/.gemini/antigravity-cli/updater/update_status.json` → `{"success":true,"message":"Already on the latest
  version."}`, with `updater/update.lock` and a `last_check.timestamp` marker. Strings show
  `Checking for updates... (current version %s)`, `Found new version %s.`, `Downloading update...`. Staging
  area at `~/.cache/antigravity/staging`. Builtin assets are integrity-checked via
  `builtin/.checksum` (`builtin:66a45eab…`).
- **Public changelog:** https://antigravity.google/changelog.

### How verified
`agy --version`, `agy changelog`, `cat` of `updater/update_status.json` and `builtin/.checksum`, log lines,
environment dump, `strings`.

### Unverified
- Update cadence/channel selection (no channel flag found); whether `agy update` can be pinned.

### Recommendation for taurhaus
Read the version with `agy --version` (cheap, no sign-in) and record it alongside every captured session, since
agy self-updates and the `stream-json` vocabulary, `/usage` payload, and hook contract are all version-coupled.
Assert a known-good range in the adapter and degrade to TUI scraping if the version is newer than tested.
**Never invoke `agy update` from taurhaus** — it mutates the user's environment.

---

## Cited sources

**Official docs (fetched):**
- https://antigravity.google/docs/cli/reference — slash commands, keybindings, `settings.json` keys/defaults
- https://antigravity.google/docs/hooks — hook events, contract, `transcriptPath`

**Referenced by the shipped docs (not individually fetched):**
- https://antigravity.google/docs · /docs/skills · /docs/rules-workflows · /docs/plugins · /docs/sidecars
  · /docs/mcp · /docs/permissions · /docs/cli/features · /docs/cli/best-practices · /changelog · /support
- https://github.com/google-antigravity/antigravity-sdk-python (Python SDK for agent leasing/orchestration —
  potentially the cleanest long-term integration; **not evaluated**)

**Shipped offline docs (authoritative, on disk):**
`~/.gemini/antigravity-cli/builtin/skills/agy-customizations/{SKILL.md,docs/{hooks,skills,rules,plugins,mcp_servers,json_configs}.md}`
and `.../antigravity_guide/references/cli.md`

**Probe artifacts (this run):**
- `…/scratchpad/agy-stream-json-sample.jsonl` — required 4-event sample
- `…/scratchpad/agy-multiturn.jsonl` — two-turn single-conversation stdin stream
- `…/scratchpad/agy-child-env-names.txt` — env names inherited by tool subprocesses
- `…/scratchpad/agy-strings.txt` (587k lines), `…/scratchpad/agy-symbols.txt` (45,872 jetski symbols)

---

## Top recommendations, ranked

1. **Use `--input-format stream-json` as the agy delivery channel** (§7). Bidirectional NDJSON over pipes,
   verified multi-turn — strictly better than tmux keystrokes.
2. **Build busy/idle on hooks + presence locks, not on the summaries DB** (§4). `Stop.fullyIdle` is the exact
   semantic taurhaus wants; `presence/*.lock` gives liveness. `conversation_summaries.not_fully_idle` looks
   perfect and is **dead** in 1.1.22.
3. **Add a workspace-trust preflight** (§3, §6). Untrusted ⇒ no `.agents/` customizations and a relocated cwd,
   both silent. This is the single most likely cause of a mysteriously broken agy lane.
4. **Ship a taurhaus plugin, not loose hooks** (§6), and verify installation with `agy -p /hooks
   --output-format json` before marking the lane healthy.
5. **Treat agy as one account per machine** (§3, §9) — no `TAURHAUS_DATA_DIR`-style isolation exists.
6. **Read transcripts via read-only SQLite** (§5); do not write a JSONL tailer, and treat compaction as
   unobservable for now.
7. **Always pass `--dangerously-skip-permissions` or pre-seed `permissions.allow` for headless lanes** (§2),
   and treat empty-response-with-denial-notice as a failure.
8. **Stop with `/exit`, never `Ctrl+C`** (§8).
9. **Pin/record `agy --version`** (§10); the machine-facing contracts are version-coupled and it self-updates.
