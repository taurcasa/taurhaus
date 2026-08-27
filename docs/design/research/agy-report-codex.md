# Antigravity CLI 1.1.22 capability-slice report

Audit date: 2026-08-28 (Europe/Berlin)

Audited binary: `~/.local/bin/agy`

Scope: external coordination of `agy` by taurhaus through process inspection, tmux, the mesh bridge, and per-tool capability slices. The taurhaus repository itself was not modified and no Git write operation was run. `agy install` and `agy update` were not run. No probe used `--dangerously-skip-permissions`; all model probes used the scratch workspace `agy-probe` and the exact harmless prompt `reply with the single word OK`.

Secret-handling note: this report gives credential key names, file modes, and value shapes only. The signed-in email, OAuth values, authorization material, and endpoint query strings are omitted. Explicit probe logs contain the account email and therefore must be treated as sensitive even though no token value was observed in the displayed portions.

Audit-side-effect disclosure: the first interactive scratch launch displayed the trust gate. An `Enter` intended for the probe selected its default “trust” choice and briefly appended only the scratch `agy-probe` path to `~/.gemini/antigravity-cli/settings.json`. I immediately restored the exact pre-probe JSON content. The content now matches the baseline byte-for-byte in meaning, but the file mtime changed. No taurhaus repository path was trusted or changed.

Primary official references used:

- [CLI reference](https://antigravity.google/docs/cli/reference)
- [Headless mode and stream protocol](https://antigravity.google/docs/cli/headless/)
- [Projects](https://antigravity.google/docs/cli/projects/)
- [Installation and authentication](https://antigravity.google/docs/cli/install/)
- [Execution modes](https://antigravity.google/docs/cli/modes/)
- [Sandbox](https://antigravity.google/docs/cli/sandbox/)
- [Settings](https://antigravity.google/docs/cli/settings/)
- [Status-line integration](https://antigravity.google/docs/cli/statusline/)
- [Terminal-title integration](https://antigravity.google/docs/cli/title/)
- [Hooks](https://antigravity.google/docs/hooks)
- [Plugins and skills](https://antigravity.google/docs/cli/plugins/)
- [Quota command](https://antigravity.google/docs/cli/commands/usage/)
- [Remote Control](https://antigravity.google/docs/remote-control/)
- [Troubleshooting and updater behavior](https://antigravity.google/docs/cli/troubleshooting/)

## 1. Process signature

### Facts

The reliable process identity is the executable resolved from `/proc/<pid>/exe`, not merely the comm name. The audited path is a single primary executable, `~/.local/bin/agy`, with these properties:

- ELF 64-bit x86-64, PIE (`ET_DYN`), stripped.
- Size `208,429,312` bytes.
- SHA-256 `2822292f90deea4556938a8728fe4ed02a1d66d1525cf75fa07a171e36a38c25`.
- GNU build ID `a9f978445e9528435a7fcaa6983687aa`.
- It is **not statically linked**. `ldd`/`DT_NEEDED` show `libresolv`, `libpthread`, `libm`, `libdl`, `librt`, `libc`, and the ELF loader.
- It is Go-based despite `go version -m` saying “not a Go executable.” The ELF has `.go.buildinfo` and `.go.module`, contains `runtime.main`/`runtime/cgo`, and its build-info section identifies an internal `go1.28-20260721-RC01` toolchain with `boringcrypto` and other Google build features. The failure of the stock `go version -m` reader is therefore a build-metadata compatibility issue, not evidence that the program is non-Go.

The argv classification is:

| Slice | Recognizable argv shape | Important detail |
| --- | --- | --- |
| Fresh interactive TUI | `agy [global flags]` | No print flag and no subcommand. |
| Interactive with initial prompt | `agy [global flags] -i='PROMPT'` or `--prompt-interactive='PROMPT'` | Still interactive after the first turn. `-i` must not be classified as print mode. |
| One-shot print/headless | `agy [global flags] -p 'PROMPT'`, `--print='PROMPT'`, or `--prompt='PROMPT'` | `-p`, `--print`, and `--prompt` are the print-mode markers. |
| Persistent print driver | `agy [global flags] --input-format stream-json --output-format stream-json` | Prompts arrive on stdin; there must be no `-p` prompt. |
| Subcommand | `agy [global flags] agent[s]`, `models`, `changelog`, `help`, `mcp ...`, `plugin[s] ...`, `mic-serve ...`, `install ...`, or `update ...` | Match the first positional token after consuming global flag values. |

The parser is Go-flag-like and ordering matters. A deliberately malformed probe placed `--output-format` immediately after bare `--print`; the CLI diagnosed that `--print` had consumed `--output-format` as its prompt and recommended attaching the prompt to the flag and moving other flags earlier. For generated argv, use separate argv elements with all option/value pairs before the prompt flag, or use `--print=...`/`--prompt-interactive=...`.

The primary process is monolithic enough that both the CLI backend and language server run in the same PID. Each live CLI instance nevertheless opens two random loopback listeners: one HTTPS/gRPC listener and one HTTP listener. The probe log recorded ports `44851` and `45027`; a separate live `agy` instance showed the same two-listener pattern at other random ports.

No direct child process was observed while polling a five-second print run every 50 ms, and none was observed during an interactive idle/short-turn probe. This is an observation for the harmless prompt, not a guarantee for tool-using sessions. The installation has a separate executable at `~/.gemini/antigravity-cli/bin/webm_encoder`, and the CLI can necessarily spawn shell commands, sandbox utilities, MCP stdio servers, browser/sign-in helpers, title/status-line commands, hooks, and plugin components when those features are used. The 1.1.21 changelog says code search uses an embedded ripgrep rather than an external `rg` process.

### How verified

- `file`, `stat`, `sha256sum`, `ldd`, `readelf -h/-S/-n/-d`, `objdump -f`, and a hex dump of `.go.buildinfo`.
- `go version -m` failure was compared with the actual Go ELF sections and runtime strings.
- `agy --help` and `agy help <subcommand>` supplied the accepted top-level forms.
- The malformed print invocation supplied an exact parser error.
- `/proc`, `ps`, tmux pane metadata, and 50 ms child polling were used during permitted scratch probes.
- Probe logs explicitly said “Language server listening on random port ... for HTTPS (gRPC)” and “... for HTTP.” `ss` corroborated two loopback listeners on another contemporaneous instance.

### Unverified

- A very short-lived helper could evade 50 ms polling.
- The exact conditions under which `webm_encoder`, browser tooling, `nsjail`, or the updater are spawned were not exercised.
- Static strings include a hidden-looking `--remote-control`, and logs say a normal launch stays disconnected when it is absent, but it is not listed by `agy --help`. Its direct CLI contract was not exercised.

### Recommendation for taurhaus

Resolve `/proc/<pid>/exe` and parse `/proc/<pid>/cmdline` as NUL-delimited argv. Classify a known subcommand first after consuming flag values; otherwise classify `-p`/`--print`/`--prompt` as print, `-i`/`--prompt-interactive` as interactive, and the no-marker form as interactive. Do not infer state from `comm=agy`, child count, or socket count. Treat the loopback language-server ports as private implementation details unless Google publishes an authenticated client contract.

## 2. Launch

### Facts

The exact launch controls present in 1.1.22 are:

- Model: `--model <slug-or-supported-name>`. In headless mode an unknown model is a hard error rather than a fallback.
- Reasoning effort: `--effort low|medium|high`.
- Agent: `--agent <agent-name>`; list choices with `agy agents` (or singular `agy agent`).
- Execution mode: `--mode accept-edits|plan`. `accept-edits` auto-approves file creation/replacement, while `plan` prepends the planning instruction and stays read-oriented until plan approval. These do **not** replace tool permissions for shell commands.
- Per-run full permission bypass: `--dangerously-skip-permissions`. The official protocol says this changes effective `permission_mode` to `always-proceed` and approves all tool calls, including file writes and command execution.
- Sandbox: `--sandbox`. Persistent equivalent: `"enableTerminalSandbox": true` in `~/.gemini/antigravity-cli/settings.json`.
- Add workspace directory: repeatable `--add-dir <path>`.

The closest persistent equivalent to `--dangerously-skip-permissions` is `"toolPermission": "always-proceed"`. Fine-grained persistent policy is safer and is represented by `permissions.allow`, `permissions.deny`, and `permissions.ask` rules in `action(target)` form. File-review behavior has two additional, distinct controls: `artifactReviewPolicy` and `agentMode`. Therefore `agentMode: accept-edits` alone is not a global permission bypass.

Conversation starts and resumes:

- Fresh conversation: omit `-c` and `--conversation`.
- Continue most recent conversation for the current workspace: `-c` or `--continue`.
- Resume by ID: `--conversation <uuid>` (or `--conversation=<uuid>`).
- Initial prompt, keep TUI open: `-i='reply with the single word OK'` or `--prompt-interactive='...'`.
- One-shot prompt: `-p 'reply with the single word OK'` or `--print='...'`.

Projects are logical organizers/owners for conversations, separate from cwd-based workspace scoping:

- No project flag selects `default-cli-project`.
- `--project <project-id-or-name>` chooses an existing project. Name support was added in 1.1.18.
- `--new-project` creates and selects a project.
- Resuming a conversation automatically switches to that conversation's associated project, even if a different project would otherwise be current.

On this machine the default project is `default-cli-project`, displayed as “CLI Project,” with an empty `projectResources` object. Its current record is `~/.gemini/config/projects/default-cli-project.json`; the selected default ID is also in `~/.gemini/antigravity-cli/cache/default_project_id.txt`. The older changelog mentions a centralized `cache/projects.json`, but that file is absent in this 1.1.22 installation.

Safe argv templates:

```text
# Interactive, initial prompt, temporary model/effort/mode/sandbox overrides
agy --model=gemini-3.7-flash-low --effort=low --mode=plan --sandbox -i='reply with the single word OK'

# One shot with machine-readable progress
agy --model=gemini-3.7-flash-low --effort=low --sandbox --output-format=stream-json --print='reply with the single word OK'

# Long-lived programmatic conversation; prompts arrive as NDJSON on stdin
agy --model=gemini-3.7-flash-low --effort=low --sandbox --input-format=stream-json --output-format=stream-json
```

`agy models` succeeded without any sign-in prompt and returned:

```text
gemini-3.7-flash-high       Gemini 3.7 Flash (High)
gemini-3.7-flash-medium     Gemini 3.7 Flash (Medium)
gemini-3.7-flash-low        Gemini 3.7 Flash (Low)
gemini-3.6-flash-high       Gemini 3.6 Flash (High)
gemini-3.6-flash-medium     Gemini 3.6 Flash (Medium)
gemini-3.6-flash-low        Gemini 3.6 Flash (Low)
gemini-3.5-flash-high       Gemini 3.5 Flash (High)
gemini-3.5-flash-medium     Gemini 3.5 Flash (Medium)
gemini-3.5-flash-low        Gemini 3.5 Flash (Low)
gemini-3.1-pro-high         Gemini 3.1 Pro (High)
gemini-3.1-pro-low          Gemini 3.1 Pro (Low)
claude-sonnet-4-6           Claude Sonnet 4.6 (Thinking)
claude-opus-4-6-thinking    Claude Opus 4.6 (Thinking)
gpt-oss-120b-medium         GPT-OSS 120B (Medium)
```

`agy agents` exited `0` without printing any agent entries. It did not prompt for sign-in.

### How verified

- Top-level and subcommand help from the installed binary.
- A live scratch print probe succeeded with `--model gemini-3.7-flash-low --effort low --sandbox`.
- `agy models` and `agy agents` were run directly; no interactive authentication appeared.
- The [official headless guide](https://antigravity.google/docs/cli/headless/), [execution-mode guide](https://antigravity.google/docs/cli/modes/), [project guide](https://antigravity.google/docs/cli/projects/), and [settings reference](https://antigravity.google/docs/cli/settings/) were checked at version 1.1.22.
- Current project JSON and default-ID files were read without mutation.

### Unverified

- `--new-project` was not exercised because it would change persistent project configuration. The exact filename/ID allocation for a newly created project is inferred from current storage and must be verified in a disposable OS account.
- Whether `agy agents` itself requires account authentication on a machine with local declarative agents but no Google session was not tested. On this signed-in machine it simply returned an empty list.
- No dangerous-permissions probe was run. Its semantics come from installed help and current official documentation.

### Recommendation for taurhaus

Always construct argv arrays, place option/value pairs before the prompt flag, pin a model slug, and request stream JSON. Make `--dangerously-skip-permissions` an explicit high-risk capability unavailable to normal taurhaus slices; use scoped `permissions.allow` rules or sandboxed execution instead. Treat project ID and cwd as separate identity dimensions and store both alongside the conversation ID.

## 3. Config and identity

### Facts

The current filesystem layout is:

```text
~/.gemini/
├── google_accounts.json                       # account selector/history; active + old
├── oauth_creds.json                           # OAuth-shaped legacy/shared credential file
├── config/
│   ├── config.json                            # shared Antigravity settings
│   ├── mcp_config.json                        # shared MCP config
│   └── projects/<project-id>.json             # current project definitions
└── antigravity-cli/
    ├── settings.json                          # primary CLI preferences
    ├── antigravity-oauth-token                # agy-specific file token store on this WSL host
    ├── cli.log -> log/cli-YYYYMMDD_HHMMSS.log # rotating current-log symlink
    ├── log/                                   # per-process glog-style logs
    ├── conversations/<conversation-id>.db     # SQLite, normally with -wal/-shm
    ├── conversation_summaries.db              # SQLite summary/index database
    ├── brain/<conversation-id>/               # artifacts and rendered transcript logs
    ├── cache/                                  # metadata, last-conversation map, changelog, project ID
    ├── presence/                               # zero-byte conversation-ID lock markers
    ├── annotations/                            # per-conversation pbtxt
    ├── crashes/                               # crash log files
    ├── updater/                               # updater status/lock
    └── bin/webm_encoder                       # separate installed media helper
```

The default log path is the symlink `~/.gemini/antigravity-cli/cli.log`, whose target rotates under `log/`. The top-level `--log-file <path>` flag overrides it. The explicit scratch probe log was plain ASCII, 160 lines/26,136 bytes, using glog-style severity/date/time/thread/source prefixes rather than JSON.

Credential/identity files were inspected by schema only:

- `~/.gemini/antigravity-cli/antigravity-oauth-token`, mode `0600`: top-level keys `token` and `auth_method`; `auth_method` is `consumer`; nested token keys are `access_token`, `refresh_token`, `token_type`, and `expiry`.
- `~/.gemini/oauth_creds.json`, mode `0600`: `access_token`, `refresh_token`, `scope`, `token_type`, `id_token`, `expiry_date`.
- `~/.gemini/google_accounts.json`, mode `0644`: `active` is a string with email shape; `old` is an array and is currently empty. The value was not printed.

This host is signed in. A print run authenticated silently using the stored consumer credential, the model call returned `OK`, the TUI showed a Google AI Pro account, and `agy models` fetched the live catalog. The probe log states that WSL caused file-based token storage, then records silent authentication succeeding and an effective consumer identity. No browser sign-in flow appeared.

No supported per-process config-root/home selector was found. Exact binary-string checks were negative for `AGY_HOME`, `AGY_CONFIG_HOME`, `AGY_CONFIG_DIR`, `AGY_DATA_DIR`, `ANTIGRAVITY_HOME`, `ANTIGRAVITY_CONFIG_HOME`, `ANTIGRAVITY_CONFIG_DIR`, `ANTIGRAVITY_CLI_HOME`, `GEMINI_HOME`, and `GEMINI_CLI_HOME`. `HOME` and `XDG_*` strings exist, but the current official docs and runtime log consistently resolve the product state beneath `~/.gemini`; the XDG strings may belong to dependencies. `ANTIGRAVITY_EXECUTABLE_DATA_DIR` exists as a static string, but nothing verified it as a user-config selector.

Supported provider-related environment variables are different from config-root selection: `GEMINI_API_KEY` selects an API-key credential only when `modelProvider` is set to `gemini` in settings, and `GOOGLE_GEMINI_BASE_URL` changes that provider's endpoint. They do not provide consumer Google multi-account selection.

### How verified

- The requested `find ~ -maxdepth 3 -newer ~/.local/bin/agy -type d` was run, followed by focused inventories of `.gemini`, config, cache, and data locations.
- `find`, `stat`, `readlink`, `jq` schema transforms, and key-name-only extraction were used. No credential value was printed.
- The binary was searched for exact candidate environment names.
- The explicit log showed the effective app-data directory, WSL file-token path behavior, auth method, and successful silent authentication.
- The TUI and harmless print response corroborated live access.
- The [official auth guide](https://antigravity.google/docs/cli/install/) and [settings guide](https://antigravity.google/docs/cli/settings/) specify the same paths.

### Unverified

- Whether changing the ordinary OS `HOME` before process start cleanly relocates **all** agy state was deliberately not tested. Standard library behavior suggests it may, but it is not a documented agy multi-account interface.
- The current CLI's use of the older/shared `~/.gemini/oauth_creds.json` was not proven; the agy-specific token file and runtime log are the stronger evidence.
- No supported consumer account-switch selector or named credential profile was found. The presence of `google_accounts.json.old` does not establish a CLI multi-account switch API.

### Recommendation for taurhaus

Use `~/.gemini/antigravity-cli` as the canonical discovered state root and store only paths/key names, never contents. Do not multiplex consumer accounts by rewriting these files. Until Google documents a selector, isolate accounts with distinct OS users/containers and genuinely distinct home directories. Treat `cli.log` and all explicit logs as sensitive because they contain email, plan tier, workspace paths, request/response IDs, and endpoints; redact before mesh delivery and avoid world-readable exported copies.

## 4. Busy/idle and session identity

### Facts

The strongest supported busy/idle signal for an interactive TUI is the custom status-line or terminal-title command contract. Whenever state changes, agy invokes the configured command and sends JSON on stdin. Relevant fields include:

- `conversation_id` and compatibility alias `session_id`.
- `agent_state`: `idle`, `thinking`, `working`, `tool_use`, or `initializing`.
- `pending_input_count`.
- `tool_confirmation_pending`.
- `task_count` for running background tasks.
- `cwd`, workspace paths, model, version, execution mode, sandbox state, transcript path, token context, quota, account tier, and email.

This is a command callback, not an MCP server. The command is configured through `settings.json` (`statusLine` or `title`) or `/statusline` and `/title`. Because the payload includes email and plan data, the coordinator callback must discard or redact fields it does not need.

For print mode, stream JSON is definitive:

1. One `init` event supplies the conversation ID, cwd, tools, effective permission mode, and optional model/agent.
2. `step_update` events use `state: ACTIVE|DONE`; tool events include tool name/details, and response deltas may arrive in multiple ACTIVE events.
3. Exactly one terminal `result` per turn supplies `status`, response, cumulative duration/turn/usage counters, and the conversation ID.

The local sample is at:

`/tmp/claude-1000/-home-mstie-projects-taurhaus/f3286b16-ffc7-4d16-915d-046705823a3d/scratchpad/agy-stream-json-sample.jsonl`

It contains eight valid JSON lines: two complete harmless four-event traces written by contemporaneous audit probes, with two different conversation IDs. Each trace is `init`, DONE user input, DONE agent response, and SUCCESS result. Consumers can split traces whenever a new `init` appears.

Other possible local signals were weaker:

- `conversation_summaries.db` has `not_fully_idle`, `killed`, `status`, `last_user_input_time`, and step-count columns. During a short interactive turn, its newest row did not change in time to show that turn, so it is not a reliable low-latency primary signal.
- `presence/<conversation-id>.lock` files are zero-byte and persisted after sessions ended. Many stale files accumulated. Filename presence means “seen/registered at some point,” not “currently busy.”
- The explicit log gives detailed transitions (`sending message`, endpoint request, shutdown), but absence of a new log line is not an idle guarantee.
- The agy PID remained mostly in `futex_wait_queue` both while idle and working. It kept numerous TCP sockets while idle; socket counts also changed during startup/model work. `/proc` wchan and fd/socket count do not distinguish working from waiting.
- An interactive tmux pane stayed `pane_current_command=agy`. The pane title remained the host's default (`whocares`) throughout because terminal-title integration was not enabled. No agy-specific title/OSC update was observed. The official `/title on` feature can provide dynamic model/workspace/state titles, but the exact OSC byte sequence was not captured.

Session identity is consistently the UUID from stream `init`, status/title payload, log messages, database filename, transcript directory, and presence filename. A fresh interactive process does not expose that generated ID in argv; a `--conversation=<id>` resume does.

### How verified

- A detached tmux TUI was inspected before and after the harmless prompt. Pane PID, command, title, tty, `/proc` wchan/fds, presence files, and summary SQLite rows were sampled.
- The one-shot process was polled every 50 ms for state, child processes, and socket count.
- The explicit log was tailed with account/query redaction.
- The sample JSONL was validated line-by-line with `jq`.
- The [official status-line schema](https://antigravity.google/docs/cli/statusline/), [title schema](https://antigravity.google/docs/cli/title/), and [headless protocol](https://antigravity.google/docs/cli/headless/) were checked.

### Unverified

- The exact OSC sequence used when `/title on` is enabled is unverified. It is likely a normal terminal title OSC, but that is an inference and should not be encoded without a raw capture.
- Summary-database update latency and whether `not_fully_idle` is authoritative for long background tasks were not established.
- No permission-question or `ask_question` turn was generated, so waiting-for-user rendering was not captured. The official status payload's `tool_confirmation_pending` is the supported signal for permission waits.

### Recommendation for taurhaus

For interactive sessions, install a minimal status-line callback that emits only `{conversation_id, agent_state, pending_input_count, tool_confirmation_pending, task_count}` onto the mesh. This is materially stronger than tmux scraping and remains independent of terminal rendering. For sessions taurhaus owns from birth, prefer the persistent stream-JSON process and drive state from ACTIVE/DONE/result events. Use log/SQLite/presence data only for recovery and post-mortem correlation.

## 5. Transcripts

### Facts

Conversation persistence is both SQLite and JSONL:

- Canonical trajectory database: `~/.gemini/antigravity-cli/conversations/<conversation-id>.db`, usually with SQLite WAL and SHM sidecars.
- Human/tool-oriented transcript: `~/.gemini/antigravity-cli/brain/<conversation-id>/.system_generated/logs/transcript.jsonl`.
- Full variant: adjacent `transcript_full.jsonl`.
- Chunk mirrors: `.system_generated/logs/chunks/transcript/00000000.jsonl` and `chunks/transcript_full/00000000.jsonl` for the short probes.
- Summary/index: `~/.gemini/antigravity-cli/conversation_summaries.db`.
- Cache indexes: `cache/conversation_metadata.json` and `cache/last_conversations.json`.

The short harmless transcript was two JSONL records and 712 bytes. Top-level keys observed across its lines were `content`, `created_at`, `source`, `status`, `step_index`, and `type`; observed types were `USER_INPUT` and `PLANNER_RESPONSE`. The rendered transcript does not need the conversation ID on every line because the containing directory names it.

The per-conversation SQLite schema is not a simple message table. It has `trajectory_meta`, `steps`, `gen_metadata`, `executor_metadata`, `parent_references`, `trajectory_metadata_blob`, and `battle_mode_infos`. Step payload, metadata, error, permission, task, and render fields are protobuf/blob columns; the harmless database had two steps and its `trajectory_meta.cascade_id` equaled the filename/conversation ID. This makes direct SQL transcript decoding brittle.

Mapping ID to cwd/project:

- `conversation_summaries` has `conversation_id`, `workspace_uris`, `project_id`, `app_data_dir`, and `agent_name`.
- `cache/conversation_metadata.json` carries analogous `ID`, `WorkspaceURIs`, `ProjectID`, `AppDataDir`, and `AgentName` fields.
- `cache/last_conversations.json` maps launch cwd strings to the last conversation ID.
- Status-line and hook payloads give the active `conversation_id`, cwd/workspace paths, and transcript path directly.
- The runtime log records both initial workspace directories and “Conversation using project ID.”

Compaction exists internally. Static identifiers include `GetCompactionInfo`, `GetCompactedAtStepIndices`, `applyCompactionInfo`, `isCompactionBoundaryStep`, and `renderCompactionMarker`. The official stream protocol documents `checkpoint` as a visible `step_type`, and changelog entries discuss checkpoints/history truncation. The short samples contained neither compaction nor checkpoint data. `transcript.jsonl` and `transcript_full.jsonl` were identical for them.

### How verified

- Read-only `sqlite3` table/schema/count queries against known harmless conversation IDs.
- `find`, `wc`, and `jq` key/type extraction on known harmless JSONL transcripts; content values were not dumped.
- Current official [hook documentation](https://antigravity.google/docs/hooks) specifies the same CLI transcript path in hook payloads.
- Static compaction identifier extraction and the official [stream event documentation](https://antigravity.google/docs/cli/headless/).

### Unverified

- Whether every SQLite blob field is stable across releases is unverified and unlikely to be a supported integration contract.
- No long conversation compacted during this audit. It is therefore unverified whether a distinct compaction event appears in stream JSON, whether only a `checkpoint` is emitted, and exactly when compact/full transcript variants diverge.
- Some fresh print conversations had null/empty `workspace_uris` in cached summaries even though the log and launch cwd were known. Do not assume that field is always populated.

### Recommendation for taurhaus

Consume the documented `transcript_path` from the status-line/hook payload and tail JSONL by inode/offset. Use `transcript_full.jsonl` for archival if present, but retain the SQLite DB path only as a recovery locator. Never couple taurhaus to numeric `step_type` values or protobuf blobs. Store the tuple `{conversation_id, cwd/workspace, project_id, transcript_path}` at init and refresh it after resume/fork.

## 6. Hooks and notify

### Facts

There is a real lifecycle hook system. Hooks live in `hooks.json` under a workspace customization directory such as `.agents/`, globally under `~/.gemini/config/`, or inside a plugin. `/hooks` lists active hooks. Supported events are:

- `PreToolUse` and `PostToolUse`, with tool-name regex matchers.
- `PreInvocation` and `PostInvocation`, around each model call.
- `Stop`, when an agent execution loop terminates.

Only command handlers are currently supported. On Unix the command is shell-executed, runs with the hooks file's directory as cwd, receives camelCase JSON on stdin, and must return JSON on stdout. Default timeout is 30 seconds and execution is synchronous. Common input includes `conversationId`, `workspacePaths`, `transcriptPath`, `artifactDirectoryPath`, and `modelName`. `Stop` additionally includes `terminationReason`, `error`, and `fullyIdle`; it may request `decision: continue`. `PreInvocation`/`PostInvocation` may inject a `userMessage`, `ephemeralMessage`, or tool call. Tool hooks can allow/deny/ask/force-ask and, where supported, rewrite arguments or grant temporary permissions.

There is no documented `SessionStart`, CLI-process-exit, or compaction hook. `Stop` is execution-loop stop, not necessarily terminal process exit, and can fire while the TUI remains open.

There is no `agy notify` subcommand or `/notify` slash command in installed help/current official command lists. Notification support instead consists of:

- `"notifications": true` in settings: desktop notification plus terminal bell when long work completes or needs attention.
- Custom status-line/title callbacks on state change.
- Hooks for model/tool/stop events.

An internal `CORTEX_STEP_TYPE_NOTIFY_USER` string exists, but it does not establish a public command/tool; it was absent from the harmless stream's public tool list.

Plugins and skills:

- Installed/imported plugins are staged under `~/.gemini/antigravity-cli/plugins/<plugin_name>/` according to current official CLI docs.
- `plugin.json` is required; current schema uses required `name`, optional `description`, and optional `$schema: https://antigravity.google/schemas/v1/plugin.json`.
- A plugin may bundle `mcp_config.json`, `hooks.json`, `skills/`, `agents/`, and `rules/`.
- Manage with `agy plugin[s] list|import|install|uninstall|enable|disable|validate|link`; these mutation commands were not run.
- The bundled customization guide also supports workspace/global discovery under `.agents/` and `~/.gemini/config/`, including `skills.json`/`plugins.json` path registries.
- Skills are Markdown with YAML frontmatter `name` and `description`, plus instructions/resources. Current official CLI docs show `.agents/skills/<name>.md`; bundled/built-in examples also use `skills/<name>/SKILL.md`, which is the layout used by bundled skills and plugin skills.
- Registered skill names become TUI slash commands, for example skill `format-tests` becomes `/format-tests`. Print mode can disable slash-command and skill expansion with `--disable-slash-commands`.
- MCP is a tool integration transport, not the hook or notification transport. MCP configs live globally at `~/.gemini/config/mcp_config.json`, in `.agents/mcp_config.json`, or within a plugin.

### How verified

- The bundled `agy-customizations` skill documentation installed with 1.1.22 was read for hooks/plugins/skills/rules/JSON configs/MCP.
- The live [official hooks page](https://antigravity.google/docs/hooks), [plugins/skills page](https://antigravity.google/docs/cli/plugins/), [settings page](https://antigravity.google/docs/cli/settings/), and installed top-level/plugin help were checked.
- Probe logs reported `loaded 0 named hooks from 0 hooks.json file(s)`, confirming the hook manager is active and that no user hooks ran during the audit.

### Unverified

- The two documented standalone skill layouts (`<name>.md` versus `<name>/SKILL.md`) were not newly created/tested because that would modify customization state. Both are documented by artifacts shipped/current with 1.1.22, but precedence/collision behavior should be tested in a disposable profile.
- No hook was installed for the audit, so actual delivery timing/jitter was not measured.
- No explicit compaction hook was found; a future version could add one.

### Recommendation for taurhaus

Use a status-line callback for continuous session state and, if lifecycle audit events are needed, a narrowly scoped read-only hook that sends only IDs/event names to the mesh. Avoid putting secrets or full tool payloads into hook output. Treat skills/plugins as capability packaging, not coordinator IPC, and keep MCP isolated to the tool slice it serves.

## 7. Delivery

### Facts

The supported non-keystroke message-delivery interface is persistent stream JSON on stdin:

```text
agy --input-format stream-json --output-format stream-json
```

One NDJSON input message per user turn:

```json
{"event":"user","message":{"content":"reply with the single word OK"}}
```

`content` may also be an array of text blocks:

```json
{"event":"user","message":{"content":[{"type":"text","text":"reply with the single word OK"}]}}
```

Only text blocks are supported. One `init` event is emitted for the process, every input turn gets its own `result`, and the conversation ID stays constant. The next prompt should be written only after the prior `result`. Closing stdin is the graceful end-of-session signal; agy completes the current turn, emits its final result, and exits `0` on a clean session.

Validation behavior is explicit:

- Invalid JSON or missing `event`: ERROR and process exit `1`.
- `control_request`/`control_response`: ERROR and exit `2`; they are expressly unsupported.
- CLI-handled slash commands such as `/model` or `/usage`: ERROR and exit `2` in streaming input mode.
- Unknown future event names: skipped with a stderr warning.
- A prompt passed with `-p` is dropped in stream-input mode; all messages must come through stdin.

For an already-running **interactive TUI**, no documented local attach/send socket or command exists. tmux keystrokes remain the direct local fallback. Hooks can inject messages only when their configured lifecycle event is already firing; they are not an arbitrary mailbox.

Every process opens private random loopback HTTPS/gRPC and HTTP language-server ports. Static strings include address and CSRF concepts. These listeners are implementation IPC and no supported external authentication/API contract was found. Connecting to them was intentionally not attempted.

Remote Control is a separate supported cloud-mediated feature. A normal probe log said `[RemoteControl] CLI launched without --remote-control, staying disconnected`. Official docs describe a headless Remote Control daemon and a browser hub that can view conversations and start agent tasks. It requires separate setup/sign-in and was not installed or enabled here. It is not a simple local injection socket for an ordinary `agy` TUI.

### How verified

- Installed `--help` text and the current [official stream-input guide](https://antigravity.google/docs/cli/headless/) provide the exact message schema and error behavior.
- A harmless stream-output probe validated the matching output side and conversation identity.
- `ss` plus explicit logs verified the two loopback listeners and the disconnected Remote Control state.
- Static strings verified the presence of internal language-server/CSRF/remote-control identifiers without exposing values.
- The [official Remote Control page](https://antigravity.google/docs/remote-control/) was checked; no installer/setup command was executed.

### Unverified

- The private HTTP/gRPC language-server protocol, authentication, and whether it contains a message method are unverified and unsupported for external use.
- The hidden-looking `--remote-control` contract is not in `agy --help`; a full Remote Control daemon/session was not launched.
- No supported mechanism was found to attach stream-json stdin to a TUI process that was not originally launched in stream-input mode.

### Recommendation for taurhaus

When taurhaus needs programmatic delivery, own the process from launch and use one long-lived stream-json process per conversation. Frame stdin writes as complete NDJSON lines, wait for each `result`, and close stdin for graceful shutdown. For pre-existing TUIs, use tmux keystrokes or require an explicit future Remote Control integration; do not reverse-engineer the localhost language-server ports.

## 8. Stop

### Facts

Interactive graceful stop options:

- `/exit`; `/quit` is its alias. `/exit` was live-tested and ended the tmux session cleanly.
- Current official 1.1.22 docs say `Ctrl+D` exits only when the prompt is empty; with non-empty prompt it is forward-delete.
- `Ctrl+C` is the protected `cli.exit` action and terminates the CLI, prompting for confirmation when the agent is working.
- `Esc` closes panels, clears an empty prompt, or halts an active stream; use this to interrupt a turn while keeping the TUI.

The bundled offline `antigravity_guide` still says `Ctrl+D Ctrl+D`, which conflicts with the current live 1.1.22 reference's single empty-prompt `Ctrl+D`. `/exit` is unambiguous and was verified.

Headless/stream stop options:

- One-shot print exits after its terminal result.
- Stream-input mode: close stdin; current turn completes and the process exits cleanly.
- SIGINT is represented by terminal result status `INTERRUPTED` when an envelope can be produced. Other documented terminal statuses are `SUCCESS`, `ERROR`, `CANCELED`, `INVALID`, `WAITING`, and `RUNNING`.

### How verified

- `/exit` was sent to the scratch tmux session; the tmux session ended.
- The current [CLI keybinding reference](https://antigravity.google/docs/cli/reference) and [headless guide](https://antigravity.google/docs/cli/headless/) were checked.
- The installed bundled guide supplied the conflicting older double-`Ctrl+D` note.

### Unverified

- Actual single-versus-double `Ctrl+D` behavior in this installed binary was not live-tested after the documentation conflict was discovered.
- Ctrl+C during an active permission dialog/background-task state was not exercised.

### Recommendation for taurhaus

Use protocol-native termination: close stdin for stream sessions and send `/exit` through tmux for interactive sessions. Use `Esc` for “cancel current turn but keep session.” Reserve SIGTERM/SIGKILL for timeout recovery after a grace period and record the absence of a terminal `result` as an abnormal stop.

## 9. Usage, quota, and accounts

### Facts

Quota/usage surfaces:

- `/usage`, alias `/quota`, refreshes model configuration and quota status from the backend and opens the Model Quotas panel.
- The panel shows per-model remaining requests/tokens and refreshes current data.
- `/credits` opens G1/AI credit information and purchase/upgrade links.
- The built-in status line can show credits/quota. A custom status-line payload includes a `quota` map whose buckets expose `remaining_fraction`, `reset_time`, and `reset_in_seconds`.
- Stream `result.usage` gives token accounting (`input_tokens`, `output_tokens`, `thinking_tokens`, `cache_read_tokens`, `total_tokens`) but this is run consumption, not remaining quota.
- Version 1.1.21 added a status-line `cost` field according to the installed changelog, although the currently rendered official schema page did not list it in its table; treat it as version-specific optional data.
- Logs show quota refresh operations and rate-limit/backend errors, but should not be the primary quota API.

The machine is authenticated to one active consumer Google account, plan shown as Google AI Pro. The account email was deliberately not recorded. `google_accounts.json` has one active email-shaped string and an empty `old` array. No account-switch flag, named consumer profile, or supported config-root selector was found.

Supported alternative identity paths include:

- Consumer account silent keyring/file token (current route).
- Gemini API key: `modelProvider: gemini` plus per-process `GEMINI_API_KEY`; optional `GOOGLE_GEMINI_BASE_URL`.
- Enterprise/ADC-related paths exist elsewhere in the product, but no consumer multi-account selector was verified.

Observed live endpoint calls in the explicit harmless-run log were to `https://daily-cloudcode-pa.googleapis.com/` methods `v1internal:loadCodeAssist`, `v1internal:fetchAvailableModels`, and `v1internal:streamGenerateContent` (query removed). Static binary endpoints also include Google OAuth, `aicode.googleapis.com`, `generativelanguage.googleapis.com`, an Antigravity Unleash service, an Antigravity auto-updater service, and the raw GitHub changelog URL. Static presence means “possible route,” not “called by this probe.”

### How verified

- TUI header, successful prompt, `agy models`, auth log, and credential-file schema (never values).
- Explicit log endpoint and quota-manager lines with identity/query redaction.
- Current [quota command docs](https://antigravity.google/docs/cli/commands/usage/), [credits docs](https://antigravity.google/docs/cli/credits/), [status-line schema](https://antigravity.google/docs/cli/statusline/), and [auth docs](https://antigravity.google/docs/cli/install/).
- Exact binary-string host/environment-name checks.

### Unverified

- No noninteractive supported command was found that prints **remaining** quota without going through a TUI-handled `/usage` report or status-line callback.
- Multi-consumer-account switching within one OS home is unverified and appears unsupported.
- Static URL strings do not prove which routes are active for every auth provider/model.

### Recommendation for taurhaus

Ingest quota from the status-line callback and token usage from stream results, keeping them as separate metrics. Associate each session with a redacted account fingerprint held outside agy; never use or copy token files. For multi-account operation use isolated OS identities/containers until Google documents a profile selector. Rate-limit routing should react to structured stream errors/status plus quota data, not scrape prose logs.

## 10. Versioning

### Facts

Programmatic version read:

```text
$ ~/.local/bin/agy --version
1.1.22
```

There is no separate `version` subcommand in help. The binary hash/build ID above can pin the exact artifact beyond SemVer.

The head of `agy changelog` is 1.1.22. It reports:

1. `/model <name>` can switch by name/slug/label and save the default, with completion help.
2. `/effort` completion now follows typed text.
3. Artifact filesystem event bursts are coalesced.
4. Gemini 3.1 Pro/3.5 Flash effort selection was fixed for Gemini API-key auth.
5. Idle redraw CPU use while task/subagent panels are open was reduced.
6. A running subagent timer no longer freezes when the parent waits.
7. HTTP 502 model endpoint failures are retried instead of ending the run.
8. `self` subagents now inherit their parent's configuration more consistently.
9. Windows file-deletion sharing violations get a short retry/backoff.
10. The built-in `migrate-workflows` skill now handles Windows paths.

`agy changelog` exited `0` and populated/used `~/.gemini/antigravity-cli/cache/CHANGELOG.md`. Its static source URL is the Google Antigravity CLI GitHub changelog, and the official web changelog is [antigravity.google/changelog](https://antigravity.google/changelog).

Automatic updater state currently says:

```json
{"success":true,"message":"Already on the latest version."}
```

The updater uses `~/.gemini/antigravity-cli/last_check.timestamp`, `updater/update.lock`, and `updater/update_status.json`. Official troubleshooting documents a 15-minute check debounce and `AGY_CLI_DISABLE_AUTO_UPDATE=true` to opt out. The binary contains the auto-updater service hostname. Neither `agy update` nor `agy install` was run.

No explicit update-channel selector (stable/beta/nightly/canary) was found in top-level/update help or updater state. Generic `stable`, `nightly`, `canary`, and `preview` strings occur throughout the large binary and are not sufficient evidence of a CLI release-channel contract.

### How verified

- Installed `--version`, binary metadata/hash, `agy changelog`, changelog cache, updater status/lock/timestamp, and exact `AGY_CLI_DISABLE_AUTO_UPDATE` string.
- Current [official troubleshooting docs](https://antigravity.google/docs/cli/troubleshooting/) for updater timing/opt-out.
- The mutation-capable install/update commands themselves were never run.

### Unverified

- The active named release channel is unverified; no channel field or flag was found.
- Whether the background updater replaces the binary atomically and how it reports rollback/failure was not exercised.
- `go version -m` cannot programmatically report module/version metadata for this internal build; use `agy --version` plus hash instead.

### Recommendation for taurhaus

At process discovery run `agy --version` once, require a supported SemVer range, and cache the binary SHA-256. For reproducible mesh behavior, set `AGY_CLI_DISABLE_AUTO_UPDATE=true` in the managed process environment and roll upgrades deliberately after protocol regression tests. Monitor `updater/update_status.json` only as advisory state, not as the version source.

## Compact JSON summary

```json
{
  "process_signature": {
    "exe": "~/.local/bin/agy",
    "interactive": "no print marker/subcommand, or -i/--prompt-interactive",
    "print": "-p/--print/--prompt, or --input-format stream-json",
    "subcommands": ["agent", "agents", "models", "changelog", "help", "mcp", "plugin", "plugins", "mic-serve", "install", "update"],
    "binary": "208429312-byte stripped PIE; Go-based/cgo, dynamically linked; in-process language server opens random localhost HTTPS-gRPC and HTTP ports"
  },
  "launch_flags": {
    "model": "--model",
    "effort": "--effort low|medium|high",
    "auto_approve": "--dangerously-skip-permissions => always-proceed",
    "persistent_auto_approve": "settings.json toolPermission=always-proceed; prefer scoped permissions.allow",
    "fresh": "omit --continue/--conversation",
    "continue": "-c|--continue",
    "resume": "--conversation <uuid>",
    "initial_interactive": "-i|--prompt-interactive",
    "sandbox": "--sandbox"
  },
  "config_dir": "~/.gemini/antigravity-cli (shared customizations/projects under ~/.gemini/config)",
  "selector_env": null,
  "identity": "signed in via consumer file-token storage on WSL; active account key is google_accounts.json.active; email redacted",
  "busy_idle": "use statusLine/title JSON agent_state for TUI; use stream-json ACTIVE/DONE/result for headless; do not use presence locks, wchan, or socket count",
  "transcripts": "SQLite conversations/<id>.db plus brain/<id>/.system_generated/logs/{transcript,transcript_full}.jsonl",
  "hooks": ["PreToolUse", "PostToolUse", "PreInvocation", "PostInvocation", "Stop"],
  "delivery": "supported: NDJSON user events on stdin with --input-format stream-json --output-format stream-json; no documented attach API for an existing ordinary TUI",
  "stop": "interactive /exit (/quit); stream close stdin; Esc cancels active stream; current docs say Ctrl+D on empty prompt and Ctrl+C exits/asks if working",
  "usage": "stream result token usage; /usage|/quota for model quota; /credits for AI credits; quota also in status-line payload",
  "report_path": "/tmp/claude-1000/-home-mstie-projects-taurhaus/f3286b16-ffc7-4d16-915d-046705823a3d/scratchpad/agy-report-codex.md",
  "unverified": [
    "supported per-process config/home selector or consumer account profile selector",
    "private localhost language-server API",
    "exact OSC title bytes",
    "exact externally visible compaction event",
    "new-project storage allocation",
    "single-vs-double Ctrl+D runtime behavior due bundled/live-doc conflict",
    "named update channel"
  ]
}
```
