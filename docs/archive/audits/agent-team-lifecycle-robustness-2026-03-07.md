# Agent and Team Lifecycle Robustness Audit

Date: 2026-03-07
Task: #523
Scope: `src-tauri/src/coordination/`, `src-tauri/src/commands/coordination.rs`, related runtime and lifecycle paths.

## Summary

The core lifecycle model is directionally sound after the recent repair work:
- pane resume now revalidates pane ownership before reuse and syncs `tmuxPaneId` back into `config.json`
- member removal now discovers live daemons beyond stale runtime metadata and clears member pidfiles
- snapshot paths are intentionally fast and no longer probe tmux/process state on the UI-critical path
- startup reconciliation clears stale runtime `daemon_pid` values and removes orphan runtime records

The remaining problems are in rollback and daemon-identity edges, not the main happy path.

## What Looks Solid

1. Resume-member pane recovery is coherent.
   - `resolve_or_create_pane_for_member(...)` handles missing, dead, and ownership-mismatched panes before resume proceeds.
   - Successful resume persists the new pane ID into runtime and syncs team config metadata afterward.
   - References: `src-tauri/src/coordination/runtime.rs`, `src-tauri/src/coordination/pipelines/members.rs`.

2. Remove-member teardown is materially stronger than before.
   - Teardown no longer trusts only `runtime.daemon_pid`; it also discovers matching live daemons by `(pane, team, member)` and clears the pidfile.
   - That closes the earlier orphan-daemon hole when runtime metadata was stale.
   - Reference: `src-tauri/src/coordination/orchestrator.rs:801` onward.

3. Fast snapshot paths are correctly isolated from runtime probes.
   - `coordination_get_project_mesh_snapshot(...)` and `coordination_get_live_team_status(...)` both use `get_team_status_fast(...)` and avoid tmux/process checks.
   - This is the right tradeoff for UI responsiveness after the freeze regression.
   - References: `src-tauri/src/commands/coordination.rs:440`, `src-tauri/src/commands/coordination.rs:713`.

4. Startup cleanup is conservative and safe.
   - `reconcile_runtime_state_on_startup()` only clears stale runtime daemon PIDs and orphan runtime files. It does not perform heavy runtime probing on app startup.
   - Reference: `src-tauri/src/coordination/state.rs:107`.

## Findings

### 1. High: disband and initialize-failure cleanup still skip lead teardown entirely

`disband_team()` tears down only non-lead members, then stops the team daemon and deletes the team directory. That is safe for a Claude lead attached to an external pane, but incorrect for mesh-backed leads (`Codex`/`Gemini`) and for lead panes created by Taurhaus.

Impact:
- a mesh-backed lead can keep a live member daemon, live pane, and mesh membership after team disband
- `cleanup_initialize_failure()` delegates to `disband_team()`, so an initialize failure after `start_daemons` or `join_mesh` can leak lead-side resources too
- deleting the team directory hides the state, which makes the leak harder to detect

References:
- `src-tauri/src/coordination/orchestrator.rs:155`
- `src-tauri/src/coordination/orchestrator.rs:185`
- `src-tauri/src/coordination/pipelines/lifecycle.rs:15`
- `src-tauri/src/coordination/pipelines/initialize.rs:160`
- `src-tauri/src/coordination/pipelines/initialize.rs:176`

Assessment:
- broken for mesh-backed leads
- over-specialized around the "Claude lead" case

### 2. High: rollback paths kill daemon processes but do not clear member pidfiles

`cleanup_add_agent_failure()` and `cleanup_resume_failure()` terminate the newly started daemon process, but neither path clears the member pidfile.

Impact:
- stale pidfiles survive failed add/resume flows
- later runtime paths may read those stale pidfiles and make wrong decisions about daemon state
- this compounds the pidfile-identity problem below

References:
- `src-tauri/src/coordination/pipelines/lifecycle.rs:22`
- `src-tauri/src/coordination/pipelines/lifecycle.rs:27`
- `src-tauri/src/coordination/pipelines/lifecycle.rs:180`
- `src-tauri/src/coordination/pipelines/lifecycle.rs:185`

Assessment:
- rollback cleanup is incomplete
- the main remove-member teardown is stronger than these rollback helpers, which is inconsistent

### 3. High: team-daemon pidfile handling trusts any live PID without verifying process identity

`spawn_team_daemon()` reuses `team.pid` if the PID is merely alive. `stop_team_daemon()` kills the PID from `team.pid` if it is merely alive. `spawn_command_and_resolve_daemon_pid()` / `wait_for_daemon_pid_file()` also accept any live PID written to the pidfile without checking that it belongs to the daemon being started.

Impact:
- a stale reused PID can make Taurhaus think the team daemon is healthy when it is not
- disband/stop can kill an unrelated process if `team.pid` points at a recycled PID
- the same startup-verification weakness exists for member-daemon spawn resolution when a stale pidfile already exists

References:
- `src-tauri/src/coordination/runtime.rs:317`
- `src-tauri/src/coordination/runtime.rs:323`
- `src-tauri/src/coordination/runtime.rs:492`
- `src-tauri/src/coordination/runtime.rs:905`
- `src-tauri/src/coordination/runtime.rs:939`
- Contrast with member-daemon identity matching: `src-tauri/src/coordination/runtime.rs:978`

Assessment:
- this is the sharpest remaining process-safety issue in the lifecycle code
- member-daemon discovery already has `/proc` command matching; team-daemon lifecycle should reach the same bar

### 4. Medium: liveness self-heal is now too narrowly reachable

`reconcile_team_liveness()` contains the recovery logic that promotes stale `SessionDead` records, adopts existing daemons, terminates duplicates, and restarts missing non-Claude daemons for live panes. After the hot-path freeze fix, that method is only called from `resume_team_with_cli_commands_and_layout()`.

Meanwhile:
- startup only runs `reconcile_runtime_state_on_startup()`
- `coordination_get_live_team_status(...)` uses `get_team_status_fast(...)`
- `coordination_get_project_mesh_snapshot(...)` also uses `get_team_status_fast(...)`

Impact:
- a live pane with a dead/missing daemon can remain degraded indefinitely unless a team-resume action happens
- the current code removed over-eager reconciliation from polling, but did not replace it with a bounded explicit maintenance trigger
- naming is also misleading: `get_live_team_status` is not actually self-healing or runtime-validated

References:
- `src-tauri/src/coordination/orchestrator.rs:389`
- `src-tauri/src/coordination/orchestrator.rs:397`
- `src-tauri/src/coordination/state.rs:107`
- `src-tauri/src/commands/coordination.rs:440`
- `src-tauri/src/commands/coordination.rs:713`

Assessment:
- not a UI-performance regression
- but still a recovery-placement gap

## Overall Assessment

The code is no longer fundamentally inconsistent, but it still has two different reliability levels:
- mainline add/resume/remove flows are mostly coherent
- rollback and daemon-identity handling are still fragile

The biggest architectural mismatch is that the code now assumes pidfiles are trustworthy in places where previous bugs already proved they are not. That assumption should be removed.

## Recommended Follow-up Tasks

1. Fix disband and initialize-failure cleanup for mesh-backed or app-owned leads.
2. Clear member pidfiles in add-agent and resume rollback paths.
3. Add identity verification for team-daemon and daemon-start pidfile adoption/termination.
4. Add a bounded explicit self-heal entry point for liveness reconciliation outside `resume_team`.

## Residual Testing Gaps

1. No regression test currently covers disbanding a team whose lead is `Codex` or `Gemini`.
2. No regression test currently covers stale pidfiles after add-agent or resume rollback.
3. No regression test currently proves that `team.pid` cannot terminate/adopt the wrong process.
4. No integration coverage currently demonstrates when liveness self-heal is expected to run once the fast snapshot path intentionally avoids runtime probes.
