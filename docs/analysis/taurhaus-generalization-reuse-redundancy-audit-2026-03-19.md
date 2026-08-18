# Taurhaus Generalization, Reuse, and Redundancy Audit

Date: 2026-03-19
Auditor: `code-quality-auditor`
Scope: targeted structural audit of `/home/user/projects/taurhaus`

## Goal

Identify the highest-leverage refactoring targets for:

- oversized files that should be split
- duplicate patterns that should be consolidated
- abstraction opportunities with immediate payoff
- reuse gaps where a shared helper already exists or should exist
- frontend/backend contract drift that is likely to keep replicating

## Method

- Read `CLAUDE.md` and `ARCHITECTURE.md` to anchor the audit in current repo boundaries
- Inspected the four requested large files in detail:
  - `src/Shell.svelte` (1503 LOC)
  - `src-tauri/src/coordination/runtime.rs` (2861 LOC)
  - `src-tauri/src/coordination/stall_detector.rs` (2978 LOC)
  - `src-tauri/src/startup/mod.rs` (1248 LOC)
- Traced adjacent code for duplication and contract drift in settings, tmux orchestration, and path identity handling
- No code changes were made in this task, so no additional test pass was required

## Executive summary

The main generalization problem is not missing abstractions in isolation. It is that Taurhaus currently has a few oversized orchestration files that also own duplicated policy. That combination makes future AI-assisted edits more likely to copy the wrong thing:

1. tmux pane/layout orchestration exists twice with near-matching behavior
2. terminal and CLI launch defaults are defined in three places across JS and Rust
3. project-path identity normalization exists in several local variants despite stronger shared helpers already existing
4. `Shell.svelte`, `runtime.rs`, `stall_detector.rs`, and `startup/mod.rs` all mix orchestration, policy, and utility code in ways that hide clean extraction seams

The best sequencing is:

1. extract shared contracts first
2. extract duplicated helpers second
3. split the large orchestrators after those shared seams exist

That order reduces churn and prevents the file splits from simply moving duplicate logic into more files.

## Recommended refactoring targets

### GR-01: Split `src/Shell.svelte` into composition plus focused controllers
**Priority**: HIGH
**Effort**: Large
**Location**: `src/Shell.svelte:1`, `src/Shell.svelte:731`, `src/Shell.svelte:748`, `src/Shell.svelte:1104`
**Why this is a target**: `Shell.svelte` is the frontend orchestration hub for startup, daemon status, project selection, deferred data loading, tab routing, markdown navigation, and window/session actions. Its size is not just cosmetic; it makes unrelated changes collide and teaches future edits to keep adding behavior to the shell instead of pushing logic outward.
**Evidence**:
- 1503 lines total
- project-selection prefetch and selection flow begins around `src/Shell.svelte:731` and `src/Shell.svelte:748`
- overview/deferred loads for relationships and commits sit in the same component around `src/Shell.svelte:905` and `src/Shell.svelte:934`
- markdown navigation and path-classification logic also lives here around `src/Shell.svelte:1104-1156`
**Suggested approach**:
- Keep `Shell.svelte` as the composition surface only
- Extract a project-selection controller/store for selection, prefetch, and deferred overview loading
- Extract a shell-startup/daemon-status controller for bootstrap and install/health state
- Extract markdown/navigation helpers into a dedicated route-navigation module
- Move tab/window/session actions behind a narrower action facade consumed by the shell
**Verify**:
- existing shell/component tests still pass
- add focused tests per extracted controller instead of expanding shell-level tests
- confirm behavior parity for project selection, markdown navigation, and daemon install flows

### GR-02: Split `coordination/runtime.rs` by runtime role, not by helper type
**Priority**: HIGH
**Effort**: Large
**Location**: `src-tauri/src/coordination/runtime.rs:47`, `src-tauri/src/coordination/runtime.rs:316`, `src-tauri/src/coordination/runtime.rs:805`, `src-tauri/src/coordination/runtime.rs:2105`
**Why this is a target**: `runtime.rs` currently contains the public coordination runtime contract, the production runtime, the recording/test runtime, tmux pane orchestration, process helpers, and path comparison helpers. That makes the module simultaneously the boundary, the implementation, the test double, and the toolbox.
**Evidence**:
- trait boundary starts at `src-tauri/src/coordination/runtime.rs:47`
- `SystemCoordinationRuntime` begins at `src-tauri/src/coordination/runtime.rs:316`
- `RecordingCoordinationRuntime` begins at `src-tauri/src/coordination/runtime.rs:805`
- tmux pane/layout helpers begin at `src-tauri/src/coordination/runtime.rs:2105`
**Suggested approach**:
- `runtime/mod.rs`: public trait plus exports
- `runtime/system.rs`: `SystemCoordinationRuntime`
- `runtime/recording.rs`: recording/test runtime
- `runtime/tmux.rs`: tmux pane creation and target-selection helpers
- `runtime/process.rs` or `runtime/host.rs`: process and PID helpers
- remove local path-comparison helpers once shared path identity is centralized
**Verify**:
- all coordination runtime tests still pass unchanged
- no public API changes leak outside the coordination module unless intentional
- tmux creation behavior remains identical for `new_window`, `split`, and `per_project`

### GR-03: Split `stall_detector.rs` into service, signal collection, transition engine, and history
**Priority**: HIGH
**Effort**: Large
**Location**: `src-tauri/src/coordination/stall_detector.rs:492`, `src-tauri/src/coordination/stall_detector.rs:1008`, `src-tauri/src/coordination/stall_detector.rs:1272`, `src-tauri/src/coordination/stall_detector.rs:1795`, `src-tauri/src/coordination/stall_detector.rs:1984`
**Why this is a target**: `stall_detector.rs` mixes configuration/types, service lifecycle, background polling, signal gathering, transition evaluation, and trigger-history bookkeeping. The current shape is a classic “state machine plus all helpers in one file” module, which is especially likely to accumulate branchy edits over time.
**Evidence**:
- `StallDetectorService` starts at `src-tauri/src/coordination/stall_detector.rs:492`
- polling lifecycle methods are at `src-tauri/src/coordination/stall_detector.rs:1008` and `src-tauri/src/coordination/stall_detector.rs:1086`
- signal collection starts at `src-tauri/src/coordination/stall_detector.rs:1272`
- transition evaluation starts at `src-tauri/src/coordination/stall_detector.rs:1795`
- history/update helpers start around `src-tauri/src/coordination/stall_detector.rs:1967-2016`
**Suggested approach**:
- `stall_detector/types.rs`: config, wire types, state structs
- `stall_detector/service.rs`: `StallDetectorService` lifecycle and polling orchestration
- `stall_detector/signals.rs`: member snapshot and signal collection
- `stall_detector/transitions.rs`: pure transition evaluation engine
- `stall_detector/history.rs`: trigger-history recording/finalization/logging
- bias toward making transition evaluation a pure, test-heavy module
**Verify**:
- preserve current test behavior and expand pure transition tests around edge windows
- confirm start/stop polling semantics and trigger-history behavior do not change
- measure compile/test readability improvement by moving focused tests next to extracted modules

### GR-04: Split `startup/mod.rs` into telemetry, setup phases, and orchestration runners
**Priority**: MEDIUM
**Effort**: Medium
**Location**: `src-tauri/src/startup/mod.rs:90`, `src-tauri/src/startup/mod.rs:405`, `src-tauri/src/startup/mod.rs:661`, `src-tauri/src/startup/mod.rs:731`, `src-tauri/src/startup/mod.rs:770`
**Why this is a target**: `startup/mod.rs` is carrying repeated startup telemetry emitters plus two orchestration paths with nearly identical watcher/search error-event construction. The file has a clean split seam already: event emission, setup helpers, and orchestration/test harnesses.
**Evidence**:
- event-emitter helpers occupy `src-tauri/src/startup/mod.rs:90-393`
- main startup orchestration begins around `src-tauri/src/startup/mod.rs:405`
- `run_startup_orchestration` begins at `src-tauri/src/startup/mod.rs:661`
- the test-oriented orchestration path starts around `src-tauri/src/startup/mod.rs:731`
- watcher/search failure event construction is duplicated around `src-tauri/src/startup/mod.rs:681-717` and `src-tauri/src/startup/mod.rs:781-819`
**Suggested approach**:
- `startup/telemetry.rs`: event builders and common failure emitters
- `startup/setup.rs`: path/db/daemon setup helpers
- `startup/orchestration.rs`: production startup runner
- `startup/test_orchestration.rs` or `startup/harness.rs`: test runner adapters
- replace repeated watcher/search failure blocks with one helper that takes event code and source error
**Verify**:
- startup event sequence tests remain green
- no event names, fields, or ordering regress
- watcher/search failure paths still emit the same observable diagnostics

### GR-05: Extract one tmux layout allocator shared by coordination and session scanning
**Priority**: HIGH
**Effort**: Medium
**Location**: `src-tauri/src/coordination/runtime.rs:2105`, `src-tauri/src/session_scanner/control.rs:107`
**Why this is a target**: Taurhaus currently has two near-duplicate implementations of “choose tmux target by layout, then create/split pane.” The duplication is behavioral, not just textual, which means the next layout tweak can drift between coordination launches and session-scanner launches.
**Evidence**:
- coordination path: `create_tmux_pane_with_layout`, `create_tmux_new_window_pane`, `create_tmux_split_pane`, `find_tmux_window_with_space`, `find_tmux_project_window` in `src-tauri/src/coordination/runtime.rs:2105-2240`
- session-scanner path: `launch_command_in_tmux_with_layout`, `split_command_in_tmux_target_pane`, `find_window_with_space`, `find_project_window` in `src-tauri/src/session_scanner/control.rs:107-257`
- both implement the same `new_window` / `split` / `per_project` selection pattern with separate tmux command wrappers and error models
**Suggested approach**:
- introduce a shared `tmux_layout` module that owns:
  - window-name derivation
  - target-pane selection for `split` and `per_project`
  - new-window vs split decision policy
- keep only thin adapters in coordination and session-scanner for error typing and post-launch side effects
- define one layout-policy enum instead of open-coded string branching in multiple places
**Verify**:
- add shared tests for layout selection policy
- prove both coordination runtime and session scanner still launch into the same pane/window choices for the same inputs
- confirm tmux-change notifications still fire in the session-scanner path

### GR-06: Centralize the terminal and CLI launch contract across frontend and backend
**Priority**: HIGH
**Effort**: Medium
**Location**: `src/lib/Settings.svelte:63`, `src/lib/Settings.svelte:135`, `src/lib/settings.test.js:218`, `src-tauri/src/models/mod.rs:232`, `src-tauri/src/models/mod.rs:285`, `src-tauri/src/session_scanner/control.rs:501`
**Why this is a target**: Terminal defaults and CLI launch commands are currently defined in multiple layers. Even where comments say they must match, there is no single authority enforcing that. This is already causing Linux terminal drift, and the same shape can easily spread to other tools or modes.
**Evidence**:
- frontend duplicates launch defaults in `CLI_DEFAULTS` at `src/lib/Settings.svelte:135-151`
- frontend also defaults every non-macOS terminal to `windows_terminal` at `src/lib/Settings.svelte:63` and `src/lib/Settings.svelte:154`
- backend owns `CliCommandSettings::default()` at `src-tauri/src/models/mod.rs:232-251`
- backend Linux terminal default is `"default"` at `src-tauri/src/models/mod.rs:285-297`
- session scanner hardcodes another launch-command catalog in `src-tauri/src/session_scanner/control.rs:501-517`
- frontend tests currently codify the drift by expecting `windows_terminal` in `src/lib/settings.test.js:218-235`
**Suggested approach**:
- make the backend the source of truth for:
  - supported terminal emulators per platform
  - default emulator per platform
  - default CLI commands per tool/mode
- expose that contract through IPC or a generated manifest
- have the frontend render and seed settings from that contract instead of local literals
- delete duplicated command catalogs from `Settings.svelte` and `session_scanner/control.rs`
**Verify**:
- add round-trip tests that compare frontend defaults against the backend contract
- add platform-specific settings tests for Linux, macOS, and Windows
- confirm launch-command overrides still serialize and apply correctly

### GR-07: Consolidate project-path identity normalization into one authority per layer
**Priority**: MEDIUM
**Effort**: Small
**Location**: `src/lib/pathUtils.js:82`, `src/lib/meshCache.svelte.js:9`, `src/lib/components/meshTabUtils.js:28`, `src-tauri/src/commands/command_center/mod.rs:161`, `src-tauri/src/coordination/runtime.rs:2250`, `src-tauri/src/provider/path.rs:155`
**Why this is a target**: The repo already contains stronger shared path-normalization helpers, but several call sites still use local variants. That is a reuse miss today and a contract-drift risk later because path identity decisions affect caching, tab lookup, foreground-project matching, and coordination behavior.
**Evidence**:
- shared frontend helper exists at `src/lib/pathUtils.js:82`
- `meshTabUtils.js` already delegates to it at `src/lib/components/meshTabUtils.js:28`
- `meshCache.svelte.js` still uses a local trim-only normalizer at `src/lib/meshCache.svelte.js:9-12`
- backend shared authority exists at `src-tauri/src/provider/path.rs:155`
- command-center still uses a weaker local `normalize_project_path_key` at `src-tauri/src/commands/command_center/mod.rs:161-170`
- coordination runtime has another local comparator at `src-tauri/src/coordination/runtime.rs:2250-2258`
**Suggested approach**:
- frontend: replace local cache normalizers with `normalizeProjectPath()` from `pathUtils.js`
- backend: route all project-identity comparison through `provider::path::normalize_project_path()`
- add one golden path corpus used by both JS and Rust tests where practical
**Verify**:
- cache lookup, mesh tab identity, foreground-project matching, and coordination path comparisons all pass with mixed slash/drive/UNC inputs
- delete local normalizers rather than leaving wrappers around shared helpers

## Suggested implementation order

### Wave 1: stop drift first

1. GR-06 terminal and CLI launch contract
2. GR-07 path identity normalization

These are medium-to-small changes with immediate payoff because they remove duplicated policy and reduce the chance that later file splits preserve the wrong defaults.

### Wave 2: extract shared reuse seams

3. GR-05 shared tmux layout allocator
4. GR-04 startup telemetry/orchestration split

These create clean reusable modules that the larger orchestrators can depend on.

### Wave 3: split the big orchestrators

5. GR-02 coordination runtime split
6. GR-03 stall detector split
7. GR-01 shell split

These are the largest edits and should happen after the shared contracts/helpers are in place, otherwise the split risks becoming a pure file shuffle.

## Highest-risk replication patterns

1. Copying literal defaults across JS and Rust with a “must match” comment instead of a shared contract
2. Adding one more helper into a large orchestration file because it is already the place “where that logic lives”
3. Re-implementing path or tmux selection logic locally because the current shared surface is not obvious enough

## Outcome

The refactoring work with the best payoff is not a blanket “modularize everything” pass. It is a directed sequence:

- centralize shared policy
- extract duplicated orchestration helpers
- then split the oversized orchestrators around those new seams

That sequence should reduce churn, preserve behavior, and improve AI edit safety at the same time.
