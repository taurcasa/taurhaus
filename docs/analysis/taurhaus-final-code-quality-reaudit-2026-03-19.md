# Taurhaus Final Code Quality Re-Audit After Quality Phase

Date: 2026-03-19
Auditor: `code-quality-auditor`
Scope: delta full-audit of `/home/user/projects/taurhaus` after the quality-phase fixes and refactors
Baselines:
- `docs/analysis/taurhaus-code-quality-full-audit-2026-03-19.md`
- `docs/analysis/taurhaus-generalization-reuse-redundancy-audit-2026-03-19.md`

## Verdict

Yes, the quality phase made a substantial structural improvement.

The strongest gains are real:

- the scan/index policy is now centralized and actually wired through project discovery, index rebuilds, startup indexing, and file-watcher updates
- tmux layout selection is now shared instead of being duplicated in two independent implementations
- path identity handling is materially more consistent across frontend and backend
- `startup/mod.rs` and `coordination/runtime.rs` were split in a meaningfully cleaner way

But the phase is not fully complete or fully clean:

- the repo no longer passes all verification gates: `just lint` fails and full `just test` fails
- terminal contract drift is reduced in the main path, but not fully eliminated in frontend fallback paths
- `stall_detector.rs` and `Shell.svelte` remain the main change-amplification hotspots, so the structural improvement is uneven

My honest assessment is: substantially better than before, but not yet in a “done, stable, and fully generalized” state.

## Verification

- `just typecheck` — passed
- `just test-fast` — passed
- `actionlint .github/workflows/*.yml` — passed
- `just lint` — failed
  - `src-tauri/src/startup/harness.rs:1` duplicated `cfg(test)` attribute
  - `src-tauri/src/models/mod.rs:383` manual `Default` impl flagged as derivable
  - `src-tauri/src/coordination/orchestrator.rs:178`
  - `src-tauri/src/coordination/orchestrator.rs:196`
- `just test` — failed
  - `src-tauri/src/commands/coordination/tests.rs:2455`
  - failing integration test: `commands_coordination::tests::live_status_provider_snapshot_yields_to_current_pane_loss`
  - assertion regressed from expected `Fresh` to actual `AttachmentsOnly`

## Before/After Structural Metrics

| Area | Before | After | Delta | Assessment |
| --- | ---: | ---: | ---: | --- |
| `src/Shell.svelte` | 1503 | 1242 | -261 (-17%) | Partial improvement. Some view extraction happened, but the shell still owns a large amount of orchestration logic. |
| `src-tauri/src/coordination/runtime.rs` | 2861 | split into `runtime/mod.rs` 890, `system.rs` 435, `recording.rs` 544, `process.rs` 837, `tmux.rs` 161 | largest single file down 1971 lines (-69%) | Strong improvement. This is a real module split, not just a rename. |
| `src-tauri/src/coordination/stall_detector.rs` | 2978 | `stall_detector.rs` 2407 plus `decisions.rs` 232, `signal_sources.rs` 222, `diagnostics.rs` 162 | largest single file down 571 lines (-19%) | Partial improvement only. The dominant service/orchestration file is still too large. |
| `src-tauri/src/startup/mod.rs` | 1248 | `mod.rs` 79 plus `telemetry.rs` 550, `setup.rs` 403, `harness.rs` 276, `orchestration.rs` 131 | largest single file down 698 lines (-56%) | Clean split with good seams and clearer responsibility boundaries. |

## Reuse and Redundancy Delta

### Real consolidation that landed

1. tmux layout allocation
   - Before: duplicated allocation logic in `coordination/runtime.rs` and `session_scanner/control.rs`
   - After: shared `src-tauri/src/tmux_layout.rs` is consumed by both `src-tauri/src/coordination/runtime/tmux.rs` and `src-tauri/src/session_scanner/control.rs`
   - Result: this duplication was genuinely eliminated

2. scan/index policy
   - Before: settings existed, but scanner and indexer ignored them
   - After: `src-tauri/src/services/scan_policy.rs` is consumed by:
     - `src-tauri/src/commands/projects.rs`
     - `src-tauri/src/bootstrap.rs`
     - `src-tauri/src/event_processor.rs`
     - `src-tauri/src/services/scanner.rs`
     - `src-tauri/src/search/indexer.rs`
   - Result: this is one of the cleanest wins of the phase

3. path identity normalization
   - Before: separate local normalizers existed in frontend cache/tab utilities and backend command/runtime paths
   - After:
     - `src/lib/meshCache.svelte.js` now imports `normalizeProjectPath` from `src/lib/pathUtils.js`
     - `src-tauri/src/commands/command_center/mod.rs` now uses `crate::provider::path::normalize_project_path`
     - `coordination/runtime` no longer carries the old local compare helper
   - Result: substantial reduction in duplicate path-identity logic

4. command defaults on the backend
   - Before: session-scanner hardcoded another command catalog
   - After: `src-tauri/src/session_scanner/control.rs:457-458` now resolves defaults through `CliCommandSettings::default()`
   - Result: backend duplication was reduced materially

5. frontend reuse helpers
   - `src/lib/errorCopy.js` is now imported by 7 UI consumers
   - `src/lib/a11y.js` is now imported by 8 UI consumers
   - Result: these shared helpers are being reused consistently enough to count as real consolidation

### Consolidation that is still incomplete

1. terminal contract fallback logic
   - `src/lib/ipc/system.js` still defines `DEFAULT_TERMINAL_CONTRACTS`
   - `src/lib/Settings.svelte` still defines its own fallback contract separately
   - Result: the main settings flow is better, but frontend fallback/default policy still has duplicate ownership

2. `stall_detector` split depth
   - The extracted submodules are useful, but the primary file still carries most of the behavioral complexity
   - Result: this is not yet the kind of decomposition that materially lowers edit risk

3. `Shell.svelte` split depth
   - The new shell components are mainly presentational composition points
   - The main shell still owns daemon recovery, selection flow, session lifecycle, navigation, and tab/window orchestration
   - Result: this reduced some UI weight, but it did not yet create the focused controllers recommended in the generalization audit

## Original Findings Status

### Q-PRD-01: Scan-directory and ignore-pattern settings were saved but not enforced
**Status**: RESOLVED
**Evidence**:
- `src-tauri/src/services/scan_policy.rs` now centralizes the policy
- `src-tauri/src/commands/projects.rs:273-283` loads policy before scanning
- `src-tauri/src/search/indexer.rs:455` and `src-tauri/src/search/indexer.rs:484` load policy during rebuild/indexing paths
- `src-tauri/src/event_processor.rs:752-792` applies the same policy to watcher-driven incremental indexing
- regression coverage exists in:
  - `src-tauri/src/commands/projects.rs` test `scan_directory_honors_saved_ignore_patterns`
  - `src-tauri/src/commands/search.rs` test `rebuild_index_honors_saved_ignore_patterns`
  - `src-tauri/src/search/indexer.rs` tests for ignore-pattern enforcement

### Q-PRD-02: Tauri fallback session polling undercounted persisted activity by 10x
**Status**: RESOLVED
**Evidence**:
- `src/lib/sessionStore.svelte.js:144-145` now uses `activePollIntervalMs`
- `src/lib/sessionStore.svelte.js:228` now uses `activePollIntervalMs` for `_activeMs`
- dedicated tests now cover both mock and Tauri interval math in `src/lib/sessionStore.test.js`

### Q-PRD-03: Linux terminal defaults drifted between frontend, backend, and tests
**Status**: PARTIALLY RESOLVED
**What improved**:
- backend now exposes a runtime `terminal_contract`
- settings commands attach and normalize that contract
- `Settings.svelte` consumes `terminal_contract` in the normal settings path
- tests now encode Linux as `manual`, not `windows_terminal`
**What remains**:
- frontend fallback/default policy is still duplicated and inconsistent between `src/lib/ipc/system.js` and `src/lib/Settings.svelte`
- the gap is much smaller than before, but not completely closed

## Findings

### Q-AI-01: The quality phase regressed clean verification gates
**Severity**: HIGH
**Location**: `src-tauri/src/startup/harness.rs:1`, `src-tauri/src/models/mod.rs:383`, `src-tauri/src/coordination/orchestrator.rs:178`, `src-tauri/src/coordination/orchestrator.rs:196`, `src-tauri/src/commands/coordination/tests.rs:2455`
**Reachability**: `just lint` -> clippy failure; `just test` -> coordination integration failure
**Description**: The repo is structurally cleaner than before, but the refactor phase left the verification baseline worse than the earlier audit. This matters more than a line-count win because it means the phase cannot honestly be called complete or cleanly landed.
**Evidence**:
- `just lint` fails on new clippy violations in startup, models, and coordination orchestrator code
- `just test` fails in `live_status_provider_snapshot_yields_to_current_pane_loss`
- the failing integration test indicates a live-status freshness regression in a real coordination path, not just test scaffolding noise
**Fix Effort**: Moderate
**Fix**:
- remove the duplicated `cfg(test)` attribute from `startup/harness.rs`
- resolve the clippy warnings in `models/mod.rs` and `coordination/orchestrator.rs`
- debug the live-status freshness regression so pane loss still downgrades provider-backed status to `Fresh` when the current pane disappears
**Verify**: rerun `just lint` and `just test` until both pass cleanly

### Q-AI-02: Terminal contract centralization improved the main path but introduced frontend fallback drift
**Severity**: MEDIUM
**Location**: `src/lib/Settings.svelte:52`, `src/lib/Settings.svelte:62`, `src/lib/Settings.svelte:82`, `src/lib/ipc/system.js:29`, `src/lib/ipc/system.js:60`, `src-tauri/src/models/mod.rs:277`
**Reachability**: settings load failure path or any frontend normalization path that falls back without a backend-provided `terminal_contract`
**Description**: The primary frontend/backend drift was reduced, but the frontend now owns two separate fallback contract definitions. `Settings.svelte` hardcodes a Linux/manual fallback contract, while `ipc/system.js` carries its own per-platform fallback catalog. That means the quality phase improved the happy path but did not fully centralize policy ownership, and it created a new frontend-only inconsistency in degraded paths.
**Evidence**:
- `Settings.svelte` fallback contract is Linux/manual only
- `ipc/system.js` defines separate Linux/macOS/Windows fallback contracts
- backend remains the real authority in `TerminalPlatformContract`, so frontend fallback policy is still duplicated
**Fix Effort**: Small
**Fix**: Make `Settings.svelte` consume one shared frontend fallback helper sourced from `ipc/system.js`, or better, have one exported frontend fallback contract builder that mirrors the backend contract in exactly one place.
**Verify**: add settings-load-failure tests for Linux, macOS, and Windows and assert fallback emulator/options match the contract shown in the normal path

### Q-AI-03: The two most behavior-dense hotspots were only partially decomposed
**Severity**: MEDIUM
**Location**: `src/Shell.svelte:1`, `src/Shell.svelte:341`, `src/Shell.svelte:517`, `src/Shell.svelte:1126`, `src-tauri/src/coordination/stall_detector.rs:510`, `src-tauri/src/coordination/stall_detector.rs:1026`, `src-tauri/src/coordination/stall_detector.rs:1290`
**Reachability**: every shell bootstrap/project-selection flow and every stall-detection lifecycle path
**Description**: The quality phase split startup and runtime well, but it did not yet finish the harder behavioral decompositions. `Shell.svelte` still concentrates shell orchestration, daemon recovery, selection lifecycle, session polling, and routing. `stall_detector.rs` still concentrates configuration, ingest, polling, escalation, and history. These remain the two most likely places for future AI edits to accumulate complexity.
**Evidence**:
- `Shell.svelte` only dropped from 1503 to 1242 lines and still owns most controller logic
- `src/lib/components/shell/ShellMainPanel.svelte` and `ShellTitlebar.svelte` exist, but the main shell still carries most stateful behavior
- `stall_detector.rs` remains 2407 lines after the refactor, far larger than the extracted support modules combined
**Fix Effort**: Significant
**Fix**:
- finish the controller extraction planned in the generalization audit for `Shell.svelte`
- continue the `stall_detector` split by moving transition evaluation, signal collection, and history orchestration behind narrower module seams
- bias toward pure modules and smaller public surfaces, not just moving functions to sibling files
**Verify**: reduce the largest single-file size and add module-local tests that let shell and stall-detector integration tests shrink rather than grow

## Honest Assessment of Maintainability

The codebase is substantially more maintainable than it was in the initial audit, but the improvement is asymmetric.

The best changes were real:

- scan/index policy now has a canonical home and is wired end to end
- tmux layout allocation is truly shared
- path normalization is materially safer
- startup and runtime boundaries are clearer

The weaker areas are also real:

- `Shell.svelte` is still a large behavioral shell, not just a composition file
- `stall_detector.rs` is still a very large policy-and-lifecycle module
- verification is red, which undercuts confidence in the final landing quality

If I had to summarize the phase in one sentence:

We did more than move code around, but we did not finish the cleanup evenly, and the final landing still needs one verification pass plus another focused refactor pass on the remaining hotspots.

## Remaining Hotspots

- `src-tauri/src/coordination/stall_detector.rs` — still the largest production hotspot
- `src-tauri/src/coordination/orchestrator.rs` — 1895 LOC and still operationally central
- `src-tauri/src/session_scanner/mod.rs` — 1945 LOC
- `src/lib/components/meshTabController.svelte.js` — 1565 LOC
- `src/Shell.svelte` — reduced, but still heavier than the new component split suggests

## Recommended Next Moves

1. Restore verification first
   - fix `just lint`
   - fix the failing coordination integration test
2. Finish the terminal fallback contract cleanup
   - remove duplicate frontend fallback ownership
3. Continue the two incomplete behavioral decompositions
   - `Shell.svelte`
   - `coordination/stall_detector.rs`

That would turn this from a successful but uneven quality phase into a consistently maintainable landing.
