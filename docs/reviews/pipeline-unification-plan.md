# Pipeline Unification Implementation Plan

## Objective

Incrementally unify the initialize, resume, and add-agent member-activation flows without breaking the running app. Each task below is scoped to half a day or less and leaves start, resume, and add-agent functional at the end of the step.

This plan assumes:

- task `#27` is already in progress and is adding streamed per-member resume progress
- tasks `#28` and `#29` remain pending frontend follow-ups for freshness and summary UX
- the architectural target is a shared member-activation pipeline with wrapper-specific policies, not a big-bang merge of all wrappers into one function

## Planning Principles

- Stabilize vocabulary first. Canonical member-stage names must land before the progress event model spreads further.
- Migrate by wrapper, not by rewrite. Shared helpers and types land first; wrappers adopt them one at a time.
- Keep event compatibility during the transition. Do not force the frontend to switch to a brand-new payload shape in one step.
- Move the highest-risk divergences first. Onboarding ordering, session capture drift, and runtime commit drift are the main bug generators.
- Keep wrapper semantics explicit. Initialize, resume, and add-agent will continue to differ on team creation, rollback, and barrier behavior.

## Phases

### Phase 0: Lock shared vocabulary around in-flight progress work

Goal:

- ensure task `#27` does not invent resume-only stage names or payload semantics that must be redesigned later

#### Task P0-T1: Define canonical member activation stage names

Category: backend
Scope: half-day

Inputs:

- [pipeline-unification-assessment.md](/home/mstie/projects/taurhaus/docs/reviews/pipeline-unification-assessment.md)
- task `#27` current streamed resume event work
- existing initialize/resume/add-agent stage names

Outputs:

- canonical shared stage vocabulary in Rust types/docs
- mapping table from existing stage names to canonical names

Acceptance criteria:

- a single authoritative stage list exists for member activation
- task `#27` can emit canonical shared member-stage names instead of resume-only names
- initialize/add-agent mapping can be expressed without losing meaning

Notes:

- Recommended canonical stages: `prepare_member`, `acquire_pane`, `launch_session`, `capture_session_identity`, `join_mesh`, `start_member_daemon`, `commit_runtime`, `deliver_onboarding`
- Keep existing top-level initialize batch stages for the current UI until later migration

#### Task P0-T2: Add an event-compatibility adapter for streamed progress

Category: integration
Scope: half-day

Inputs:

- canonical stage vocabulary from `P0-T1`
- current `coordination-step-progress` event usage
- task `#27` backend implementation

Outputs:

- compatibility layer or mapping helper that lets resume progress emit canonical stages without breaking current frontend assumptions

Acceptance criteria:

- `resume_team` progress can use canonical member-stage names
- existing initialize event handling still works
- no frontend break is introduced by task `#27`

Notes:

- Prefer additive event metadata over breaking event schema changes
- If a new resume-specific event is added, it should still use canonical shared stage names

### Phase 1: Extract shared member-activation contracts and helpers

Goal:

- create the minimal shared substrate before migrating wrapper behavior

#### Task P1-T1: Introduce a normalized member activation context

Category: backend
Scope: half-day

Inputs:

- current initialize, resume, and add-agent request/member data

Outputs:

- internal context type describing operation kind, member identity, lead identity, pane policy, delivery policy, roster policy, and runtime commit policy

Acceptance criteria:

- initialize/add-agent/resume can all construct the context without changing user-facing behavior
- no wrapper is migrated yet; this task only adds types/builders

#### Task P1-T2: Extract shared session launch and identity capture helper

Category: backend
Scope: half-day

Inputs:

- `capture_session_id_for_member`
- `launch_resume_session`
- `launch_session_for_agent`

Outputs:

- one canonical helper for launch/session detection behavior

Acceptance criteria:

- Claude/Codex session detection logic exists in one shared path
- initialize, resume, and add-agent can call the shared helper
- existing tests for session id/jsonl path capture still pass after adaptation

#### Task P1-T3: Extract shared mesh join and member-daemon helpers

Category: backend
Scope: half-day

Inputs:

- initialize inline `join_mesh` / `start_daemons`
- resume `resume_join_mesh` / `resume_start_daemon`
- add-agent `join_mesh_for_agent` / `start_daemon_for_agent`

Outputs:

- shared helper(s) for `join_mesh_if_required` and `start_member_daemon_if_required`

Acceptance criteria:

- wrapper-specific policy hooks still allow stale-pid termination for resume
- member tool differences remain correct for Claude vs mesh-sidecar tools
- current wrapper behavior is unchanged

#### Task P1-T4: Move team-daemon ownership to wrapper-level helpers

Category: backend
Scope: half-day

Inputs:

- current `ensure_team_daemon_running_best_effort` / `spawn_team_daemon` calls in initialize/resume/add-agent

Outputs:

- explicit wrapper-level helper for team-daemon ensure
- removal of duplicate member-level team-daemon ensuring where unnecessary

Acceptance criteria:

- each wrapper has one clear place that owns team-daemon ensuring
- no duplicate team-daemon side effects remain in the migrated paths
- start/resume/add-agent still leave the team daemon running as before

### Phase 2: Make onboarding ordering and runtime commit policies explicit

Goal:

- remove the most dangerous implicit divergence before broader wrapper migration

#### Task P2-T1: Introduce an explicit onboarding delivery policy

Category: backend
Scope: half-day

Inputs:

- initialize `send_onboarding_messages`
- resume `resume_send_onboarding`
- add-agent `send_onboarding_for_agent`
- commit `3b17397` and its retry semantics

Outputs:

- policy model such as `deferred_barrier` vs `immediate`
- shared onboarding delivery entry point using that policy

Acceptance criteria:

- onboarding ordering is chosen by wrapper policy, not by incidental helper placement
- retry behavior from `3b17397` is preserved
- initialize/resume/add-agent continue to work after the refactor

#### Task P2-T2: Add regression tests for onboarding barrier semantics

Category: testing
Scope: half-day

Inputs:

- current onboarding race history
- new delivery policy abstraction

Outputs:

- regression coverage for:
  - initialize deferred-barrier onboarding
  - resume immediate onboarding or later chosen policy
  - inbox-not-ready delivery retry behavior

Acceptance criteria:

- the original race class is documented in tests
- policy differences are intentional and test-visible

#### Task P2-T3: Introduce a shared runtime commit helper

Category: backend
Scope: half-day

Inputs:

- initialize partial runtime writes
- resume `update_runtime`
- add-agent `update_roster`

Outputs:

- shared helper for committing member runtime/session/pane/daemon state

Acceptance criteria:

- pane id, session id, jsonl path, daemon pid, health, and metadata sync are committed through one path
- initialize can still preseed baseline runtime records separately
- wrapper behavior remains unchanged

### Phase 3: Migrate wrappers onto the shared member-activation stages

Goal:

- start consuming the shared substrate in working increments

#### Task P3-T1: Migrate resume member to the shared member-activation executor

Category: backend
Scope: half-day

Inputs:

- shared context/helpers from phases 1-2
- current `resume_member_with_cli_commands_and_layout`

Outputs:

- resume member implemented as a thin wrapper over the shared executor

Acceptance criteria:

- step order and outcome semantics remain unchanged for resume member
- task `#27` progress emission still works and uses canonical stage names
- existing resume-member tests continue to pass

#### Task P3-T2: Migrate add-agent to the shared member-activation executor

Category: backend
Scope: half-day

Inputs:

- shared executor from `P3-T1`
- current add-agent wrapper

Outputs:

- add-agent wrapper using shared execution with create-member and rollback policies

Acceptance criteria:

- add-agent still leaves the system working
- onboarding/delivery auditing still routes correctly
- rollback behavior remains member-scoped

#### Task P3-T3: Add wrapper-level progress mapping helpers

Category: integration
Scope: half-day

Inputs:

- canonical member-stage vocabulary
- shared executor
- current initialize batch progress and resume member progress

Outputs:

- mapping helpers from shared member stages to:
  - resume member/team streamed progress
  - current initialize top-level progress

Acceptance criteria:

- task `#27` keeps working on the migrated resume path
- initialize can continue exposing its existing high-level steps for now
- no frontend break is introduced

### Phase 4: Migrate initialize without breaking current UX

Goal:

- move initialize onto the shared member activation engine while preserving current team-setup behavior and UI

#### Task P4-T1: Introduce an initialize wrapper that pre-seeds team and roster, then delegates member activation

Category: backend
Scope: half-day

Inputs:

- shared executor
- existing initialize wrapper
- roster seeding logic

Outputs:

- initialize wrapper that still:
  - validates full team config
  - creates the team
  - pre-seeds roster/runtime baseline
  - delegates member activation through the shared executor per member

Acceptance criteria:

- initialize still works end to end
- failure still tears down the team as today
- no frontend changes are required yet

#### Task P4-T2: Preserve current initialize UI semantics through a batch-stage adapter

Category: integration
Scope: half-day

Inputs:

- shared member-stage execution
- current initialize step list used by `MeshInitProgress`

Outputs:

- adapter that translates shared member-activation execution back into current initialize batch steps

Acceptance criteria:

- `MeshInitProgress` still receives the same high-level initialize step semantics
- initialize no longer depends on bespoke inline loops for launch/join/daemon/onboarding behavior
- the app remains working after the migration

#### Task P4-T3: Add cross-wrapper parity tests

Category: testing
Scope: half-day

Inputs:

- migrated resume member, add-agent, initialize

Outputs:

- tests proving shared stage behavior is aligned across wrappers where it should be

Acceptance criteria:

- parity coverage exists for session capture, mesh join skip behavior, daemon start rules, onboarding policy, and runtime commit
- legitimate wrapper differences are explicitly asserted rather than assumed

### Phase 5: Finish frontend alignment and cleanup

Goal:

- land the remaining frontend work after the backend contracts stop moving

#### Task P5-T1: Land task `#28` after canonical stage/event stabilization

Category: frontend
Scope: half-day

Inputs:

- stable live-status normalization
- canonical progress vocabulary already fixed in prior phases

Outputs:

- frontend support for `runtime_snapshot_freshness`

Acceptance criteria:

- runtime bar/view clearly distinguishes live, cached, and attachment-only state
- no coupling to unstable progress payload shapes remains

#### Task P5-T2: Land task `#29` after resume progress semantics stabilize

Category: frontend
Scope: half-day

Inputs:

- stable `ResumeTeamReport` semantics
- task `#27` resume progress UI

Outputs:

- final resume summary includes warnings and team-daemon outcome

Acceptance criteria:

- resume completion copy reflects warning and daemon state honestly
- no backend contract changes are needed after UI work lands

#### Task P5-T3: Remove obsolete wrapper-specific helpers and dead mappings

Category: backend
Scope: half-day

Inputs:

- all wrappers migrated
- compatibility adapters no longer needed

Outputs:

- deleted dead helper paths, duplicate launch/session helpers, and transitional mappings

Acceptance criteria:

- there is one canonical member-activation execution path
- wrapper-specific code is policy-oriented, not stage-implementation-oriented

## Task Inventory

Total tasks: 18

By phase:

- Phase 0: 2 tasks
- Phase 1: 4 tasks
- Phase 2: 3 tasks
- Phase 3: 3 tasks
- Phase 4: 3 tasks
- Phase 5: 3 tasks

By category:

- Backend: 11
- Frontend: 2
- Integration: 3
- Testing: 2

Note:

- some tasks blend backend and integration concerns; the category listed is the primary owner lane

## Dependency Graph

### Core dependency chain

- `P0-T1` blocks `P0-T2`, `P3-T3`, `P5-T1`, and any finalization of task `#27`
- `P0-T2` blocks safe completion of task `#27`
- `P1-T1` blocks `P1-T2`, `P1-T3`, `P2-T1`, `P2-T3`
- `P1-T2`, `P1-T3`, and `P2-T3` block `P3-T1`
- `P2-T1` blocks `P2-T2`, `P3-T1`, `P3-T2`, `P4-T1`
- `P3-T1` blocks `P3-T3`
- `P3-T1` and `P2-T3` block `P3-T2`
- `P3-T1`, `P3-T2`, and `P3-T3` block `P4-T1`
- `P4-T1` blocks `P4-T2` and `P4-T3`
- `P4-T2` blocks final cleanup in `P5-T3`
- stable backend contracts from phases 0-4 block `P5-T1` and `P5-T2`

### Parallelizable groups

- `P1-T2` and `P1-T3` can run in parallel after `P1-T1`
- `P2-T2` can run in parallel with `P2-T3` after `P2-T1`
- `P5-T1` and `P5-T2` can run in parallel after backend/event stabilization

### External dependencies

- task `#27` should complete after `P0-T1` and `P0-T2` concepts are agreed, even if the exact code lands in dev-1's lane
- tasks `#28` and `#29` should wait until the event vocabulary and backend semantics stop moving in phases 0-4

## Ordering Strategy

### Recommended execution order

1. `P0-T1`
2. `P0-T2`
3. Allow task `#27` to align and continue on canonical stage names
4. `P1-T1`
5. `P1-T2` and `P1-T3` in parallel
6. `P1-T4`
7. `P2-T1`
8. `P2-T2` and `P2-T3` in parallel
9. `P3-T1`
10. `P3-T2`
11. `P3-T3`
12. `P4-T1`
13. `P4-T2`
14. `P4-T3`
15. `P5-T1` and `P5-T2` in parallel
16. `P5-T3`

### Why this order

- Risk reduction first:
  - stage vocabulary
  - onboarding policy
  - launch/session detection
- Shared substrate before wrapper migration:
  - extract types/helpers first so resume/add-agent migrations are thin
- Resume path before initialize:
  - resume is already closest to the target architecture
  - it is the most active path due to task `#27`
- Initialize last:
  - initialize currently has the most wrapper-specific semantics and UI coupling
- Frontend late:
  - task `#28` and `#29` should not chase moving backend/event contracts

## Migration Approach

### Incremental transition

Use an adapter-based migration, not a branch rewrite:

1. Define canonical internal types and stage names.
2. Add shared helpers while old wrappers still call their legacy code.
3. Switch one wrapper at a time to the shared executor.
4. Keep compatibility adapters for existing progress/UI contracts during the transition.
5. Remove dead helper paths only after all wrappers have been migrated.

### Working-state rule

Each migration step must preserve:

- initialize still launches teams successfully
- resume still resumes individual members and full teams
- add-agent still adds one member safely
- current frontend progress/UI contracts still receive valid payloads

### Suggested merge boundaries

- one PR per task
- no PR should mix:
  - stage vocabulary definition
  - helper extraction
  - wrapper migration
  - frontend UX cleanup

This keeps blame clear and rollback easy.

## Risk Notes

### Phase 0 risks

- task `#27` could hard-code resume-only stage names before the shared vocabulary lands
- a rushed event schema change could break the current frontend listener

Watch for:

- duplicated stage naming in Rust and JS
- frontend assumptions about `operation` and step labels

### Phase 1 risks

- helper extraction can accidentally alter sequencing even if behavior looks unchanged
- launch/session detection changes can regress tool-specific behavior for Claude or Codex

Watch for:

- session id/jsonl path persistence drift
- mesh-sidecar skip behavior for Claude members

### Phase 2 risks

- making onboarding policy explicit can accidentally change delivery timing in one wrapper
- runtime commit extraction can expose hidden assumptions in liveness or snapshot code

Watch for:

- inbox readiness regressions
- stale runtime state becoming visible earlier or later than before

### Phase 3 risks

- migrating resume/add-agent first could break task `#27` progress reporting if event mapping is incomplete
- wrapper rollback semantics may be lost if the shared executor over-generalizes

Watch for:

- partial success handling in resume team
- audited onboarding delivery in add-agent

### Phase 4 risks

- initialize migration has the highest chance of breaking the setup UI because current progress is batch-oriented
- initialize cleanup semantics may regress if the new wrapper does not preserve whole-team teardown on failure

Watch for:

- initialize failure rollback
- current `MeshInitProgress` step order and user-visible messages

### Phase 5 risks

- deleting transitional adapters too early can strand the frontend on stale assumptions
- frontend summary/freshness work can accidentally reintroduce backend coupling if done before contracts stabilize

Watch for:

- hidden backend fields still being normalized ad hoc in JS
- dead helper code that still has one remaining caller

## Immediate Coordination Notes

For dev-1 on task `#27`:

- use canonical shared member-stage names from `P0-T1`
- avoid resume-specific stage wording in emitted event payloads
- keep payload design additive so initialize can adopt the same vocabulary later

For tasks `#28` and `#29`:

- do not start until the stage vocabulary and resume progress payload shape are stable enough that the frontend will not need a second redesign

## Recommended Next Action

Create and assign tasks in this order:

1. `P0-T1` canonical stage vocabulary
2. `P0-T2` event compatibility adapter / task `#27` alignment
3. `P1-T2` shared launch/session helper
4. `P2-T1` explicit onboarding policy

Those four tasks reduce the largest architectural risks fastest while keeping the app working at every step.
