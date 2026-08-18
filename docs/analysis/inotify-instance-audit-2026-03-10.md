# Inotify Instance Audit — 2026-03-10

## Scope

Urgent assessment of `fs.inotify.max_user_instances` exhaustion on WSL2. This is about the number of inotify file descriptors, not the number of watched paths within a descriptor.

Relevant code paths:

- [src-tauri/src/daemon/watch.rs](/home/user/projects/taurhaus/src-tauri/src/daemon/watch.rs)
- [src-tauri/src/daemon/server.rs](/home/user/projects/taurhaus/src-tauri/src/daemon/server.rs)
- [src-tauri/src/daemon_lifecycle.rs](/home/user/projects/taurhaus/src-tauri/src/daemon_lifecycle.rs)
- [src-tauri/src/daemon/compaction.rs](/home/user/projects/taurhaus/src-tauri/src/daemon/compaction.rs)
- [src-tauri/src/session_scanner/compaction_watcher.rs](/home/user/projects/taurhaus/src-tauri/src/session_scanner/compaction_watcher.rs)
- [src-tauri/src/session_scanner/compaction_extractor.rs](/home/user/projects/taurhaus/src-tauri/src/session_scanner/compaction_extractor.rs)
- [src-tauri/src/fs/watcher.rs](/home/user/projects/taurhaus/src-tauri/src/fs/watcher.rs)
- [src-tauri/src/startup/watchers.rs](/home/user/projects/taurhaus/src-tauri/src/startup/watchers.rs)

Reference context:

- [inotify-watch-audit-2026-03-09.md](/home/user/projects/taurhaus/docs/analysis/inotify-watch-audit-2026-03-09.md)

## Live evidence

### Process breakdown

Snapshot taken on 2026-03-10 from `/proc/*/fd -> anon_inode:inotify`.

Top offenders:

| Process | Instances |
| --- | ---: |
| `taurhaus-daemon` | 66 |
| 13 `mesh` daemons | 26 total |
| `systemd` | 3 |
| Codex/Claude sessions | 11 total |
| everything else | 20 total |

Total live instances at capture time: `126`.

This matches the reported failure mode: the system had been operating near the old `128` instance ceiling, so one more daemon start could fail with `EMFILE`.

### Current user-facing app state

At the time of capture there was exactly one Windows Taurhaus app process:

- `taurhaus.exe` PID `79020`

There were not multiple visible Windows app instances. So the daemon-side duplication is not explained by two obvious apps being open.

### Current watch plan

The live Windows DB at `/mnt/c/Users/user/AppData/Roaming/com.taurhaus.dev/taurhaus.db` currently holds:

- `20` projects
- thresholds `active=4`, `recent=12`, `stale=30`
- `16` projects in `active` or `recent`, which means `16` daemon activity-watch targets today
- plus one `.claude/tasks` watch path

So the expected current daemon activity-watch plan is `17` unique watch targets, not `50+`.

## Root cause of the daemon's 66 instances

The `66` are not mostly compaction watchers.

That split is:

1. `4` compaction-related daemon instances
2. roughly `62` ordinary daemon watch instances

### Why compaction is only 4

The current daemon compaction architecture uses:

- `1` `RecommendedWatcher` for the transcript extractor service in [compaction_extractor.rs:257](/home/user/projects/taurhaus/src-tauri/src/session_scanner/compaction_extractor.rs#L257)
- `1` `RecommendedWatcher` per team signal watcher in [compaction_watcher.rs:372](/home/user/projects/taurhaus/src-tauri/src/session_scanner/compaction_watcher.rs#L372)
- `1` recursive teams-topology watcher in [compaction.rs:181](/home/user/projects/taurhaus/src-tauri/src/daemon/compaction.rs#L181)

Current live state has:

- `2` teams with compaction signal logs
- `1` extractor service
- `1` topology watcher

So the compaction runtime explains `4` daemon instances, not `66`.

This corrects a common misread: transcript count affects watch descriptors inside the extractor instance, not the number of inotify instances.

### Why ordinary daemon project watching dominates

The real offender is [handle_watch()](/home/user/projects/taurhaus/src-tauri/src/daemon/watch.rs#L27).

Every successful daemon `watch` request creates a fresh `notify::RecommendedWatcher`:

- creation in [watch.rs:74](/home/user/projects/taurhaus/src-tauri/src/daemon/watch.rs#L74)
- dedupe key lives only in the per-connection `active_watches` map passed from [server.rs:248](/home/user/projects/taurhaus/src-tauri/src/daemon/server.rs#L248)

That means dedupe is **connection-scoped**, not daemon-global.

Each TCP connection handled by [handle_connection()](/home/user/projects/taurhaus/src-tauri/src/daemon/server.rs#L234) gets its own `WatchRuntime`:

- `let mut watch_runtime = WatchRuntime::new();` at [server.rs:248](/home/user/projects/taurhaus/src-tauri/src/daemon/server.rs#L248)
- the watches are only dropped when that specific connection exits, at [server.rs:319](/home/user/projects/taurhaus/src-tauri/src/daemon/server.rs#L319)

So if the same Linux project path is watched on three or four separate event-listener connections, the daemon allocates three or four separate inotify instances for the same tree.

### Why this is happening in practice

The app-side daemon watch lifecycle has a race-prone design:

1. startup bootstrap spawns `start_daemon_watches(...)` in a background thread from [watchers.rs:76](/home/user/projects/taurhaus/src-tauri/src/startup/watchers.rs#L76)
2. startup also immediately spawns a separate `reconcile_activity_watches(..., "startup")` thread from [watchers.rs:97](/home/user/projects/taurhaus/src-tauri/src/startup/watchers.rs#L97)
3. later periodic/settings/project reconcilers can also call `apply_daemon_watch_plan(...)`

`apply_daemon_watch_plan(...)` only treats a plan as unchanged once `runtime.plan` has already been stored:

- unchanged check in [daemon_lifecycle.rs:184](/home/user/projects/taurhaus/src-tauri/src/daemon_lifecycle.rs#L184)
- `runtime.plan = Some(...)` is not stored until after the daemon listener has connected and all `watch()` calls have been issued at [daemon_lifecycle.rs:257](/home/user/projects/taurhaus/src-tauri/src/daemon_lifecycle.rs#L257)

That leaves a race window where multiple callers can all decide "no plan is active yet" and each open their own daemon event-listener connection.

Because daemon-side dedupe is only per connection, each of those listeners re-registers the same watch set and allocates another full set of inotify instances.

### Why the live numbers strongly support this diagnosis

Live daemon facts:

- total daemon inotify instances: `66`
- expected compaction instances: `4`
- remaining ordinary watch instances: about `62`
- expected unique activity-watch targets today: `17`

`62 / 17 ≈ 3.65`

That is not consistent with "the app legitimately needs 62 different watch roots".
It is consistent with "roughly the same watch plan has been registered about four times".

The `/proc/726119/fdinfo/*` snapshot reinforces this:

- `66` total inotify FDs
- only `28` unique watched inode sets
- several identical watch sets are repeated `5x`, `4x`, and `3x`

Examples from the live daemon:

- `5` identical instances each holding `148` watch descriptors
- `5` identical instances each holding `19` watch descriptors
- `5` identical instances each holding `8` watch descriptors
- `4` identical instances each holding `75` watch descriptors
- `3` identical instances each holding `150` watch descriptors

Repeated identical watched inode sets are exactly what we expect from duplicated watch registrations across multiple daemon listener connections.

### Additional supporting evidence

The daemon currently had multiple simultaneous TCP connections on port `17233`, including:

- `8` established connections
- `1` `CLOSE-WAIT` connection

The `CLOSE-WAIT` connection matters because daemon watch instances are only dropped when the connection handler exits and clears `watch_runtime.active_watches`. A half-closed or delayed-to-exit connection can keep duplicate watchers alive longer than intended.

## App-side watcher count

For this incident, the app-side local watcher is not the main contributor because the live app is Windows-first and WSL projects are daemon-owned.

But the same architectural issue exists outside this exact path:

- local `ProjectWatcher::watch_project()` creates one `RecommendedWatcher` per watched project in [fs/watcher.rs:571](/home/user/projects/taurhaus/src-tauri/src/fs/watcher.rs#L571)
- local `watch_file()` creates a separate `RecommendedWatcher` per watched file in [fs/watcher.rs:632](/home/user/projects/taurhaus/src-tauri/src/fs/watcher.rs#L632)
- on non-Windows app startup, local compaction adds one extractor watcher plus one watcher per team in [startup/compaction.rs:23](/home/user/projects/taurhaus/src-tauri/src/startup/compaction.rs#L23)

So the app process can hit the same kernel limit on Linux/macOS dev setups even after the daemon issue is fixed.

## Scaling analysis

### Current architecture

Current daemon instance usage is approximately:

`project_watch_instances + claude_tasks_watch + compaction_extractor + team_signal_watchers + topology_watcher + duplicate_listener_factor`

In the current live Taurhaus Windows setup:

- `16` active/recent project watch targets
- `1` Claude tasks watch
- `1` extractor watcher
- `2` team signal watchers
- `1` topology watcher

Expected healthy total with one listener:

- `16 + 1 + 1 + 2 + 1 = 21`

Actual total:

- `66`

So the current architecture is burning about `45` extra instances above the healthy single-listener expectation.

### Project scaling if we fix only the duplication bug

If all watched projects are daemon-owned and the daemon keeps one global listener:

`instances ~= active_or_recent_projects + 1(.claude/tasks) + 1(extractor) + codex_teams + 1(topology)`

If all projects are active and each project has a managed Codex team:

| Projects | Activity paths | Team signal watchers | Fixed daemon watchers | Total daemon instances |
| --- | ---: | ---: | ---: | ---: |
| 10 | 10 | 10 | 3 | 23 |
| 20 | 20 | 20 | 3 | 43 |
| 50 | 50 | 50 | 3 | 103 |

That is acceptable under `512`, but already uncomfortably close to `128` at `50` active projects.

### Project scaling if full Codex teams are all attached

If transcript watching is ever refactored back toward one watcher per session, or if app-side local watching keeps the current per-project instance model, scale gets much worse quickly.

Even with the current extractor design, a power user with many live agent teams is still exposed through:

- one signal watcher per Codex-enabled team
- one local/daemon project watcher instance per watched project
- duplicated instances whenever the watch listener lifecycle misbehaves

The main conclusion is:

- fixing listener duplication is mandatory
- but a larger shared-watcher architecture is still worthwhile if Taurhaus is meant to scale to dozens of active projects and many teams

## Resolution proposals

### Proposal 1: Global daemon watch registry with one shared watcher instance

Do not allocate one `RecommendedWatcher` per `watch` RPC.

Instead:

- move watch state from per-connection `WatchRuntime` into a daemon-global registry
- keep one shared `RecommendedWatcher` for ordinary project watching
- attach multiple subscriber connections to logical paths instead of giving each connection its own watcher

This is the highest-value fix because it removes the largest multiplier immediately.

### Proposal 2: Single owner for daemon watch listener lifecycle in the app

The app must stop allowing startup bootstrap, startup reconcile, reconnect, and periodic/settings/project-triggered reconciles to race through separate listener creation windows.

Concretely:

- have one watch-listener owner task/thread
- send desired-plan updates to that owner
- let that owner diff and mutate subscriptions in place
- never tear down and recreate the listener just because thresholds or project state changed

This removes the app-side duplication trigger.

### Proposal 3: Separate logical subscriptions from physical watcher resources

Right now a logical subscription and a physical watcher are too tightly coupled.

Instead:

- maintain `path -> subscriber set`
- physical watch exists once while subscriber count > 0
- connection loss only removes that connection from the subscriber set
- other subscribers do not lose the watch

This is the correct long-term daemon architecture.

### Proposal 4: Reduce app-side local instance count too

`ProjectWatcher` should move away from one `RecommendedWatcher` per project and one per watched file.

At minimum:

- one shared watcher for all local project trees
- one shared watcher for singleton files like `.claude/tasks` and `tmux-focus.json`

Otherwise Linux/macOS app runs will eventually recreate the same problem locally.

### Proposal 5: Add instance telemetry and alerting

We were blind until the limit was almost exhausted.

Add periodic telemetry for:

- process-local inotify instance count
- process-local inotify watch-descriptor count
- daemon listener connection count
- ordinary watch subscription count
- global user-level inotify instance count when available

This should produce warnings well before hard failure.

## Recommended follow-up task drafts

### Task draft 1

Subject:

- `Daemon: replace per-connection watch runtime with global shared watch registry`

Description:

- Move daemon `watch`/`unwatch` state out of per-connection `WatchRuntime` into a global registry. Keep one physical `RecommendedWatcher` for ordinary project watching and fan out daemon events to subscriber connections. Preserve canonical-path dedupe across all clients, not just one TCP connection.

### Task draft 2

Subject:

- `App: make daemon activity watch listener single-owner and race-free`

Description:

- Replace `start_daemon_watches` plus `reconcile_daemon_activity_watches` restart behavior with one owned listener lifecycle that receives watch-plan diffs. Eliminate bootstrap/startup-reconcile/reconnect races that can create duplicate daemon listener connections.

### Task draft 3

Subject:

- `App/local watcher: collapse per-project RecommendedWatcher instances into shared pools`

Description:

- Refactor local `ProjectWatcher` and singleton file watches so the app process does not create one `RecommendedWatcher` per project/file. Use one shared tree watcher pool and one shared singleton-file watcher pool, while preserving current event classification behavior.

### Task draft 4

Subject:

- `Diagnostics: add inotify instance telemetry and exhaustion warnings`

Description:

- Emit structured telemetry for inotify instance count, watch-descriptor count, daemon listener count, and logical subscription count. Surface early warnings in logs and diagnostics before the system approaches `max_user_instances`.

## Final assessment

The immediate band-aid of raising `fs.inotify.max_user_instances` to `512` was justified, but it is not the fix.

The real root cause is architectural:

1. ordinary daemon watchers are created per `watch` request in [watch.rs:27](/home/user/projects/taurhaus/src-tauri/src/daemon/watch.rs#L27)
2. dedupe is scoped to a single connection in [server.rs:248](/home/user/projects/taurhaus/src-tauri/src/daemon/server.rs#L248)
3. the app can create duplicate listener connections during startup/reconcile windows from [watchers.rs:46](/home/user/projects/taurhaus/src-tauri/src/startup/watchers.rs#L46) and [daemon_lifecycle.rs:176](/home/user/projects/taurhaus/src-tauri/src/daemon_lifecycle.rs#L176)

That combination multiplies inotify instances silently. The current live `66` daemon instances are the result.

## Runtime telemetry signals

The runtime now emits structured `inotify.*` diagnostics to `taurhaus.log.jsonl` so instance exhaustion is visible before failure.

- `inotify.telemetry`
  - emitted by the daemon at startup and on a periodic cadence
  - emitted by the app backend on local watch reconcile cycles when Linux inotify stats are observable
  - key fields:
    - `process_local_inotify_instances`
    - `process_local_inotify_watch_descriptors`
    - `daemon_listener_connections`
    - `physical_watch_registrations`
    - `logical_watch_subscriptions`
    - `system_user_inotify_instances`
    - `system_user_inotify_instance_limit`
    - `system_user_inotify_instance_pct`

- `inotify.capacity.warning`
  - emitted when `system_user_inotify_instance_pct >= 75`

- `inotify.capacity.error`
  - emitted when `system_user_inotify_instance_pct >= 90`

Operator interpretation:

- If `process_local_inotify_instances` is high but `logical_watch_subscriptions` is low, the process is burning instances on fixed watcher infrastructure or duplicated physical watchers rather than on legitimate subscriptions.
- If `daemon_listener_connections` is much larger than expected and `logical_watch_subscriptions` is close to `physical_watch_registrations`, suspect duplicate client connections rather than a single inflated watch tree.
- If `system_user_inotify_instance_pct` crosses the warning/error thresholds while app-local counts stay low, the daemon or other user processes are the likely offender rather than the app backend.
