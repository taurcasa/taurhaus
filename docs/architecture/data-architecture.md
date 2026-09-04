# Data Architecture

This document is the authoritative data architecture reference for Taurhaus as of `v0.5.9+`.

It complements [data-model.md](./data-model.md), which still covers the SQLite schema and search index in detail, but does not fully describe the live coordination/runtime filesystem model that Taurhaus now depends on.

This document is the authoritative active reference for:

- current persistent-store inventory
- ownership boundaries and authority levels
- coordination filesystem truth vs derived state

Neighboring docs have narrower jobs:

- [`ARCHITECTURE.md`](../../ARCHITECTURE.md): top-level system overview and module map
- [`../coordination-architecture.md`](../coordination-architecture.md): coordination design decisions, invariants, and runtime behavior
- [`data-model.md`](./data-model.md): SQLite schema and search-index structure

## Scope

This document covers:

- persistent data stores Taurhaus reads or writes
- ownership boundaries between Taurhaus, `mesh`, and external CLI tools
- which stores are authoritative vs derived
- the logical member/session/transcript model
- the key runtime data flows
- documentation gaps found during the audit

It does not attempt to restate every table column or every IPC command. Use [data-model.md](./data-model.md) and [ipc-reference.md](./ipc-reference.md) for that level of detail.

## Core Rules

Current implementation follows these rules:

1. Live coordination truth is filesystem-first, not SQLite-first.
2. Team membership and role composition are durable facts in `teams/<team>/config.json`.
3. Current attachment state is durable-but-rebuildable in `teams/<team>/runtime/<member>.json`.
4. Scanner output is observation, not authority.
5. Compaction delivery state and operational snapshot files are derived/supporting state, not roster truth.
6. Tool transcript files remain external source material owned by the CLI that wrote them (Claude Code, Codex, Antigravity, Grok), not by Taurhaus.

## App and Daemon Protocol Pairing

The current daemon protocol is **24** (`daemon/protocol.rs`). Protocols 11–14 generalized accounts and the harness vocabulary; 15–22 moved coordination operations and every team-state write into the daemon; 23 added stable member account ids and the daemon-owned selector-account switch run; 24 added per-team root authority and Claude team account switching. App and daemon reject a mismatched peer instead of attempting partial compatibility.

## Storage Inventory

### 1. App-Owned Persistent State

| Store | Path | Format | Primary writer | Primary readers | Authority level | Notes |
|---|---|---|---|---|---|---|
| Main app DB | `app_data_dir()/taurhaus.db` | SQLite | Taurhaus backend | Taurhaus backend | Authoritative for app metadata/history | Initialized in `src-tauri/src/db/mod.rs`; WAL + FK enabled. `project_tool_accounts` stores one `pinned` or `last_used` account id per project and tool |
| Search index | `app_data_dir()/search_index/` | tantivy index dir | Taurhaus backend | Taurhaus backend | Derived / rebuildable | Projection of filesystem + commits + sessions |
| Structured log | `app_data_dir()/taurhaus.log.jsonl` | JSONL + rotated segments | Taurhaus frontend/backend | Humans, scripts, diagnostics | Audit/telemetry | Async append-only sink from `commands/logging.rs` |
| Template storage | `app_data_dir()/templates/` or `<TAURHAUS_DATA_DIR>/templates/` | YAML + git metadata | Taurhaus backend | Taurhaus backend + UI | Authoritative for user templates | Separate from coordination state |
| Codex notify sink | `app_data_dir()/codex-notify.jsonl` | JSONL | `taurhaus-daemon codex-notify` | Codex idle resolver | Derived edge log | Bounded at 5 MB; one record per Codex turn-complete notification |

### 1b. Hook Installations in Tool-Owned Files

| Store | Path | Primary writer | Notes |
|---|---|---|---|
| Claude compact hook | `<claude_dir>/settings.json` + `<claude_dir>/hooks/taurhaus-session-start-compact.{sh,cmd}` + `hooks/taurhaus-session-start-compact.executable` | Taurhaus | A taurhaus-authored entry inside a Claude-owned settings file. The `.executable` marker records the app exe path so the wrapper is re-installed when it moves |
| Codex compact hook | `<CODEX_HOME>/hooks.json` + `<CODEX_HOME>/hooks/…` | Taurhaus | Installed and reconciled by default for managed Codex >= 0.147; the taurhaus entry is idempotently repaired without replacing foreign hooks |

### 2. Coordination Shared State Under Claude Account Roots

`PlatformPaths::teams_dir()` remains the default authority: `claude_dir()/teams`, where `claude_dir()` is the `TAURHAUS_CLAUDE_DIR` override, else the WSL-UNC `~/.claude` on Windows, else `~/.claude`. Before locating a team by name, the process-wide `CoordinationState` consults the daemon-owned registry under that default account root. A missing registry or team entry resolves byte-identically to the historical default path and creates nothing. Root-keyed orchestrators preserve one lock domain per teams root, while enumeration consumers scan the default root plus every registered root.

`TAURHAUS_CLAUDE_DIR` is a taurhaus-only variable. Claude Code itself reads `CLAUDE_CONFIG_DIR`; see [path-handling-guide.md](./path-handling-guide.md) for how a launch bridges the two.

| Store | Path | Format | Primary writer | Primary readers | Authority level | Notes |
|---|---|---|---|---|---|---|
| Team-root registry | `<default claude dir>/.taurhaus/team-roots.json` | JSON | Taurhaus daemon | Taurhaus daemon + app readers | Authoritative bootstrap routing | Schema v1 maps `teamName → teams_dir`. Default-root teams have no row. Writes take `team-roots.lock` and publish by temp-file rename; Claude switches move and verify team state before this authority changes |
| Team config | `teams/<team>/config.json` | JSON | Taurhaus + mesh-compatible tooling | Taurhaus, mesh, UI/backend | Authoritative logical roster | Members, roles, role metadata, project path, tool, `model`, `reasoningEffort` (separate field). Team and member objects carry `#[serde(flatten)] extra`, so mesh-owned keys (`controlAuthTokenHash`, `lastActivityAt`/`Reason`, `statusState`/`Reason`/`SetAt`, `isActive`, unknown future keys) survive every taurhaus save |
| Member runtime | `teams/<team>/runtime/<member>.json` | JSON | Taurhaus + mesh | Taurhaus, mesh | Authoritative current attachment | Schema v3: the record carries its own `schema_version` (stamped from `RUNTIME_SCHEMA_VERSION` on every save, defaulted on read), `member_name`, pane id, `pane_pid`, `pane_start_time`, session id, transcript path, `cli_tool`, `project_path`, daemon pid, health, `delivery_lease`, `attached_at`, `last_seen_at`, `appliedEffort`, `effort_resume_failure`, and the flattened launch-account result (`accountApplied`, `accountNote`, `accountNoteDetail`). The record's own keys are snake_case, and the decoder accepts a camelCase alias for each explicitly aliased operational field (`deliveryLease`, `attachedAt`, `lastSeenAt`, `effortResumeFailure`, `accountApplied`, … — `coordination/stores/runtime.rs:647-683`); `schema_version` and `member_name` have no alias and are read in snake_case only. Below the record, casing is per struct rather than uniform. `DeliveryLease` carries no rename, so its keys stay snake_case — `owner_pid`, `instance_uuid`, `hostname`, `heartbeat_at`, `started_at` — and none of them is defaulted (`coordination/domain.rs:76-84`); a lease written in camelCase does not merely lose fields, it fails to parse and takes the whole record with it, which the commit gate then reports as the `record` sentinel (`coordination/stores/runtime.rs:313-331`). The one camelCase nested struct is `effort_resume_failure`, which holds `taskId`, `level`, `attempts` and a terminal `reason` (`coordination/stores/runtime.rs:87-101`); there a `task_id` spelling is silently dropped on read and leaves `taskId` empty, which the retry gate reads as the pre-task-identity record that matches every task *that asks for the level the record names*: the gate pairs the task match with an ASCII-case-insensitive level match, so one task's spent attempts would bound the next task's only where both request that same level (`coordination/pipelines/effort.rs:633-637`). The launch-account result is not nested at all: `LaunchAccountResult` is flattened onto the record (`coordination/stores/runtime.rs:80-82`), so `accountApplied`, `accountNote` and `accountNoteDetail` are top-level camelCase keys. `appliedEffort` is a first-class field with **two** writers: mesh writes it before typing `/effort` into the pane, and a taurhaus launch seeds it from the effort its own command carried. Liveness has one narrow exception: replacing an existing session id with a detected foreign id clears `appliedEffort` in the same compared commit; the first id capture after taurhaus's own launch preserves the seeded level. A save that owns nothing about the level re-reads it under the target-file lock and carries it forward. Other mesh-owned and unknown keys survive through the flattened `extra` map and merge-on-save. Taurhaus probes without a lock, then commits through `commit_if_unchanged`: team lock → target-file lock → re-read → compare → mutate → atomic save. A moved dependency skips the commit and names what moved (`pane_id`, `pane_pid`, `pane_start_time`, `session_id`, `daemon_pid`, `health`, `appliedEffort`, or the sentinel `record` when the file appeared, vanished or would not parse) |
| Mesh inbox | `teams/<team>/inboxes/<member>.json` | JSON array | Taurhaus (`MeshInboxStore::append`), mesh | Taurhaus, mesh daemons/agents | Authoritative message queue for file-based delivery | Shared protocol surface; single taurhaus writer (see below). A successful append is reported independently as `delivered: true` and `durable: true`; the wake disposition and any `post_write_warnings` (an operational-snapshot or runtime-record update that failed after the append) are separate facts and cannot trigger a second append |
| Operational snapshot | `teams/<team>/state/operational/<member>.json` | JSON | Taurhaus | Taurhaus reinjection/delivery | Derived contextual snapshot | Current task, assignment footer, working set, override state, and task-deadline markers. Refreshes re-read those markers under the team lock before saving, so a stale refresh cannot erase a committed one-shot action |
| Member activity snapshot | `teams/<team>/state/activity/<member>.json` | JSON | Daemon session hub (`coordination/activity_export.rs`), on every activity change or when the 30 s refresh is due | mesh idle monitor, Taurhaus deadline pass | Derived | Schema v1 (`coordination/activity_schema.rs`): `stall_recent_activity`, `stall_no_output`, `stall_no_active_process`, `active_non_shell_process`, `recent_io`, `pane_alive`, `pane_foreign`, `last_output_age_secs`, `activity_confidence`. The shared typed reader treats the exporter's `active` and `likely_working` verdicts as fresh work for deadline-nudge suppression. A focus move is not activity and writes nothing. Skipped for teams with no live pane; degraded scan cycles export nothing |
| Team-daemon credential | `teams/<team>/state/control_auth/<member>.json` | JSON | mesh | mesh | mesh-owned | Taurhaus reads nothing from the file. Before `mesh team-daemon start` it checks three gates and logs `coordination.team_daemon.skipped` with the first failing `reason`: file present (`missing_lead_control_credential`), lead's `config.json` carries a non-empty `controlAuthTokenHash` (`missing_lead_control_auth_token_hash`), lead is not `isActive: false` (`inactive_lead_control_identity`) |
| Seam leases | `teams/<team>/state/leases/<name>.json` | JSON | mesh | mesh, Taurhaus (read-only, best-effort) | mesh-owned | Compaction cards and resume onboarding list the member's held/handback-ready seams and waiting positions. Unlocked filename-keyed reads: oversized and zero-byte records skipped (zero-byte is mesh's first-acquire transient), unreadable records warn-skipped, an absent dir composes the card exactly as before |
| Member compaction state | `teams/<team>/state/compaction/<member>.json` | JSON | Taurhaus | Taurhaus | Derived idempotency/audit state | Last compaction handled + terminal result |
| Routing telemetry | `teams/<team>/state/telemetry/<task_id>.jsonl` | JSONL | Taurhaus daemon / WSL-native hook process | `just routing-report` | Derived observational history | Append-only per-task launch, effort, deadline, and completion observations. `_unattributed.jsonl` holds only task-less launches and is rewritten under lock to retain the newest launch per member |

#### Telemetry (Stage 1)

Routing telemetry is observational only. It is appended beside operations the
daemon already performs and is never consulted by launch rendering, assignment,
effort, deadline, or scheduling decisions. The app process does not write these
files. A write failure produces one process-bounded warning and never changes
the result of the wrapped operation.

Each team root stores small per-task sidecars at
`teams/<team>/state/telemetry/<task_id>.jsonl`; they do not rotate. The special
`_unattributed.jsonl` file is capped by rewriting it under its existing lock to
keep only the newest task-less launch per member. Readers are tolerant: a
missing file, an individual corrupt or partially written line, or a sidecar
over the 8 MiB cap is skipped; the oversized case emits one process-bounded
warning. The event vocabulary is:

- `launch_rendered`: member, role, tool, and the model and applied effort from
  `RenderedLaunch`, plus the model catalog's capability tier and rank.
- `effort_switch`: the existing assignment-effort outcome, attempt number, and
  previous/requested effort.
- `nudge_sent` and `task_staled`: the already-committed deadline action and its
  deadline fields.
- `completion_observed`: a terminal status seen by the daemon task scanner and
  whether that parsed ledger record carried a review ruling. Its `timestamp`
  is the task's state-change time (falling back to task update time, then scan
  time); `observed_at` separately records when the daemon scan saw it.

The operational snapshot normally identifies the one task held by a member at
the launch seam. When it does not, the rendered launch is retained in
`_unattributed.jsonl` with `task_id: null`. If a later daemon-owned snapshot
first names a task for that running member, taurhaus copies the member's latest
render-authoritative launch fields into the task sidecar at that attribution
time. Thus a launch-once member's later work appears in the report without
inventing a requested model or requiring another relaunch.

`just routing-report [DAYS]` (30 days by default) enumerates the default and all
registered team roots, tolerantly reads the sidecars, and rejoins every task to
the current mesh ledger record. It prints per `(role, model)` rows and a
per-model rollup with tasks touched, accepted, completed-but-unruled,
relaunches, completed effort switches, nudges, stale actions, and median elapsed
time from first render to the terminal state-change timestamp. Acceptance follows Amendment
4 exactly: only ledger status `completed` with a sequenced review ruling counts.
A bare completed status is `completed_unruled`, never accepted. Tokens are not
collected in Stage 1; the report header identifies wall-time as the cost proxy.
Until mesh writes `metadata.rulings`, `accepted` remains zero and completed
records appear under `completed_unruled`.

### 3. External Tool Data Taurhaus Observes But Does Not Own

| Tool | Data | Typical path | Ownership | Taurhaus role |
|---|---|---|---|---|
| Claude Code | session transcripts | `~/.claude/projects/<slug>/*.jsonl` | Claude Code | observe/parse |
| Claude Code / mesh | task files | `~/.claude/tasks/{source-key}/*.json` | Claude Code / mesh, plus Taurhaus deadline pass for one field | observe/import; deadline pass may set `status` to `stale` |
| Claude Code | sessions registry | `<CLAUDE_CONFIG_DIR>/sessions/<pid>.json` | Claude Code | observe — **authoritative** session identity and `busy`/`idle`/`waiting`/`shell` state |
| Claude Code | account config dirs and OAuth usage | `~/.claude`, `~/.claude-*` (`.credentials.json`, `.claude.json`) | Claude Code | observe identities and read a token at request time for the native usage endpoint; taurhaus never writes, persists, logs, or refreshes credentials |
| Codex | session transcripts | `~/.codex/sessions/YYYY/MM/DD/*.jsonl` | Codex | observe/parse |
| Antigravity CLI | conversations | `~/.gemini/antigravity-cli/conversations/*.db` + `cache/last_conversations.json` | Antigravity CLI | observe (identity only; the SQLite transcript is never parsed) |
| Antigravity CLI | presence locks | `~/.gemini/antigravity-cli/presence/*.lock` | Antigravity CLI | observe (advisory flock) |
| Antigravity CLI | activity hooks | `<app data>/agy-hooks.jsonl` | taurhaus (hook sink) | own/append |
| Antigravity CLI | hook registrations | `~/.gemini/config/hooks.json` | shared with the user and the TUI | merge one `taurhaus` entry by name; write through a symlinked target |
| Grok CLI | live session registry | `<GROK_HOME>/active_sessions.json` | Grok CLI | observe/parse |
| Grok CLI | turn lifecycle | `<GROK_HOME>/sessions/<encoded-cwd>/<session-id>/events.jsonl` | Grok CLI | observe/tail |
| Grok CLI | compaction hooks | `<GROK_HOME>/hooks/*.json` | taurhaus (managed installer) | own/write |

These files are not part of Taurhaus master data. They are upstream evidence sources used to derive runtime/session/task state, with one narrow deadline exception: Taurhaus may change a mesh task's `status` from `in_progress` to `stale`. That compare-and-write takes the task file's target lock, preserves unknown fields, validates the bounded record's id and owner, and abandons the write if mesh already moved the status on.

The sessions registry is resolved per process, not per app: taurhaus reads the registry-declared account selector of that pid (`/proc/<pid>/environ` on Linux, `ps -Eww` on macOS) and guards the record with `procStart`, so subscriptions running side by side never read each other's state. Account memory is recorded in SQLite as `project_tool_accounts(project_id, tool, account_id, origin)`; `origin` is `pinned` or `last_used`. The legacy `projects.claude_account_id` column remains only for compatibility after migration 013: it is no longer used or written, but the project queries still select it and decode it into a discarded `_legacy_claude_account_id` binding so the row shape stays stable (`db/queries.rs:15`, `:56`, `:77`).

Usage snapshots and OAuth tokens are intentionally absent from this inventory: snapshots live in process memory only, and tokens exist only inside one provider fetch call. Polling uses injected fake HTTP clients in tests and never contacts a live endpoint there.

## Authoritative Answers

Current implementation answers common questions from different stores:

| Question | Canonical store |
|---|---|
| Who belongs to team `X`? | `teams/<team>/config.json` |
| What role/context does member `Y` have? | `teams/<team>/config.json` |
| What tool/model is member `Y` configured to use? | `teams/<team>/config.json` |
| Which pane is member `Y` attached to right now? | `teams/<team>/runtime/<member>.json` |
| Which session/transcript is member `Y` attached to right now? | `teams/<team>/runtime/<member>.json` |
| Which tool/project does member `Y` currently attach to? | `teams/<team>/runtime/<member>.json` |
| What message queue should member `Y` read? | `teams/<team>/inboxes/<member>.json` |
| What was the last handled compaction for member `Y`? | `teams/<team>/state/compaction/<member>.json` |
| What is the current task/working set used for post-compaction reinjection? | `teams/<team>/state/operational/<member>.json` |
| What files/commits/tasks belong to a registered project? | SQLite + search index + filesystem/tool sources |

## Logical Data Model

### Team and Member Model

```text
Team
  -> logical members (config.json)
      -> role / context-steering metadata
      -> project identity
      -> tool + model defaults
  -> shared inboxes
  -> per-member runtime attachments
      -> pane_id + pane_pid + pane_start_time
      -> session_id
      -> jsonl_path
      -> cli_tool
      -> project_path
      -> daemon_pid
      -> health
  -> per-member operational context snapshots
  -> per-member activity snapshots (daemon-exported)
  -> per-member compaction delivery state
```

### Project and Content Model

```text
Filesystem project root
  -> source files (truth)
  -> git history (truth)
  -> tool task/session files (external truth)
  -> Taurhaus SQLite rows (projection / metadata / history)
  -> Tantivy docs (search projection)
```

### Important Separation

There are three different concepts that often get conflated:

1. `logical member`
   - durable identity on the team
   - lives in `config.json`
2. `runtime attachment`
   - which pane/session/transcript currently realizes that member
   - lives in `runtime/<member>.json`
3. `scanner observation`
   - what the current process/session scan sees right now
   - ephemeral, computed at runtime

Native compaction hooks resolve their logical member from authoritative runtime attachment state: `session_id` first, then normalized project path. Runtime records persist `cli_tool`, `project_path`, and `jsonl_path`, and config + runtime join through the shared roster view (`get_team_roster_with_attachments()`).

## Ownership Boundaries

### Taurhaus Owns

- SQLite schema and all app metadata/history
- search index lifecycle
- structured log sink
- coordination orchestrator logic
- `runtime/` attachment state
- operational snapshots
- compaction hook processing (one bridge, `coordination/compact_hook.rs`, for Claude, Codex and Grok), managed hook reconciliation, and delivery bookkeeping through `record_delivery_at(teams_dir, …)`
- the single inbox writer `MeshInboxStore::append` for every taurhaus-originated message
- member launch/liveness ownership checks and authoritative `runtime/` pane identity
- UI projections and diagnostics

### mesh Owns

- file-based messaging protocol semantics
- member daemon behavior, including revalidating pane existence and configured `cli_tool` immediately before tmux injection
- team-daemon behavior
- mesh-native status/activity fields in shared config space

### Shared / Compatibility Surface

- `teams/<team>/config.json`
- `teams/<team>/inboxes/`

These files must remain compatible across Taurhaus and mesh. Taurhaus owns the snake-case `cli_tool` member extension used by its runtime and writes it into `config.json`; mesh consumes that field only as a delivery safety check. Taurhaus-specific attachment semantics remain in `runtime/` and should not migrate into shared config unless mesh truly consumes them semantically.

**Taurhaus write discipline on the shared surface:**

- `config.json` — taurhaus patches only its own authored keys, under the team lock, tmp+rename. Everything else round-trips through `#[serde(flatten)] extra`.
- `inboxes/<member>.json` — exactly one taurhaus writer, `MeshInboxStore::append`: a per-target `flock` with an inode re-check (`TargetFileLock`), read-modify-write, then tmp+rename while the lock is still held. `externalRelay` and unknown message keys are preserved via `extra`; a corrupt inbox is quarantined to `<member>.json.corrupt.<ts>` and logged as `mesh.inbox.corrupt` (warn).

Taurhaus inbox producers route through that writer — operator notices for bridged members, operator notices for Claude members, and grok compaction cards. Operator-originated traffic is sent as `taurhaus` (`MeshInboxMessage::operator_originated`) and reports `DeliveryMethod::InboxFile` truthfully; no `mesh send` sender-candidate chain remains. Claude and Codex compaction cards return on native hook stdout and do not touch the inbox.

## Key Data Flows

### 1. Team Initialize / Composition

```text
Template storage / customizer UI
  -> coordination initialize request
  -> compose logical roster
  -> write teams/<team>/config.json
  -> launch/attach members
  -> write runtime/<member>.json
  -> write initial operational snapshots
  -> start member daemons / team daemon as needed
```

Key point:
- `config.json` is written first as durable logical definition.
- `runtime/` is then filled with current attachment facts.

### 2. Runtime Session Observation

```text
process / tmux / transcript scanners
  -> RuntimeSession observations
  -> orchestrator reconciliation
  -> runtime/<member>.json updates
  -> live status IPC / UI projections
```

Key point:
- scanner results do not replace the roster
- they inform or repair runtime attachment state

### 3. Codex Compaction Detection and Reinjection

```text
managed Codex >= 0.147
  -> startup/settings reconciliation installs the managed SessionStart(compact) hook
  -> Codex automatic compaction invokes the runtime-appropriate wrapper
  -> compact_hook.rs resolves team/member via runtime attachment
  -> build reinjection card from operational snapshot + role/config metadata
  -> return hookSpecificOutput.additionalContext on stdout
  -> record delivered / skipped / failed in per-member compaction state
```

Key point:
- the native Codex hook is the only detection path; transcripts are not tailed for compaction
- the authoritative member binding is runtime attachment, not transcript path heuristics

### 4. Post-Compaction Reinjection

```text
operational snapshot + role/config metadata + runtime attachment
  -> build reinjection card
  -> hook path (`compact_hook.rs`, Claude and supported managed Codex)
     or Grok hook path -> MeshInboxStore::append -> member mesh daemon wakes the pane
  -> record terminal delivery result via record_delivery_at(teams_dir, tool, ...)
```

Key point:
- operational snapshots are supporting context only
- they do not define membership or session attachment
- taurhaus performs no tmux injection on this path

**One hook bridge, one detection path.** `coordination/compact_hook.rs` serves Claude, Codex and Grok from a single payload parser: the tool is inferred from the reserved `GROK_*` hook env and otherwise from `transcript_path`, and the member is resolved by runtime `session_id` first and normalized `cwd` second. The registry decides where the composed card goes — the hook's stdout for Claude and Codex, the member's mesh inbox for grok. Managed Codex hooks are on by default when Codex >= 0.147; older versions log one unsupported event and receive no reinjection. Antigravity declares no compaction hook.

### 5. Project Metadata and Search

```text
filesystem / git / tool data
  -> import / scan / watcher events
  -> SQLite row updates
  -> tantivy index updates
  -> UI queries
```

Key point:
- content truth remains the filesystem and git
- SQLite/search are optimized projections

## Documentation Boundaries

Active documentation now follows this split:

1. `docs/architecture/data-architecture.md`
   - authoritative inventory and ownership model
2. `docs/architecture/data-model.md`
   - detailed SQLite + search schema reference
3. `docs/coordination-architecture.md`
   - coordination decisions, invariants, and runtime semantics
4. `ARCHITECTURE.md`
   - system overview and module map

The point-in-time reconciliation audit that led to this split is archived in [`../archive/architecture/architecture-doc-reconciliation-notes-2026-03-19.md`](../archive/architecture/architecture-doc-reconciliation-notes-2026-03-19.md).

## Contributor Rules

When adding a new persistent store:

1. Decide whether it is authoritative, rebuildable, or purely diagnostic.
2. Decide whether it belongs in app data, team data, or an external tool surface.
3. Document the writer, reader, and lifecycle.
4. Avoid introducing a second writable truth source for an already-owned concept.
5. If the store binds a logical member to a live session/transcript, prefer runtime attachment state over scanner inference.

## Related Documents

- [data-model.md](./data-model.md)
- [ipc-reference.md](./ipc-reference.md)
- [daemon-protocol.md](./daemon-protocol.md)
- [event-driven-compaction-detection.md](./event-driven-compaction-detection.md)
- [post-compaction-reinjection.md](./post-compaction-reinjection.md)
- [team-member-master-data-audit.md](../analysis/team-member-master-data-audit.md)
- [path-handling-guide.md](./path-handling-guide.md)
