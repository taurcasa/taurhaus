# Auto Pane-ID Reconciliation for Restarted Team Agents

Date: 2026-03-07
Task: #475

## Conclusion

The minimal fix is not a scanner-driven reconciliation loop.

The minimal fix is to make the generic taurhaus resume/continue entrypoint delegate to the existing coordination resume pipeline when the requested session belongs to a known team member.

Reason:

- the coordination resume pipeline already resolves a new pane, persists `pane_id`, rewrites `config.json`, rejoins mesh, and restarts the per-agent mesh daemon
- the generic command-center launch path currently does none of those things
- session-scanner reconciliation or tmux hooks would be broader, more ambiguous, and still need extra work to relaunch per-agent daemons

## What is the source of truth today?

`config.json` is not the authoritative pane source.

The authoritative pane ID is the per-member runtime record at:

- `teams/<team>/runtime/<member>.json`

That runtime record contains `pane_id`, `session_id`, `daemon_pid`, health, and timestamps.

`config.json` only gets `tmuxPaneId` when `TeamConfigStore::save()` serializes the team and projects runtime metadata into the mesh-compatible wire format.

Current write pattern:

1. initialization / add-agent / coordination-resume writes `runtime/<member>.json`
2. `sync_team_config_metadata()` reloads the team config and saves it again
3. that save rewrites `config.json` with `tmuxPaneId` populated from runtime

So the real question is not "who writes config pane IDs?" but "which flows update runtime and then trigger config sync?"

## Where pane IDs are updated correctly today

### Coordination-aware resume

The mesh resume pipeline already handles pane replacement correctly.

In `src-tauri/src/coordination/pipelines/members.rs`:

- `resolve_resume_pane()` decides whether to reuse the old pane or create a new one
- on success, the pipeline writes the new `runtime_record.pane_id`
- then it saves runtime and calls `sync_team_config_metadata()`

In `src-tauri/src/coordination/pipelines/lifecycle.rs`:

- `resume_join_mesh()` rejoins mesh membership
- `resume_start_daemon()` starts the per-agent mesh daemon against the resolved pane

This is already the behavior we want.

## Where the stale pane bug comes from

### Generic command-center launch / resume

The generic sidebar/context-menu session launcher goes through `launch_cli_session` in `src-tauri/src/commands/command_center.rs`.

That path:

- resolves the project path
- launches or resumes the CLI in tmux
- returns the new `tmux_pane`

But it does not:

- look up team membership
- update `runtime/<member>.json`
- rewrite `config.json`
- rejoin mesh
- restart the per-agent mesh daemon

So if a team agent is restarted through this generic path, taurhaus knows the new pane for that one launch result, but team runtime/config remain stale.

That explains the observed symptom:

1. old team runtime still points at the dead pane
2. `config.json` still projects that stale pane as `tmuxPaneId`
3. mesh/live-team views still read the stale runtime pane
4. non-Claude members also keep stale daemon state because their mesh daemon was tied to the old pane

## Can the session scanner reconcile this instead?

### Technically possible

Yes, partially.

The session scanner can detect a live CLI process in a new tmux pane, and command-center enrichment already has logic to associate sessions with team members by project path, CLI tool, and runtime pane hints.

### Why it is not the best minimal fix

It is a weaker fit for the real problem.

Problems:

1. matching is inherently heuristic outside the explicit resume flow
   - project path + tool can be ambiguous
   - same-tool teammates on one project need extra matching logic
2. scanner-based reconciliation would run after the fact, not at the exact point where the new pane is created
3. it only solves pane discovery, not mesh daemon relaunch by itself
4. it risks updating runtime for sessions that are not meant to be bound to a team member

Conclusion:

A scanner-based fallback could be useful later as a drift repair mechanism, but it should not be the primary fix.

## Can mesh rejoin/reconnect update pane ID?

Not by itself.

The mesh join/rejoin flow is team/member oriented. It does not discover tmux pane changes independently.

The useful path is the coordination resume pipeline because it already has the new pane ID before mesh rejoin and daemon restart happen.

So the answer is:

- mesh rejoin is necessary for non-Claude agents
- mesh rejoin is not the right discovery mechanism for pane drift
- it should consume the new pane ID from the resume flow, not try to discover it later

## Would tmux event hooks be better?

Not for this scope.

A tmux hook approach could theoretically watch pane creation/destruction and trigger reconciliation, but it is overkill here.

Problems:

- hook plumbing is more invasive than the current architecture needs
- pane events still do not tell us which logical team member should own the new pane without extra correlation logic
- we would still need a second step to update runtime/config and restart mesh daemons
- cross-platform behavior and failure handling get more complicated quickly

Conclusion:

Tmux hooks are harder than the problem requires.

## Recommended implementation

### Primary change

When the user uses the generic resume/continue action, detect whether the request maps to a unique team member and, if so, delegate to coordination resume instead of doing a raw command-center launch.

Recommended rule:

- scope only `LaunchMode::Resume` and `LaunchMode::Continue`
- resolve `project_id -> project_path`
- search all team configs for members where:
  - `member.project_path == project_path`
  - `member.cli_tool == requested cli tool`
- if there is exactly one matching member, call the coordination resume pipeline for that `(team_name, member_name)`
- return its resolved `pane_id` as the launch result
- if there are zero matches, keep the existing generic launch behavior
- if there are multiple matches, keep the existing generic launch behavior and log a warning rather than guessing

Why this is the smallest correct change:

- no new reconciliation subsystem
- no tmux hooks
- no duplicate daemon-restart logic
- no heuristic post-facto repair for the primary path
- reuses the already-tested coordination resume semantics

### Required behavior after delegation

The delegated path should preserve the existing command-center UX contract:

- caller still gets `(tmux_session, tmux_window, tmux_pane)` back
- terminal opening/focus behavior remains unchanged
- if coordination resume fails, return that failure clearly instead of silently falling back to a raw launch

### Secondary fallback (optional, later)

If more resilience is needed later, add a low-frequency reconciliation job that only repairs obvious one-to-one matches:

- team member runtime pane missing/dead
- exactly one scanned live session matches `(project_path, cli_tool)`
- no competing team member matches that tuple

That should be treated as a safety net, not the main path.

## Implementation sketch

1. Add a helper in coordination/command-center boundary code:
   - input: resolved project path + `CliTool`
   - output: `None`, one unique `(team_name, member_name)`, or ambiguous

2. In `launch_cli_session_impl`:
   - before raw tmux launch, if mode is `resume` or `continue`, try that lookup
   - if unique match exists, call the same orchestrator path used by `coordination_resume_member`

3. Convert the `ResumeAgentReport` into `LaunchSessionResult`:
   - `tmux_session`: existing tmux session constant / detected session
   - `tmux_window`: pane window if known, or derive the same way command-center does today
   - `tmux_pane`: `report.pane_id`

4. Keep raw `launch_cli_session` for:
   - non-team sessions
   - ambiguous team matches
   - fresh new sessions

5. Add tests for:
   - unique team-member generic resume delegates and updates runtime/config
   - delegated resume restarts daemon for non-Claude agents
   - ambiguous match does not guess
   - non-team session still uses raw launch path

## Direct answers to the investigation questions

1. Where does pane ID get written today?

- authoritative write: `MemberRuntimeStore::save()`
- projected config write: `TeamConfigStore::save()` via `sync_team_config_metadata()`
- this already happens during initialization, add-agent, and coordination resume
- it does not happen in generic command-center resume/continue

2. Can the session scanner detect a new pane and update config?

- yes, in principle
- but it would be heuristic and is not the best minimal fix

3. Can mesh rejoin/reconnect update the pane ID?

- not as the discovery mechanism
- but the coordination resume flow can pass the new pane into mesh rejoin and daemon restart, which is the right place to do it

4. Event-driven tmux hooks or periodic reconciliation?

- periodic reconciliation is simpler than tmux hooks
- neither is the best first fix
- direct resume-aware delegation is simpler than both

5. Minimal change to make this just work?

- route generic resume/continue for uniquely matched team members through the coordination resume pipeline

## Recommendation

Implement the resume-aware delegation first.

That addresses both stale `tmuxPaneId` and stale per-agent mesh daemons at the exact moment taurhaus creates the replacement pane, using code that already exists.
