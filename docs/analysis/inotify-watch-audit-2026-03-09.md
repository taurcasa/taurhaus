# Inotify Watch Audit — 2026-03-09

## Scope

This is a code audit of every taurhaus watch-registration site that creates `notify` watchers or causes the daemon to create them indirectly. It covers:

- app-local watcher infrastructure
- daemon-owned recursive project watches
- task-directory and tmux-focus watches
- compaction topology, signal-log, and transcript watchers
- current filtering behavior and whether it reduces watch count

This is analysis only. No code was changed.

## Executive summary

The watch footprint is dominated by recursive project-tree watchers, not by the small coordination/compaction side channels.

Key conclusions:

- The main watch creators are `ProjectWatcher::watch_project(...)` in the app process and `daemon::watch::handle_watch(...)` in the daemon. Both register recursive watchers on whole trees.
- Current gitignore handling for project watches is **post-filtering of events**, not pre-pruning of watch registration. It reduces downstream work, but it does **not** materially reduce inotify watch count.
- The auxiliary watchers are small by comparison:
  - tmux focus file: 1 watch
  - compaction signal watchers: about 1-2 watches per managed team
  - compaction transcript watchers: about 1 watch per active Codex transcript file
  - compaction topology watcher: 1 recursive watch over `~/.claude/teams/`
- The reported large watch footprint is therefore coming primarily from watched project source trees and secondarily from the recursive `.claude/tasks/` watch.
- The best optimization opportunities are on the recursive project/task-tree layer: avoid watching trees that do not need real-time indexing, and move ignore handling from post-filter to pre-prune if watch-count reduction is the goal.

## Inventory of watch sites

### 1. App-local project watcher

- Module: [watcher.rs](/home/mstie/projects/taurhaus/src-tauri/src/fs/watcher.rs)
- Registration path: `ProjectWatcher::watch_project(...)`
- Setup callers:
  - [watchers.rs](/home/mstie/projects/taurhaus/src-tauri/src/startup/watchers.rs) via `reconcile_activity_watches(...)`
  - same file via `ensure_task_directory_watch(...)`
- Paths watched:
  - active/recent local project roots selected by activity thresholds
  - `~/.claude/tasks/` when the app owns that watch path
- Watch mode:
  - `RecursiveMode::Recursive`
- Gitignore filtering:
  - yes for normal file events, via `ignore::gitignore::Gitignore`
  - but only after the watcher is already registered and events are received
- Filtering type:
  - **post-filter**, not pre-filter
- Approximate watch-count contribution:
  - dominant source
  - each recursive root consumes roughly one watch per subdirectory in that tree on Linux/inotify
  - this site alone can explain tens of thousands of watches if many large repos are active

Notes:

- `classify_notify_event(...)` ignores most `.git/*` internals except `HEAD`, `index`, and branch refs, and ignores obvious build/tool directories like `node_modules`, `target`, `dist`, `.next`, `.nuxt`, `.svelte-kit`, `.cache`, and Python cache directories.
- That filtering happens only after recursive registration, so ignored directories still contribute inotify watches.

### 2. App-local single-file watch for tmux focus

- Module: [watcher.rs](/home/mstie/projects/taurhaus/src-tauri/src/fs/watcher.rs)
- Registration path: `ProjectWatcher::watch_file(...)`
- Setup caller:
  - [watchers.rs](/home/mstie/projects/taurhaus/src-tauri/src/startup/watchers.rs) via `ensure_tmux_focus_watch(...)`
- Path watched:
  - `tmux-focus.json` under app data dir
- Watch mode:
  - `RecursiveMode::NonRecursive`
- Gitignore filtering:
  - no
- Filtering type:
  - exact-path match only
- Approximate watch-count contribution:
  - negligible, effectively 1 watch

### 3. Daemon-owned recursive project watches

- Module: [watch.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/watch.rs)
- Registration path:
  - `handle_watch(...)`
- Call chain:
  - [daemon_lifecycle.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon_lifecycle.rs) builds a daemon watch plan
  - [event_listener.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/event_listener.rs) sends `WATCH` RPCs to the daemon
  - daemon dispatches to `handle_watch(...)`
- Paths watched:
  - WSL/Linux project roots selected by activity thresholds
  - `~/.claude/tasks/` when daemon-owned
- Watch mode:
  - `RecursiveMode::Recursive`
- Gitignore filtering:
  - yes for event classification, via the same shared `build_gitignore(...)` and `classify_notify_event(...)`
- Filtering type:
  - **post-filter**, not pre-filter
- Approximate watch-count contribution:
  - dominant source on Windows/WSL deployments
  - this is the site most likely responsible for the majority of the observed `~60k-75k` daemon watch counts

Notes:

- This is structurally the same as the app-local recursive watcher, just moved into the daemon for WSL project ownership.
- Since the daemon owns WSL project trees on Windows, this site is the most relevant one for the large inotify numbers seen in daemon monitoring.

### 4. Daemon event-listener watch registration bridge

- Module: [event_listener.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/event_listener.rs)
- Registration path:
  - `DaemonEventListener::watch(...)`
- Paths watched:
  - same WSL project roots / tasks directory as above, but indirectly through daemon RPC
- Watch mode:
  - not a direct `notify` site itself
- Gitignore filtering:
  - delegated to daemon side
- Filtering type:
  - N/A locally, indirect trigger only
- Approximate watch-count contribution:
  - none directly, but it is the app-side entrypoint that causes daemon recursive watches to exist

This is not a `notify` constructor, but it is part of the live watch creation path and is worth documenting so the topology is understandable.

### 5. Daemon compaction topology watcher on `~/.claude/teams/`

- Module: [compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/compaction.rs)
- Registration path:
  - `start_team_topology_watcher(...)`
- Paths watched:
  - `~/.claude/teams/` recursively
  - effectively interested in team directory create/delete and `config.json` / `config.json.tmp`
- Watch mode:
  - `RecursiveMode::Recursive`
- Gitignore filtering:
  - no
- Filtering type:
  - path-based post-filter in `is_team_topology_event(...)`
- Approximate watch-count contribution:
  - small to medium
  - proportional to number of directories under `~/.claude/teams/`, not project source-tree size
  - probably tens or hundreds, not tens of thousands

Notes:

- This watcher exists only in the daemon-owned compaction runtime path.
- Recent changes already moved this from polling to file watching.

### 6. Per-team compaction signal watchers

- Module: [compaction_watcher.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/compaction_watcher.rs)
- Registration paths:
  - `CompactionSignalWatcher::start_at(...)`
  - `run_watcher_loop(...)`
- Setup callers:
  - app path: [startup/compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/startup/compaction.rs)
  - daemon path: [daemon/compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/compaction.rs)
- Paths watched:
  - per team signal directory (non-recursive)
  - optionally the concrete `codex-compaction-signals.jsonl` file itself (non-recursive) when present
- Watch mode:
  - directory: `RecursiveMode::NonRecursive`
  - signal file: `RecursiveMode::NonRecursive`
- Gitignore filtering:
  - no
- Filtering type:
  - exact signal-path gating in `should_process_signal_event(...)`
  - plus reconciliation polling every few seconds as fallback
- Approximate watch-count contribution:
  - low
  - roughly 1-2 watches per managed team

Notes:

- This is operational state under `~/.claude/teams/...`, not repo content. Gitignore is not relevant here.
- This is not a meaningful contributor to the inotify budget.

### 7. Compaction transcript extractor watchers

- Module: [compaction_extractor.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/compaction_extractor.rs)
- Registration path:
  - `reconcile_watched_transcripts(...)`
- Setup callers:
  - app path: [startup/compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/startup/compaction.rs)
  - daemon path: [daemon/compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/compaction.rs)
- Paths watched:
  - active Codex transcript JSONL files, one watch per file
- Watch mode:
  - `RecursiveMode::NonRecursive`
- Gitignore filtering:
  - no
- Filtering type:
  - explicit path membership in `watched_paths`
  - periodic reconciliation fallback
- Approximate watch-count contribution:
  - low
  - roughly one watch per active Codex transcript file

Notes:

- This path scales with active Codex sessions, not with source-tree size.
- Even with dozens of active sessions, it stays small compared with recursive project-tree watchers.

### 8. App startup compaction watcher bootstrap

- Module: [startup/compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/startup/compaction.rs)
- Role:
  - not itself a `notify` constructor, but a startup owner of the compaction extractor + per-team signal watchers on non-Windows platforms
- Runtime relevance:
  - disabled on Windows
  - relevant on Linux/macOS app-owned compaction mode
- Approximate watch-count contribution:
  - indirect only, equal to sites 6 and 7 when app-owned

### 9. Daemon compaction runtime bootstrap

- Module: [daemon/compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/compaction.rs)
- Role:
  - not itself a single watch site, but the owner of:
    - topology watcher
    - per-team signal watchers
    - transcript extractor watchers
- Runtime relevance:
  - only under WSL today
- Approximate watch-count contribution:
  - indirect only, equal to sites 5, 6, and 7 combined

### 10. Event processor

- Module: [event_processor.rs](/home/mstie/projects/taurhaus/src-tauri/src/event_processor.rs)
- Watch creation:
  - none
- Role:
  - consumes `WatchEvent`s and updates git/search/UI state

This file is part of the watch pipeline but does not allocate any watches.

## Gitignore filtering assessment

### Where gitignore filtering is appropriate

These should respect repo ignore rules:

- app-local recursive project watchers on real project roots
- daemon-owned recursive project watchers on WSL/Linux project roots

Reason:

- these are code/content trees where user intent is usually “react to meaningful source changes, not generated junk”

### Where gitignore filtering is not appropriate

These should **not** be gitignore-based:

- `~/.claude/tasks/`
- `tmux-focus.json`
- `~/.claude/teams/` topology watcher
- per-team compaction signal logs
- active transcript JSONL files

Reason:

- these are operational state files, not git-managed project trees
- many are outside repos entirely
- correctness depends on seeing all changes regardless of `.gitignore`

### Current nuance for the task-directory watch

`ensure_task_directory_watch(...)` currently uses the same `watch_project(...)` path for `~/.claude/tasks/`. That means it inherits the generic gitignore/event classifier logic even though the directory is not a repo. In practice that mostly behaves like “no gitignore filtering” because there is usually no `.gitignore` there, but it still uses the same recursive watch machinery and the same hardcoded directory exclusions.

That is functionally acceptable, but it is conceptually mismatched with the project-tree watcher abstraction.

## Current filtering implementation: pre-filter or post-filter?

For the main project watch system, filtering is **post-filter**.

Evidence:

- both `ProjectWatcher::watch_project(...)` and `daemon::watch::handle_watch(...)` register recursive watchers on the full root immediately
- only after events arrive does `classify_notify_event(...)`:
  - rebuild `.gitignore` matchers
  - discard ignored regular-file paths
  - debounce git internals
  - suppress build/tool directories from downstream handling

Implication:

- ignored directories still get watched by `notify`
- ignored content still consumes inotify watch slots
- current gitignore logic reduces event processing cost, not watch-count cost

This is the most important structural point in the audit.

## Watch count breakdown

Precise live watch count depends on the current active project set, but the code structure makes the relative split clear.

Observed baselines already in repo analysis/history:

- prior daemon audit cited median inotify watches around `74,872`
- later resource monitor samples show daemon watch counts around `60,508-60,514`

Those totals are consistent with the following breakdown.

### Dominant contributors

#### Recursive project-tree watches

Includes:

- app-local `watch_project(...)` for local/native projects
- daemon `handle_watch(...)` for WSL projects

Expected contribution:

- overwhelming majority of total watch count
- roughly proportional to the total number of subdirectories across all watched project roots

Why:

- recursive `notify` on Linux/inotify consumes one watch per directory
- large repos with `node_modules`, build outputs, vendored SDKs, caches, or large generated trees are expensive even if later event filtering ignores them

#### Recursive `~/.claude/tasks/` watch

Includes:

- app-local or daemon-owned tasks watch, depending on platform/daemon ownership

Expected contribution:

- can be meaningful if the tasks directory has many nested project/task folders
- still usually much smaller than the sum of watched source trees

### Small contributors

#### `~/.claude/teams/` recursive topology watcher

Expected contribution:

- low
- depends on number of team dirs and internal state dirs/files

#### Per-team compaction signal watchers

Expected contribution:

- very low
- about `1-2 * number_of_managed_teams`

#### Transcript-file watchers

Expected contribution:

- low
- about `1 * number_of_active_codex_transcripts`

#### `tmux-focus.json`

Expected contribution:

- negligible
- exactly 1 file watch

## Optimization opportunities

### 1. Move project ignore handling from event post-filter to watch pre-prune

Impact:

- highest leverage for reducing inotify slots

Current problem:

- project watchers recurse whole trees, including ignored/generated directories
- `.gitignore` only suppresses downstream event handling

Opportunity:

- build an allowlist/pruned directory set before registering watches, or switch to a watcher model that can avoid descending into ignored subtrees
- at minimum, prune known heavy directories before recursive registration

Expected payoff:

- direct reduction in watch count
- especially large wins for `node_modules`, `target`, framework caches, and vendored/generated trees

### 2. Split repo-content watching from operational-directory watching

Impact:

- medium

Current problem:

- `watch_project(...)` is reused for both real project roots and `~/.claude/tasks/`
- the abstraction mixes git-aware repo semantics with non-repo operational directories

Opportunity:

- keep repo-content watchers git-aware
- give operational dirs dedicated, simpler watchers with exact rules

Expected payoff:

- cleaner behavior boundaries
- easier to reason about optimization without accidentally breaking non-repo state watching

### 3. Re-check whether recursive watch of `~/.claude/tasks/` needs the full tree

Impact:

- medium

Current problem:

- the tasks directory is recursively watched as a whole tree

Opportunity:

- determine whether only a subset of files or a shallower directory level is actually required
- if task state lives in predictable filenames, narrow the watch scope

Expected payoff:

- could cut a meaningful secondary block of watches if the tasks tree is deep

### 4. Tighten activity-based watch eligibility further

Impact:

- medium to high depending on project count

Current state:

- local/daemon project watches already use activity-based reconciliation
- stale and dormant projects are not supposed to remain watched

Opportunity:

- audit thresholds and ensure active/recent classification is not too generous
- confirm startup and explicit reconcile paths do not temporarily over-watch large inactive sets

Expected payoff:

- fewer recursive project roots live at once
- direct watch-count and event-volume reduction

### 5. Detect and surface heavy watch roots explicitly

Impact:

- medium

Opportunity:

- log or snapshot per-root directory counts when a recursive watch is registered
- identify which projects contribute the majority of watch slots

Expected payoff:

- makes future optimization targeted instead of speculative
- likely reveals a few pathological repos dominate total watch count

### 6. Keep compaction watchers event-driven; do not chase them as the main inotify problem

Impact:

- low

Reason:

- compaction topology/signal/transcript watchers are tiny compared with recursive source-tree watches
- they are not the source of the reported large watch count

### 7. Consider polling only for low-value operational watches, not for source trees

Impact:

- selective

Best candidates:

- low-frequency operational state where missing an event is tolerable and watch count matters more than latency

Bad candidates:

- live project source trees that drive sidebar/search/git freshness

## Recommended priorities

1. Reduce recursive project-tree watch breadth.
2. Measure per-root watch contribution so the worst offenders are explicit.
3. Reassess recursive `~/.claude/tasks/` scope.
4. Keep compaction/tmux watchers as-is unless correctness problems appear.

## Bottom line

If taurhaus is sitting around `~60k-75k` inotify watches, the problem is not the coordination extras. It is the recursive project/task tree layer.

The current gitignore implementation is useful for event suppression but does not save watch slots, because it runs after registration. Any serious watch-count reduction effort needs to attack recursive watch breadth directly.
