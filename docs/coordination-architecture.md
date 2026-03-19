# Coordination Subsystem — Architecture

> Taurhaus + mesh integration for multi-agent team orchestration.
> Designed 2026-03-01. Collaboratively architected by Claude Code + Codex (GPT-5.3).

![Coordination Architecture](images/coordination-architecture.jpg)

## Overview

Taurhaus gains the ability to create, monitor, and manage multi-agent teams that collaborate via the filesystem. The integration leverages mesh (a Rust CLI for non-Claude agents) and Claude Code's native team system, with the filesystem (`~/.claude/`) as the shared API surface.

This document is the active coordination design reference for:

- subsystem decisions and invariants
- runtime/recovery semantics
- delivery and orchestration behavior

For the authoritative inventory of coordination files, ownership boundaries, and source-of-truth rules, use [`architecture/data-architecture.md`](architecture/data-architecture.md). [`../ARCHITECTURE.md`](../ARCHITECTURE.md) remains the top-level system overview.

**Core principles:**
- Filesystem IS the API — no code coupling between taurhaus and mesh
- Taurhaus owns orchestration + GUI, mesh owns protocol + CLI
- Either works standalone
- Cleanest solution wins — effort is not a constraint

## Current orchestration direction (2026-03-06)

This document captures the shipped/active coordination subsystem architecture in taurhaus.

- The practical orchestration direction for auto-idle and communication quality now lives in [`architecture/orchestration-practical-auto-idle-and-communication.md`](architecture/orchestration-practical-auto-idle-and-communication.md).
- The v0.2.0 protocol exploration is archived in [`archive/architecture/orchestration-protocol-design.md`](archive/architecture/orchestration-protocol-design.md) and is not an active implementation target.
- Taurhaus now owns a broader operational context layer under `~/.claude/teams/{team}/state/`, including per-member operational snapshots, canonical Codex compaction signal logs, and per-member compaction delivery state.
- The historical `state/activity/` export from `coordination/stall_detector.rs` is no longer the main reinjection context path.
- Codex compaction reinjection is no longer poll-based. The shipped path is extractor -> watcher -> processor -> mesh inbox append.
- Claude compaction reinjection is hook-based and now installs runtime-appropriate wrappers (`.sh` for WSL/Linux Claude runtimes, `.cmd` for native Windows Claude runtimes), normalizes current hook payload variants, and logs standalone hook execution into the canonical JSONL sink.

## Design Decision Log

Status labels use current implementation state:
- **Implemented**: shipped and used by current code paths
- **Partial**: scaffolding exists, but behavior is incomplete or not fully wired
- **Planned**: design intent only; not implemented in active path

### D1: Trait, not enum, for backend abstraction

**Status**: Implemented

**Decision**: `CoordinationBackend` is a Rust trait, not an enum with match arms.

**Rationale**: The trait enforces the backend boundary at the type system level. Call sites cannot reach into backend-specific internals. Testable via mock injection. Adding a third backend is additive (new file), not surgical (touch every match block). The effort difference between trait and enum is negligible for AI implementers — only code quality matters.

### D2: Durable config separated from volatile runtime state

**Status**: Implemented

**Decision**: Team configuration and runtime coordination state live in separate files. Durable roster/config state, runtime attachment state, inboxes, and compaction/operational state remain distinct stores rather than collapsing into one mutable blob.

Current file-level inventory, writers/readers, and authority levels are maintained in [`architecture/data-architecture.md`](architecture/data-architecture.md).

**Rationale**: Eliminates config.json write contention. Config is mostly write-once. Runtime files are hot and disposable — delete them and taurhaus rebuilds from live session scanning.

**Invariant**: Runtime state is always reconstructible. It must never silently become durable authority.

### D3: JSON is live truth, SQLite is projection/history

**Status**: Partial

**Decision**: JSON files in `~/.claude/` are the source of truth for live coordination state. SQLite stores projections for history, querying, and UI ergonomics where those projections exist.

**Invariant**: SQLite must never be a competing writable source of truth for live coordination. Edits flow through coordination stores first, then get projected to SQLite.

**Flow**:
1. User action or daemon event
2. Orchestrator mutates coordination source of truth (JSON stores)
3. Taurhaus may record resulting events/snapshots in SQLite or other derived stores when the product surface needs history/query support
4. UI reads a mix of live view + derived persisted data

### D4: Logical team membership separated from session attachment

**Status**: Implemented

**Decision**: A team member is a logical role that persists independently of any specific tmux pane or process.

- **Logical member** (durable, in config.json): name, role, instructions, projectPath
- **Attachment** (volatile, in runtime/): pane_id, process info, session/jsonl attachment, daemon pid, delivery lease, health state

Members can be "detached" (pane died) but remain on the team. Rebind via process scanning without re-joining.

### D5: Two-tier launch strategy

**Status**: Partial

**Decision**: Claude Code agents use native CLI flags. Codex/Gemini agents use mesh daemon bridge.

| Agent Type | Launch Method | Delivery | Messaging |
|---|---|---|---|
| Claude Code | Native CLI flags (`--team-name`, `--agent-name`, etc.) | Inbox file write → native poller, plus `SessionStart(source=compact)` hook bridge | Native `SendMessage` tool |
| Codex / Gemini | tmux + `mesh daemon` | Mesh inbox append + wake prompt | `mesh send` / `mesh read` CLI |

**Rationale**: Claude Code is the only CLI tool with native local team capabilities (researched 2026-03-01). Codex has a hidden `multi_agent` experimental flag but no public surface. Gemini CLI has no team features.

### D6: Delivery lease for daemon conflict avoidance

**Status**: Partial

**Decision**: Per-member runtime lease file with PID, instance UUID, hostname, and heartbeat timestamp.

- Taurhaus writes its PID when it takes delivery ownership
- Mesh daemon checks lease before starting — backs off if fresh, reclaims if stale
- Atomic create via rename prevents startup races
- PID + instance UUID handles PID-reuse edge case

This lease still exists for daemon ownership coordination, but it is no longer the compaction delivery gate. Codex compaction delivery now validates current managed attachment directly from roster/runtime state and records idempotency separately in per-member compaction state.

### D7: Explicit health state machine

**Status**: Partial

**Decision**: Health monitoring uses explicit states, events, and a deterministic transition function for runtime monitoring and UI state.

**States**: Healthy, AwaitingRead, SuspectedStuck, Rebriefed, Suppressed, SessionDead

**Events**: UnreadDetected, IdleThresholdMet, IoResumed, InboxCleared, CooldownExpired, SessionMissing, DeliveryFailed, ManualSuppress, ManualResume

**Recovery evidence**:
- Weak: terminal IO resumed after injection → clears toward healthy monitoring
- Strong: inbox unread count decreased or task activity → clears to Healthy

### D8: Typed delivery payloads

**Status**: Implemented

**Decision**: `DeliveryRequest` is a Rust enum with per-variant payload structs.

```rust
enum DeliveryRequest {
    Bootstrap(BootstrapDelivery),
    RecoveryNudge(RecoveryNudgeDelivery),
    OperatorNotice(OperatorNoticeDelivery),
}
```

Each variant carries structured fields reflecting intent. Backend implementations render variant-appropriate content (inbox JSON for Claude, tmux text for mesh-bridged).

### D9: Closed, taurhaus-centric capability model

**Status**: Implemented

**Decision**: `BackendCapabilities` is a closed struct describing operational semantics, not vendor features.

```rust
struct BackendCapabilities {
    can_launch_with_identity: bool,
    supports_out_of_band_delivery: bool,
    supports_native_peer_messaging: bool,
    supports_native_shared_tasks: bool,
    supports_attachment_rebind: bool,
    requires_sidecar_delivery: bool,
}
```

### D10: Backend selection — auto-detect with override

**Status**: Partial

**Decision**: `BackendSelector` supports override → force-mesh → CLI-tool auto-detection ordering.
`BackendSelector` supports override + auto-detect, but current setup/runtime flows still force M0-style `MeshBridged` selection.

### D11: Channel-based daemon → orchestrator event pipeline

**Status**: Partial

**Decision**: Daemon pushes normalized events onto a bounded channel. Orchestrator consumes, deduplicates, and acts.

- Daemon: detect + normalize + emit (dumb)
- Orchestrator: dedupe/coalesce + decide + execute side effects (smart)
- Safety net: periodic reconcile scan catches dropped/coalesced events

### D12: Module structure — self-contained `coordination/` subsystem

**Status**: Implemented

**Decision**: New top-level module, not flattened into existing structure.

```
src-tauri/src/
  coordination/
    mod.rs              # Public API surface
    domain.rs           # Team, Member, RuntimeState, DeliveryLease, etc.
    backend/
      mod.rs            # CoordinationBackend trait + BackendCapabilities
      claude.rs         # ClaudeNativeBackend
      bridged.rs        # MeshBridgedBackend
      selector.rs       # BackendSelector
    stores/
      mod.rs
      config.rs         # TeamConfigStore (JSON, ~/.claude/teams/)
      runtime.rs        # MemberRuntimeStore (JSON, runtime/)
    health/
      mod.rs
      state.rs          # HealthState enum
      transition.rs     # transition(state, event, ctx) -> HealthTransition
      policy.rs         # Cooldown, escalation, recovery thresholds
    audit.rs            # Typed audit events
    orchestrator.rs     # Top-level coordination service
```

### D13: Never modify agent config files

**Status**: Implemented

**Decision**: Taurhaus never writes to CLAUDE.md, .codex-instructions.md, or any agent configuration files. All agent context delivery happens through tmux injection or inbox file writes — external and ephemeral.

### D14: Team visibility scoped to managed teams

**Status**: Partial

**Decision**: Taurhaus team discovery currently comes from filesystem config (`TeamConfigStore::discover` under `~/.claude/teams`), then the Mesh tab scopes active restoration by lead project path.

This means teams are not sourced from SQLite ownership records; visibility is project-context aware but still driven by `config.json` discovery.

### D15: Structured member removal with guarded teardown + lead notice

**Status**: Implemented

**Decision**: Runtime member removal is a multi-step operation with explicit diagnostics and safety guards.

- IPC contract: `coordination_remove_member` returns `RemoveAgentReport` (not `()`), including `steps[]` and `warnings[]`.
- Lead protection: team lead entries are non-removable through this path.
- Pane safety: pane teardown requires pre-checking pane ownership against the member project path before killing tmux panes.
- Operator visibility: after removal, taurhaus sends a removal notice to the team lead summarizing whether cleanup was full or partial.

**Rationale**: Removal is an operational workflow, not a simple config mutation. Structured reporting and ownership checks prevent silent failures and reduce accidental pane/process termination risk.

### D16: Resume lifecycle is a first-class member and team operation

**Status**: Implemented

**Decision**: Offline recovery is handled through dedicated resume pipelines and IPC surfaces, not by remove/re-add.

- IPC:
  - `coordination_resume_member`
  - `coordination_resume_team`
- Types:
  - `ResumeContextMode`
  - `ResumeMemberRequest` / `ResumeAgentReport`
  - `ResumeTeamRequest` / `ResumeTeamReport`
- Runtime behavior:
  - member resume resolves/reuses a pane when possible, launches mode-aware CLI commands, restores mesh membership + per-agent daemon state for non-Claude members, and persists runtime attachment
  - team resume loads the persisted roster, resumes the lead first, then resumes the remaining members sequentially through the existing member-resume pipeline
  - partial success is preserved; already resumed members stay up while failed members remain retryable
- Snapshot/UI contract:
  - project mesh snapshot returns `teamRuntimeState` with `none | active | degraded | cold_resume`
  - runtime header/UI maps that into `none | active | degraded | coldResume`
  - cold restart recovery is surfaced in the runtime header (`MeshRuntimeBar`) with `Resume Team`
  - degraded teams surface `Resume Offline (n)` plus per-member retry from node detail
  - in-flight team resume shows per-member progress rows and disables conflicting runtime actions until completion

**Rationale**: Resume preserves team identity and historical context while minimizing operator friction and avoiding destructive config churn. Team-level resume gives cold-restart recovery a single explicit action without inventing a second launch pipeline.

### D17: Snapshot reads stay fast; liveness repair is explicit or background

**Status**: Implemented

**Decision**: UI snapshot reads are disk-first and avoid runtime probing. Liveness repair runs only in explicit recovery flows and the background self-heal loop.

- Fast-path reads:
  - `coordination_get_project_mesh_snapshot`
  - `coordination_get_live_team_status`
  - both read persisted config/runtime via `get_team_status_fast(...)`
  - no tmux probing, process scans, or WSL interop on the snapshot IPC path
- Repair paths:
  - `coordination_resume_member`
  - `coordination_resume_team`
  - background `run_background_self_heal_pass()`
- Repair conditions:
  - missing `pane_id`
  - `pane_exists == false`
  - `pane_is_dead == true`
  - pane command resolves to shell (`pane_is_shell == true`)
  - daemon pid missing/dead/drifted from current mesh binary
  - team-daemon pid drifted from current mesh binary
- Repair mutation:
  - `health -> SessionDead` when panes are gone/dead/shell
  - `session_id -> None` when drift is confirmed
  - non-Claude member daemons are adopted/restarted/terminated as needed
  - team daemon is best-effort stopped/restarted when binary drift is detected
- Concurrency rule:
  - background self-heal runs on a dedicated orchestrator instance, not the app-owned cached orchestrator
  - foreground snapshot IPC therefore does not contend with background liveness repair on the shared coordination mutex

**Rationale**: This keeps first-render and polling snapshots cheap and predictable while still giving taurhaus a bounded repair path for stale panes/daemons and cold-restart recovery.

### D20: Display and runtime session views are explicitly split

**Status**: Implemented

**Decision**: Session scanning and daemon RPC expose two distinct views:

- `DisplaySession` / `list_display_sessions` for UI consumers
- `RuntimeSession` / `list_runtime_sessions` for transcript-aware coordination and compaction logic

`DisplaySession` intentionally strips transcript metadata such as `session_id` and `jsonl_path`. Runtime correlation, task sync, and compaction processing must use `RuntimeSession`.

**Rationale**: This prevents UI-safe data stripping from silently leaking into runtime logic when Windows/daemon and native/local session paths diverge.

### D21: Codex compaction is event-driven and file-backed

**Status**: Implemented

**Decision**: Codex compaction delivery now flows through a file-backed event pipeline instead of the old poll-based reinjection path.

- `CompactionSignalExtractor` tails active managed Codex `RuntimeSession` transcripts, persists per-file offsets, collapses paired `compacted` + `context_compacted` boundaries, and appends canonical signal records
- `CompactionSignalWatcher` watches the low-traffic signal log, replays missed events from durable offsets, and hands each signal to the processor once per watcher state
- `CompactionSignalProcessor` resolves the managed member from roster/runtime context, loads the operational snapshot, renders a bounded reinjection card, appends it to the mesh inbox, and records the delivery result in per-member compaction state

**Current delivery semantics**:
- The legacy poll-based `session_scanner/compaction.rs` module is gone.
- Delivery guards based on stale deferred state were removed. The processor now checks only current managed-member attachment and pane liveness before delivery.
- Idempotency is recorded in `MemberCompactionStore` by `session_id` + compaction timestamp.
- Stale compaction records are persisted as `Stale` results instead of being injected late.
- Delivery is suppressed when the operational snapshot no longer contains a resumable task (for example, the last task was already `completed` or `deleted`), so finished work does not generate stale resume cards.
- Mesh inbox corruption now fails closed: corrupt inbox files are quarantined and logged as `mesh.inbox.corrupt`, and append/load return an error instead of silently treating corruption as empty.
- Claude hook delivery uses the same resumable-task guard and emits `compaction.claude_hook.received/resolved/delivered/failed` events for transport diagnostics.

**End-to-end path**:
1. `RuntimeSession` discovery identifies active managed Codex transcripts.
2. `CompactionSignalExtractor` tails those JSONL files and emits canonical signal records into the team compaction signal log.
3. `CompactionSignalWatcher` watches the signal log, advances durable offsets, and replays missed records after watcher drift or restart.
4. `CompactionSignalProcessor` resolves the target member, validates current attachment, composes the reinjection card, appends the inbox message, and records the delivery outcome.
5. Coordination audit surfaces and runtime diagnostics read the resulting signal and delivery state for operator visibility.

**Rationale**: This keeps transcript watching on the hot path, gives restart-safe durable offsets, moves delivery to an observable lower-traffic signal boundary, and avoids the old poller/delivery-guard failure mode.

### D18: Implementation tasks require a Rust quality gate before completion

**Status**: Implemented

**Decision**: Implementation tasks must pass `just check-quick` before being reported complete.

- Runs `cargo check --tests`, frontend typecheck, and frontend unit tests
- Captures compile/type/test regressions for routine iteration
- Ensures integration shim breakages are caught when included source files gain new imports

**Rationale**: Coordination changes span runtime, orchestrator, IPC, and integration shims. A strict pre-completion gate prevents partially validated changes from being reported as done while keeping iteration fast. The full `just check` gate is reserved for serialized team-lead/pre-release runs.

### D19: Team template composition feeds the existing initialize pipeline

**Status**: Implemented

**Decision**: Template-driven team setup remains an adapter layer above coordination initialization, not a second orchestration path.

- Backend template IPC (`templates_*`) handles role/preset storage, composition, history, diff, and revert.
- Frontend template flow now centers on `MeshTeamBuilder` inside `MeshSetupView`, while `TemplateBrowserPanel` and `TeamCustomizerPanel` remain advanced catalog/history/edit surfaces. All of them still resolve to the same `InitializeTeamRequest` shape used by manual setup.
- Coordination runtime continues to start teams only through `coordination_initialize_team` / `templates_apply_composition` (which forwards into the same initialize pipeline).

**Integration points**:
- `src-tauri/src/templates/storage/`: git-backed template persistence and pending-action state
- `src-tauri/src/templates/composition.rs`: deterministic roster composition + validation
- `src-tauri/src/commands/templates.rs`: template command surface (`templates_get_history`, `templates_get_diff`, `templates_revert`, etc.)
- `src/lib/components/MeshSetupView.svelte`: setup shell that hosts `MeshTeamBuilder` and the init-progress/runtime handoff
- `src/lib/components/MeshTeamBuilder.svelte`: primary quick-preset, filter, and drag/drop team builder

**Rationale**: This preserves one runtime lifecycle for team launch/resume/remove while enabling reusable template authoring and auditability through git-backed history.

## Historical Planning

The older milestone-plan snapshot that used to live in this file is now archived in [`archive/architecture/architecture-doc-reconciliation-notes-2026-03-19.md`](archive/architecture/architecture-doc-reconciliation-notes-2026-03-19.md). Active implementation state should be read from the decision log above, the codebase, and the current changelog/task history rather than from historical milestone buckets.

## Research Sources

- Integration proposal: `mesh/docs/taurhaus-integration-proposal.md`
- Architecture diagram: `mesh/docs/taurhaus-integration-architecture-v2.jpg`
- Practical orchestration direction: [`architecture/orchestration-practical-auto-idle-and-communication.md`](architecture/orchestration-practical-auto-idle-and-communication.md)
- Archived protocol exploration: [`archive/architecture/orchestration-protocol-design.md`](archive/architecture/orchestration-protocol-design.md)
