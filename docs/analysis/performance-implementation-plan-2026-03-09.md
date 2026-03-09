# Performance Implementation Plan — 2026-03-09

Task: `#811`
Input document reviewed: [performance-improvement-possibilities-2026-03-09.md](/home/mstie/projects/taurhaus/docs/analysis/performance-improvement-possibilities-2026-03-09.md)

## Review outcome

The unified performance document is directionally accurate. The biggest current opportunities are still:

1. daemon steady-state CPU reduction
2. app startup/request-path latency cleanup
3. frontend markdown/rendering weight reduction
4. mesh scaling fixes around inbox/task storage

The main filtering result is that some findings are observations, not implementation tasks:

- `DaemonCompactionRuntime` duplicate polling is already fixed and should not be planned again.
- TCP accept-loop and long-poll plumbing are not current hotspots.
- high daemon thread count and high watch count are real, but they are secondary after scanner cadence/classification.
- large command surface and single SQLite mutex are architectural concerns, but not current top performance work.

## Validation matrix

### Mesh

| Finding | Verdict | Notes |
|---|---|---|
| Inbox read/append is full-file parse + rewrite | Valid | Confirmed in [inbox.rs](/home/mstie/projects/mesh/src/inbox.rs:43) and [inbox.rs](/home/mstie/projects/mesh/src/inbox.rs:63). Recent read-state race fix did not change the storage model. |
| Task-directory changes trigger full task rescans | Valid | `check_tasks()` still relies on [list_tasks()](/home/mstie/projects/mesh/src/tasks.rs:18) and daemon wake flow in [daemon.rs](/home/mstie/projects/mesh/src/daemon.rs:647). |
| Team-daemon uses fixed 1 Hz wake loop and periodic full idle passes | Valid | Confirmed in [team_daemon.rs](/home/mstie/projects/mesh/src/team_daemon.rs:48) and [team_daemon.rs](/home/mstie/projects/mesh/src/team_daemon.rs:80). |
| Read-oriented CLI commands still cause config writes | Valid but lower priority | Still structurally true in `mesh`, but not the highest-value current performance item. |
| Team-config reads are cheap today | Valid | No evidence this area should be optimized now. |

### Daemon

| Finding | Verdict | Notes |
|---|---|---|
| Display-session scanning is the dominant steady-state CPU cost | Valid | Confirmed by metrics and by [session_activity.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/session_activity.rs:16), [session_activity.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/session_activity.rs:85), [session_activity.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/session_activity.rs:194). |
| Per-session idle classification dominates scan time | Valid | Confirmed by metrics and by [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:661), [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:688), [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs:731), [codex.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/idle/codex.rs:191). |
| Tmux mapping still causes burst cost | Valid secondary | Real, but not first-order. |
| Thread count is high but stable | Valid observation | Not enough evidence to make this a first-wave task. |
| Inotify/watch footprint is large | Valid observation | Capacity/footprint concern; defer until after CPU fixes. |
| Compaction runtime is still a main CPU problem | Invalid / already fixed | Current code no longer has the old duplicate compaction scan; see [compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/compaction.rs:36). |
| TCP accept loop / long-poll delivery are hotspots | Invalid as a primary optimization target | They exist but do not match measured hot metrics. |

### App backend

| Finding | Verdict | Notes |
|---|---|---|
| App process is not the main sustained resource problem | Valid | Consistent with the source audit and current architecture. |
| Startup still burns ~2s in daemon bootstrap/reconnect | Valid | Hardcoded reconnect delay still exists in [daemon.rs](/home/mstie/projects/taurhaus/src-tauri/src/startup/daemon.rs:80). |
| Live mesh status does synchronous repair/reconcile on request path | Valid | [coordination.rs](/home/mstie/projects/taurhaus/src-tauri/src/commands/coordination.rs:548). |
| Foreground project lookup composes focus read + session listing + project scan | Valid | [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/commands/command_center/mod.rs:177), [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/commands/command_center/mod.rs:184). |
| `list_cli_sessions` fallback can do full scanner work on app thread | Valid | [session_listing.rs](/home/mstie/projects/taurhaus/src-tauri/src/commands/command_center/session_listing.rs:55). |
| Watcher init/reconcile still does whole-project DB work every 60s | Valid | [watchers.rs](/home/mstie/projects/taurhaus/src-tauri/src/startup/watchers.rs:89), [watchers.rs](/home/mstie/projects/taurhaus/src-tauri/src/startup/watchers.rs:97), [watchers.rs](/home/mstie/projects/taurhaus/src-tauri/src/startup/watchers.rs:118). |
| Single SQLite mutex, search writer budget, large command surface | Valid but not first-wave | Real architectural debt, not top-impact performance work right now. |

### Frontend

| Finding | Verdict | Notes |
|---|---|---|
| Markdown/Shiki/Mermaid path is the biggest frontend performance risk | Valid | Current code still uses Shiki highlighter + per-render fenced-language loading in [markdown.js](/home/mstie/projects/taurhaus/src/lib/markdown.js:74), [markdown.js](/home/mstie/projects/taurhaus/src/lib/markdown.js:161), plus lazy Mermaid rendering in [MarkdownRenderer.svelte](/home/mstie/projects/taurhaus/src/lib/MarkdownRenderer.svelte:94). Dependencies remain in [package.json](/home/mstie/projects/taurhaus/package.json:55). |
| Initial project bootstrap schedules a second full load after 1.5s | Valid | [Shell.svelte](/home/mstie/projects/taurhaus/src/Shell.svelte:620). |
| Project switch still fans out six IPC calls | Valid | [Shell.svelte](/home/mstie/projects/taurhaus/src/Shell.svelte:641), [projectSelection.js](/home/mstie/projects/taurhaus/src/lib/projectSelection.js:63). |
| Git history lacks virtualization | Valid | `GitTab` mounts all loaded rows via `{#each commits ...}` in [GitTab.svelte](/home/mstie/projects/taurhaus/src/lib/GitTab.svelte:604). |
| Code/file open still does full-content highlight/render | Valid | [CodeViewer.svelte](/home/mstie/projects/taurhaus/src/lib/CodeViewer.svelte:24) and markdown pipeline above. |
| Frontend logger forwards production console traffic over IPC | Valid | [logger.js](/home/mstie/projects/taurhaus/src/lib/logger.js:140), [logger.js](/home/mstie/projects/taurhaus/src/lib/logger.js:203). |
| File-tree flatten recompute / mesh-canvas layout reads / sidebar row derivation | Plausible but lower priority | Worth a second wave after heavier hotspots. |

## Filtered findings

### Already fixed or no longer relevant

- Do not plan work around the old redundant daemon compaction polling loop. That bug class is already removed.
- Do not prioritize TCP accept-loop or long-poll tuning in the daemon.
- Do not prioritize team-config read optimization in `mesh`.

### Valid, but defer until after higher-impact work

- daemon thread-count reduction
- daemon watch-count reduction
- app-wide SQLite mutex redesign
- search writer memory tuning
- frontend logger IPC trimming
- frontend file-tree flatten optimization
- frontend mesh canvas layout-read reduction
- mesh read-command implicit config-write cleanup

These are real, but they are not the first tasks I would assign if the goal is visible performance improvement.

## Proposed implementation tasks

Ordered by expected impact.

### 1. Reduce daemon steady-state scan cadence and split fast-vs-full classification

- Title: `Daemon: split display-session liveness from expensive idle classification`
- Scope:
  - keep a cheap fast path for process/pane presence and obvious activity changes
  - run full idle classification less often in steady state
  - make `SessionActivityHub` back off more aggressively than the current `30` stable-idle cycle gate
  - preserve correctness for transitions and active sessions
- Affected files:
  - [session_activity.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/session_activity.rs)
  - [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs)
  - [process.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/process.rs)
  - [proc_io.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/proc_io.rs)
- Complexity: `L`
- Expected impact: `Very high` — this is the clearest route to cutting the daemon’s `~24%` steady-state CPU load.

### 2. Make Codex idle resolution attachment-driven instead of broad project transcript search

- Title: `Daemon: reduce Codex classifier cost with stronger runtime transcript binding`
- Scope:
  - stop paying broad project-path transcript search cost in the hot loop when runtime attachment already knows the active transcript
  - bias classifier inputs toward authoritative member attachment / persisted transcript binding
  - reserve broad transcript search for recovery or uncertainty paths only
- Affected files:
  - [codex.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/idle/codex.rs)
  - [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/mod.rs)
  - [compaction_extractor.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/compaction_extractor.rs)
  - runtime attachment code under `src-tauri/src/coordination/`
- Complexity: `L`
- Expected impact: `High` — this attacks the heaviest classifier path directly and complements task 1.

### 3. Remove deterministic 2-second daemon bootstrap delay from startup

- Title: `App backend: replace fixed daemon reconnect sleep with readiness-based bootstrap`
- Scope:
  - remove the fixed `2s` sleep after background daemon start
  - reconnect as soon as the daemon is actually ready
  - retain protocol/version verification and watch respawn behavior
- Affected files:
  - [daemon.rs](/home/mstie/projects/taurhaus/src-tauri/src/startup/daemon.rs)
  - [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/startup/mod.rs)
  - daemon launcher/client readiness helpers
- Complexity: `M`
- Expected impact: `High` for startup latency — straightforward win with bounded scope.

### 4. Move live mesh status to fast snapshot first, repair second

- Title: `Coordination: take reconciliation off the live-status request path`
- Scope:
  - stop performing synchronous `reconcile_team_presence_for_live_status()` during every live-status request
  - serve fast snapshot/status immediately
  - trigger bounded repair asynchronously or on a slower maintenance path
- Affected files:
  - [coordination.rs](/home/mstie/projects/taurhaus/src-tauri/src/commands/coordination.rs)
  - orchestrator implementation under `src-tauri/src/coordination/orchestrator.rs`
- Complexity: `M`
- Expected impact: `High` for mesh/runtime UI refresh responsiveness.

### 5. Remove duplicate first-project bootstrap load in the shell

- Title: `Frontend: eliminate delayed second project selection during startup`
- Scope:
  - remove or replace the `1500ms` delayed second `selectProject(...)`
  - preserve the “sidebar first, details later” intent without a duplicate full detail load
- Affected files:
  - [Shell.svelte](/home/mstie/projects/taurhaus/src/Shell.svelte)
  - [projectSelection.js](/home/mstie/projects/taurhaus/src/lib/projectSelection.js)
- Complexity: `S`
- Expected impact: `High` relative to effort — easy, user-visible startup churn reduction.

### 6. Narrow project-switch fan-out into staged loading

- Title: `Frontend: split project selection into critical and deferred sections`
- Scope:
  - stop treating all six project-selection IPC calls as one equally urgent batch
  - load critical detail/session summary first
  - defer README, relationships, and lower-value sections until after the core switch lands
  - keep degraded fallback behavior
- Affected files:
  - [Shell.svelte](/home/mstie/projects/taurhaus/src/Shell.svelte)
  - [projectSelection.js](/home/mstie/projects/taurhaus/src/lib/projectSelection.js)
  - any dependent overview components
- Complexity: `M`
- Expected impact: `High` for perceived project-switch latency and render stability.

### 7. Slim the markdown/rendering path before deeper UI micro-optimizations

- Title: `Frontend: reduce markdown/Shiki/Mermaid loading and rerender cost`
- Scope:
  - narrow the default syntax/highlighting footprint
  - prevent repeated full markdown post-processing where content did not materially change
  - ensure Mermaid only loads when a mermaid block is actually present
  - review the current assumption in `markdown.js` that bundle size is irrelevant
- Affected files:
  - [markdown.js](/home/mstie/projects/taurhaus/src/lib/markdown.js)
  - [MarkdownRenderer.svelte](/home/mstie/projects/taurhaus/src/lib/MarkdownRenderer.svelte)
  - [CodeViewer.svelte](/home/mstie/projects/taurhaus/src/lib/CodeViewer.svelte)
  - [package.json](/home/mstie/projects/taurhaus/package.json)
- Complexity: `L`
- Expected impact: `High` on frontend startup and large-document interaction.

### 8. Replace mesh inbox JSON-array storage with append-friendly storage

- Title: `mesh: remove full-file inbox rewrite behavior`
- Scope:
  - move inbox storage away from parse-whole-array / rewrite-whole-file semantics
  - preserve locking, corrupt-file handling, and read filtering behavior
  - keep the recently fixed “mark only displayed messages” semantics intact
- Affected files:
  - [inbox.rs](/home/mstie/projects/mesh/src/inbox.rs)
  - [main.rs](/home/mstie/projects/mesh/src/main.rs)
  - [daemon.rs](/home/mstie/projects/mesh/src/daemon.rs)
  - any inbox tests affected by storage format
- Complexity: `L`
- Expected impact: `High` for mesh scaling and write amplification reduction.

### 9. Replace mesh full task-directory rescans with incremental task mutation tracking

- Title: `mesh: stop waking daemons into whole-directory task rescans`
- Scope:
  - stop reparsing every task JSON file on every relevant directory event
  - use task journal / targeted reads / cached ownership state to detect assignment changes incrementally
- Affected files:
  - [daemon.rs](/home/mstie/projects/mesh/src/daemon.rs)
  - [tasks.rs](/home/mstie/projects/mesh/src/tasks.rs)
  - [task_journal.rs](/home/mstie/projects/mesh/src/task_journal.rs)
- Complexity: `L`
- Expected impact: `Medium-high`, especially as task counts and daemon counts grow.

### 10. Make watcher reconcile and foreground lookup consume cached fast state

- Title: `App backend: remove expensive composed slow paths from foreground and watcher flows`
- Scope:
  - reduce `get_foreground_project` dependence on full `list_cli_sessions` + DB project scan where possible
  - reduce `startup/watchers` periodic reconcile cost by caching watch-target inputs and avoiding unconditional full reconcile every 60s
- Affected files:
  - [mod.rs](/home/mstie/projects/taurhaus/src-tauri/src/commands/command_center/mod.rs)
  - [session_listing.rs](/home/mstie/projects/taurhaus/src-tauri/src/commands/command_center/session_listing.rs)
  - [watchers.rs](/home/mstie/projects/taurhaus/src-tauri/src/startup/watchers.rs)
- Complexity: `M`
- Expected impact: `Medium-high` — improves tails and removes avoidable request-path work.

## Suggested execution order

If this becomes the actual work queue, I would run it in this order:

1. Task 1 — daemon cadence split
2. Task 3 — startup bootstrap sleep removal
3. Task 5 — duplicate startup project load removal
4. Task 4 — live mesh status fast snapshot path
5. Task 6 — staged project-switch loading
6. Task 2 — Codex classifier binding cleanup
7. Task 7 — markdown/rendering reduction
8. Task 8 — mesh inbox storage change
9. Task 9 — mesh task-scan incrementalization
10. Task 10 — foreground + watcher reconcile cleanup

## Not recommended right now

I would not spin off dedicated tasks yet for:

- daemon TCP accept-loop tuning
- daemon long-poll transport tuning
- daemon thread-count reduction as a primary target
- daemon watch-count reduction before scanner CPU is addressed
- app-wide SQLite concurrency redesign
- frontend logger bridge trimming as a first-wave optimization
- frontend mesh-canvas DOM-read cleanup
- frontend file-tree flatten optimization

Those are either second-order costs or riskier than the expected payoff right now.
