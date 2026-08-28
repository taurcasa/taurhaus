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
5. Compaction, watcher, and operational snapshot files are derived/supporting state, not roster truth.
6. Tool transcript files remain external source material owned by the CLI that wrote them (Claude Code, Codex, Antigravity, Grok), not by Taurhaus.

## App and Daemon Protocol Pairing

The current daemon protocol is **13** (`daemon/protocol.rs`). Protocol 11 (shipped in 0.7.0) replaced the Claude-only account methods with generic account discovery, usage refresh and tool-agnostic transcript lookup; 12 replaced the retired Google tool wire value with `agy`; 13 added `grok`. Each of those changed the contract in both directions, so the app and daemon ship as a pair and reject a mismatched peer instead of attempting partial compatibility.

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
| Codex compact hook | `<CODEX_HOME>/hooks.json` + `<CODEX_HOME>/hooks/…` | Taurhaus | Written **only** when `harness.codex_compaction=hooks` and Codex ≥ 0.147; removed when the setting flips back to `transcript` |

### 2. Coordination Shared State Under `~/.claude/teams/`

There is exactly one teams-dir authority: `PlatformPaths::teams_dir()` = `claude_dir()/teams`, where `claude_dir()` is the `TAURHAUS_CLAUDE_DIR` override, else the WSL-UNC `~/.claude` on Windows, else `~/.claude`. Every store and the daemon resolve the root through it — the former per-module `default_*_teams_dir` copies are gone.

`TAURHAUS_CLAUDE_DIR` is a taurhaus-only variable. Claude Code itself reads `CLAUDE_CONFIG_DIR`; see [path-handling-guide.md](./path-handling-guide.md) for how a launch bridges the two.

| Store | Path | Format | Primary writer | Primary readers | Authority level | Notes |
|---|---|---|---|---|---|---|
| Team config | `teams/<team>/config.json` | JSON | Taurhaus + mesh-compatible tooling | Taurhaus, mesh, UI/backend | Authoritative logical roster | Members, roles, role metadata, project path, tool, `model`, `reasoningEffort` (separate field). Team and member objects carry `#[serde(flatten)] extra`, so mesh-owned keys (`controlAuthTokenHash`, `lastActivityAt`/`Reason`, `statusState`/`Reason`/`SetAt`, `isActive`, unknown future keys) survive every taurhaus save |
| Member runtime | `teams/<team>/runtime/<member>.json` | JSON | Taurhaus | Taurhaus, mesh read-only | Authoritative current attachment | Schema v3: pane id, `pane_pid`, `pane_start_time`, session id, transcript path, `cli_tool`, `project_path`, daemon pid, health, `delivery_lease`, `attached_at`, `last_seen_at` |
| Mesh inbox | `teams/<team>/inboxes/<member>.json` | JSON array | Taurhaus (`MeshInboxStore::append`), mesh | Taurhaus, mesh daemons/agents | Authoritative message queue for file-based delivery | Shared protocol surface; single taurhaus writer (see below) |
| Operational snapshot | `teams/<team>/state/operational/<member>.json` | JSON | Taurhaus | Taurhaus reinjection/delivery | Derived contextual snapshot | Current task, assignment footer, working set, override state |
| Member activity snapshot | `teams/<team>/state/activity/<member>.json` | JSON | Daemon session hub (`coordination/activity_export.rs`), on every activity change or when the 30 s refresh is due | mesh idle monitor | Derived | Schema v1 (`coordination/activity_schema.rs`): `stall_recent_activity`, `stall_no_output`, `stall_no_active_process`, `active_non_shell_process`, `recent_io`, `pane_alive`, `pane_foreign`, `last_output_age_secs`, `activity_confidence`. A focus move is not activity and writes nothing. Skipped for teams with no live pane; degraded scan cycles export nothing |
| Team-daemon credential | `teams/<team>/state/control_auth/<member>.json` | JSON | mesh | mesh | mesh-owned | Taurhaus reads nothing from the file. Before `mesh team-daemon start` it checks three gates and logs `coordination.team_daemon.skipped` with the first failing `reason`: file present (`missing_lead_control_credential`), lead's `config.json` carries a non-empty `controlAuthTokenHash` (`missing_lead_control_auth_token_hash`), lead is not `isActive: false` (`inactive_lead_control_identity`) |
| Member compaction state | `teams/<team>/state/compaction/<member>.json` | JSON | Taurhaus | Taurhaus | Derived idempotency/audit state | Last compaction handled + terminal result |
| Compaction signal log | `teams/<team>/state/compaction/signals/codex-compaction-signals.jsonl` | JSONL | Taurhaus extractor | Taurhaus watcher/processor/diagnostics | Derived canonical signal stream | Normalized Codex compaction records |
| Compaction extractor state | `teams/<team>/state/compaction/extractor-state.json` | JSON | Taurhaus extractor | Taurhaus diagnostics | Derived processing checkpoint | Tracked transcript offsets + last error by file |
| Compaction watcher state | `teams/<team>/state/compaction/signal-watcher-state.json` | JSON | Taurhaus watcher | Taurhaus diagnostics | Derived processing checkpoint | Last consumed offset + recovery stats |

### 3. External Tool Data Taurhaus Observes But Does Not Own

| Tool | Data | Typical path | Ownership | Taurhaus role |
|---|---|---|---|---|
| Claude Code | session transcripts | `~/.claude/projects/<slug>/*.jsonl` | Claude Code | observe/parse |
| Claude Code | task files | `~/.claude/tasks/{session-id}/*.json` | Claude Code | observe/import |
| Claude Code | sessions registry | `<CLAUDE_CONFIG_DIR>/sessions/<pid>.json` | Claude Code | observe — **authoritative** session identity and `busy`/`idle`/`waiting`/`shell` state |
| Claude Code | account config dirs and OAuth usage | `~/.claude`, `~/.claude-*` (`.credentials.json`, `.claude.json`) | Claude Code | observe identities and read a token at request time for the native usage endpoint; taurhaus never writes, persists, logs, or refreshes credentials |
| Codex | session transcripts | `~/.codex/sessions/YYYY/MM/DD/*.jsonl` | Codex | observe/parse |
| Antigravity CLI | conversations | `~/.gemini/antigravity-cli/conversations/*.db` + `cache/last_conversations.json` | Antigravity CLI | observe (identity only; the SQLite transcript is never parsed) |
| Antigravity CLI | presence locks | `~/.gemini/antigravity-cli/presence/*.lock` | Antigravity CLI | observe (advisory flock) |
| Antigravity CLI | activity hooks | `<app data>/agy-hooks.jsonl` | taurhaus (opt-in hook sink) | own/append |
| Grok CLI | live session registry | `<GROK_HOME>/active_sessions.json` | Grok CLI | observe/parse |
| Grok CLI | turn lifecycle | `<GROK_HOME>/sessions/<encoded-cwd>/<session-id>/events.jsonl` | Grok CLI | observe/tail |
| Grok CLI | compaction hooks | `<GROK_HOME>/hooks/*.json` | taurhaus (managed installer) | own/write |

These files are not part of Taurhaus master data. They are upstream evidence sources used to derive runtime/session/task state.

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
| What compaction signals have been emitted but not yet consumed? | signal log + watcher offset under `state/compaction/` |
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
  -> per-member compaction delivery state
  -> team-level compaction signal stream and watcher checkpoints
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

The compaction bugs audited on `2026-03-08` happened when transcript ownership was inferred from scanner/project heuristics instead of consuming authoritative runtime attachment state directly. Current code fixes that by persisting `cli_tool`, `project_path`, and `jsonl_path` on runtime records and by joining config + runtime through the shared roster view (`get_team_roster_with_attachments()`).

## Ownership Boundaries

### Taurhaus Owns

- SQLite schema and all app metadata/history
- search index lifecycle
- structured log sink
- coordination orchestrator logic
- `runtime/` attachment state
- operational snapshots
- compaction hook processing (one bridge, `coordination/compact_hook.rs`, for Claude, Codex and Grok) and the default Codex transcript signal extraction/watching, plus delivery bookkeeping through `record_delivery_at(teams_dir, …)`
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

All three taurhaus producers route through that writer — operator notices for bridged members, operator notices for Claude members, and compaction cards. Operator-originated traffic is sent as `taurhaus` (`MeshInboxMessage::operator_originated`) and reports `DeliveryMethod::InboxFile` truthfully; no `mesh send` sender-candidate chain remains.

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

### 3. Codex Compaction Detection

```text
managed runtime attachments
  -> current transcript set
  -> compaction extractor tails transcript files
  -> canonical signal JSONL append
  -> signal watcher consumes from offset
  -> compaction processor resolves team/member via roster + runtime attachment
  -> inject / skip / stale / fail
  -> update per-member compaction state
```

Key point:
- the canonical trigger stream is the signal log, not the raw Codex transcript
- the authoritative member binding is still runtime attachment, not transcript path heuristics

### 4. Post-Compaction Reinjection

```text
operational snapshot + role/config metadata + runtime attachment
  -> build reinjection card
  -> hook path (`compact_hook.rs`, Claude always, Codex when harness.codex_compaction=hooks)
     or Codex transcript path -> MeshInboxStore::append -> member mesh daemon wakes the pane
  -> record terminal delivery result via record_delivery_at(teams_dir, tool, ...)
```

Key point:
- operational snapshots are supporting context only
- they do not define membership or session attachment
- taurhaus performs no tmux injection on this path

**One hook bridge, one owner.** `coordination/compact_hook.rs` serves Claude, Codex and Grok from a single payload parser: the tool is inferred from the reserved `GROK_*` hook env and otherwise from `transcript_path`, the member is resolved by runtime `session_id` first and normalized `cwd` second. The registry decides where the composed card goes — the hook's stdout for Claude and Codex, the member's mesh inbox for grok. `harness.codex_compaction` (a `TerminalSettings` field) defaults to `transcript`; `hooks` is opt-in and only active when Codex ≥ 0.147 and the hook is installed. Antigravity declares no compaction hook.

Compaction has exactly one owner at a time, logged as `compaction.owner.selected {owner: hooks | daemon | app}`: hooks when active, otherwise the daemon when it is configured and connected, otherwise the app — and the app-owned fallback is released again when the daemon recovers.

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
