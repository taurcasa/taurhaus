# Feature Matrix

Current shipped status for key mesh coordination capabilities.

## Coordination Features

| Feature | Status | Version | Backend Surface | UI Surface | Notes |
|---|---|---|---|---|---|
| Team setup and initialize | Implemented | 0.4.0 | `coordination_initialize_team` | Mesh setup flow | Full initialize pipeline with progress reporting |
| Add member to running team | Implemented | 0.4.0 | `coordination_add_agent` | MeshTeamRoster Add Agent | Creates pane/session + mesh onboarding |
| Remove non-lead member | Implemented | 0.4.3 | `coordination_remove_member` + `RemoveAgentReport` | MeshTeamRoster Remove action | Includes pane ownership guard and lead notice |
| Resume offline member (Continue/Fresh) | Implemented | 0.4.3 | `coordination_resume_member` + `ResumeMemberRequest` / `ResumeAgentReport` | MeshTeamRoster Resume action on offline rows | Preserves member identity and runtime history |
| Live-status liveness reconciliation | Implemented | 0.4.3 | `reconcile_team_liveness` + `coordination_get_live_team_status` | MeshTeamRoster status refresh | Write-on-drift checks pane missing/dead/shell (`pane_is_shell`) |
| Non-Claude daemon cleanup on offline drift | Implemented | 0.4.3 | Runtime PID checks + terminate/clear | Transparent | Prevents stale sidecar processes after CLI exits |
| Periodic event-driven health escalation | Partial | N/A | Health/reconcile modules | Not exposed | Explicit state machine exists; advanced escalation not fully wired |

## Engineering Workflow Gates

| Gate | Status | Required For | Command |
|---|---|---|---|
| Rust implementation quality gate | Implemented | Rust implementation tasks before completion | `just agent-quality` |

## References

- [Mesh view design](mesh-view-design.md)
- [Phase 4 architecture](phase-4-architecture.md)
- [Coordination architecture](coordination-architecture.md)
- [Architecture overview](../ARCHITECTURE.md)
- [Changelog](../CHANGELOG.md)
