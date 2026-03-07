# Team And Agent Lifecycle UX

Date: 2026-03-07
Owner: developer1
Task: #493

## Summary

The Mesh tab should stop treating offline agents as a banner-worthy alert. Team lifecycle is an operational workflow, not an exception path.

Recommended direction:

1. Keep the runtime canvas as the spatial overview, but move lifecycle control into a persistent runtime header and member detail actions.
2. Replace the current `Team is offline` / `Some team members are offline` banner with calm state summaries and explicit management affordances.
3. Treat three states as normal runtime variants, not warnings:
   - fully running
   - partially available
   - fully stopped but restorable
4. Reserve true warning styling for broken operations only: failed resume, failed disband, CLI unavailable, config corruption.
5. Make the primary actions obvious at the right level:
   - team-level actions in the runtime header
   - member-level actions in node detail and roster views
   - destructive actions behind confirmation

The right mental model is closer to Docker Desktop or a cloud instance console than to an alert banner. Users manage a fleet. Some members being stopped is normal.

## Core UX Principles

### 1. State Is Information, Not Alarm

`offline` means `not currently running`. It does not mean `something is wrong`.

The UI should only escalate visually when:
- a user action failed
- required infrastructure is unavailable
- persisted state cannot be trusted

### 2. Users Think In Teams First, Members Second

When users open Mesh, they first want to know:
- Does this team exist?
- Is it running?
- Can I start or continue work?

Only after that do they care which individual member needs attention.

### 3. Actions Should Match Scope

- Team-level actions: `Start team`, `Resume offline`, `Stop all`, `Disband`
- Member-level actions: `Resume`, `Stop`, `Focus pane`, `Remove`

Do not force team recovery through repeated per-member actions when the user intent is obviously team-wide.

### 4. Preserve Spatial Context

The canvas is valuable even when nothing is running because it reminds the user who belongs to the team and how work is partitioned.

A stopped team should still render as a valid team, not collapse back into setup mode.

### 5. No Persistent Warning Banner For Normal States

A banner is appropriate for transient error feedback, not for the steady-state condition of a stopped or mixed-availability team.

## User Journey Model

### Stage 1: No Team Yet

User intent:
- create a new team for this project

What the user should see:
- existing setup flow / preset flow
- optional discovery note if another team already exists elsewhere

Primary action:
- `Start Team`

Secondary actions:
- `Browse presets`
- `Recover existing team` if a persisted team already exists for this project

ASCII wireframe:

```text
+------------------------------------------------------------+
| Mesh                                                       |
|------------------------------------------------------------|
|  No team running for this project                          |
|  Create a new team or recover an existing one.             |
|                                                            |
|  [ Start Team ]   [ Browse Presets ]   [ Recover Team ]    |
+------------------------------------------------------------+
```

Recommendation:
- if a persisted team exists, the empty state should offer `Recover Team` directly instead of forcing the user to infer that setup is not the right path

### Stage 2: Team Running Normally

User intent:
- check team state quickly
- open a member
- add a member
- stop/disband if done

What the user should see:
- runtime header with team status summary
- canvas
- optional compact roster/list view for scanning statuses faster than the canvas

Runtime header contents:
- team name
- lifecycle summary: `5 members • 3 active • 2 idle`
- primary action: `Add Agent`
- overflow / segmented lifecycle actions: `Stop all`, `Disband`

ASCII wireframe:

```text
+------------------------------------------------------------+
| architecture-final                           [Add] [More v]|
| 5 members   3 active   2 idle                              |
|------------------------------------------------------------|
|                      runtime canvas                        |
|                                                            |
|   lead                builder              reviewer        |
|                                                            |
+------------------------------------------------------------+
```

Recommendation:
- `Disband` should move out of being visually co-equal with normal actions; put it in an overflow menu or danger section in the header
- the default visible team action in healthy runtime is `Add Agent`, not `Disband`

### Stage 3: One Or More Members Offline

User intent:
- understand who is unavailable
- resume only what matters
- keep working with live members

What the user should see:
- same runtime header, but with a neutral degraded summary: `5 members • 3 active • 2 offline`
- a compact inline lifecycle chip or action group in the header:
  - `Resume Offline (2)`
- offline members rendered as stopped, dimmed nodes in the canvas
- node detail for an offline member exposes `Resume` as the primary action

Notably absent:
- no yellow/orange warning banner across the top

ASCII wireframe:

```text
+---------------------------------------------------------------+
| architecture-final             [Resume Offline (2)] [Add] [v] |
| 5 members   3 active   2 offline                              |
|---------------------------------------------------------------|
|    lead(active)         builder(offline)      reviewer(active)|
|                                                                |
|  Selected: builder                                              |
|  Status: Offline                                                |
|  Last seen: 14m ago                                             |
|  [ Resume ]  [ Remove ]                                         |
+---------------------------------------------------------------+
```

Recommendation:
- degraded state should feel calm and actionable
- use neutral stopped styling for offline members, not warning tones
- reserve warning color only when a resume attempt fails

### Stage 4: Full Team Offline After Restart / WSL Loss

User intent:
- restore the whole team quickly
- understand that the team still exists even though nothing is running

What the user should see:
- same runtime shell, not a jarring banner or fallback mode
- header summary: `5 members • 0 running • ready to resume`
- primary CTA in header: `Resume Team`
- secondary CTA in overflow: `Resume Selected`, `Disband`
- canvas still visible, all nodes in stopped state

ASCII wireframe:

```text
+------------------------------------------------------------+
| architecture-final                 [Resume Team] [More v]  |
| 5 members   0 running   ready to resume                    |
|------------------------------------------------------------|
|                                                            |
|   lead(stopped)      builder(stopped)     reviewer(stopped)|
|                                                            |
|  Team configuration is intact. Start the full team or      |
|  resume individual members from their detail cards.        |
+------------------------------------------------------------+
```

Recommendation:
- this is a `restorable stopped` state, not a `warning` state
- the explanatory sentence can live under the header summary or in an empty-state panel above the canvas, but it should read like operational guidance, not an alert
- avoid words like `offline after restart` as the main title; prefer `ready to resume`, `team stopped`, or `team not running`

### Stage 5: Resume In Progress

User intent:
- understand whether progress is happening
- know which members succeeded or failed
- avoid duplicate clicks

What the user should see:
- same runtime header with actions temporarily disabled where needed
- a compact progress tray beneath the header, scoped to the current operation
- each member row shows one of: `pending`, `resumed`, `failed`

ASCII wireframe:

```text
+------------------------------------------------------------+
| architecture-final                 [Resuming Team...]       |
| 5 members   2 resumed   1 failed   2 pending               |
|------------------------------------------------------------|
| Resume progress                                             |
|  ✓ lead        Resumed                                      |
|  ✓ builder     Resumed                                      |
|  × reviewer    Mesh join failed                             |
|  … qa          Waiting                                      |
|  … ui          Waiting                                      |
+------------------------------------------------------------+
```

Recommendation:
- keep progress localized to the operation area
- on completion, collapse the tray into a dismissible result summary after a short delay
- do not leave a permanent “failed” warning panel behind if only some members remain offline; just keep those members in the stopped state with retry actions

### Stage 6: Partial Success After Resume Attempt

User intent:
- continue with recovered members
- retry or inspect failed members later

What the user should see:
- runtime header summary updates immediately: `3 active • 2 offline`
- a temporary toast or inline result note: `Resumed 3 members. 2 still stopped.`
- failed members remain visibly stopped with retry actions in node detail

ASCII wireframe:

```text
+---------------------------------------------------------------+
| architecture-final             [Resume Offline (2)] [Add] [v] |
| 5 members   3 active   2 offline                              |
|---------------------------------------------------------------|
| Last action: Resumed 3 members. 2 still stopped.              |
|                                                               |
|  reviewer (offline)   [Resume]                                |
|  qa       (offline)   [Resume]                                |
+---------------------------------------------------------------+
```

Recommendation:
- partial success is a normal outcome and should not trigger a full-screen error state
- never roll back recovered members just because one later member failed

### Stage 7: Team Finished / Stop All / Disband

User intent:
- either stop work temporarily or remove the team permanently

The current model overemphasizes `Disband`. The UX should separate two very different intents:

1. `Stop All`
   - temporary
   - preserves team composition
   - leads naturally to `Resume Team` later

2. `Disband`
   - permanent cleanup
   - removes team definition and runtime state
   - should be a deliberate destructive action

ASCII wireframe for overflow menu:

```text
More
----
Resume Selected Member
Stop All Members
Disband Team...
```

Recommendation:
- add `Stop All Members` as a first-class lifecycle action
- move `Disband Team` behind confirmation and danger styling
- this better matches real user intent: many users want to stop work without deleting team structure

## Recommended Information Hierarchy

From top to bottom:

1. Team identity
2. Lifecycle summary
3. Team-level actions
4. Canvas / roster state
5. Member detail actions
6. Temporary operation progress or result notes

The current banner inverts this. It puts a warning container above the actual team information, which makes the abnormal framing dominate the page.

## Recommended Runtime Header Design

Replace the current footer-like runtime bar plus alert banner combination with a stronger top header.

Suggested structure:

```text
+----------------------------------------------------------------+
| architecture-final                                             |
| 5 members • 3 active • 2 offline                               |
| [Resume Offline (2)] [Add Agent] [More v]                      |
+----------------------------------------------------------------+
```

Header action rules:
- healthy team: show `Add Agent` and `More`
- degraded team: show `Resume Offline (n)` and `Add Agent`
- fully stopped team: show `Resume Team` as the dominant primary action
- while operation running: replace primary CTA with progress/disabled state

`More` menu contents:
- `Stop All Members`
- `Disband Team...`
- optional future: `Open Team Terminal`, `Copy Team Command`, `Inspect Runtime`

## Member-Level UX Recommendations

Offline member detail should use this action ordering:

1. `Resume`
2. `Focus pane` only when a pane exists
3. `Remove`

Active/idle member detail should use:

1. `Focus pane`
2. `Stop`
3. `Remove`

Metadata that matters in detail:
- status
- tool/model
- project scope
- pane/session identifiers only in secondary text
- `last seen` or `last active` for stopped members if available

Metadata that is mostly noise in normal runtime:
- verbose diagnostic explanation for why a member is offline
- infrastructure-heavy wording like `pane missing after reconciliation`

## Roster Addition

The canvas is spatial, but lifecycle management is often faster in a list. Add an optional compact roster strip or side list under the header:

```text
Members
lead        Active     Open
builder     Offline    Resume
reviewer    Active     Open
qa          Offline    Resume
```

Why this matters:
- lists are faster for operational scanning
- they complement the canvas instead of replacing it
- they make degraded teams manageable without repeated node hunting

## Visual Language Recommendations

Use these tones:
- active: success/accent dot only
- idle: muted neutral with subtle warmth
- offline/stopped: neutral muted gray, not warning amber
- failed action: danger red attached only to the failed operation result

This is important. If `offline` stays yellow, the UI will keep implying that a routine stopped state is a warning.

## Copy Recommendations

Replace current copy like:
- `Team is offline after restart`
- `Some team members are offline`

With calmer operational copy:
- `Team ready to resume`
- `2 members are stopped`
- `Resume offline members`
- `Team stopped`
- `Resumed 3 members. 2 still stopped.`

Preferred tone:
- factual
- low drama
- action-oriented

## Concrete UI Changes

### 1. Remove Persistent Runtime Warning Banner

In the runtime view, remove the current banner as the primary degraded/cold-resume treatment.

### 2. Promote Runtime Header

Evolve `MeshRuntimeBar` into the main lifecycle control surface:
- bring it to the top of the runtime view
- include dominant primary CTA based on lifecycle state
- move destructive actions into overflow

### 3. Add `Stop All Members`

This is the missing lifecycle action between `doing work` and `disbanding forever`.

### 4. Add Optional Compact Roster

This gives a better operational view for degraded and large teams.

### 5. Keep Progress Inline And Temporary

Progress belongs near the team action, not in a full alert block.

## Proposed State Model For UX

Use these UX states:
- `no_team`
- `running`
- `partially_running`
- `stopped_restorable`
- `resume_in_progress`
- `operation_error`

Mapping from current backend-ish model:
- `active` -> `running`
- `degraded` -> `partially_running`
- `cold_resume` -> `stopped_restorable`

This naming is more human-facing and should guide copy.

## Why This Is Better Than The Current Banner Approach

The current approach fails because:
- it frames normal runtime variance as an alert
- it visually interrupts the runtime view
- it competes with the actual team controls
- it scales poorly for repeated degraded states
- it offers little distinction between `recoverable stopped` and `operation actually failed`

The proposed model is better because:
- it keeps the user in one stable runtime shell
- it makes actions available at the correct scope
- it preserves team context even when nothing is running
- it matches real fleet-management patterns from infrastructure tools
- it distinguishes state from error

## Implementation Guidance

Phase 1:
- replace runtime banner with header-based lifecycle summary + CTA
- add `Resume Team` / `Resume Offline (n)` logic to the runtime header
- move `Disband` out of the default visible action row
- keep current canvas and node detail interaction model

Phase 2:
- add compact roster list under the header
- add `Stop All Members`
- add `last seen` data if backend supports it cleanly

Phase 3:
- consider sidebar/session-level lifecycle affordances for stopped teams across projects

## Final Recommendation

Do not iterate on the existing offline banner.

Instead, redesign Mesh runtime around a persistent lifecycle header and calm stateful controls:
- running teams feel like a control surface
- partially offline teams feel manageable, not broken
- fully stopped teams feel recoverable, not alarming
- disband remains possible, but no longer dominates the runtime UI

That is the right user journey for a long-lived team orchestration tool.
