# Audit #571: taurhaus Full Code Quality Audit

Scope: `/home/user/projects/taurhaus`

Automated checks run:
- `cargo fmt -- --check` -> passed
- `cargo clippy --all-targets -- -D warnings` -> passed
- `cargo test` -> passed
- `bun run lint` (`svelte-check`) -> passed, 0 errors, 0 warnings
- `bun run typecheck` (`svelte-check`) -> passed, 0 errors, 0 warnings
- `bun run test` -> passed, 72 files / 1025 tests
- CI config linting -> not applicable; no repo-local CI config was present

### Q-PRD-01: Quality Gate Exists Only Locally
**Severity**: HIGH
**Priority**: P1
**Location**: justfile:28
**Reachability**: Developer change -> local-only `just check` gate -> merge/release without server-side enforcement
**Description**: The repository defines a strong local quality gate, but there is no repo-local CI configuration to enforce it on every branch or release. That means formatting, lint, tests, and resource-prep assumptions are optional in practice, even though the project just shipped `v0.5.4`.
**Evidence**: `justfile:28-45` defines the full gate (`fmt`, `lint`, `typecheck`, `test`), but `find /home/user/projects/taurhaus -maxdepth 2` found no `.github/workflows`, `.gitlab-ci.yml`, `.circleci`, Buildkite, or Azure pipeline files.
**Fix Effort**: Moderate
**Fix**: Add a CI workflow that runs `ensure-tauri-resources`, Rust fmt/clippy/test, frontend check/test, and at least one smoke/E2E lane on pull requests. Treat the local `just check` contract as the CI source of truth instead of a manual convention.
**Verify**: Open a PR with an intentional fmt or test failure and confirm CI blocks the merge.

### Q-AI-02: `Shell.svelte` Is a Main-Surface God Component
**Severity**: HIGH
**Priority**: P1
**Location**: src/Shell.svelte:1
**Reachability**: App startup -> project selection -> session bridge -> daemon status -> navigation -> settings
**Description**: The main UI shell combines startup bootstrap, daemon status/update handling, polling lifecycle, event listener registration, project loading, navigation state, theme persistence, and layout orchestration in one 1302-line component. Any change to the top-level user flow now shares failure surface with unrelated concerns, which is exactly the edit shape that degrades fastest in AI-assisted repos.
**Evidence**: `src/Shell.svelte` is 1302 lines. It imports cross-cutting concerns at `:2-22`, owns global session polling at `:365-389`, registers Tauri event listeners at `:391-500`, and performs project-selection orchestration at `:560-615`.
**Fix Effort**: Significant
**Fix**: Split Shell into thin composition plus focused controllers/modules for startup+daemon, project loading/navigation, and session-bridge lifecycle. Keep the Svelte component responsible for layout and bindings only.
**Verify**: A project-load change should stop requiring edits in the daemon/session bridge path, and `shell.test.js` should shrink into smaller module-specific tests.

### Q-PERF-03: Project-Selection Timeouts Do Not Cancel Underlying IPC Work
**Severity**: MEDIUM
**Priority**: P2
**Location**: src/lib/projectSelection.js:5
**Reachability**: Sidebar project switch -> six parallel IPC requests -> user switches again before completion
**Description**: The timeout wrapper converts slow section loads into degraded UI data, but it does not cancel the underlying IPC work. `Shell.svelte` guards only the final state commit, so superseded project selections still run all backend work to completion. On large repositories or rapid navigation, this creates avoidable load, stale logging noise, and UI responsiveness risk.
**Evidence**: `projectSelection.js:5-19` uses `Promise.race` with a timer but no cancellation mechanism. `projectSelection.js:61-68` starts six requests per selection. `src/Shell.svelte:568-580` checks `selectLoadGuard` only after awaiting the full batch, so stale requests still execute.
**Fix Effort**: Moderate
**Fix**: Introduce cancellation/request tokens at the IPC boundary, or replace the fan-out with a backend summary endpoint that can be superseded as one unit. At minimum, debounce rapid project changes and suppress duplicate in-flight loads for the same project.
**Verify**: Add a regression test that simulates rapid project switching and confirms superseded requests are cancelled or never started.

### Q-PRD-04: Session Activity Stats Are Dropped When Polling Stops
**Severity**: MEDIUM
**Priority**: P2
**Location**: src/lib/sessionStore.svelte.js:236
**Reachability**: Polling-mode session starts -> tab hidden or bridge becomes live -> `stopPolling()` clears trackers -> session disappears later
**Description**: Session activity persistence only happens when a tracked PID disappears during `applySessions`. `stopPolling()` clears trackers immediately, and `Shell.svelte` calls it whenever the document is hidden or the daemon bridge becomes authoritative. That drops accumulated activity history instead of flushing it, making the dedicated `record_session_activity` path nondeterministic.
**Evidence**: `sessionStore.svelte.js:189-208` persists stats only on disappearance. `sessionStore.svelte.js:236-243` clears `trackers` and `projectIdByPath` without flushing. `src/Shell.svelte:365-389` stops polling on visibility changes and cleanup, including the bridge handoff path at `:369-370`.
**Fix Effort**: Moderate
**Fix**: Flush active trackers before clearing them, or move duration accounting fully into the backend/daemon so visibility state in the WebView cannot discard history.
**Verify**: Add a regression test where a tracked session exists, `stopPolling()` is called, and the final activity record is still written exactly once.

### Q-AI-05: `command_center.rs` Is a Rust Boundary Hotspot
**Severity**: MEDIUM
**Priority**: P2
**Location**: src-tauri/src/commands/command_center.rs:1
**Reachability**: Session listing/launch/stop/navigation/activity IPCs
**Description**: The command-center layer is acting as one large mixed-responsibility module. It handles daemon RPC translation, tmux introspection, coordination-member matching, activity promotion, terminal orchestration, navigation, and database-backed activity persistence. That makes the Rust IPC boundary expensive to modify safely and encourages future edits to pile more logic into the same file.
**Evidence**: The file is 1693 lines. It mixes session listing (`:27-90`), activity promotion (`:263-353`), launch orchestration (`:355-577`), navigation (`:629-710`), and session-activity persistence (`:712-773`) behind one module.
**Fix Effort**: Significant
**Fix**: Split the file into submodules such as `session_listing`, `launching`, `navigation`, and `activity_tracking`, leaving `#[tauri::command]` entry points as thin wrappers over smaller units.
**Verify**: A launch/resume change should no longer require touching activity-promotion or persistence code, and focused unit tests should exist per submodule.

### Q-BLD-06: Operational Docs and Build Metadata Have Started to Drift
**Severity**: LOW
**Priority**: P3
**Location**: justfile:188
**Reachability**: Contributor onboarding -> local maintenance commands -> architecture/reference docs
**Description**: Several repo contracts now disagree with the implementation. That raises replication risk because future edits will copy the wrong operational story.
**Evidence**: `justfile:188-194` still says database reset/migration is "pending" even though `src-tauri/src/db/migrations.rs` and the migrations directory exist. `ARCHITECTURE.md:111` documents "IPC Commands (80)", while `src-tauri/src/lib.rs:170-270` registers 82 commands. `package.json:7-20` declares Bun-only package management, but both `bun.lock` and `package-lock.json` are committed at repo root.
**Fix Effort**: Trivial
**Fix**: Either implement/remove the placeholder `db-*` recipes, generate IPC counts from source (or avoid hard-coding counts), and choose one lockfile policy that matches `packageManager`.
**Verify**: The docs/recipes match the live command surface and package-manager contract after the next release cut.
