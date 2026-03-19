# Architecture Doc Reconciliation Notes

Point-in-time notes captured while reconciling `ARCHITECTURE.md`, `docs/coordination-architecture.md`, and `docs/architecture/data-architecture.md` on 2026-03-19.

These notes were removed from the active docs once the document boundaries were made explicit, but they remain useful as historical context for why the split changed.

## Former `data-architecture.md` Audit Notes

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

### Stale Or Risky Simplifications

1. The older “three storage layers” framing was too narrow for current Taurhaus.
   It captured app metadata/content/search but did not capture live coordination/master-data state.
2. “SQLite is source of truth for structured data” was only locally true for app metadata.
   It was false for live coordination/team runtime data.
3. “Coordination storage” described as only `config.json` plus `runtime/` was no longer sufficient.
   Current implementation also depends on inboxes, operational snapshots, compaction state, signal logs, and watcher/extractor checkpoints.

### Recommended Documentation Shape

1. `docs/architecture/data-architecture.md`
   authoritative inventory and ownership model
2. `docs/architecture/data-model.md`
   detailed SQLite + search schema reference
3. `docs/coordination-architecture.md`
   decisions, invariants, and subsystem rationale
4. topic-specific docs
   compaction pipeline, path handling, daemon protocol, etc.

## Former `coordination-architecture.md` Milestone Snapshot

### M0: Usable MeshBridged Vertical Slice

- Coordination scaffolding (types, trait, errors, audit event types)
- File stores (TeamConfigStore + MemberRuntimeStore, schema v1, atomic writes)
- MeshBridged backend (OperatorNotice delivery only)
- Orchestrator (create/disband/add/remove/list/status/deliver, idempotent)
- Daemon event channel wiring
- IPC commands + runtime UI baseline (team setup, init progress, runtime canvas/status, hot-add/reonboard/disband actions)

### M1: Backend Parity + Health/Recovery

- ClaudeNative backend (Planned; current backend exists as placeholder and launch is not implemented)
- BackendSelector auto-detect + override
- Full delivery variants (Bootstrap, RecoveryNudge, OperatorNotice)
- Health state machine v1 (transitions, cooldown, escalation)

### M2: Product UI + Task Integration

- Team dashboard/visualization
- TaskBoard integration (owner routing + unassigned bucket)
- Extended IPC for UI needs

### M3: Hardening/Polish

- Audit trail (SQLite projections + query surfaces)
- Bootstrap/settings integration (wizard, preferences)
- Schema migration tooling
- Perf/reliability tuning
