# Coordination Subsystem — Architecture

> Taurhaus + mesh integration for multi-agent team orchestration.
> Designed 2026-03-01. Collaboratively architected by Claude Code + Codex (GPT-5.3).

![Coordination Architecture](images/coordination-architecture.jpg)

## Overview

Taurhaus gains the ability to create, monitor, and manage multi-agent teams that collaborate via the filesystem. The integration leverages mesh (a Rust CLI for non-Claude agents) and Claude Code's native team system, with the filesystem (`~/.claude/`) as the shared API surface.

**Core principles:**
- Filesystem IS the API — no code coupling between taurhaus and mesh
- Taurhaus owns orchestration + GUI, mesh owns protocol + CLI
- Either works standalone
- Cleanest solution wins — effort is not a constraint

## Current orchestration direction (2026-03-06)

This document captures the shipped/active coordination subsystem architecture in taurhaus.

- The practical orchestration direction for auto-idle and communication quality now lives in [`architecture/orchestration-practical-auto-idle-and-communication.md`](architecture/orchestration-practical-auto-idle-and-communication.md).
- The v0.2.0 protocol exploration is archived in [`architecture/orchestration-protocol-design.md`](architecture/orchestration-protocol-design.md) and is not an active implementation target.
- Taurhaus currently contributes runtime activity handoff by exporting per-member snapshots under `~/.claude/teams/{team}/state/activity/{member}.json` from `coordination/stall_detector.rs`.

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

**Decision**: Team configuration and runtime coordination state live in separate files.

| Layer | Path | Owner | Contents |
|-------|------|-------|----------|
| Durable config | `teams/<team>/config.json` | Shared (mesh + taurhaus) | Members, roles, instructions, projectPath |
| Protocol data | `teams/<team>/inboxes/` | Shared | Messages |
| Runtime state | `teams/<team>/runtime/<member>.json` | Taurhaus (mesh read-only) | Pane binding, delivery lease, health snapshot |

**Rationale**: Eliminates config.json write contention. Config is mostly write-once. Runtime files are hot and disposable — delete them and taurhaus rebuilds from live session scanning.

**Invariant**: Runtime state is always reconstructible. It must never silently become durable authority.

### D3: JSON is live truth, SQLite is projection/history

**Status**: Planned

**Decision**: JSON files in `~/.claude/` are the source of truth for live coordination state. SQLite stores projections for history, querying, and UI ergonomics.

**Invariant**: SQLite must never be a competing writable source of truth for live coordination. Edits flow through coordination stores first, then get projected to SQLite.

**Flow**:
1. User action or daemon event
2. Orchestrator mutates coordination source of truth (JSON stores)
3. Orchestrator records resulting event/snapshot in SQLite
4. UI reads a mix of live view + derived persisted data

### D4: Logical team membership separated from session attachment

**Status**: Implemented

**Decision**: A team member is a logical role that persists independently of any specific tmux pane or process.

- **Logical member** (durable, in config.json): name, role, instructions, projectPath
- **Attachment** (volatile, in runtime/): pane_id, process info, delivery lease, health state

Members can be "detached" (pane died) but remain on the team. Rebind via process scanning without re-joining.

### D5: Two-tier launch strategy

**Status**: Partial

**Decision**: Claude Code agents use native CLI flags. Codex/Gemini agents use mesh daemon bridge.

| Agent Type | Launch Method | Delivery | Messaging |
|---|---|---|---|
| Claude Code | Native CLI flags (`--team-name`, `--agent-name`, etc.) | Inbox file write → native poller | Native `SendMessage` tool |
| Codex / Gemini | tmux + `mesh daemon` | Daemon inotify → tmux send-keys | `mesh send` / `mesh read` CLI |

**Rationale**: Claude Code is the only CLI tool with native local team capabilities (researched 2026-03-01). Codex has a hidden `multi_agent` experimental flag but no public surface. Gemini CLI has no team features.

### D6: Delivery lease for daemon conflict avoidance

**Status**: Partial

**Decision**: Per-member runtime lease file with PID, instance UUID, hostname, and heartbeat timestamp.

- Taurhaus writes its PID when it takes delivery ownership
- Mesh daemon checks lease before starting — backs off if fresh, reclaims if stale
- Atomic create via rename prevents startup races
- PID + instance UUID handles PID-reuse edge case

### D7: Explicit health state machine

**Status**: Partial

**Decision**: Health monitoring uses explicit states, events, and a deterministic transition function.

**States**: Healthy, AwaitingRead, SuspectedStuck, Rebriefed, Suppressed, SessionDead

**Events**: UnreadDetected, IdleThresholdMet, IoResumed, InboxCleared, CooldownExpired, SessionMissing, DeliveryFailed, ManualSuppress, ManualResume

**Recovery evidence**:
- Weak: terminal IO resumed after injection → clears to Monitoring
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

### D16: Resume lifecycle is a first-class member operation

**Status**: Implemented

**Decision**: Offline members are resumed through a dedicated pipeline and IPC surface, not by remove/re-add.

- IPC: `coordination_resume_member`
- Types: `ResumeContextMode`, `ResumeMemberRequest`, `ResumeAgentReport`
- Runtime behavior: resolve/reuse pane when possible, launch mode-aware CLI commands, restore mesh daemon path for non-Claude members, persist runtime attachment
- UI contract: offline rows expose resume actions (`Continue` and `Fresh`)

**Rationale**: Resume preserves team identity and historical context while minimizing operator friction and avoiding destructive config churn.

### D17: Liveness reconciliation is write-on-drift at live-status read time

**Status**: Implemented

**Decision**: Team liveness is reconciled inside orchestrator before returning live team status.

- Drift conditions:
  - missing `pane_id`
  - `pane_exists == false`
  - `pane_is_dead == true`
  - pane command resolves to shell (`pane_is_shell == true`)
- Drift mutation:
  - `health -> SessionDead`
  - `session_id -> None`
  - non-Claude `daemon_pid` checked/terminated/cleared
- Persistence policy: write only when stored health is stale (write-on-drift)

**Rationale**: This closes the gap between cached runtime metadata and real pane/process state without requiring a constantly running reconciliation loop.

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
- Frontend template flow (`TemplateBrowserPanel` -> `TeamCustomizerPanel` -> `MeshSetupView`) resolves to the same `InitializeTeamRequest` shape used by manual setup.
- Coordination runtime continues to start teams only through `coordination_initialize_team` / `templates_apply_composition` (which forwards into the same initialize pipeline).

**Integration points**:
- `src-tauri/src/templates/storage/`: git-backed template persistence and pending-action state
- `src-tauri/src/templates/composition.rs`: deterministic roster composition + validation
- `src-tauri/src/commands/templates.rs`: template command surface (`templates_get_history`, `templates_get_diff`, `templates_revert`, etc.)
- `src/lib/components/MeshSetupView.svelte`: template-first + blank-slate UX bridge into initialize payload

**Rationale**: This preserves one runtime lifecycle for team launch/resume/remove while enabling reusable template authoring and auditability through git-backed history.

## Milestone Plan

### M0: Usable MeshBridged vertical slice
- Coordination scaffolding (types, trait, errors, audit event types)
- File stores (TeamConfigStore + MemberRuntimeStore, schema v1, atomic writes)
- MeshBridged backend (OperatorNotice delivery only)
- Orchestrator (create/disband/add/remove/list/status/deliver, idempotent)
- Daemon event channel wiring
- IPC commands + runtime UI baseline (team setup, init progress, runtime canvas/status, hot-add/reonboard/disband actions)

### M1: Backend parity + health/recovery
- ClaudeNative backend (Planned; current backend exists as placeholder and launch is not implemented)
- BackendSelector auto-detect + override
- Full delivery variants (Bootstrap, RecoveryNudge, OperatorNotice)
- Health state machine v1 (transitions, cooldown, escalation)

### M2: Product UI + task integration
- Team dashboard/visualization
- TaskBoard integration (owner routing + unassigned bucket)
- Extended IPC for UI needs

### M3: Hardening/polish
- Audit trail (SQLite projections + query surfaces)
- Bootstrap/settings integration (wizard, preferences)
- Schema migration tooling
- Perf/reliability tuning

## Research Sources

- Integration proposal: `mesh/docs/taurhaus-integration-proposal.md`
- Architecture diagram: `mesh/docs/taurhaus-integration-architecture-v2.jpg`
- Practical orchestration direction: [`architecture/orchestration-practical-auto-idle-and-communication.md`](architecture/orchestration-practical-auto-idle-and-communication.md)
- Archived protocol exploration: [`architecture/orchestration-protocol-design.md`](architecture/orchestration-protocol-design.md)
