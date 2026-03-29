# Mesh Resume/Start Progress UX Assessment

## Summary

The current Mesh UX treats initial team start and team resume very differently:

- Initial start has a dedicated initializing mode, a visible step list, elapsed time, failure recovery, and a streamed step event channel.
- Team resume has only a runtime-bar CTA plus a static per-member placeholder list that stays unchanged until the blocking `coordination_resume_team` IPC call returns.

That asymmetry creates the "dead air" problem users are reporting. The backend already has enough structure to do materially better than the current resume UX, but the useful resume data exists at the wrong layer today:

- `resume_member` produces step-by-step reports.
- `resume_team` aggregates those reports only after all members finish.
- the frontend only receives the aggregate report at the end.

The most practical recommendation is to add a dedicated streamed resume-progress event for team resume and render it as per-member, per-stage progress in the runtime view. That preserves the current architecture, reuses existing resume-member semantics, and avoids exposing raw daemon/tmux noise as the primary UX.

## Current UX Flow

### 1. Entry and discovery

On Mesh tab hydration, the frontend:

- shows `gate` mode with "Checking project team state..." when no cached snapshot exists
- requests `coordination_get_project_mesh_snapshot`
- classifies the team as `none`, `coldResume`, `degraded`, or `active`
- switches to `empty` or `runtime` based on whether a team was discovered

Current user-visible states:

- `gate`: short static copy only
- `runtime`: team canvas + runtime bar
- `empty`: builder

Current gap:

- discovery does not expose refresh/poll freshness to the user, even though runtime status may still be attachment-only or pending a live refresh

### 2. Initial start flow

Initial team creation goes through `MeshSetupView` + `MeshInitProgress`.

What the user sees:

- dedicated `initializing` mode
- static mesh canvas preview
- ordered step list
- running/succeeded/failed glyphs
- brief per-step descriptions while a step is running
- elapsed timer
- explicit success/failure recovery affordances

Current visible steps:

1. `validate_configuration`
2. `create_team`
3. `create_panes`
4. `launch_sessions`
5. `join_mesh`
6. `start_daemons`
7. `send_onboarding`

Strengths:

- user always sees that work is progressing
- failures are attached to a named step
- the event model already exists and is frontend-consumable

Remaining gap:

- several long steps are still coarse batches
- `create_panes`, `launch_sessions`, `join_mesh`, `start_daemons`, and `send_onboarding` can take noticeable time with no per-member breakdown

### 3. Team resume flow

Resume happens inside runtime mode via `MeshRuntimeBar` and `meshTabRuntime.svelte.js`.

What the user sees after clicking resume:

- runtime actions disable
- a progress panel appears
- each target member is listed immediately
- every listed member starts in `pending` with "Waiting to resume"
- the panel stays static until the full `coordination_resume_team` IPC returns
- once the call completes, all members update at once to `Resumed` / `Failed`
- a final summary message appears

Current dead-air gaps:

- no indication of which member is actively being resumed
- no stage detail inside a member resume
- no distinction between "request sent", "pane created", "CLI launched", "mesh joined", "daemon started", and "onboarding delivered"
- no visibility into team-daemon work
- no elapsed time, no "still working" heartbeat, no activity stream

This is the core UX problem.

## Current Backend Signals Available Today

### Signals already exposed to the frontend

#### Project/runtime discovery

`coordination_get_project_mesh_snapshot` exposes:

- team presence
- `teamRuntimeState` (`coldResume`, `degraded`, `active`, `none`)
- per-member attachment-derived status in the fast snapshot
- environment availability warnings (`meshAvailable`, `tmuxAvailable`, warnings)

`coordination_get_live_team_status` exposes:

- per-member live `sessionStatus`
- `paneId`
- member metadata used by the runtime canvas

These power the runtime bar and canvas, but they are periodic snapshots, not operation progress.

#### Step-progress event channel

The app already has a Tauri event channel, `coordination-step-progress`, and a frontend listener, `onCoordinationStepProgress`.

Today that channel is emitted for:

- `initialize_team`
- `add_agent`
- `resume_member`

It is not emitted for:

- `resume_team`

That is the most important architectural finding in this review.

#### Final team resume report

`coordination_resume_team` returns a final `ResumeTeamReport` with:

- `totalMembers`
- `resumedMembers`
- `failedMembers`
- `warnings`
- `startedTeamDaemon`
- `teamDaemonWarning`

The frontend uses only the member success/failure lists to build the runtime resume panel after completion.

Current gap:

- `warnings`
- `startedTeamDaemon`
- `teamDaemonWarning`

are not surfaced meaningfully in the resume UX.

### Signals that exist but are currently dropped or hidden

#### Runtime snapshot freshness

Backend live-status responses include `runtime_snapshot_freshness` (`fresh`, `cached`, `attachments_only`).

This is architecturally useful because it tells the UI whether it is showing real live state or only attachment-derived state. The frontend normalization currently drops that field, so the user cannot tell whether the runtime view is authoritative yet.

#### Resume-member stage detail

The member-resume pipeline already has meaningful steps:

1. `validate`
2. `load_member`
3. `resolve_pane`
4. `launch_session`
5. `join_mesh`
6. `start_daemon`
7. `send_onboarding`
8. `update_runtime`

Those steps already exist in `ResumeAgentReport.steps`. During team resume, they are computed internally for each member, but only the final aggregate outcome escapes to the frontend.

#### Backend structured logs

The backend emits structured log events for:

- `coordination.step.*`
- `coordination.pipeline.*`
- `daemon.rpc.*`

These are useful for diagnostics and could support a future activity drawer, but they are not currently exposed as a user-facing progress channel.

### Signals that likely exist only as internal runtime behavior

The lower layers know when:

- a pane was reused vs created
- a CLI launch command was sent
- mesh join completed
- a per-member daemon was spawned
- the team daemon was ensured

Some of these are already summarized in reports; others are only implicit in internal calls and logs. They are feasible to surface, but not all are currently shaped as stable IPC payloads.

## Resume vs Initial Start: Key Differences

### Initial start

- explicit dedicated mode
- step stream already wired into the UI
- operation narrative is stage-based
- failure handling is step-specific
- user sees elapsed time and ongoing activity

### Resume

- stays in runtime mode
- only aggregate result is returned
- operation narrative is absent during execution
- team daemon work is invisible
- panel content is mostly placeholders until completion

Architecturally, initial start is a first-class long-running workflow. Resume is still implemented as a blocking mutation with a post-hoc summary.

## Feasible Additional Signals

### Low to medium complexity

#### 1. Streamed team-resume progress events

Add a dedicated event payload for team resume, for example:

- `operation: "resume_team"`
- `memberName`
- `memberIndex`
- `memberCount`
- `stage`
- `status`
- `message`

This could be emitted by the command layer while iterating members, or by the orchestrator via a progress callback.

Why this is attractive:

- it matches the mental model users need
- it does not require exposing raw tmux/daemon internals
- it reuses already-defined member-resume stages

#### 2. Expose runtime freshness to the frontend

Surface `runtime_snapshot_freshness` in the normalized live-status response and render small status copy such as:

- "Live runtime confirmed"
- "Using cached runtime snapshot"
- "Showing attachment-only state while live status refreshes"

This improves trust during both resume and initial load with very little UX risk.

#### 3. Surface final team-daemon status

The resume result already knows:

- whether the team daemon was started
- whether there was a warning ensuring it

That should appear in the final resume summary, especially when member resumes succeed but daemon ensuring is degraded.

### Medium complexity

#### 4. Add per-member progress to initialization

Initialization already has a top-level step stream, but the longest steps are still opaque batches. Extending the init stream with optional member-scoped progress would improve honesty during larger team creation flows.

Suggested model:

- keep current top-level steps
- optionally nest member progress underneath long-running steps such as `create_panes`, `launch_sessions`, `join_mesh`, `start_daemons`, and `send_onboarding`

#### 5. Add operation elapsed time and heartbeat copy to resume

Even before deeper event plumbing, resume should show:

- elapsed time
- current member count completed, e.g. `2/5 members resumed`
- current active member/stage if known

This is medium complexity only if tied to real streamed progress. If implemented without real signals, it risks becoming decorative theater.

### High complexity

#### 6. Activity log / event stream drawer

Expose a domain-level event stream for advanced users:

- "Opening pane for reviewer"
- "Launching Codex in %7"
- "Joining mesh for reviewer"
- "Starting daemon for reviewer"
- "Ensuring team daemon"

This could be fed either by a new progress event model or by adapting structured backend log events.

Tradeoff:

- valuable for diagnostics
- easy to overbuild
- raw `daemon.rpc.*` and tmux detail will be too noisy for the default UX

Recommendation: keep this as an optional secondary surface, not the primary progress UI.

## Ranked Recommendations

### 1. Add streamed team-resume progress and render per-member stages

Rank: Highest
Complexity: Medium

Recommendation:

- introduce a dedicated `resume_team` progress event stream
- show one active member at a time with explicit stage copy
- keep the existing per-member result list, but update rows live instead of only at the end

Suggested UX:

- header: `Resuming 2 of 5 members`
- active row copy: `reviewer -> Starting CLI session`
- completed rows lock to `Resumed`
- failed rows show step-specific failure copy
- footer summary shows team-daemon outcome when the run finishes

Why this should be first:

- directly fixes the dead-air complaint
- aligns resume with the architectural shape already used by initialize
- provides honest progress without requiring a major redesign

### 2. Surface runtime freshness and loading authority in runtime mode

Rank: High
Complexity: Low

Recommendation:

- preserve `runtime_snapshot_freshness` through frontend normalization
- show a small runtime-status badge or subtitle in the bar/panel

This clarifies whether the user is seeing live runtime truth or attachment-derived fallback state, which matters during cold resume and shortly after initialization.

### 3. Expand final resume summary to include warnings and team-daemon outcome

Rank: High
Complexity: Low

Recommendation:

- show `warnings` from `ResumeTeamReport`
- show whether the team daemon was ensured successfully
- distinguish:
  - member resume success with daemon warning
  - partial member failure
  - no members needed resume

This closes a functional honesty gap. The backend already knows more than the UI currently admits.

### 4. Add optional member-scoped sub-progress inside initialization

Rank: Medium
Complexity: Medium to High

Recommendation:

- preserve the current stage list
- add optional per-member detail under long-running initialize steps

This is valuable, but it is a second-order improvement. Resume is the sharper problem because it currently has near-zero live feedback.

### 5. Add an optional advanced activity stream

Rank: Medium
Complexity: High

Recommendation:

- only pursue after structured resume progress exists
- keep it collapsible or secondary
- translate raw backend events into domain language rather than exposing raw log records

This is useful for debugging and power users, but not necessary to solve the primary UX issue.

## Technical Constraints and Tradeoffs

### Constraint 1: `resume_team` is currently architected as an aggregate call

The current command path calls `resume_team_with_cli_commands_and_layout` and returns only the final aggregate report. That means the frontend cannot know which member is in flight without new progress plumbing.

Implication:

- polling live team status alone is not enough for good progress UX
- polling can show members flipping from offline to active, but it cannot explain what is happening before that

### Constraint 2: existing step events are operation-shaped, not member-scoped

The existing `coordination-step-progress` payload only includes:

- `teamName`
- `operation`
- `progress`

It does not include member identity. That is fine for initialize, but insufficient for team resume progress where the user needs per-member granularity.

Implication:

- either extend the existing event schema
- or add a dedicated resume-progress event with member metadata

Recommendation:

- prefer a dedicated payload rather than overloading the current generic event too aggressively

### Constraint 3: raw low-level events are not good default UX

Daemon RPCs and tmux actions are valuable telemetry, but they are not the right primary surface for most users.

Implication:

- do not make the default progress UI a firehose of raw runtime activity
- translate low-level events into domain stages first

### Constraint 4: avoid architectural theater

It would be easy to add animated copy, timers, or rotating labels without new backend truth. That would improve cosmetics but not honesty.

Implication:

- any new "active stage" UI should be backed by a real progress signal
- synthetic estimated timing should be clearly labeled and secondary

## Recommended Implementation Direction For Follow-Up Discussion

If the team wants the smallest worthwhile change:

1. Keep the current runtime layout.
2. Add a streamed `resume_team` progress payload with member + stage + status.
3. Update the existing resume panel live as each member advances.
4. Surface final warnings and team-daemon outcome.
5. Preserve `runtime_snapshot_freshness` and show it in the runtime bar.

If the team wants a broader long-running-operations pattern:

1. Define a shared operation-progress model that supports optional `memberName` and `stageCategory`.
2. Use it for initialize, add-agent, resume-member, and resume-team.
3. Keep raw structured logs separate from user-facing progress.

That broader model is cleaner long-term, but it is not required to fix the current user pain.

## Bottom Line

The current resume UX is under-instrumented, not under-designed. The UI already has a place to show progress; what it lacks is truthful mid-flight signal data.

The best next step is not a visual redesign. It is to promote team resume from a blocking aggregate mutation to a streamed operation with member-scoped progress updates, then render those updates in the existing runtime progress panel.
