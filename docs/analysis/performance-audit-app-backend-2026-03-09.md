# Performance Audit: Rust App Backend (Windows-Native)

Date: 2026-03-09
Owner: developer2
Scope: `taurhaus.exe` app-process backend behavior on Windows-native runs, using current production logs and `/tmp/taurhaus-resource-monitor-v2.csv`

## Executive Summary

The Windows app-process backend is not under steady-state CPU or memory pressure.

Measured steady-state for the latest deployed `taurhaus.exe` process (`pid=75688`) after the first 60 seconds:

- CPU: avg `0.02%`, median `0.02%`, p95 `0.08%`, max `0.49%`
- RSS: avg `44.65 MB`, median `44.37 MB`, p95 `47.68 MB`, max `49.39 MB`
- Threads: avg `70.34`, median `70`, p95 `73`, max `78`
- Handles: avg `455.34`, median `455`, p95 `460`, max `475`

That means the main opportunities are synchronous latency paths, not steady-state resource burn.

The dominant app-side costs are:

1. startup daemon phase latency
2. live mesh-status reconciliation on request paths
3. foreground/session listing paths that compose multiple expensive backend operations synchronously

## Data Sources

- Resource monitor: `/tmp/taurhaus-resource-monitor-v2.csv`
- Structured app logs: `C:\\Users\\user\\AppData\\Roaming\\com.taurhaus.dev\\taurhaus.log.jsonl` and rotated siblings
- Current backend source tree under `src-tauri/src/`

## Measured Baselines

### App Process Baseline

Latest observed app PID in the resource monitor: `75688`

Startup, first 30 seconds:

- CPU: avg `0.92%`, median `0.26%`, p95 `2.84%`, max `2.88%`
- RSS: avg `36.20 MB`, median `37.98 MB`, p95 `39.54 MB`, max `39.55 MB`
- Threads: avg `68.42`, median `77`, max `77`
- Handles: avg `440.08`, median `458`, p95 `461`, max `465`

Startup, first 60 seconds:

- CPU: avg `0.45%`, median `0.02%`, p95 `2.68%`, max `2.88%`
- RSS: avg `37.94 MB`, median `39.54 MB`, p95 `39.71 MB`, max `39.73 MB`
- Threads: avg `72.08`, median `76`, max `77`
- Handles: avg `447.33`, median `455`, p95 `459`, max `465`

Steady-state after 60 seconds:

- CPU: avg `0.02%`, median `0.02%`, p95 `0.08%`, max `0.49%`
- RSS: avg `44.65 MB`, median `44.37 MB`, p95 `47.68 MB`, max `49.39 MB`
- Threads: avg `70.34`, median `70`, p95 `73`, max `78`
- Handles: avg `455.34`, median `455`, p95 `460`, max `475`

Interpretation:

- The app process is effectively idle when users are not actively driving a slow IPC path.
- There is no evidence of sustained renderer/backend CPU churn in `taurhaus.exe`.
- The backend latency budget is being spent in blocking operations, not background burn.

### Startup Timing from Logs

Recent startup telemetry shows:

- `startup.database.completed`: avg `13.1 ms`, median `9 ms`, p95 `19 ms`, max `190 ms`
- `startup.daemon_phase.completed`: avg `1429.1 ms`, median `2114 ms`, p95 `2150 ms`, max `30124 ms`
- `startup.daemon_connect.deferred`: avg `2036.7 ms`, median `2037 ms`, p95 `2053 ms`, max `2058 ms`
- `startup.daemon_connect.succeeded`: avg `659.3 ms`, median `102.5 ms`, p95 `137 ms`, max `30124 ms`
- `startup.daemon_bootstrap.completed`: avg `2304.3 ms`, median `2298 ms`, p95 `2348 ms`, max `2375 ms`
- `startup.watchers.initialized`: avg `96.8 ms`, median `0 ms`, p95 `456 ms`, max `475 ms`
- `startup.search.initialized`: avg `12.7 ms`, median `4 ms`, p95 `46 ms`, max `224 ms`
- `startup.orchestration.completed`: avg `110.2 ms`, median `5 ms`, p95 `462 ms`, max `482 ms`

Interpretation:

- Database and search initialization are already cheap.
- Cold-start cost is dominated by daemon connect/bootstrap and, secondarily, watcher initialization.

### IPC Timing from Logs

Notable app-process IPC timings:

- `coordination_get_live_team_status`: avg `1753 ms`, median `1868 ms`, p95 `3255 ms`, max `34575 ms`
- `coordination_get_project_mesh_snapshot`: avg `712.9 ms`, median `336 ms`, p95 `2668 ms`, max `3163 ms`
- `list_cli_sessions`: avg `106 ms`, median `4 ms`, p95 `267 ms`, max `26089 ms`
- `get_foreground_project`: avg `653.3 ms`, median `332 ms`, p95 `2294 ms`, max `2339 ms`
- `get_recent_commits`: avg `64.8 ms`, median `45 ms`, p95 `90 ms`, max `3702 ms`
- `list_projects`: avg `0.9 ms`, median `0 ms`, p95 `3 ms`, max `291 ms`
- `get_project`: avg `2.1 ms`, median `1 ms`, p95 `3 ms`, max `225 ms`

Interpretation:

- simple DB reads are fast
- mesh runtime/status calls are still the most expensive user-facing backend calls
- session/foreground paths are not reliably cheap enough for dense UI refresh usage

### Daemon RPC Timing Seen from App Logs

The app frequently waits on daemon-backed work:

- `get_project_tasks`: avg `58 ms`, p95 `127 ms`, max `7077 ms`
- `ping`: avg `44.8 ms`, p95 `47 ms`, max `4712 ms`
- `git_log`: avg `48.7 ms`, p95 `75 ms`, max `2531 ms`
- `list_claude_sessions`: avg `322.2 ms`, p95 `498 ms`, max `1500 ms`
- `list_display_sessions`: avg `143 ms`, p95 `368 ms`, max `854 ms`

Interpretation:

- the app process itself is light, but synchronous daemon waits still surface as app latency

## Findings

### Critical

None.

There is no evidence of a critical app-process resource problem in the current Windows-native build. The app backend is cheap in steady state and currently bottlenecked by blocking latency, not runaway CPU or memory.

### High

#### 1. Startup still spends about 2 seconds in the daemon phase on the setup path

Evidence:

- `startup.daemon_phase.completed` median `2114 ms`
- `startup.daemon_connect.deferred` median `2037 ms`
- recent runs show `startup.daemon_phase.completed` clustered around `2136-2160 ms`

Code path:

- setup performs daemon-phase determination before the rest of orchestration in [`src-tauri/src/startup/mod.rs:356-390`](../../src-tauri/src/startup/mod.rs)
- daemon connect/validation happens synchronously in [`src-tauri/src/startup/mod.rs:422-510`](../../src-tauri/src/startup/mod.rs)

Why it matters:

- this is front-loaded into app startup, even though the app process itself is otherwise cheap
- it directly works against the repo’s “snappy” constraint

#### 2. Background daemon bootstrap still hardcodes a 2-second reconnect delay

Evidence:

- `startup.daemon_bootstrap.completed` median `2298 ms`, p95 `2348 ms`
- the timing distribution matches the fixed sleep in code

Code path:

- daemon bootstrap sleeps `2` seconds before reconnect in [`src-tauri/src/startup/daemon.rs:79-83`](../../src-tauri/src/startup/daemon.rs)

Why it matters:

- this is deterministic startup latency, not real work
- it also delays recovery after daemon restarts

#### 3. Live mesh status still performs repair/reconciliation synchronously on the request path

Evidence:

- `coordination_get_live_team_status` avg `1753 ms`, median `1868 ms`, p95 `3255 ms`, max `34575 ms`
- `coordination_get_project_mesh_snapshot` avg `712.9 ms`, p95 `2668 ms`

Code path:

- live status calls `reconcile_team_presence_for_live_status` directly inside the IPC path in [`src-tauri/src/commands/coordination.rs:544-552`](../../src-tauri/src/commands/coordination.rs)
- project mesh snapshot stays faster than live status because it now uses roster/discovery without that same repair step in [`src-tauri/src/commands/coordination.rs:779-808`](../../src-tauri/src/commands/coordination.rs)

Why it matters:

- this is a user-visible hot path for mesh runtime surfaces
- the current numbers are too high for frequent refresh without visible UI lag

### Medium

#### 4. Foreground project lookup composes focus-state read, session listing, and project scan every call

Evidence:

- `get_foreground_project` avg `653.3 ms`, median `332 ms`, p95 `2294 ms`, max `2339 ms`

Code path:

- `get_foreground_project_impl` reads focus state, then calls `list_cli_sessions_impl`, then locks the DB and scans all projects in [`src-tauri/src/commands/command_center/mod.rs:172-205`](../../src-tauri/src/commands/command_center/mod.rs)

Why it matters:

- this path is tied to foreground indicator behavior and should be comfortably sub-100ms
- it currently compounds multiple backend operations that each have their own tail risk

#### 5. `list_cli_sessions` still has a slow-path full scanner fallback on the app thread

Evidence:

- `list_cli_sessions` median `4 ms` when fast, but p95 `267 ms` and max `26089 ms`
- `session_scanner.scan.completed` appears `91829` times in the logs, which confirms scanner activity is frequent and operationally important

Code path:

- daemon-first path with local fallback in [`src-tauri/src/commands/command_center/session_listing.rs:7-63`](../../src-tauri/src/commands/command_center/session_listing.rs)
- fallback scanner does process scan, tmux mapping, idle detection, IO ownership checks, and compaction runtime publish in [`src-tauri/src/session_scanner/mod.rs:661-728`](../../src-tauri/src/session_scanner/mod.rs)
- scan telemetry is emitted in [`src-tauri/src/session_scanner/mod.rs:341-352`](../../src-tauri/src/session_scanner/mod.rs)

Why it matters:

- this path is fine when the daemon is healthy, but brittle tails still show up in app-facing IPC
- any caller that composes on top of `list_cli_sessions` inherits that tail

#### 6. Watcher initialization and periodic reconcile still do whole-project DB work under the global connection mutex

Evidence:

- recent startups show `startup.watchers.initialized` around `441-468 ms`
- periodic reconcile runs every 60 seconds regardless of actual topology changes

Code path:

- watcher initialization bootstraps state and spawns reconcile threads in [`src-tauri/src/startup/watchers.rs:30-103`](../../src-tauri/src/startup/watchers.rs)
- reconcile locks the DB, loads all projects and settings, then mutates watch state in [`src-tauri/src/startup/watchers.rs:105-220`](../../src-tauri/src/startup/watchers.rs)

Why it matters:

- watcher init is now the second-largest startup component after daemon startup
- periodic whole-project reconciliation raises contention risk even if each individual run is acceptable

### Low

#### 7. The app backend still serializes all SQLite work through one mutex-wrapped connection

Evidence:

- `DbState` is a single `Mutex<Connection>` in [`src-tauri/src/commands/projects.rs:59-60`](../../src-tauri/src/commands/projects.rs)
- command handlers like `list_projects` and `get_project` lock that shared connection directly in [`src-tauri/src/commands/projects.rs:80-109`](../../src-tauri/src/commands/projects.rs)
- `db::init_db` enables WAL, which helps readers, but the app layer still funnels everything through one connection in [`src-tauri/src/db/mod.rs:17-28`](../../src-tauri/src/db/mod.rs)

Why it matters:

- current simple DB reads are fast
- the real risk is lock amplification when higher-level commands compose DB work with other slow operations

#### 8. Search keeps a 50 MB writer budget resident and rebuilds read-side search machinery per query

Evidence:

- Tantivy writer heap size is `50 * 1024 * 1024` in [`src-tauri/src/search/indexer.rs:8-9`](../../src-tauri/src/search/indexer.rs)
- persistent writer is created on open in [`src-tauri/src/search/indexer.rs:52-84`](../../src-tauri/src/search/indexer.rs)
- each search creates a reader, searcher, query parser, and snippet generator in [`src-tauri/src/search/query.rs:27-87`](../../src-tauri/src/search/query.rs)

Why it matters:

- this likely contributes to the app process’s steady-state RSS floor
- it is not currently urgent, but it is the cleanest low-priority memory/per-query cleanup candidate

#### 9. The backend command surface is large enough that command fan-out discipline matters

Evidence:

- current `generate_handler!` registration count is `85`, based on the live block in [`src-tauri/src/lib.rs:177-280`](../../src-tauri/src/lib.rs)

Why it matters:

- this is not itself a bug
- it does mean frontend orchestration should keep avoiding multi-command cold-path fan-out on interaction-critical surfaces

## Recommendations

### 1. Remove synchronous daemon connect/validate from the setup critical path

Priority: High

Recommended change:

- make startup always register a disconnected provider immediately
- move daemon connect, binary validation, and reconnect attempts fully into background orchestration
- emit readiness transitions through events instead of holding setup on a likely-absent daemon socket

Expected result:

- removes the current ~2 second startup median from the app critical path

### 2. Replace fixed bootstrap sleep with readiness polling or bounded backoff

Priority: High

Recommended change:

- replace the hardcoded `sleep(Duration::from_secs(2))` with fast reconnect polling and a bounded deadline

Expected result:

- cuts deterministic daemon bootstrap latency
- improves daemon restart recovery without changing steady-state behavior

### 3. Split live mesh status into fast snapshot reads plus async repair

Priority: High

Recommended change:

- make `coordination_get_live_team_status` mirror the faster snapshot pattern already used by `coordination_get_project_mesh_snapshot`
- return the persisted status immediately
- schedule repair/self-heal separately

Expected result:

- should collapse the current `coordination_get_live_team_status` p95 from multi-second territory into snapshot-read territory

### 4. Make foreground resolution use a persisted project id or a cached normalized-path map

Priority: Medium

Recommended change:

- avoid calling `list_cli_sessions_impl` and `list_projects` on every `get_foreground_project`
- either persist the resolved project id into the focus file/event payload
- or keep a cached normalized `project_path -> project_id` map in managed state

Expected result:

- should move `get_foreground_project` from hundreds of milliseconds to low double-digit milliseconds

### 5. Keep app-facing session listing off the local scanner except as a last-resort diagnostic path

Priority: Medium

Recommended change:

- keep daemon-backed session snapshots as the normal source everywhere on Windows
- if local fallback remains, gate it behind explicit degraded-mode handling and clearer timeout protection

Expected result:

- removes the 26-second worst-case tail from normal UI paths

### 6. Make watcher reconcile event-driven first and cache the project/settings inputs

Priority: Medium

Recommended change:

- keep the periodic safety pass, but stop making it the main refresh mechanism
- cache the project/settings snapshot used for watch-target planning
- only refresh that cache on real project/settings mutations

Expected result:

- trims startup watcher cost
- lowers incidental DB mutex contention

### 7. Defer DB pool work until a real lock-contention signal appears

Priority: Low

Recommended change:

- do not replace the single SQLite connection yet
- first instrument DB lock wait duration in the app process and look for real contention spikes

Expected result:

- avoids premature architectural churn while still giving a clean trigger for future pooling work

### 8. If memory pressure becomes a release issue, audit Tantivy writer residency next

Priority: Low

Recommended change:

- test a smaller writer heap
- or lazy-open the writer on mutation paths only

Expected result:

- modest RSS reduction without touching the highest-impact latency issues first

## Bottom Line

The Windows-native Rust app backend is already efficient in steady state. The audit does not support a “backend is heavy” narrative for `taurhaus.exe`.

The highest-value work is now:

1. remove startup daemon blocking from the app critical path
2. stop doing live coordination repair on request paths
3. simplify foreground/session hot paths so they stop composing expensive calls synchronously

Those changes are more likely to produce visible UX wins than any additional steady-state CPU or memory tuning inside the app process.
