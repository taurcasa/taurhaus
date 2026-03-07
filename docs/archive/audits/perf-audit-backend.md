# Rust Backend Performance Audit (Task #171)

Read-only audit of `src-tauri/` backend code paths, with targeted local latency sampling against the daemon API on this repository.

## IPC Command Latency
**Current**: Local daemon benchmark samples (p50/p95, ms): `ping` 43.94/47.95, `git_status` 47.95/47.99, `git_log(limit=200)` 205.4/209.61, `file_tree` 187.45/192.5, `get_project_tasks` 1041.7/1050.39, `git_commits_in_range(1y)` 1776.09/1781.8.  
**Issue**: IPC handlers are synchronous and backend state is heavily mutex-serialized (`DbState`, `SearchState`, daemon connection mutex), so frontend parallel invokes queue behind shared locks/connection. There is also a stable per-request floor (~40-50ms on localhost daemon path).  
**Impact**: High — user-visible latency accumulates quickly for multi-command screen loads and chained operations.  
**Fix**: Enable lower-latency socket behavior (`TCP_NODELAY`) and reduce serialized paths (separate read channels or small connection pool, narrower lock scopes, move heavyweight handlers to worker threads) — effort: M.

## Git Operations (libgit2)
**Current**: Git-heavy calls are top-latency operations in sampled data: `git_log(limit=200)` ~205ms p50 and `git_commits_in_range` ~1.78s p50.  
**Issue**: `commits_in_range` does duplicate traversal work by running separate walks for commits and changed files. Diff/range operations scale poorly with deep history and broad time windows; no memoization for repeated range queries.  
**Impact**: High — timeline/history enrichment and commit-range views can block for seconds on medium/large repos.  
**Fix**: Merge commit+file aggregation into a single revwalk, add optional range caps/pagination, and cache hot ranges briefly per repo — effort: M.

## Session Scanning
**Current**: Daemon scanner runs every 500ms. Runtime telemetry (`session_scanner metrics`) shows stable cycle cost with 5 sessions: `process_scan_ms` ~51, `tmux_ms` ~51, `classify_ms` ~12-15, total ~114-118ms/cycle.  
**Issue**: Fixed high-frequency polling consumes substantial background CPU even when session state is stable. Poll cost is dominated by repeated process + tmux scans.  
**Impact**: High — sustained background CPU/battery cost and potential contention with foreground work.  
**Fix**: Introduce adaptive poll cadence (fast only during transitions, slower when stable), plus memoized/differential process and tmux scans — effort: M.

## Search Indexing (tantivy)
**Current**: Initial build indexes project files + sessions + commits; incremental updates run via watcher events. On this repo, indexable text set is ~376 files / ~4.58MB.  
**Issue**: Incremental file indexing commits per file (`update_file` commits each update), which amplifies disk/index overhead under bursts. Session indexing performs an N+1 pattern (`list_sessions` then per-session detail lookup). Full rebuild paths can hold DB/search locks for extended spans.  
**Impact**: Medium-High — mostly background, but bursty file changes and rebuild windows can degrade responsiveness.  
**Fix**: Batch incremental updates and commit once per flush, replace session N+1 with a single detail query path, and offload rebuild work with reduced lock hold windows — effort: M.

## SQLite Queries
**Current**: WAL mode is enabled in DB init. Most core queries are indexed, but query plans show temp-sort penalties in key paths.  
**Issue**: `sessions WHERE project_id ... ORDER BY date DESC` uses `idx_sessions_project_id` and then temp B-tree sort (missing composite order index). Some archived-task and relationship list queries also require temp sorts for current ORDER BY patterns.  
**Impact**: Medium — costs increase with larger per-project session/task histories and relationship volumes.  
**Fix**: Add composite indexes aligned to ORDER BY/filter shapes (notably `(project_id, date DESC)` for sessions and an archived task timeline order index), and review relationship sort/index pairing — effort: S/M.

## File Watching (notify)
**Current**: Batch-and-flush strategy is in place (300ms quiet window, 2s max wait), plus git event debounce (2s).  
**Issue**: Search update path still commits per changed file inside a batch, reducing batching benefits. Recursive watcher scaling can hit watch limits; overflow handling currently warns and skips projects. `.gitignore` change handling logs that watch rebuild is not implemented.  
**Impact**: Medium — can create indexing inefficiency and blind spots in high-project environments.  
**Fix**: Commit once per project batch, add proactive watch-limit telemetry/fallback strategy, and implement `.gitignore`-driven refresh flow — effort: M.

## Startup Time
**Current**: Startup path correctly pushes several expensive tasks to background threads after setup (activity reseed, session scan, index build-if-empty, task sync).  
**Issue**: Setup still performs synchronous DB/open/watch initialization before UI is fully ready, and there is limited startup milestone instrumentation (first frame / first interactive command) for regression tracking.  
**Impact**: Medium-Low — likely acceptable on small installs, but can become variable with many projects/watchers.  
**Fix**: Add startup milestone timing instrumentation, then defer non-critical watch/bootstrap phases progressively after first render — effort: S/M.
