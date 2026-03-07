# Team Resume Lifecycle After Cold Restart

Date: 2026-03-07
Owner: architect
Task: #488

## Summary

The cold-restart resume flow should be a team-level orchestrator that wraps the existing per-member resume pipeline. It must not duplicate pane resolution, CLI launch, mesh join, daemon restart, or runtime persistence logic that already exists in `coordination_resume_member`.

Recommended shape:

1. Add a team-level IPC command and orchestrator method that loops over persisted team members.
2. Trigger it from the Mesh tab when a persisted team exists but the runtime is fully offline after liveness reconciliation.
3. Resume the lead first, then resume the rest of the team sequentially in phase 1.
4. Treat partial success as a first-class outcome: resumed members stay up, failed members remain offline and retryable.
5. Start any future mesh team-daemon as a best-effort final step, not as the owner of CLI session lifecycle.

## Why This Is Additive

The existing individual resume pipeline already handles the hard parts:

- pane reuse vs replacement in [`members.rs`](/home/mstie/projects/taurhaus/src-tauri/src/coordination/pipelines/members.rs)
- mesh rejoin and per-agent daemon restart in [`lifecycle.rs`](/home/mstie/projects/taurhaus/src-tauri/src/coordination/pipelines/lifecycle.rs)
- generic resume delegation from command-center in [`command_center.rs`](/home/mstie/projects/taurhaus/src-tauri/src/commands/command_center.rs)

The missing capability is orchestration across all persisted members after tmux, daemons, and live panes were wiped out by a WSL reboot.

## Goals

- Restore a persisted team from disk after full runtime loss.
- Reuse the existing member resume pipeline without forking logic.
- Give the Mesh tab an explicit recovery state instead of showing a dead-looking runtime view with no clear next step.
- Return structured per-member results so the UI can show partial recovery honestly.

## Non-Goals

- No automatic relaunch on app startup.
- No replacement of individual member resume.
- No ownership shift where a mesh team-daemon starts owning CLI session creation.
- No speculative parallel runtime refactor in phase 1.

## Current Baseline

Existing building blocks:

- Persisted team config + runtime files survive under `teams/<team>/...`.
- `coordination_get_project_mesh_snapshot` restores Mesh tab context from disk in [`coordination.rs`](/home/mstie/projects/taurhaus/src-tauri/src/commands/coordination.rs).
- `reconcile_team_liveness()` marks members offline when panes are missing/dead/shell in [`orchestrator.rs`](/home/mstie/projects/taurhaus/src-tauri/src/coordination/orchestrator.rs).
- `coordination_resume_member` already performs pane resolution, CLI launch, mesh join, daemon start, onboarding, and runtime update.

Current gap:

- after a full WSL restart, the team still exists on disk, but there is no single action to bring all members back
- current UX only exposes per-member resume from runtime detail
- fast snapshot restore does not explicitly classify "persisted team exists, but everything is cold"

## Recommended UX

### Primary Entry Point

Use the Mesh tab as the primary entry point.

When a project has a discovered team and that team is classified as cold/offline, show:

- runtime canvas as usual, so membership remains visible
- a persistent banner above the canvas:
  - title: `Team is offline after restart`
  - body: `Config still exists on disk, but no live panes were found.`
  - primary CTA: `Resume Team`
  - secondary CTA: `Resume Selected Member`
  - optional tertiary CTA: `Disband Team`

This keeps the user in the existing per-project mesh mental model.

### Secondary Entry Point

Add an optional command-center or sidebar action later:

- `Resume Team` when a project has a persisted team and no live sessions

This is useful, but not required for phase 1.

### No Automatic Resume

Do not auto-launch all CLIs on app startup.

Reason:

- it is expensive and surprising
- it may reopen tools/models/projects the user does not want immediately
- it creates a bad failure mode if tmux/mesh is temporarily unavailable during startup

Detection should be automatic; relaunch should be explicit.

## State Detection

### Desired Classification

Introduce an explicit team runtime classification returned with the project mesh snapshot:

- `none`: no team on disk
- `active`: discovered team with at least one live member
- `degraded`: discovered team with a mix of live and offline members
- `cold_resume`: discovered team exists, but all members are offline after liveness reconciliation

### How To Compute It

Recommended backend behavior:

1. `coordination_get_project_mesh_snapshot` discovers the team by project path.
2. If a team exists and `tmux` is available, run `reconcile_team_liveness(team_name)` before building the returned snapshot.
3. Build snapshot members from the reconciled runtime/config state.
4. Derive `teamRuntimeState` from member statuses.

This avoids the current gap where the first render can show stale pane bindings until the 2s live poll fixes them.

### Why Snapshot-Time Reconciliation Is Acceptable

- the app already treats live status as the authority for pane drift
- cold-restart detection depends on missing panes, which is exactly what liveness reconciliation already knows how to classify
- this is still lightweight compared to launching sessions or spawning daemons

## Proposed Backend API

### New IPC Command

`coordination_resume_team`

Request:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeTeamRequest {
    pub team_name: String,
    pub context_mode: ResumeContextMode,
}
```

Response:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeTeamReport {
    pub team_name: String,
    pub resumed: bool,
    pub total_members: usize,
    pub resumed_members: Vec<String>,
    pub failed_members: Vec<ResumeTeamMemberFailure>,
    pub warnings: Vec<String>,
    pub started_team_daemon: bool,
    pub team_daemon_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeTeamMemberFailure {
    pub member_name: String,
    pub message: String,
    pub retryable: bool,
}
```

Notes:

- `context_mode` applies uniformly in phase 1 for the whole team
- later, the UI can grow an advanced mode picker per member if needed
- include full per-member subreports internally, but the top-level IPC contract only needs summary plus failures

## Orchestration Flow

### High-Level Flow

```text
Mesh tab cold state
    -> user clicks Resume Team
        -> coordination_resume_team(team_name, context_mode)
            -> load persisted config
            -> classify members
            -> resume lead
            -> resume remaining members
            -> optionally start team-daemon
            -> return structured team report
        -> refresh snapshot/live status
        -> show success/partial-failure banner
```

### Detailed Sequence

```text
Frontend
  -> coordination_resume_team

Commands layer
  -> load terminal settings
  -> orchestrator.resume_team_with_cli_commands_and_layout(...)

Orchestrator
  -> TeamConfigStore::load(team)
  -> reconcile_team_liveness(team)
  -> pick lead member
  -> for member in ordered_members:
       resume_member_with_cli_commands_and_layout(
         ResumeMemberRequest { team_name, member_name, context_mode },
         cli_commands,
         tmux_layout,
       )
  -> optional: ensure mesh team-daemon is running
  -> return ResumeTeamReport

Frontend
  -> coordination_get_live_team_status(team)
  -> update runtime canvas
  -> surface warnings / failures
```

## Ordering Recommendation

### Lead First

Resume the lead first.

Reasons:

- the lead is the conceptual anchor for the team
- the lead project anchors the Mesh tab
- if Claude lead context needs to observe subsequent recovery, bringing it up first is more coherent
- onboarding and operator notices for later members conceptually point back to the lead

### Phase 1: Sequential, Not Parallel

Resume the remaining members sequentially in phase 1.

This is the correct practical choice because:

- `CoordinationState::with_orchestrator` already serializes orchestration work
- the existing member-resume pipeline is already tested and deterministic
- failure reporting is simpler and safer
- it avoids interleaving pane creation, mesh join, and daemon startup across multiple members

Parallel launch is not forbidden forever, but it would require deliberate refactoring of orchestrator locking and more careful failure isolation. That is outside this task.

Recommended order:

1. lead
2. same-project members
3. cross-project members

That ordering keeps the primary project responsive first and pushes less-central members later.

## Integration with Existing Individual Resume

This is the core architectural rule: team resume is a loop over `resume_member`, not a new pipeline.

### Reuse Points

- request validation and member loading:
  - [`members.rs`](/home/mstie/projects/taurhaus/src-tauri/src/coordination/pipelines/members.rs)
- pane reuse / recreation:
  - [`runtime.rs`](/home/mstie/projects/taurhaus/src-tauri/src/coordination/runtime.rs)
- mesh rejoin / daemon restart / onboarding:
  - [`lifecycle.rs`](/home/mstie/projects/taurhaus/src-tauri/src/coordination/pipelines/lifecycle.rs)
- generic resume delegation pattern:
  - [`command_center.rs`](/home/mstie/projects/taurhaus/src-tauri/src/commands/command_center.rs)

### New Orchestrator Method

Add:

```rust
pub fn resume_team_with_cli_commands_and_layout(
    &mut self,
    request: &ResumeTeamRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
) -> Result<ResumeTeamReport, CoordinationError>
```

Responsibilities:

- load config once
- compute ordered member list
- call `resume_member_with_cli_commands_and_layout(...)` for each member
- aggregate results
- never duplicate per-member step execution logic

## Error Handling

### Partial Success Is The Normal Failure Model

Do not roll back already resumed members if a later member fails.

This is different from initialize:

- initialize is all-or-nothing because it creates a new team
- cold resume is recovery against existing durable membership

If `frontend-dev` fails but `team-lead` and `reviewer` are back up, that is better than tearing them down again.

### Result Policy

- any resumed member stays resumed
- failed members remain offline
- top-level report is:
  - `resumed = true` if at least one member resumed successfully
  - plus explicit failed member list
- UI message examples:
  - success: `Resumed 5 of 5 team members`
  - partial: `Resumed 4 of 5 team members; 1 still needs attention`
  - failure: `Team resume failed; no members were resumed`

### Retry Model

After a partial result:

- keep the team in runtime mode
- keep failed rows individually resumable
- leave `Resume Team` available as `Resume Remaining`

## Mesh Team-Daemon Handling

### Recommendation

Treat a mesh team-daemon as a best-effort monitoring helper, not as a hard prerequisite for team resume.

If the mesh side gains a central `team-daemon` process that owns IdleMonitor or restart-all coordination:

- start or reattach it after member resume completes
- do not make it responsible for launching member CLI sessions
- if it fails, return a warning, not a hard failure for the whole team resume

### Why Start It Last

- member CLI recovery is the primary user goal
- starting the team-daemon first does not restore useful work by itself
- failure isolation is cleaner if the team is already back when monitoring bootstrap fails

### Phase 1 Fallback

If no team-daemon control surface exists yet, ship team resume without it.

The existing per-agent daemon restart behavior for non-Claude members is already enough to restore communication.

## UX State Model

### Current Problem

Today the Mesh tab effectively flips between:

- `empty`
- `runtime`

That is too coarse for cold restart.

### Recommended Frontend Mode

Keep `mode = 'runtime'`, but add a derived runtime banner state:

- `runtimeBanner = 'cold_resume' | 'degraded' | null`

Reason:

- membership and topology are still meaningful after cold restart
- dropping back to `empty` would incorrectly imply no persisted team exists
- a banner is enough; no new page shell is required

### Banner Rules

- `cold_resume`: all members offline
  - CTA: `Resume Team`
- `degraded`: some members offline
  - CTA: `Resume Offline Members`

### During Team Resume

- disable add/disband/resume actions while team resume is in flight
- show a compact progress list:

```text
Resuming team...
  ✓ team-lead
  ✓ frontend-dev
  x reviewer (tmux unavailable)
```

- on completion, immediately refresh live team status

## Concrete Implementation Recommendations

### Backend

1. Add `ResumeTeamRequest` / `ResumeTeamReport` to:
   - [`coordination_types.rs`](/home/mstie/projects/taurhaus/src-tauri/src/commands/coordination_types.rs)
   - [`coordination/requests.rs`](/home/mstie/projects/taurhaus/src-tauri/src/coordination/requests.rs)
2. Add `coordination_resume_team` command in:
   - [`coordination.rs`](/home/mstie/projects/taurhaus/src-tauri/src/commands/coordination.rs)
   - register in [`lib.rs`](/home/mstie/projects/taurhaus/src-tauri/src/lib.rs)
3. Add orchestrator method in:
   - [`orchestrator.rs`](/home/mstie/projects/taurhaus/src-tauri/src/coordination/orchestrator.rs)
   - pipeline implementation in [`members.rs`](/home/mstie/projects/taurhaus/src-tauri/src/coordination/pipelines/members.rs)
4. Extend project snapshot response with `teamRuntimeState` so the frontend can detect cold-restart state on first render.
5. Before building snapshot for a discovered team, run liveness reconciliation when `tmuxAvailable` is true.

### Frontend

1. Add `coordinationResumeTeam()` IPC wrapper in:
   - [`coordination.js`](/home/mstie/projects/taurhaus/src/lib/ipc/coordination.js)
2. Extend snapshot normalization and controller state in:
   - [`meshTabController.svelte.js`](/home/mstie/projects/taurhaus/src/lib/components/meshTabController.svelte.js)
3. Add runtime banner + CTA in:
   - [`MeshTab.svelte`](/home/mstie/projects/taurhaus/src/lib/components/MeshTab.svelte)
   - or the current runtime header component if extracted later
4. Reuse the existing node detail resume action for per-member retries after partial failure.

## ASCII State Diagram

```text
No team on disk
  -> Empty Mesh tab

Team on disk + some live members
  -> Runtime view

Team on disk + all members offline
  -> Runtime view + cold-resume banner
       -> Resume Team
            -> resume lead
            -> resume members
            -> success .......... -> Runtime view
            -> partial failure .. -> Runtime view + degraded banner
            -> total failure .... -> Runtime view + cold-resume banner + error
```

## Acceptance Criteria

1. A persisted team that survives WSL reboot is detected without appearing as a fresh/empty project.
2. The Mesh tab shows a clear `Resume Team` action when all members are offline.
3. Team resume reuses the existing member resume pipeline for each member.
4. Lead resumes first.
5. Phase 1 executes sequentially and returns structured per-member outcomes.
6. Partial success leaves resumed members running and failed members retryable.
7. The UI refreshes to live runtime state immediately after the team-level action.
8. Team-daemon startup, if present, is best-effort and non-blocking.

## Recommended Task Split

1. Backend contract + IPC command + aggregate report types.
2. Orchestrator team-resume loop over existing member resume.
3. Snapshot classification for `cold_resume` vs `degraded`.
4. Mesh tab runtime banner + `Resume Team` CTA + in-flight progress.
5. Rust/Vitest coverage for:
   - all members offline => cold-resume banner
   - partial team resume
   - lead-first ordering
   - no rollback of already resumed members on later failure
