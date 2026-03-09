# Data Architecture

This document is the authoritative data architecture reference for Taurhaus as of `v0.5.6+`.

It complements [data-model.md](./data-model.md), which still covers the SQLite schema and search index in detail, but does not fully describe the live coordination/runtime filesystem model that Taurhaus now depends on.

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
6. Tool transcript files remain external source material owned by Claude/Codex/Gemini, not by Taurhaus.

## Storage Inventory

### 1. App-Owned Persistent State

| Store | Path | Format | Primary writer | Primary readers | Authority level | Notes |
|---|---|---|---|---|---|---|
| Main app DB | `app_data_dir()/taurhaus.db` | SQLite | Taurhaus backend | Taurhaus backend | Authoritative for app metadata/history | Initialized in `src-tauri/src/db/mod.rs`; WAL + FK enabled |
| Search index | `app_data_dir()/search_index/` | tantivy index dir | Taurhaus backend | Taurhaus backend | Derived / rebuildable | Projection of filesystem + commits + sessions |
| Structured log | `app_data_dir()/taurhaus.log.jsonl` | JSONL + rotated segments | Taurhaus frontend/backend | Humans, scripts, diagnostics | Audit/telemetry | Async append-only sink from `commands/logging.rs` |
| Template storage | `app_data_dir()/templates/` or `<TAURHAUS_DATA_DIR>/templates/` | YAML + git metadata | Taurhaus backend | Taurhaus backend + UI | Authoritative for user templates | Separate from coordination state |

### 2. Coordination Shared State Under `~/.claude/teams/`

All of the following are rooted under:

- `<TAURHAUS_CLAUDE_DIR>/teams/` when overridden
- Windows-resolved mesh teams dir when applicable
- otherwise `~/.claude/teams/`

| Store | Path | Format | Primary writer | Primary readers | Authority level | Notes |
|---|---|---|---|---|---|---|
| Team config | `teams/<team>/config.json` | JSON | Taurhaus + mesh-compatible tooling | Taurhaus, mesh, UI/backend | Authoritative logical roster | Members, roles, role metadata, project path, tool, model |
| Member runtime | `teams/<team>/runtime/<member>.json` | JSON | Taurhaus | Taurhaus, mesh read-only | Authoritative current attachment | Pane, session id, transcript path, `cli_tool`, `project_path`, daemon pid, health |
| Mesh inbox | `teams/<team>/inboxes/<member>.json` | JSON array | Taurhaus, mesh | Taurhaus, mesh daemons/agents | Authoritative message queue for file-based delivery | Shared protocol surface |
| Operational snapshot | `teams/<team>/state/operational/<member>.json` | JSON | Taurhaus | Taurhaus reinjection/delivery | Derived contextual snapshot | Current task, assignment footer, working set, override state |
| Member compaction state | `teams/<team>/state/compaction/<member>.json` | JSON | Taurhaus | Taurhaus | Derived idempotency/audit state | Last compaction handled + terminal result |
| Compaction signal log | `teams/<team>/state/compaction/signals/codex-compaction-signals.jsonl` | JSONL | Taurhaus extractor | Taurhaus watcher/processor/diagnostics | Derived canonical signal stream | Normalized Codex compaction records |
| Compaction extractor state | `teams/<team>/state/compaction/extractor-state.json` | JSON | Taurhaus extractor | Taurhaus diagnostics | Derived processing checkpoint | Tracked transcript offsets + last error by file |
| Compaction watcher state | `teams/<team>/state/compaction/signal-watcher-state.json` | JSON | Taurhaus watcher | Taurhaus diagnostics | Derived processing checkpoint | Last consumed offset + recovery stats |

### 3. External Tool Data Taurhaus Observes But Does Not Own

| Tool | Data | Typical path | Ownership | Taurhaus role |
|---|---|---|---|---|
| Claude Code | session transcripts | `~/.claude/projects/<slug>/*.jsonl` | Claude Code | observe/parse |
| Claude Code | task files | `~/.claude/tasks/{session-id}/*.json` | Claude Code | observe/import |
| Codex | session transcripts | `~/.codex/sessions/YYYY/MM/DD/*.jsonl` | Codex | observe/parse |
| Gemini CLI | chats | `~/.gemini/tmp/<dir-or-hash>/chats/*.json` | Gemini CLI | observe/parse |
| Gemini CLI | task file | `TODO.md` in project root | Gemini/user | observe/import |

These files are not part of Taurhaus master data. They are upstream evidence sources used to derive runtime/session/task state.

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
      -> pane_id
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
- compaction signal extraction/watching and delivery bookkeeping
- UI projections and diagnostics

### mesh Owns

- file-based messaging protocol semantics
- member daemon behavior
- team-daemon behavior
- mesh-native status/activity fields in shared config space

### Shared / Compatibility Surface

- `teams/<team>/config.json`
- `teams/<team>/inboxes/`

These files must remain compatible across Taurhaus and mesh, but Taurhaus-specific attachment semantics should not migrate into mesh as generic source-of-truth fields unless mesh truly consumes them semantically.

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
  -> Claude hook path or Codex tmux/inbox path
  -> record terminal delivery result
```

Key point:
- operational snapshots are supporting context only
- they do not define membership or session attachment

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

## Current Documentation Audit

### Accurate But Incomplete

#### `docs/architecture/data-model.md`

Accurate on:

- SQLite tables
- tantivy index
- high-level filesystem truth model

Incomplete on:

- `runtime/*.json` now includes `session_id`, `jsonl_path`, `daemon_pid`, delivery lease, and health semantics
- `state/operational/` is not documented
- `state/compaction/` is significantly under-documented
- the authoritative/derived split across config/runtime/scanner/signal state is not explicit enough

#### `ARCHITECTURE.md`

Accurate on:

- overall module map
- dual-process app/daemon overview
- high-level coordination direction

Incomplete on:

- precise ownership matrix for live data stores
- distinction between app DB/search state and coordination filesystem state
- transcript/signal/operational snapshot roles

#### `docs/coordination-architecture.md`

Accurate on:

- major decisions and invariants
- config vs runtime split
- filesystem-first coordination stance

Incomplete on:

- concrete inventory of all currently shipped coordination store files
- operational snapshot and compaction signal stores as first-class entries
- relationship to SQLite/search/app-data stores

### Stale or Risky Simplifications

1. The older “three storage layers” framing is too narrow for current Taurhaus.
   - it captures app metadata/content/search
   - it does not capture live coordination/master-data state

2. “SQLite is source of truth for structured data” is only locally true for app metadata.
   - it is false for live coordination/team runtime data

3. “Coordination storage” described as only `config.json` plus `runtime/` is no longer sufficient.
   - current implementation also depends on inboxes, operational snapshots, compaction state, signal logs, and watcher/extractor checkpoints

## Recommended Documentation Shape

Going forward, docs should keep this split:

1. `docs/architecture/data-architecture.md`
   - authoritative inventory and ownership model
2. `docs/architecture/data-model.md`
   - detailed SQLite + search schema reference
3. `docs/coordination-architecture.md`
   - decisions, invariants, and subsystem rationale
4. topic-specific docs
   - compaction pipeline, path handling, daemon protocol, etc.

That keeps one place for “what data exists and who owns it” without forcing every subsystem doc to repeat the whole store model.

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
