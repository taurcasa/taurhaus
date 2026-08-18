# Mesh Tab Load Analysis

Date: 2026-03-06  
Task: #428

## Executive summary

The Mesh tab feels slower because it is architected unlike the other tabs.

Other tabs:

- render immediately from Shell-owned state or tab-local skeletons
- load data in parallel
- do not block the first meaningful paint on environment checks

Mesh:

1. enters a blocking `gate` mode
2. runs prerequisite checks first
3. only then runs team discovery
4. if a runtime team exists, only then runs live-status refresh

That is a waterfall, not a fast tab surface.

## What happens on Mesh tab mount

### Frontend path

`Shell.svelte` mounts `MeshTab` only after the tab is first visited:

- `switchTab('mesh')` marks the tab visited and sets `activeTab`
- `MeshTab.svelte` creates a controller with `createMeshTabController(...)`
- controller starts in `mode = 'gate'`

`MeshSetupView.svelte` then renders `MeshAvailabilityGate` first.

### Gate phase

`MeshAvailabilityGate.svelte` blocks the UI behind:

```js
Promise.all([
  checkMeshInstallStatus(),
  coordinationPreflightCheck(minimalPreflightRequest(projectPath)),
])
```

Only after both complete does the “Checking project team state...” child render. That child uses `onGateReady`, which triggers:

```js
controller.ensureGateReady()
```

### Discovery phase

`ensureGateReady()` -> `bootstrapFromGate()` -> `bootstrapFromGateWorkflow(...)`

That calls:

```js
coordinationListTeams()
```

If a matching team exists for the current project path, the controller switches to runtime mode and immediately calls:

```js
coordinationGetLiveTeamStatus(teamName)
```

### Runtime refresh phase

On the Rust side, `coordination_get_live_team_status` is not just a read. It does:

1. `orchestrator.reconcile_team_liveness(team_name)?`
2. `orchestrator.get_team_status(team_name)`

`reconcile_team_liveness` can call tmux liveness checks per member:

- `pane_exists`
- `pane_is_dead`
- `pane_is_shell`

So runtime-mode load can include multiple tmux command invocations before the tab is considered ready.

## Why other tabs feel instant

### Overview

Overview is fed by Shell-owned project data that is loaded earlier via `loadProjectSelectionData(...)`, which resolves:

- project details
- commits
- latest session
- session history
- README
- relationships

Those requests are issued in parallel and degrade independently with fallbacks.

### Files / Git / Tasks

These tabs render their shell immediately after tab activation and fetch their own data without a blocking prerequisite gate. They may show loading states, but the tab itself appears right away.

### Mesh difference

Mesh is the only tab whose first visible content is gated by:

- environment checks
- team discovery
- runtime liveness refresh

That is the architectural reason it feels slower.

## Bottleneck breakdown

## 1. Blocking prerequisite gate

This is the first and most important issue.

Mesh blocks the tab behind prerequisite checks that the other tabs do not perform on activation.

Worse, the gate duplicates some work:

- `checkMeshInstallStatus()` checks installed mesh version / bundled version
- `coordinationPreflightCheck(...)` re-runs baseline availability checks (`mesh`, `tmux`) and also probes `claude`, `codex`, and `gemini`

That means tab-open does more process probing than the UI needs just to decide whether to render the Mesh surface.

## 2. Waterfall sequencing

The current phases are:

1. gate checks
2. team discovery
3. live team status refresh

These are not collapsed into one backend snapshot, and the next phase does not start until the previous phase resolves.

## 3. Runtime status path is potentially expensive

If the project already has a team, `coordinationGetLiveTeamStatus()` is likely the heaviest step because it performs write-on-drift liveness reconciliation before returning.

That means the “show runtime canvas” path is coupled to tmux health probing.

For a 5-member team, the code path can reach roughly:

- up to 3 tmux checks per member
- plus runtime/config reads

So the runtime branch is materially heavier than a simple team-config load.

## 4. Team discovery scales with number of stored teams

`coordinationListTeams()` reads all team folders and loads each config to discover lead project anchors.

That is not the biggest bottleneck by itself, but it is still extra work on the critical path.

In the current local environment there are 13 team directories under `~/.claude/teams`.

## What is not the main bottleneck

- role template loading: only happens when opening quick-preset hydration, template browser, or add-agent flows
- canvas layout/rendering: not the first-order cause of the initial delay
- simple tab-switch wiring: `switchTab()` itself is trivial

## Timing data

No in-app instrumentation exists yet for the Mesh tab path, so exact IPC timings were not directly captured from the app. Rough local shell timings for the native Linux equivalents:

| Probe | Rough local time |
|---|---|
| `test -x ~/.local/bin/mesh` | ~10ms |
| `which tmux` | ~0ms |
| `which claude` / `codex` / `gemini` | ~0ms each |
| `~/.local/bin/mesh --version` | ~10ms |

Interpretation:

- on native Linux, the raw subprocess probes are small individually
- on Windows/WSL, the same checks are more expensive because they cross `wsl.exe`
- even on native Linux, the staged gate -> discovery -> runtime sequence creates visible latency because the UI waits between phases

## Important nuance on “every switch”

Within the same project, `MeshTab` should not fully remount after the first visit because Shell keeps visited tabs mounted and only toggles visibility.

So the architecture above explains:

- first Mesh-tab visit per project
- Mesh after project switches
- Mesh when the tab was never visited in the current project

If users still observe the same delay on repeated same-project revisits, that suggests a second issue such as:

- expensive re-paint/reflow of the runtime surface
- animation compounding the perception of delay
- some child component doing extra work on visibility change

That is not visible from the current controller path and would need browser profiling to confirm.

## Ranked recommendations

### 1. Replace the mount-time waterfall with one backend snapshot command

Best fix.

Add a single IPC like:

```text
coordination_get_project_mesh_snapshot(projectPath)
```

Return:

- feature availability summary
- matched team name or empty state
- runtime config / live summary if a team exists
- warnings

That turns:

- gate check
- team discovery
- runtime fetch

into one round trip from the frontend’s perspective.

### 2. Remove `coordinationPreflightCheck()` from tab-open

High impact, low risk.

The current gate runs full preflight on tab mount, but preflight is really needed when the user is about to initialize a team.

On tab open, the UI only needs:

- is Mesh available?
- is tmux available?
- is there already a team for this project?

The per-agent CLI warning path (`claude`, `codex`, `gemini`) can be deferred until:

- initialize click
- template/customizer open
- add-agent flow

### 3. Show last-known Mesh state immediately, refresh in background

If a project has a known team:

- render the last known runtime or empty state immediately
- run liveness refresh in the background
- patch the UI when the new report returns

This matches how the other tabs feel fast: paint first, reconcile second.

### 4. Cache project -> team mapping and runtime snapshot across tab switches

Mesh currently rediscovers from coordination state on demand.

A small frontend cache keyed by `projectPath` would let the tab open instantly after the first successful load within a session.

### 5. Split “fast snapshot” from “heavy liveness reconciliation”

`coordination_get_live_team_status()` currently does both:

- liveness reconciliation
- status read

Consider a two-tier model:

- fast read: config + stored runtime snapshot
- background refresh: reconcile tmux/daemon liveness

That keeps runtime mode visually responsive without giving up correctness.

### 6. Prefetch mesh snapshot at project selection or startup

Lower priority than the items above, but still useful.

Shell already preloads Overview data when the project changes. Mesh could piggyback on that pattern for the selected project or for projects that already have teams.

## Recommended plan

1. Replace frontend waterfall with one snapshot IPC.
2. Move full preflight out of mount-time gate.
3. Render cached or last-known Mesh state immediately.
4. Reconcile liveness in background, not before first meaningful paint.

That is the path most aligned with how the fast tabs already work.

## Final conclusion

The Mesh tab is slower because it is architected as a gated workflow surface, not as a fast read-mostly tab.

The biggest problem is not one slow IPC. It is the combination of:

- blocking prerequisite checks
- sequential discovery
- heavy runtime liveness refresh before the tab is usable

To make Mesh feel as fast as Overview/Files/Tasks/Git, taurhaus should stop treating tab-open as the moment to prove the whole environment. Paint something useful immediately, and move the expensive correctness checks behind that first render.

## Sources

- Frontend:
  - [`MeshTab.svelte`](/home/user/projects/taurhaus/src/lib/components/MeshTab.svelte)
  - [`meshTabController.svelte.js`](/home/user/projects/taurhaus/src/lib/components/meshTabController.svelte.js)
  - [`MeshSetupView.svelte`](/home/user/projects/taurhaus/src/lib/components/MeshSetupView.svelte)
  - [`MeshAvailabilityGate.svelte`](/home/user/projects/taurhaus/src/lib/components/MeshAvailabilityGate.svelte)
  - [`projectSelection.js`](/home/user/projects/taurhaus/src/lib/projectSelection.js)
  - [`Shell.svelte`](/home/user/projects/taurhaus/src/Shell.svelte)
- Backend:
  - [`commands/mesh.rs`](/home/user/projects/taurhaus/src-tauri/src/commands/mesh.rs)
  - [`commands/coordination.rs`](/home/user/projects/taurhaus/src-tauri/src/commands/coordination.rs)
  - [`backend/bridged.rs`](/home/user/projects/taurhaus/src-tauri/src/coordination/backend/bridged.rs)
  - [`orchestrator.rs`](/home/user/projects/taurhaus/src-tauri/src/coordination/orchestrator.rs)
  - [`config.rs`](/home/user/projects/taurhaus/src-tauri/src/coordination/stores/config.rs)
