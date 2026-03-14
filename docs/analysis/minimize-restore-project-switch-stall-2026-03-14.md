# Minimize / Restore Project-Switch Stall Investigation

## Scope

Task `#1293`: investigate the reported Taurhaus hang where the app is minimized,
restored, and then stalls for roughly `10-20 s` when the user switches
projects. The goal of this note is to capture the current code behavior,
separate what minimize/restore actually does from what project switching does,
rank the likely root-cause hypotheses, and recommend the next bounded change
set.

## Evidence Reviewed

Frontend:

- `src/Shell.svelte`
- `src/lib/shell/events.svelte.js`
- `src/lib/projectSelection.js`
- `src/lib/sessionStore.svelte.js`
- `src/lib/shell.test.js`

Backend:

- `src-tauri/src/provider/daemon_client.rs`
- `src-tauri/src/daemon_lifecycle.rs`
- `src-tauri/src/startup/watchers.rs`
- `src-tauri/src/fs/watcher.rs`
- `src-tauri/src/event_processor.rs`
- `src-tauri/src/commands/projects.rs`
- `src-tauri/src/commands/files.rs`
- `src-tauri/src/commands/git.rs`
- `src-tauri/src/commands/sessions.rs`
- `src-tauri/src/commands/relationships.rs`

Earlier related notes:

- `docs/analysis/windows-app-stall-investigation-2026-03-11.md`
- `docs/analysis/taurhaus-startup-freeze-investigation-2026-03-13.md`
- `docs/analysis/session-ui-resilience-experiments-2026-03-14.md`
- `docs/analysis/wsl-daemon-connection-stability-experiments-2026-03-14.md`
- `docs/analysis/performance-audit-frontend-2026-03-09.md`
- `docs/analysis/performance-implementation-plan-2026-03-09.md`

## Current Behavior

## 1. Minimize is just a window call

The Shell minimize button in `src/Shell.svelte` calls:

- `getCurrentWindow().minimize()`

That is the whole minimize path. There is no Tauri window listener for:

- restore
- unminimize
- focus regained
- window-visible catch-up refresh

There is also no explicit "after restore, refresh the selected project" path.

So today minimize itself does not schedule any special recovery work. Restore is
not a first-class lifecycle event in the Shell. After the window comes back, the
app simply continues with whatever background state and pending work it already
had.

## 2. Background session behavior depends on bridge state, not restore state

`src/lib/shell/events.svelte.js` and `src/Shell.svelte` currently treat session
presence like this:

- if Tauri is running and `sessionBridgeLive` is true, the UI is event-driven
  from daemon `sessions-updated`
- if the daemon leaves `connected`, `Shell.svelte` sets
  `sessionBridgeLive = false` and marks session presence stale
- when `sessionBridgeLive` is false, `setupSessionPollingLifecycle(...)`
  starts the fallback poller and uses `document.visibilitychange` to stop/start
  polling

Important consequences:

1. There is no restore-specific catch-up path.
2. Hidden/visible handling only matters for the polling fallback lane.
3. If the bridge stays live, minimize/restore does not trigger any explicit
   refresh at all.
4. If the bridge is not live, restore only resumes the polling fallback. It
   does not proactively rehydrate the current project selection.

This means the first real user action after restore can still be the first time
the app pays for stale or contended backend state.

## 3. Project switching is still one broad barrier

`selectProject(...)` in `src/Shell.svelte` still waits for
`loadProjectSelectionData(...)` before it mutates the selected-project shell
state.

The current selection batch is:

- `getProject`
- `getRecentCommits`
- `getLatestSession`
- `listSessions`
- `getReadme`
- `getRelationships`

Those requests are launched in parallel in `src/lib/projectSelection.js`, but
the switch still behaves as one barrier because `selectProject(...)` does not
apply the new shell state until all six wrapped results resolve.

The guards are only post-resolution guards:

- `selectLoadGuard`
- `sessionsLoadGuard`
- `readmeLoadGuard`
- `relationshipsLoadGuard`

They prevent stale results from winning, but they do **not** cancel or bypass
the underlying work once it has started.

That means a slow project switch can still consume the full wait budget even if
the user immediately switches again.

## 4. The project switch mixes cheap local sections with potentially slow daemon-backed sections

For a local project, most of the selection batch is cheap:

- `getProject`, `getLatestSession`, `listSessions`, and `getRelationships`
  are SQLite reads
- `getReadme` and `getRecentCommits` use the local provider

For a WSL-backed project with a connected daemon, the mix changes:

- `getProject`, `getLatestSession`, `listSessions`, and `getRelationships`
  are still local DB work
- `getReadme` can route through the daemon provider
- `getRecentCommits` can route through the daemon provider

That matters because the daemon provider still uses a single shared connection
mutex for normal requests in `src-tauri/src/provider/daemon_client.rs`.

Only `send_status_request(...)` was hardened to fast-fail on a busy connection.
Normal project-selection calls still go through:

- `self.conn.lock()`
- then one shared TCP request path
- with `30 s` git timeout for git-backed calls
- with `10 s` file timeout for file reads

So the project-selection path remains exposed to exactly the shared-connection
blocking shape that previously froze startup, except that only the status lane
was fixed. The general read lane was not.

## 5. Project selection also triggers more backend work than the user-visible read suggests

`get_project(...)` in `src-tauri/src/commands/projects.rs` does more than read:

- it calls `project::touch_activity(...)`
- it then calls `enqueue_activity_watch_reconcile(app.clone(), "project_selected")`

That reconciliation is bounded by an atomic "queued" flag, but it still means a
project switch can trigger:

- local watch-target reconciliation
- daemon-watch reconciliation
- watcher telemetry updates

So after restore, a switch is not just "load visible project data." It is also
"promote project activity and potentially re-evaluate watcher subscriptions."

## 6. Watcher/event processing can add post-restore background pressure

The local file watcher/event processor already has bounded batching:

- `300 ms` quiet window
- `2 s` max wait

That batching is correct, but the flush work is still real:

- touch project activity
- refresh git status
- update search index
- import sessions
- emit `project-files-changed`

If minimize/restore coincides with pending file churn or WSL daemon reconnect
activity, the app can return to the foreground while these background lanes are
still draining.

There is no explicit restore policy that says:

- defer non-critical maintenance for a short window after restore
- prioritize user-initiated project selection first

## Expected Behavior

The user-visible expectation should be:

1. minimizing Taurhaus does not create a heavy catch-up penalty on restore
2. restoring Taurhaus quickly re-establishes honest session/daemon state
   without blocking the shell
3. switching projects applies the new selection immediately or near-immediately
   for the critical data path
4. slow secondary sections such as README, relationships, or commit history do
   not hold the entire shell hostage
5. background daemon/watcher recovery work never forces the first foreground
   project switch to pay the entire contention cost

Current code does not meet that bar.

## Root-Cause Hypotheses

## 1. Most likely: project switch is waiting on slow daemon-backed sections after restore

Confidence: high

This is the strongest explanation for a `10-20 s` stall.

Why:

- restore has no dedicated catch-up path
- the next project switch still waits on the full six-section batch
- two of those sections can be daemon-backed for WSL projects:
  - `getRecentCommits`
  - `getReadme`
- normal daemon-backed reads still serialize through the single shared daemon
  connection lock
- earlier startup investigation already confirmed that shared-connection
  blocking can freeze the foreground
- the WSL stability experiments showed the daemon itself staying healthy while
  client-side shared-lane contention was easy to reproduce

In plain language:

after minimize/restore, the app likely is not "hung because minimize broke
something." It is more likely that restore leaves the app in a mildly degraded
or busy state, and the next project switch is the first foreground action that
waits on contended daemon-backed project-selection sections.

## 2. Missing restore-specific recovery makes the first project switch absorb backlog

Confidence: medium-high

The code today has no explicit "app became visible again" recovery routine for:

- daemon status
- session snapshot
- selected-project freshness
- foreground project freshness

So restore does not actively smooth the transition back to foreground use. It
just leaves the next real interaction to discover whatever state drift happened
while the app was hidden.

That does not by itself create a 20-second stall, but it makes a stalled first
project switch much more plausible.

## 3. Project selection triggers extra reconcile work that can worsen tails

Confidence: medium

`get_project(...)` promotes activity and enqueues watcher reconciliation on
every explicit project selection. That is reasonable in steady state, but it is
extra work on the same edge where the user is already waiting for the new
project to appear.

This is probably not the primary 10-20 second blocker, but it is unnecessary
tail pressure on the same path.

## 4. Polling fallback visibility behavior is a smaller contributing risk

Confidence: low-medium

When `sessionBridgeLive` is false, `setupSessionPollingLifecycle(...)` starts
polling immediately and only then registers `visibilitychange` control.

That means the fallback lane is not especially restore-aware and may briefly do
background work even while the document is hidden. This is unlikely to be the
main cause of the reported switch stall, but it is another example of restore
not being treated as an explicit lifecycle boundary.

## Recommended Action Plan

## 1. Instrument the actual restore -> switch path before changing semantics

Add one bounded measurement chain around this exact user report:

- `shell.window.visibility_hidden`
- `shell.window.visibility_visible`
- `shell.project_selection.started`
- `shell.project_selection.section.completed`
- `shell.project_selection.completed`

For each project-selection section, record:

- section name
- project id
- duration
- provider route (`local` vs `daemon`)
- whether the daemon connection was already busy when the call started

Without this, future fixes will still be partly inferential.

## 2. Split project selection into critical and deferred phases

Do not keep the whole switch behind one six-section barrier.

Recommended split:

Critical first:

- `getProject`
- `getLatestSession`
- `listSessions`

Deferred:

- `getRecentCommits`
- `getReadme`
- `getRelationships`

User-visible goal:

- apply the new selected project and its core shell state immediately
- allow overview enrichment to fill in afterward
- keep degraded fallback behavior, but stop making README/commits/relationships
  gate the switch itself

This is the highest-value user-facing change regardless of whether restore is
the trigger.

## 3. Add an explicit restore catch-up path

Treat restore/visibility-visible as a first-class event.

Recommended restore behavior:

- non-blocking daemon status refresh
- non-blocking session snapshot hydrate if the bridge is not currently live
- non-blocking foreground-project refresh
- optional prefetch for the currently selected project's deferred sections

Important:

- this should **not** block shell interactivity
- it should be best-effort and cancel-safe

The goal is to make restore itself absorb the cheap recovery work, so the first
manual project switch is not the first place backlog becomes visible.

## 4. Stop letting ordinary project-selection reads queue behind the shared daemon lane

The startup/status fix solved only the status path. This report suggests the
same principle now needs to reach ordinary foreground selection reads.

Bounded options, in order:

1. if the shared daemon connection is busy, fail fast or degrade for
   non-critical project-selection sections such as README and commits
2. use a separate daemon read lane for foreground project-selection reads
3. add tighter per-section budgets for selection-time daemon reads than the
   generic `30 s` / `10 s` transport timeouts

At minimum, the foreground selection path should not inherit the full daemon
transport budget when the shell already knows how to show partial data.

## 5. Reconsider `project_selected` watcher reconcile on the immediate read path

`get_project(...)` should not force selection latency to carry watcher-policy
maintenance unless it is truly necessary.

Recommended change:

- keep `touch_activity(...)`
- move watcher reconcile farther off the immediate selection path, or debounce
  it more aggressively, or run it only when activity thresholds actually cross
  a boundary

This is a smaller win than staged project loading, but it is a sensible tail
reduction.

## Bottom Line

This does not look like a raw minimize/restore bug. The code today treats
minimize as a plain window operation and restore as an implicit background
continuation. The more likely failure is:

- restore leaves the app without an explicit catch-up phase
- the next project switch still waits on a broad six-section selection batch
- some of those sections can still queue behind the shared daemon connection
  for WSL projects
- the user experiences that combined cost as a post-restore "hang"

The best next move is not a broad rewrite. It is:

1. instrument restore -> selection timing with provider-route visibility
2. split project switching into critical and deferred phases
3. add a non-blocking restore catch-up routine
4. stop ordinary foreground project-selection reads from paying full
   shared-daemon contention costs
