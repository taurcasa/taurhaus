# Phase 4 Architecture

This document is a compact Phase 4 view of coordination architecture and recent backend behavior changes.

Primary source: [coordination-architecture.md](coordination-architecture.md)

## Scope

Phase 4 covers mesh-backed multi-agent orchestration in `src-tauri/src/coordination/`:

- team/member lifecycle
- runtime attachment state
- onboarding/message delivery
- resume flows for offline members
- live-status liveness reconciliation

## Resume Lifecycle (Implemented)

- IPC command: `coordination_resume_member`
- Request/report types: `ResumeMemberRequest`, `ResumeAgentReport`
- Resume modes: `ResumeContextMode::{Continue, Fresh}`
- Pipeline owner: `coordination/pipelines.rs`
- UI entrypoint: `MeshTeamRoster` Resume action on offline rows

Behavior summary:

1. Validate team/member and runtime preconditions
2. Resolve existing pane or create a new pane
3. Launch CLI command based on selected resume mode
4. Re-attach mesh/daemon path for non-Claude members
5. Persist runtime updates and return step report

## Liveness Reconciliation (Implemented)

- Orchestrator method: `reconcile_team_liveness(&mut self, team_name: &str)`
- Runtime helper: `pane_is_shell(pane_id: &str)` via tmux `#{pane_current_command}`
- Invoked from `coordination_get_live_team_status` before status projection

Drift rules:

- no `pane_id` => offline
- pane missing => offline
- pane dead => offline
- pane alive but command is shell (`bash`, `zsh`, `sh`, `fish`) => offline

Drift mutation:

- `health = SessionDead`
- `session_id = None`
- non-Claude daemon PID is checked and terminated/cleared when needed
- writes are persisted only when health actually drifted (write-on-drift)

## Quality Gate

Before reporting Rust implementation work as complete, run:

```bash
just agent-quality
```

This runs:

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo check --tests`

## Related References

- [mesh-view-design.md](mesh-view-design.md)
- [coordination-architecture.md](coordination-architecture.md)
- [ARCHITECTURE.md](../ARCHITECTURE.md)
- [CHANGELOG.md](../CHANGELOG.md)
