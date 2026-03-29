# Initialize vs Resume Pipeline Unification Assessment

## Summary

Initialize and resume already share most of the same member-activation concerns:

- acquire a pane
- launch a CLI session
- detect/capture session identity
- join mesh when required
- start a member daemon when required
- deliver onboarding/operator notice
- persist runtime state

But they do not share a single member-level execution model. Instead:

- initialize is a batch team pipeline with broad team-level stages that loop over all members inside each stage
- resume is a true per-member activation pipeline wrapped by `resume_team`

That split is the main source of divergence risk. The onboarding race behind commit `3b17397` is the clearest example: initialize had an implicit team-wide readiness barrier before onboarding, while resume delivered onboarding immediately after each member became individually ready. The retry in `3b17397` mitigates the symptom, but it does not remove the structural divergence.

The recommendation is to unify around a shared member-activation pipeline with wrapper-specific policies:

- initialize, resume, and add-agent become orchestration wrappers
- shared member stages execute through one canonical path
- wrappers decide which stages are allowed, when to commit runtime state, and whether onboarding is immediate or held behind a barrier

That approach gives the team one place to fix activation bugs, one place to emit progress events, and one place to evolve stage semantics.

## Current Pipeline Shapes

### Initialize

`initialize_team_with_cli_commands_and_layout` is a team-batch pipeline:

1. `validate_configuration`
2. `create_team`
3. `add_lead`
4. `create_panes`
5. `launch_sessions`
6. `join_mesh`
7. `start_daemons`
8. `send_onboarding`
9. best-effort `ensure_team_daemon_running_best_effort`

Important property:

- stages 4 through 8 are batch stages over the whole roster, not member-scoped transactions

### Resume team

`resume_team_with_cli_commands_and_layout` is a wrapper:

1. validate/load team
2. reconcile liveness
3. order members for resume
4. for each member, call `resume_member_with_cli_commands_and_layout`
5. ensure team daemon via `spawn_team_daemon`
6. aggregate partial success/failure report

Important property:

- resume team is already built on a reusable member-level pipeline

### Resume member

`resume_member_with_cli_commands_and_layout` is the actual activation pipeline:

1. `validate`
2. `load_member`
3. `resolve_pane`
4. `launch_session`
5. `join_mesh`
6. `start_daemon`
7. `send_onboarding`
8. `update_runtime`
9. best-effort `ensure_team_daemon_running_best_effort`

### Add agent

`add_agent_to_team_with_cli_commands_and_layout` is also effectively a member-activation pipeline:

1. `validate`
2. `create_pane`
3. `launch_session`
4. `join_mesh`
5. `start_daemon`
6. `send_onboarding`
7. `update_roster`
8. best-effort `ensure_team_daemon_running_best_effort`

This matters because resume and add-agent already point toward the architecture initialize should converge toward.

## Stage-by-Stage Comparison

| Concern | Initialize | Resume | Assessment |
|---|---|---|---|
| Request validation | `validate_initialize_configuration` | `validate_resume_request` + existing runtime health/offline checks in `load_resume_member_state` | Legitimately different entry validation |
| Team creation | `create_team` | none | Legitimately initialize-only |
| Roster persistence | `seed_initialize_roster` writes full team + all runtime records up front | `load_resume_member_state` reads existing roster; no new members created | Legitimately different, but initialize bypasses reusable add-member style paths |
| Pane acquisition | `create_panes` loops over members and immediately launches commands while creating panes | `resolve_resume_pane` reuses or creates pane per member | Shared concern, different implementation |
| CLI launch/session identity | initialize launches during `create_panes`, then detects session ids later in `launch_sessions` via `capture_session_id_for_member` | resume launches and detects in one per-member stage via `launch_resume_session` | Shared concern with duplicated behavior |
| Mesh join | initialize loops inline in `join_mesh` | resume uses `resume_join_mesh` helper | Shared concern with minor duplication |
| Member daemon start | initialize loops inline in `start_daemons` and saves runtime after each spawn | resume uses `resume_start_daemon` helper with stale-pid termination | Shared concern with real behavioral divergence |
| Onboarding delivery | initialize batch-delivers after all joins/daemons complete | resume delivers immediately after each member activation | Shared concern with major divergence |
| Runtime commit | initialize writes pane/runtime partial state early, then syncs metadata after `launch_sessions` | resume commits runtime near the end in `update_runtime` | Shared concern with major lifecycle divergence |
| Team daemon ensure | initialize best-effort ensure at end | resume member best-effort ensure plus resume team final ensure | Shared concern with duplicated ownership |
| Failure semantics | initialize tears down the whole team on post-create failures | resume member rolls back only that member; resume team keeps earlier successes | Legitimately different wrapper semantics |

## Shared Stages: Same Path, Duplicate Path, or Different Behavior

### Pane acquisition

Status: Different behavior

Initialize:

- creates panes for all members in one stage
- launches the command as part of pane creation
- writes pane/runtime metadata immediately

Resume:

- resolves pane per member
- may reuse existing pane or create a new one
- delays final runtime commit until later

Assessment:

- the differing pane policy is legitimate
- the execution shape is not unified enough
- a shared "acquire member pane" stage should exist with policy inputs such as `create_new` vs `reuse_or_create`

### CLI launch and session detection

Status: Duplicated logic with different sequencing

Initialize:

- launches during `create_panes`
- later detects session id in a separate `launch_sessions` stage using `capture_session_id_for_member`

Resume:

- launches and detects in one function, `launch_resume_session`

Add-agent:

- uses `launch_session_for_agent`

Assessment:

- this is unnecessary divergence
- session detection logic is already drifting across three flows
- future changes to Codex/Claude session detection are likely to land in one path first and lag in others

### Mesh join

Status: Semantically shared, lightly duplicated

Initialize:

- inline loop in `join_mesh`

Resume:

- `resume_join_mesh`

Add-agent:

- `join_mesh_for_agent`

Assessment:

- this should be one canonical helper that accepts a normalized member activation context
- current differences are mostly request-shape differences, not real semantics

### Member daemon start

Status: Semantically shared, behaviorally divergent

Initialize:

- inline loop in `start_daemons`
- no stale-pid handling
- persists daemon pid immediately after spawn

Resume:

- `resume_start_daemon`
- explicitly terminates stale daemon pid first
- records warnings on stale-pid cleanup failure

Add-agent:

- `start_daemon_for_agent`

Assessment:

- some divergence is legitimate because resume may inherit stale state
- but there should still be one shared "ensure member daemon" stage with mode-specific preconditions

### Onboarding delivery

Status: Shared concept, major behavioral divergence

Initialize:

- `send_onboarding_messages`
- delivers to the whole roster after all joins and daemon starts finish

Resume:

- `resume_send_onboarding`
- delivers per member immediately after that member's join/daemon work

Add-agent:

- `send_onboarding_for_agent`
- also immediate

Assessment:

- this is the highest-risk unnecessary divergence
- commit `3b17397` did not unify the behavior; it added delivery retries in the bridged backend so resume is less likely to fail when inbox creation lags behind join
- the underlying ordering difference remains and can still hide future race classes

### Runtime persistence

Status: Shared concept, major behavioral divergence

Initialize:

- seeds all runtime records up front as `SessionDead`
- writes pane ids and marks runtime healthy during pane creation
- captures session ids later

Resume:

- loads existing runtime record
- commits updated runtime near the end after onboarding

Assessment:

- initialize exposes more partial state during activation
- resume is closer to a transactional member activation model
- this is a likely source of future edge-case drift around failure recovery, liveness reconciliation, and user-visible runtime status

### Team daemon ensure

Status: Duplicate ownership

Initialize:

- best-effort team-daemon ensure at pipeline end

Resume member:

- also best-effort ensure at member completion

Resume team:

- additionally ensures the team daemon again after the loop

Add-agent:

- best-effort ensure after one member add

Assessment:

- ownership is unclear
- wrapper-level operations should own team-daemon ensuring
- member-level activation should not also be responsible unless there is a documented reason

## Specific Divergence Behind Commit `3b17397`

Commit `3b17397` added retry delays when sending operator notices through the bridged backend.

What it fixed:

- if a resumed member's inbox was not ready immediately after join, onboarding delivery could fail with a "no inbox" error
- retries allow resume to succeed once inbox readiness catches up

What it did not fix:

- initialize and resume still have different readiness assumptions
- initialize effectively uses a team-wide barrier before onboarding
- resume still uses per-member immediate delivery

Architectural conclusion:

- `3b17397` is a resilience patch, not a true pipeline unification fix
- it reduced one manifestation of divergence, but the divergence remains a bug generator

## Unnecessary Divergences With Bug Risk

### 1. Different onboarding barriers

Risk: High

Initialize waits until all member mesh/daemon work is done before onboarding. Resume and add-agent deliver onboarding immediately after one member is ready.

Why this is risky:

- readiness assumptions drift silently
- backend/inbox timing bugs only appear in some flows
- fixes tend to land as local retries instead of structural alignment

### 2. Session detection implemented in three shapes

Risk: High

Initialize uses `capture_session_id_for_member`; resume uses `launch_resume_session`; add-agent uses `launch_session_for_agent`.

Why this is risky:

- future runtime-session detection changes can regress one path but not the others
- session-id/jsonl-path persistence semantics can drift

### 3. Runtime commit timing differs substantially

Risk: Medium to High

Initialize publishes partial runtime state much earlier than resume.

Why this is risky:

- recovery and liveness code sees different invariants depending on flow
- UI snapshots can observe different activation phases
- rollback behavior will stay harder to reason about

### 4. Team-daemon ownership is duplicated

Risk: Medium

Multiple flows ensure the team daemon, and resume does it at both member and team wrapper levels.

Why this is risky:

- duplicate side effects
- more confusing progress/event ownership
- harder to explain final state in the UI

### 5. Mesh join and member-daemon start semantics are copied, not canonicalized

Risk: Medium

These stages are logically shared but implemented separately enough that future tool-specific changes will likely diverge.

## Legitimate Differences That Should Remain Separate

### Team creation and full-roster seeding

Initialize must:

- create the team
- create all members
- write all base runtime records

Resume must not recreate those artifacts.

### Pane reuse vs forced creation

Resume legitimately needs reuse-or-create behavior and stale-pane handling. Initialize legitimately starts from a clean creation bias.

### Offline validation and partial success semantics

Resume must:

- refuse to resume members that are not offline
- preserve earlier successes if a later member fails

Initialize is all-or-nothing after team creation and tears down on failure.

### Lead-mode semantics

Initialize still supports lead launch-mode distinctions such as `AttachExisting` vs `LaunchNew`. Resume is reactivating persisted members and should not inherit those decisions blindly.

## Recommended Unification Strategy

### Recommendation

Adopt a shared member-activation pipeline with orchestration wrappers.

Do not try to collapse initialize and resume into one monolithic end-to-end function. The wrappers genuinely differ. The member-level activation stages do not.

### Canonical model

Define a normalized member activation context with inputs such as:

- operation kind: `initialize`, `resume`, `add_agent`
- team name
- lead name
- member definition / persisted member
- pane policy: `create_new`, `reuse_or_create`
- roster policy: `preseeded`, `create_member`, `existing_member`
- delivery policy: `deferred_barrier`, `immediate`
- runtime commit policy: `staged`, `finalize_at_end`

Then implement canonical shared stages:

1. `prepare_member`
2. `acquire_pane`
3. `launch_session`
4. `capture_session_identity`
5. `join_mesh_if_required`
6. `start_member_daemon_if_required`
7. `commit_runtime`
8. `deliver_onboarding`

Wrapper responsibilities become:

#### Initialize wrapper

- validate full config
- create team
- seed full roster/runtime baseline
- run shared activation stages for all members
- apply a wrapper-level onboarding barrier policy when needed
- ensure team daemon once at the end
- on failure, disband/cleanup whole team

#### Resume team wrapper

- reconcile liveness
- order members
- run shared activation stages per member
- preserve partial success
- ensure team daemon once at the end

#### Resume member wrapper

- run the same member activation pipeline for one persisted member
- no wrapper-level aggregation

#### Add-agent wrapper

- create/persist new member
- run the same member activation pipeline
- rollback only that member on failure

### Why this is the right level of unification

- it preserves legitimate wrapper differences
- it removes drift in shared activation semantics
- it provides one canonical stage vocabulary for progress events
- it lowers the chance of local fixes like `3b17397` being needed in only one path

## Interaction With Task `#27` Progress Streaming Work

The in-flight resume progress work should be designed against the unified member-stage vocabulary, not the current resume-only shape.

Recommendation:

- define canonical member stage names now
- let `resume_team` progress events emit those stage names
- keep initialize's existing top-level team stages for current UX if needed
- optionally add nested member progress later using the same canonical stage vocabulary

This avoids a near-term trap:

- if task `#27` invents a resume-specific progress schema
- and unification later invents a different shared member-stage schema
- the team will need to rewrite the event model twice

Best path:

- keep a two-layer model
- wrapper-level stage: initialize batch phase or resume-team aggregate phase
- member-level stage: canonical activation stage shared by initialize/resume/add-agent

## Ranked Recommendations

### 1. Unify on a shared member-activation pipeline

Rank: Highest
Complexity: Medium to High

This is the structural fix that addresses the real bug class.

### 2. Canonicalize onboarding policy as an explicit wrapper decision

Rank: Highest
Complexity: Medium

Do not let onboarding ordering emerge accidentally from where helper calls happen. Make it explicit whether a wrapper uses immediate delivery or a readiness barrier.

### 3. Merge session capture/detection into one shared stage

Rank: High
Complexity: Medium

This is the most obvious duplicated technical behavior after onboarding.

### 4. Move team-daemon ensure ownership to wrappers only

Rank: High
Complexity: Low to Medium

This simplifies side effects and progress reporting.

### 5. Normalize runtime commit semantics

Rank: Medium
Complexity: Medium

Initialize should move closer to a clearer staged/finalized runtime model so wrapper invariants are easier to reason about.

## Bottom Line

Initialize and resume should not be fully merged, but their member-activation logic should.

Today, resume is already architected around a member-level pipeline while initialize is still a batch pipeline with member loops hidden inside broad stages. That is why the same conceptual stage can behave differently across flows and produce bugs like the onboarding inbox race.

The right target is:

- shared member activation stages
- explicit wrapper-level policies for barriers, failure semantics, and cleanup
- one canonical progress/event vocabulary built on those shared stages

That gives the team a cleaner architecture and a better foundation for the streamed progress work happening now.
