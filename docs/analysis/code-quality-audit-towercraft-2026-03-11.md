# Towercraft Code Quality Audit - 2026-03-11

## Scope

Task `#926` requested a full Towercraft code-quality audit of Taurhaus, focused on maintainability, architecture coherence, module boundaries, correctness risks, async/state patterns, test quality, dead code, duplication, error handling consistency, and operational reliability debt.

Primary scope reviewed:

- Frontend: `src/`, especially `src/Shell.svelte`, `src/lib/components/`, `src/lib/ipc/`
- Backend: `src-tauri/src/`, especially `coordination/`, `commands/`, `startup/`, `provider/`
- Architecture docs: `ARCHITECTURE.md`, `docs/coordination-architecture.md`, `docs/architecture/`

## Methods

### Automated Checks

- `just fmt` -> PASS
- `just lint` -> PASS
- `just typecheck` -> PASS
- `bun run test` -> PASS (`76` test files, `1103` tests)
- `just test-rust` -> PASS
- `cargo tree -d --manifest-path src-tauri/Cargo.toml` -> PASS, duplicate transitive crates present but no clear direct-prune candidate from the app layer
- `cargo +nightly udeps --manifest-path src-tauri/Cargo.toml` -> PASS (`All deps seem to have been used.`)
- `cargo machete --with-metadata src-tauri` -> FAIL, but triaged as false positive noise for `tauri-build`
- `bunx knip` -> FAIL, found one unused export:
  - `composeConfigFromPayload` in `src/lib/components/meshTabUtils.js:417`
- `bunx dependency-cruiser --no-config --include-only '^src' --output-type err-long src` -> PASS (`198` modules, `207` dependencies cruised, no violations)
- `actionlint .github/workflows/*.yml` -> not applicable, repo has no checked-in `.github/workflows`
- `semgrep scan` -> skipped, no checked-in quality-only ruleset was available to run reproducibly

### Directed Audits

- Architecture & Modularity
- Production Readiness
- AI Maintainability
- Testing Strategy
- Dependency Quality
- Rust Quality
- TypeScript Quality
- Observability & Operability
- Error Handling
- Replication Risk

### Manual Tracing

- App startup: `src/main.js` -> `src/App.svelte` -> `src-tauri/src/startup/mod.rs`
- Mesh runtime flow: `src/Shell.svelte` -> `src/lib/components/MeshTab.svelte` -> `src/lib/components/meshTabController.svelte.js` -> `src-tauri/src/commands/coordination.rs` -> `src-tauri/src/coordination/orchestrator.rs` / `src-tauri/src/coordination/runtime.rs`
- Cross-layer payload normalization: backend serde models -> frontend IPC/utils -> Mesh/Shell rendering and control paths

## Confirmed Findings

### Q-AI-01: Mesh controller is a single high-churn god function
**Severity**: HIGH
**Location**: `src/lib/components/meshTabController.svelte.js:46`
**Reachability**: `Shell.svelte` -> `MeshTab.svelte` -> `createMeshTabController(...)` -> gate/setup/runtime polling/add-agent/resume/disband/capture-role flows
**Category**: Q-AI
**Description**: The entire Mesh frontend state machine is concentrated in one factory function that owns preset definitions, state hydration, payload normalization, runtime polling, agent mutation flows, dialog state, timers, and user messaging. This is the highest-replication-risk surface in the repo: one edit point now carries setup, runtime, and recovery concerns simultaneously.
**Evidence**:
- `createMeshTabController` spans `1672` SLOC with cognitive complexity `508` and cyclomatic complexity `490`.
- The file is one of the highest-churn frontend files in the last `90` days (`25` touches).
- The function directly embeds payload normalization (`teamName/team_name`, `memberName/member_name`, `cliTool/cli_tool`) alongside runtime actions and UI state.
**Fix Effort**: Significant
**Fix**: Split the controller into explicit subcontrollers with one responsibility each:
- setup/catalog composition
- runtime polling and refresh scheduling
- resume/disband/add-agent actions
- payload normalization and view-model shaping

Keep `createMeshTabController` as a thin composition root only.
**Verify**:
- `bun run test`
- `bunx dependency-cruiser --no-config --include-only '^src' --output-type err-long src`
- confirm Mesh changes touch one focused controller module instead of the full `createMeshTabController` body

### Q-AI-02: Backend coordination behavior is still effectively file-wide despite nominal module boundaries
**Severity**: HIGH
**Location**: `src-tauri/src/coordination/runtime.rs:1`, `src-tauri/src/commands/coordination.rs:70`, `src-tauri/src/coordination/orchestrator.rs:86`
**Reachability**: Tauri IPC `coordination_*` commands -> command normalization/emit -> orchestrator policy -> runtime tmux/mesh/process side effects
**Category**: Q-AI
**Description**: The coordination subsystem has nominal package boundaries, but critical behavior is still concentrated in a few multi-thousand-line files. `commands/coordination.rs` mixes IPC transport, request normalization, progress emission, and orchestration entrypoints. `runtime.rs` mixes tmux pane resolution, daemon PID validation, process inspection, mesh command launching, and retry logic. That keeps the real edit boundary much larger than the module map suggests.
**Evidence**:
- `src-tauri/src/coordination/runtime.rs` is `2647` lines.
- `find_existing_mesh_daemon_pids_system` alone reaches cognitive complexity `18` and cyclomatic complexity `18`.
- `src-tauri/src/commands/coordination.rs` is `1812` lines and was touched `43` times in the last `90` days.
- Recent changes to add-agent, resume, compaction, and daemon lifecycle repeatedly hit the same files.
**Fix Effort**: Significant
**Fix**: Extract backend submodules that align with actual change axes:
- `request_normalization.rs`
- `live_status.rs`
- `pane_resolution.rs`
- `daemon_pid_validation.rs`
- `mesh_runtime.rs`
- `resume_pipeline.rs`

Keep IPC command files thin and keep runtime helpers grouped by side effect boundary.
**Verify**:
- `just test-rust`
- `cargo test --manifest-path src-tauri/Cargo.toml tests/module_boundary_assertions.rs -- --test-threads=1`
- measure that add-agent or resume changes no longer require editing both the command transport and broad runtime helper files

### Q-AI-03: Cross-layer payload normalization is duplicated across UI code instead of centralized at the boundary
**Severity**: MEDIUM
**Location**: `src/lib/components/meshTabController.svelte.js:252`, `src/lib/components/meshTabUtils.js:417`, `src/Shell.svelte:353`, `src-tauri/src/models/mod.rs:21`
**Reachability**: Rust/daemon/template payloads -> frontend IPC/utils -> Shell and Mesh runtime rendering/actions
**Category**: Q-AI
**Description**: Taurhaus still resolves many payload shapes ad hoc in view/controller code by reading both camelCase and snake_case forms directly. The backend models already define canonical serde shapes, but the frontend has normalization logic spread through Shell, Mesh controllers, Mesh utils, and components. This is a contract-drift hazard: one boundary change can remain green in one surface while silently breaking another.
**Evidence**:
- `src-tauri/src/models/mod.rs` uses `#[serde(rename_all = "camelCase")]` for canonical IPC models.
- `src/lib/components/meshTabController.svelte.js` repeatedly normalizes `teamName/team_name`, `memberName/member_name`, `cliTool/cli_tool`, `projectId/project_id`, `roleId/role_id`, and similar fields.
- `src/lib/components/meshTabUtils.js` repeats the same field fallback pattern for lead/agent payloads.
- `src/Shell.svelte` performs the same dual-shape normalization for daemon/session payloads.
- Recent churn already includes fixes triggered by casing drift (`camelCase`/`snake_case`) in coordination-related flows.
**Fix Effort**: Moderate
**Fix**: Centralize normalization per domain in adapter modules under `src/lib/ipc/` or dedicated `normalize*.js` helpers, then consume only canonical frontend models from controllers and Svelte components. Add a local rule/checklist item that forbids new snake_case fallback reads in view code.
**Verify**:
- add adapter-level regression tests for canonicalization
- `rg -n "project_id|team_name|cli_tool|role_id|focus_area" src/lib/components src/Shell.svelte`
- the remaining matches should live only in adapter/normalizer modules, not in controllers/views

### Q-REP-04: Quality enforcement is manual and the default frontend gate misses structural hygiene regressions
**Severity**: HIGH
**Location**: `.github/workflows (missing)`, `justfile:28`, `package.json:16`, `src/lib/components/meshTabUtils.js:417`
**Reachability**: every branch/merge path -> local `just check` / `check-quick` usage -> shipped code
**Category**: Q-REP
**Description**: Taurhaus currently relies on local discipline rather than checked-in CI enforcement. That would already be risky, but the local frontend gate is also incomplete: `lint` and `typecheck` both run `svelte-check`, so structural issues can pass the nominal gate. The audit reproduced that gap directly: the normal lint/typecheck path passed while `knip` still found dead code.
**Evidence**:
- The repo has no `.github/workflows`.
- `justfile` defines local quality commands (`check`, `check-quick`, `lint`, `typecheck`, `test`) but no checked-in CI handoff.
- `package.json` maps both `lint` and `typecheck` to `bun run check`, which resolves to `svelte-check`.
- `bunx knip` found unused export `composeConfigFromPayload` in `src/lib/components/meshTabUtils.js:417` even though `just lint` and `just typecheck` both passed.
**Fix Effort**: Moderate
**Fix**:
- add a checked-in CI workflow for at least `just fmt`, `just lint`, `just test-fast`
- separate frontend lint from typecheck
- add `knip` and `dependency-cruiser` as scheduled or path-scoped CI checks for Mesh/IPC-heavy areas
**Verify**:
- open a branch that leaves the unused export in place and confirm CI fails
- remove the dead export and confirm the branch goes green

### Q-PRD-05: Startup composition root has very little behavioral test coverage relative to its blast radius
**Severity**: MEDIUM
**Location**: `src-tauri/src/startup/mod.rs:356`
**Reachability**: app launch -> `startup::setup` -> path/log initialization -> DB init -> daemon phase detection -> watcher/search bootstrap
**Category**: Q-PRD
**Description**: Startup is the highest-blast-radius composition root in the application, but its tests cover only event-name inventory and Claude tasks directory resolution. The critical behavior branches that can fail production startup (`initialize_database`, `connect_daemon_provider`, `run_startup_orchestration`) are not exercised directly. That is a noticeable gap compared with the much deeper coverage in coordination runtime and compaction flows.
**Evidence**:
- Critical startup flow lives in `setup()` plus `initialize_database()`, `connect_daemon_provider()`, and `run_startup_orchestration()`.
- The in-file tests at the bottom of `startup/mod.rs` only validate emitted event names and `resolve_claude_tasks_dir()`.
- No focused tests were found for database-init failure, daemon fast-path vs deferred connection, or watcher/search bootstrap failure mapping.
**Fix Effort**: Moderate
**Fix**: Introduce injected helpers or test seams for startup orchestration so the app can unit-test:
- database init failure mapping
- daemon fast-path connect vs deferred startup
- watcher/search initialization failure behavior
- state registration under startup variants
**Verify**:
- add `cargo test startup::` cases that exercise success, deferred, and failure branches
- confirm startup error events and returned errors match the expected path for each branch

## Likely Audit Noise / Needs Validation

- `cargo machete` flagged `tauri-build` as unused, but this is false-positive noise. `src-tauri/build.rs` calls `tauri_build::build()`, and `cargo +nightly udeps` reported `All deps seem to have been used.`
- `actionlint` was not runnable as a repo quality check because the repo has no `.github/workflows`.
- `semgrep` was intentionally skipped because no checked-in quality-only ruleset was available. Running a non-curated default pack would not have met the Towercraft requirement.

## Risk Summary

Taurhaus is in a better-than-average state for behavioral coverage: core Rust and frontend tests are extensive and passed in this audit, and dependency-cycles/unused-Rust-dependency checks are largely clean. The main risks are structural, not incidental:

1. Mesh/coordination change surfaces are too concentrated.
2. Cross-layer payload contracts are normalized too late and too often.
3. Quality enforcement still depends on local discipline instead of reproducible CI.

Those risks reinforce each other. Large edit surfaces plus duplicated contract handling plus manual gates are exactly the shape that accumulates AI-assisted quality entropy.

## Recommended Remediation Priority

1. Add checked-in CI plus structural frontend checks so regressions stop escaping the normal path.
2. Split `meshTabController.svelte.js` and `coordination/runtime.rs`/`commands/coordination.rs` by real change boundary.
3. Centralize payload normalization into adapter modules and ban new snake_case fallback reads in controllers/views.
4. Add startup behavior tests for the critical success/deferred/failure paths.
