# Performance Improvement Possibilities — Unified Audit Compilation

Date: 2026-03-09
Owner: developer2
Task: `#810`

Source reports compiled here:

1. [performance-audit-mesh-2026-03-09.md](./performance-audit-mesh-2026-03-09.md)
2. [performance-audit-daemon-2026-03-09.md](./performance-audit-daemon-2026-03-09.md)
3. [performance-audit-app-backend-2026-03-09.md](./performance-audit-app-backend-2026-03-09.md)
4. [performance-audit-frontend-2026-03-09.md](./performance-audit-frontend-2026-03-09.md)

This document is a union of all findings from those four audits. Nothing is intentionally filtered out.

## Executive Summary

Overall performance posture is mixed:

- `taurhaus.exe` is already cheap in steady state. The app process is not the main sustained resource problem.
- `taurhaus-daemon` remains the main steady-state CPU budget problem, driven by frequent session scanning and expensive idle classification.
- The frontend’s largest risk is the markdown/rendering stack, followed by project-switch fan-out and a few still-broad shell-level hot paths.
- The `mesh` binary and mesh daemons are mostly light at current scale, but they contain structural polling and full-file/full-directory scan patterns that will become the next bottlenecks as data grows.

Biggest wins available:

1. reduce daemon scan frequency and decouple cheap liveness from expensive idle classification
2. cut frontend markdown/rendering weight and full-document work
3. remove app startup daemon blocking and other synchronous request-path repair work
4. replace mesh inbox/task full-file rescans with append-only or incremental models

## Mesh

Source: [performance-audit-mesh-2026-03-09.md](./performance-audit-mesh-2026-03-09.md)

### High

#### Inbox operations are full-file parse and rewrite hot paths

Current behavior:

- inbox reads parse the full JSON array
- inbox appends read, parse, append, and rewrite the full JSON file
- `mesh read` loads the whole inbox before filtering or slicing
- `mesh send` uses the same append path

Measured impact from the source audit:

- `mesh send` grows from `3.5 MiB` RSS on an empty inbox to `65.5 MiB` on a `100,000` message inbox
- `mesh read --json` grows from `3.3 MiB` RSS at `100` messages to `56.5 MiB` at `100,000` messages

Why it matters:

- inbox size is the clearest measured scaling slope in the mesh stack
- append rewrites increase both latency and write amplification as history grows

### Medium

#### Every task-directory change wakes every agent daemon and triggers a full task scan

Current behavior:

- each agent daemon watches the full team task directory
- a task event triggers `check_tasks()`
- that path reparses every task JSON file in the directory

Measured impact from the source audit:

- `mesh tasks --all --json` grows from `3.3 MiB` / `0.00s` at `10` tasks to `8.5 MiB` / `0.11s` at `10,000` tasks
- `task assign` itself stays cheap, but one assign can still wake many daemons into redundant rescans

Why it matters:

- this is a multiplicative N-daemon cost, not just a single command cost

#### The team-daemon is light at idle, but still uses a fixed 1 Hz wake loop and 30-second full idle-monitor passes

Current behavior:

- the team-daemon wakes once per second regardless of activity
- every idle-monitor cycle rereads config and all task files before iterating members

Why it matters:

- current idle baseline is acceptable
- the design is still poll-driven and will scale poorly as team state grows

### Low

#### Many read-oriented CLI commands still rewrite `config.json` for implicit activity tracking

Current behavior:

- commands like `send`, `read`, `tasks`, and several task subcommands trigger implicit activity writes

Why it matters:

- otherwise-read-heavy operations become config-write traffic
- it adds unnecessary write amplification and future lock-contention risk

#### Team-config reads are not the current bottleneck

Current behavior:

- member/config enumeration remains cheap at the currently tested scales

Why it matters:

- this area does not need immediate optimization effort

## Daemon

Source: [performance-audit-daemon-2026-03-09.md](./performance-audit-daemon-2026-03-09.md)

### High

#### Session activity scanning is still the dominant steady-state CPU cost

Measured baseline from the source audit:

- warm daemon CPU mean `23.38%`
- warm daemon CPU median `23.45%`
- warm daemon CPU p95 `27.50%`

Observed scanner behavior:

- scan interval mean `0.521s`
- scan interval median `0.500s`
- `<= 0.6s` intervals for `11081 / 11718` cycles

Current behavior:

- `SessionActivityHub` keeps a background scanner thread alive
- it scans display sessions every cycle
- cadence remains `500ms` whenever anything changed or any session is not idle
- the slower cadence rarely engages in real workloads

Why it matters:

- this is the current primary steady-state CPU problem in the backend stack

#### Per-session idle classification dominates scan time

Measured per-cycle timing from the source audit:

- `duration_ms` mean `142.45`, median `152`, p95 `209`
- `idle_ms` mean `104.72`, median `104`
- `classify_ms` mean `105.48`, median `105`

Current behavior:

- full classification runs for each detected process
- Codex resolution is the most expensive path, including transcript candidate enumeration and attribution checks

Why it matters:

- the steady-state budget is being spent on classification work, not process enumeration

### Medium

#### Tmux mapping still creates burst cost, even if it is not the baseline driver

Current behavior:

- tmux pane metadata is cached briefly, but refreshes still produce spike cost

Why it matters:

- this is a secondary burst source on top of the main classification cost

#### Thread count is high but stable

Measured baseline from the source audit:

- daemon threads median `77`

Why it matters:

- not the current root cause of CPU burn
- still a footprint and complexity concern

#### Inotify/watch footprint is large

Measured baseline from the source audit:

- inotify watches median about `74872`
- latest sampled daemon row exceeded `79k`

Why it matters:

- this looks more like memory/FD/capacity pressure than the main CPU driver
- still deserves dedicated reduction work

### Low

#### Compaction runtime is no longer the main daemon CPU problem

Current state:

- older duplicate compaction polling was already removed
- remaining daemon CPU cost is not explained by the current event-driven compaction runtime

Why it matters:

- future work should not keep optimizing already-removed compaction scaffolding

#### TCP accept loop and long-poll delivery are not current hotspots

Current state:

- these exist, but they do not match the measured hot metrics

Why it matters:

- they are lower priority than scanner cadence and classification cost

## App Backend

Source: [performance-audit-app-backend-2026-03-09.md](./performance-audit-app-backend-2026-03-09.md)

### Critical

#### No critical app-process resource defect is currently evident

Measured steady-state from the source audit for the latest app process:

- CPU avg `0.02%`, median `0.02%`, p95 `0.08%`, max `0.49%`
- RSS avg `44.65 MB`, median `44.37 MB`, p95 `47.68 MB`, max `49.39 MB`

Why it matters:

- the app process is not the place to look for sustained CPU or memory waste first

### High

#### Startup still spends about 2 seconds in the daemon phase on the setup path

Measured from the source audit:

- `startup.daemon_phase.completed` median `2114 ms`
- `startup.daemon_connect.deferred` median `2037 ms`

Why it matters:

- this is front-loaded into app startup and directly fights the repo’s snappy startup goal

#### Background daemon bootstrap still hardcodes a 2-second reconnect delay

Measured from the source audit:

- `startup.daemon_bootstrap.completed` median `2298 ms`

Why it matters:

- this is deterministic latency, not useful work

#### Live mesh status still performs synchronous repair/reconciliation on the request path

Measured from the source audit:

- `coordination_get_live_team_status` avg `1753 ms`, median `1868 ms`, p95 `3255 ms`, max `34575 ms`
- `coordination_get_project_mesh_snapshot` avg `712.9 ms`, p95 `2668 ms`

Why it matters:

- this is a user-visible hot path and currently too slow for frequent refresh

### Medium

#### Foreground project lookup composes focus-state read, session listing, and project scan every call

Measured from the source audit:

- `get_foreground_project` avg `653.3 ms`, median `332 ms`, p95 `2294 ms`

Why it matters:

- this is too slow for a foreground-indicator path

#### `list_cli_sessions` still has a slow-path full scanner fallback on the app thread

Measured from the source audit:

- median `4 ms` when fast
- p95 `267 ms`
- max `26089 ms`

Why it matters:

- any caller composed on top of this path inherits that tail risk

#### Watcher initialization and periodic reconcile still do whole-project DB work under the global connection mutex

Measured from the source audit:

- recent startup watcher initialization around `441-468 ms`

Why it matters:

- watcher init is now a visible startup contributor
- periodic reconcile adds contention risk

### Low

#### The app backend still serializes all SQLite work through one mutex-wrapped connection

Current state:

- simple DB reads are fast today
- the design still risks lock amplification when combined with slower higher-level commands

#### Search keeps a 50 MB writer budget resident and rebuilds read-side search machinery per query

Current state:

- not urgent now
- clean low-priority memory and per-query cleanup candidate

#### The backend command surface is large enough that command fan-out discipline matters

Current state:

- current registered handler count is `85`

Why it matters:

- large command surfaces amplify orchestration mistakes on the frontend

## Frontend

Source: [performance-audit-frontend-2026-03-09.md](./performance-audit-frontend-2026-03-09.md)

### Critical

#### Markdown and code rendering carry the largest startup and interaction cost

Build evidence from the source audit:

- `dist/assets/index-BUrrhqcF.js` around `633.57 kB`
- large Shiki language chunks between roughly `622-780 kB`
- `vendor-markdown` plus Mermaid-related large chunks

Current behavior:

- full Shiki bundle mindset is still present
- fenced languages preload sequentially
- markdown rendering performs multiple post-render DOM passes
- `CodeViewer` reruns full-file highlighting on code/language/theme changes

Why it matters:

- this is the frontend’s largest startup and interaction risk

#### Mermaid pulls in a very heavy dependency chain for an often-incidental feature

Build evidence from the source audit:

- `mermaid.core`, `cytoscape`, and `treemap` chunks all land in the roughly `442-476 kB` range

Why it matters:

- opening a README with Mermaid can trigger a large code-load and client-side render spike

### High

#### Initial project bootstrap intentionally schedules a second full project load after 1.5 seconds

Current behavior:

- first project selection happens immediately
- then the shell schedules a second `selectProject(...)` for the same project after `1500ms`

Why it matters:

- this adds deterministic post-launch churn and extra startup work

#### Every project switch still fans out six IPC calls, then mutates broad shell state in one hot path

Current behavior:

- project selection loads details, commits, latest session, session history, README, and relationships together
- shell-level state is then updated broadly in one pass

Why it matters:

- project switching still produces a coordinated IPC burst and wide rerender wave

#### Git history uses incremental fetch but not DOM virtualization

Current behavior:

- loading is paged correctly
- all loaded commit rows stay mounted

Why it matters:

- deep history browsing increases DOM size and render cost steadily

### Medium

#### File-tree virtualization exists, but visible-row flattening still recomputes the full expanded tree

Why it matters:

- virtualization only reduces mounted DOM
- large repos still pay whole-tree traversal cost on expansion and refresh

#### Opening files still does full-content reads and full highlight/render passes

Why it matters:

- large markdown and source files still pay full read plus full render/highlight cost

#### The frontend logger forwards production console traffic over IPC

Current behavior:

- production warnings/errors and other forwarded logs serialize and cross the IPC boundary

Why it matters:

- this adds interaction-path overhead that is avoidable in quieter production builds

#### Mesh canvas keeps anchor placement accurate with repeated DOM reads plus global listeners

Why it matters:

- current mesh sizes are probably fine
- this is still a layout-read-heavy design that will age poorly if the canvas gets more dynamic

### Low

#### Sidebar virtualization is solid, but per-row session derivation still runs during render

Why it matters:

- this is not a primary bottleneck today
- it is still repeated render work that can multiply under session churn

## Cross-Cutting Themes

### 1. Polling is still more common than it should be

Examples across the audits:

- daemon session scanner is still effectively a `500ms` loop most of the time
- mesh team-daemon still wakes once per second and does 30-second full idle passes
- app watcher reconcile still runs every 60 seconds regardless of actual topology changes

Pattern:

- event-driven infrastructure exists in several places now
- but the system still falls back to frequent polling for correctness
- that is the main recurring source of wasted steady-state work

### 2. Full-file and full-directory work remains a recurring scaling pattern

Examples across the audits:

- mesh inbox append/read rewrites whole JSON arrays
- mesh task reactions rescan whole task directories
- frontend file opens still read full file contents and render/highlight them fully
- frontend git history keeps all loaded rows mounted

Pattern:

- several hot paths still assume whole-document or whole-directory work is acceptable
- this is manageable at current scale, but it is the clearest common scaling risk

### 3. Cheap steady-state paths are often contaminated by correctness-heavy slow paths

Examples across the audits:

- daemon steady-state scanning pays full idle classification repeatedly
- app live team status pays synchronous repair on the request path
- app foreground resolution composes session-list and project-scan work for a small answer
- frontend project selection still fans out multiple section loads before mutating state

Pattern:

- correctness and completeness are often mixed directly into interaction-critical paths
- the recurring missing split is “fast snapshot now, expensive repair/enrichment later”

### 4. Large bundle and dependency surfaces still matter

Examples across the audits:

- frontend markdown/Shiki/Mermaid path ships very large chunks
- app search keeps a large Tantivy writer resident
- daemon thread/watch/FD counts are stable but high

Pattern:

- not all footprint is urgent
- but the codebase still has several large always-on or frequently-loaded subsystems whose size now matters in practice

### 5. The system often does the right thing functionally, but with too much repeated orchestration

Examples across the audits:

- project selection does many correct subrequests, but still broad fan-out
- foreground lookup resolves correctly, but by composing several expensive steps
- mesh task assignment is cheap, but still fans out redundant daemon scans

Pattern:

- many paths are correct but orchestration-heavy
- the next wave of wins should come from narrower fast paths, not feature redesign

## Prioritized Improvement Roadmap

This is a deduplicated, impact-first roadmap compiled from all four audits.

### 1. Reduce daemon display-session scan pressure in steady state

Why first:

- it is the clearest current sustained CPU problem in the entire stack

Work items:

- raise steady-state scan cadence above `500ms`
- split cheap liveness from expensive idle classification
- classify less often when runtime state is stable
- special-case Codex transcript resolution so it is not repeatedly rediscovered in the hot loop

Sources:

- daemon audit
- app-backend audit, because app session paths inherit daemon behavior

### 2. Cut frontend markdown/rendering weight and full-document work

Why second:

- it is the clearest frontend startup and interaction hotspot

Work items:

- shrink default Shiki language/theme footprint
- parallelize fenced-language loading
- add explicit size thresholds for degraded rendering/highlighting
- make Mermaid opt-in, visibility-gated, or otherwise more selective

Sources:

- frontend audit

### 3. Remove startup daemon blocking from the app critical path

Why third:

- startup cost is visibly dominated by daemon-phase work, not DB/search

Work items:

- stop synchronously connecting/validating the daemon during setup
- replace the fixed `2s` bootstrap sleep with readiness polling or bounded backoff
- move daemon readiness and recovery fully into background orchestration

Sources:

- app-backend audit

### 4. Move interaction-critical app/backend paths to fast snapshot plus async repair

Why fourth:

- this pattern shows up repeatedly in live status, foreground resolution, and session lookup

Work items:

- make live team status return a fast persisted snapshot first
- stop doing repair/reconciliation inline on request paths
- cache or persist foreground project resolution inputs
- keep `list_cli_sessions` on daemon-backed snapshots and treat local fallback as degraded mode only

Sources:

- app-backend audit
- daemon audit

### 5. Replace mesh inbox storage with append-only or rotated history

Why fifth:

- it is the clearest measured scaling slope in the mesh stack

Work items:

- move inbox storage away from full JSON-array rewrites
- or introduce aggressive archival/rotation thresholds if format compatibility must remain

Sources:

- mesh audit

### 6. Replace mesh task full-directory rescans with journal-driven or incremental detection

Why sixth:

- task mutations currently fan out redundant work across many agent daemons

Work items:

- drive assignment/task updates from a mutation journal
- or add per-owner filtering / mtime-aware incremental scans

Sources:

- mesh audit

### 7. Narrow frontend project-switch and shell-state hot paths

Why seventh:

- project selection is still broad and orchestration-heavy

Work items:

- remove the delayed second bootstrap load
- aggregate initial project selection data better
- lazy-load lower-priority sections by tab
- reduce broad shell-level state mutation on each switch

Sources:

- frontend audit

### 8. Make watcher and reconcile systems event-driven first, with cached inputs

Why eighth:

- recurring theme across app backend, mesh, and daemon

Work items:

- reduce whole-project or whole-team periodic passes
- cache watch-planning inputs
- keep periodic reconcile only as a bounded safety net

Sources:

- app-backend audit
- mesh audit
- daemon audit

### 9. Trim footprint-only concerns once the main latency and CPU issues are solved

Why ninth:

- these are real, but secondary

Work items:

- investigate daemon watch-count reduction
- reduce daemon thread footprint where safe
- revisit Tantivy writer residency if app RSS becomes a release concern
- virtualize git history and revisit sidebar/render-time derivations

Sources:

- daemon audit
- app-backend audit
- frontend audit

### 10. Defer low-signal changes until data shows they matter

Examples:

- mesh team-config/member enumeration
- app SQLite pooling or connection fan-out

Why last:

- current evidence says these are not the first bottlenecks

## Bottom Line

The system does not need one giant “performance rewrite.” The audits point to a smaller set of repeated architectural problems:

1. polling where event-driven or cached-state approaches should dominate
2. expensive full-file/full-directory/full-document work on paths that need to scale
3. correctness-heavy repair work still happening inline on interactive request paths

If the next implementation wave stays focused on those three patterns, the likely payoff is high across all four surfaces: mesh, daemon, app backend, and frontend.
