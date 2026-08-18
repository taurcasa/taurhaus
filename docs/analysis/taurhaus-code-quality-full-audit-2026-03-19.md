# Taurhaus Full Code Quality Audit

Date: 2026-03-19
Auditor: `code-quality-auditor`
Scope: full-repo, non-delta audit of `/home/user/projects/taurhaus`

## Automated checks

- `just lint` — passed
- `just typecheck` — passed
- `just test-fast` — passed
- `just test` — passed
- `actionlint .github/workflows/*.yml` — passed
- `cargo tree -d` — completed; duplicate transitive crates present, but no single blocker stood out above the findings below
- `semgrep --version` — available (`1.151.0`), but no verified quality-only ruleset was present in-repo, so I did not run an ad hoc scan

## Findings

### Q-PRD-01: Scan-directory and ignore-pattern settings are saved but not enforced
**Severity**: HIGH
**Location**: `src/lib/Settings.svelte:58`, `src-tauri/src/commands/projects.rs:490`, `src-tauri/src/services/scanner.rs:5`, `src-tauri/src/search/indexer.rs:262`
**Reachability**: Settings UI -> `updateSettings()` persists `scan_directories` / `ignore_patterns` -> project discovery uses `scan_directory()` -> backend scanner/indexer continue using hardcoded rules
**Category**: Q-PRD
**Description**: Taurhaus exposes scan directories and ignore patterns as live settings, but the runtime paths that discover projects and rebuild the search index do not consume them. Users can save exclusions and believe they are active, while project discovery and search indexing continue to use hardcoded defaults. This is functional dishonesty in a user-facing control surface, and it is especially risky for ignore patterns because users may rely on them to keep noisy or sensitive paths out of indexing.
**Evidence**: `Settings.svelte` saves `scan_directories` and `ignore_patterns` via `saveScanDirs()` / `saveIgnore()`. `commands/projects.rs` calls `crate::services::scanner::scan_directory(..., 2)` without loading settings. `services/scanner.rs` hardcodes `SKIP_DIRS`. `search/indexer.rs` rebuilds with `.gitignore` rules only and never consults saved ignore patterns.
**Fix Effort**: Moderate
**Fix**: Make scanner and index rebuild paths load the saved settings and pass an explicit scan/index policy object through the backend. If the feature is intentionally deferred, remove the editable controls or label them as not yet active until the backend wiring exists.
**Verify**: Add an integration test that saves a custom ignore pattern and proves both `scan_directory` and `rebuild_index` exclude matching paths. Add a UI test that only shows these controls as active when the backend honors them.

### Q-PRD-02: Session activity persistence undercounts Tauri fallback polling by 10x
**Severity**: HIGH
**Location**: `src/lib/sessionStore.svelte.js:26`, `src/lib/sessionStore.svelte.js:144`, `src/lib/sessionStore.svelte.js:228`, `src/Shell.svelte:583`, `src-tauri/src/db/activity_queries.rs:13`
**Reachability**: App shell startup -> `setupSessionPollingLifecycle()` -> `startSessionPolling({ intervalMs: DEFAULT_TAURI_POLL_INTERVAL_MS })` -> session trackers accumulate ticks -> persisted `active_duration_ms` / `total_duration_ms` are computed with `500ms` instead of the active poll interval
**Category**: Q-PRD
**Description**: The frontend session tracker supports a 5000ms Tauri fallback poll interval, but the duration math still multiplies ticks by the mock-mode constant `POLL_INTERVAL_MS = 500`. In Tauri fallback mode, persisted session durations and `_activeMs` are therefore undercounted by a factor of ten. Those bad values are written into `session_activity` and later aggregated into project activity stats, which makes the analytics and any UI derived from them unreliable.
**Evidence**: `Shell.svelte` starts polling with `DEFAULT_TAURI_POLL_INTERVAL_MS` when running in Tauri. `sessionStore.svelte.js` stores the active interval in `activePollIntervalMs`, but both `flushTrackedActivity()` and `_activeMs` still use the fixed `POLL_INTERVAL_MS`. `db/activity_queries.rs` persists and sums those values directly. Existing frontend tests only assert `recordSessionActivity` receives `expect.any(Number)` rather than validating exact durations.
**Fix Effort**: Trivial
**Fix**: Replace duration calculations to use the active interval actually in force for the tracker, or store per-tracker timestamps instead of tick counts so the math is interval-independent. Add exact-value tests for both mock-mode and Tauri-mode polling intervals.
**Verify**: Add a unit test that starts polling with `5000ms`, simulates two active ticks, and asserts `recordSessionActivity(..., 10000, 10000)`. Add an end-to-end activity aggregation test that proves `get_project_activity` returns the expected totals.

### Q-PRD-03: Linux terminal settings drift between frontend, backend, and tests
**Severity**: MEDIUM
**Location**: `src/lib/Settings.svelte:63`, `src/lib/Settings.svelte:433`, `src-tauri/src/models/mod.rs:285`, `src/lib/settings.test.js:218`
**Reachability**: Open Settings on Linux or recover from settings fallback -> terminal emulator field uses `windows_terminal` in the frontend -> backend default remains `"default"` on Linux -> tests enforce the wrong frontend value
**Category**: Q-PRD
**Description**: The frontend terminal settings code assumes every non-macOS platform should default to `windows_terminal`, while the backend model defaults Linux to `"default"` and the docs describe Linux as custom-only. That creates a cross-layer contract mismatch in a user-facing settings surface, and the current tests codify the wrong behavior instead of catching it. This kind of platform drift is likely to keep replicating because both the UI fallback and the test suite teach the wrong default.
**Evidence**: `Settings.svelte` uses `platform === 'macos' ? 'iterm2' : 'windows_terminal'` for fallback/default values and select rendering. `models/mod.rs` returns `"default"` for Linux. `settings.test.js` asserts the emulator select value is `windows_terminal`, reinforcing the mismatch instead of validating Linux behavior.
**Fix Effort**: Moderate
**Fix**: Centralize terminal-emulator defaults and allowed values behind one shared platform contract, preferably sourced from the backend or a single frontend helper mirrored by tests. Update the settings UI so Linux renders only valid Linux choices and add platform-specific test coverage.
**Verify**: Add explicit macOS/Windows/Linux settings tests that assert the default emulator and available options per platform. Add a round-trip IPC test proving frontend normalization preserves the backend contract on Linux.

## Adversarial maintainability model

1. Most copyable bad pattern: user-facing settings that persist successfully while backend behavior ignores them.
2. Easiest boundary to violate accidentally: frontend/backend platform contracts, especially terminal and session/runtime behavior split across JS and Rust defaults.
3. Smallest refactor that would reduce entropy now: introduce shared backend-owned config contracts for scanner policy and terminal-emulator capabilities, then consume those contracts from the UI instead of duplicating defaults.
4. Most expensive component to modify safely today: [`src/Shell.svelte`](/home/user/projects/taurhaus/src/Shell.svelte), because it remains the main orchestration surface for daemon state, project loading, navigation, tab routing, and session lifecycle in a single 1503-line component.

## Residual risks not elevated to findings

- Orchestration hotspots remain large: `src/Shell.svelte` (1503 lines), `src-tauri/src/startup/mod.rs` (1248), `src-tauri/src/coordination/runtime.rs` (2861), and `src-tauri/src/coordination/stall_detector.rs` (2978). Current test coverage is strong, so I did not elevate these as standalone findings, but they are the main future change-amplification zones.
- `cargo tree -d` shows substantial duplicate transitive dependencies from the Tauri stack. I did not find a targeted, low-risk remediation that clearly beats the higher-priority findings above.
