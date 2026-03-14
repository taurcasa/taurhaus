# Taurhaus Full Code Quality Audit

Date: March 14, 2026
Repository: `/home/mstie/projects/taurhaus`
Auditor: `code-quality-auditor`

## Scope

This audit started from the paths called out in the assignment and recent churn:

- startup/bootstrap: `src-tauri/src/startup/`
- shell and project selection: `src/Shell.svelte`, `src/lib/projectSelection.js`
- daemon/provider/runtime status: `src-tauri/src/provider/daemon_client.rs`, `src-tauri/src/daemon/`
- coordination IPC/backend boundary: `src-tauri/src/commands/coordination.rs`
- packaging/build surface: `justfile`, `src-tauri/tauri.conf.json`, `src-tauri/build.rs`

I also broadened into adjacent call chains where the critical flows crossed module boundaries.

## Automated Check Summary

Executed on March 14, 2026:

- Passed: `just check-quick`
  - Includes `cargo fmt`, `cargo check --tests`, `bun run typecheck`, and frontend unit tests.
  - Frontend unit tests passed: 78 files, 1147 tests.
- Passed: `bun run lint`
  - `knip` and `dependency-cruiser` both passed.
- Passed: `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`
- Passed: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -W clippy::all`
- Passed: `cargo machete` in `src-tauri/`
- Passed: `cargo +nightly udeps` in `src-tauri/`
- Passed: `actionlint .github/workflows/*.yml`
- Completed for review: `cargo tree -d`
  - Duplicate dependency inventory was produced; no changed-path blocker was escalated from it in this audit.
- Completed for review: `rust-code-analysis-cli --metrics`
  - Notable hotspots:
    - `src-tauri/src/startup/mod.rs`: 1247 SLOC, 125 file-level cyclomatic complexity
    - `src-tauri/src/commands/coordination.rs`: 1131 SLOC, 202 file-level cyclomatic complexity
    - `src-tauri/src/provider/daemon_client.rs`: 1243 SLOC, 190 file-level cyclomatic complexity
- Failed: `cargo test --manifest-path src-tauri/Cargo.toml`
  - 1491 passed, 1 failed, 4 ignored
  - Failing test: `daemon::server::tests::server_emits_state_changed_inotify_telemetry_after_watch_registration`
- Skipped: `semgrep`
  - Reason: the repo still does not contain a checked-in quality-only ruleset to run reproducibly.

## Findings

### Q-PRD: Bundled Mesh manifest is written and read, but not packaged
**Severity**: HIGH
**Location**: `src-tauri/tauri.conf.json:31`, `src-tauri/build.rs:1`, `src-tauri/src/commands/mesh.rs:13`, `src/lib/components/MeshAvailabilityGate.svelte:135`
**Reachability**: packaged app startup or Mesh setup UI -> `check_mesh_install_status` / `ensure_bundled_mesh_installed` -> `read_bundled_mesh_contract` -> `read_mesh_manifest_resource` -> bundled manifest missing
**Category**: Q-PRD
**Description**: Taurhaus now treats `mesh.manifest.json` as the bundled compatibility contract for Mesh install/update decisions, but the Tauri bundle config does not include that file. The release recipes and docs write the manifest into `src-tauri/resources/`, while runtime code fails if it cannot find that asset under `resource_dir()/resources`. In packaged builds, this can break Mesh availability checks and bundled Mesh installation even though the local source tree looks correct.
**Evidence**: `justfile` writes `src-tauri/resources/mesh.manifest.json`, `commands/mesh.rs` reads `mesh.manifest.json` and errors if absent, but `tauri.conf.json` only bundles `taurhaus-daemon`, `mesh`, and `mesh.version`. `build.rs` also omits `mesh.manifest.json` from `rerun-if-changed`.
**Fix Effort**: Moderate
**Fix**: Add `resources/mesh.manifest.json` to `bundle.resources`, add a matching `cargo:rerun-if-changed` entry in `build.rs`, and add one package-level regression check that inspects the built app resource directory or installer contents for the manifest.
**Verify**: Build a packaged artifact, inspect its resource directory for `mesh.manifest.json`, then run `check_mesh_install_status` and the Mesh availability gate against that packaged build without seeing a missing-manifest error.

### Q-PRD: Live team status silently degrades to stale attachment data and duplicates the runtime-snapshot path
**Severity**: HIGH
**Location**: `src-tauri/src/commands/coordination/live_status.rs:30`, `src-tauri/src/commands/command_center/session_listing.rs:35`, `src/lib/components/meshTabGateWorkflow.js:10`
**Reachability**: Mesh tab runtime refresh -> `coordination_get_live_team_status` -> `daemon_runtime_session_snapshot` -> daemon busy/error path returns `Ok(None)` -> fallback to `reconcile_team_presence_for_live_status` + attachment roster
**Category**: Q-PRD
**Description**: The coordination live-status path reimplements the daemon runtime-session snapshot RPC instead of reusing the command-center snapshot helper. The duplicate path has weaker behavior: on daemon busy, daemon error, or daemon-side response error it silently falls back to attachment-based roster data and does not surface a degraded/stale state to the caller. The frontend then uses that response to rebuild runtime team config as if it were authoritative.
**Evidence**: `coordination/live_status.rs` returns `Ok(None)` on busy/error paths and immediately falls back to attachment reconciliation. `command_center/session_listing.rs` calls the same RPC but adds reconnect/cache behavior and centralizes payload decoding. Tests in `commands/coordination/tests.rs` cover the no-provider helper path, but this audit did not find provider-backed tests for busy/error/decode-failure behavior in the coordination route.
**Fix Effort**: Moderate
**Fix**: Extract one shared runtime-session snapshot service and make both command-center and coordination use it. Return an explicit freshness/degraded indicator to callers instead of silently collapsing into the attachment path, and add provider-backed tests for busy transport, daemon error, and malformed payload branches.
**Verify**: Add tests that simulate a busy shared daemon connection and malformed snapshot payloads, confirm both command-center and coordination behave identically, and confirm the frontend can distinguish fresh runtime data from degraded fallback data.

### Q-AI: The coordination IPC boundary is still a god module with high change-amplification risk
**Severity**: HIGH
**Location**: `src-tauri/src/commands/coordination.rs:72`
**Reachability**: any team lifecycle command -> IPC entrypoint -> request normalization -> orchestrator call -> snapshot sync -> progress emission -> contract mapping -> response shaping
**Category**: Q-AI
**Description**: `coordination.rs` still concentrates too many responsibilities in one file: Tauri IPC entrypoints, blocking-task wrappers, post-write snapshot synchronization, progress event emission, preflight validation, legacy compatibility helpers, and request/response contract mapping. Recent extractions (`live_status.rs`, `request_normalization.rs`) helped, but the main boundary still requires non-local edits for routine feature work and remains an attractive copy target for AI-assisted changes.
**Evidence**: `rust-code-analysis-cli` reported 1131 SLOC, 202 file-level cyclomatic complexity, and 64 functions in `coordination.rs`. The file spans distinct change zones: entrypoints at the top, internal orchestration around lines 322-706, synchronization helpers around lines 527-550, and contract/preflight mapping from roughly lines 768-1120. A representative feature change such as adding a request field or a new progress concept still requires edits across several of those zones.
**Fix Effort**: Significant
**Fix**: Split the file by responsibility: keep thin IPC adapters in one module, move lifecycle implementations into per-operation modules, and isolate contract mapping/preflight shaping into a dedicated translation layer with round-trip tests.
**Verify**: Measure a representative lifecycle change after refactor; it should touch one command module plus one mapper/test module instead of several unrelated sections of a single file. File size and function count should drop materially below the current boundary.

### Q-REP: The default Rust test gate is unstable because the inotify telemetry test is timing-coupled
**Severity**: MEDIUM
**Location**: `src-tauri/src/daemon/server.rs:619`
**Reachability**: `cargo test` -> `daemon::server::tests::server_emits_state_changed_inotify_telemetry_after_watch_registration` -> async telemetry thread + log polling timeout
**Category**: Q-REP
**Description**: The full Rust suite currently fails on a test that waits for asynchronously emitted telemetry to appear in a global JSONL log file. That makes the default verification gate unreliable and reduces trust in release checks, especially for changes near daemon/watch registration paths.
**Evidence**: Audit run on March 14, 2026: `cargo test --manifest-path src-tauri/Cargo.toml` finished with 1491 passed, 1 failed, 4 ignored. The failing test timed out waiting for `"event":"inotify.telemetry"` with `"reason":"state_changed"`. The test polls the log file for up to 5 seconds, while the server emits telemetry from a separate thread on a one-second cadence and depends on the global log sink for observability.
**Fix Effort**: Moderate
**Fix**: Replace the log-scraping assertion with a deterministic hook or synchronization primitive around telemetry emission. If the production code must stay asynchronous, inject a test observer so the test waits on an explicit signal instead of filesystem polling.
**Verify**: Re-run the targeted test in a loop and then re-run the full `cargo test` suite. The gate should stay green without timing-related retries.

## Additional Notes

- The recent project-selection refactor is directionally good. The critical/deferred split, stale-generation discard, and daemon-lane fail-fast behavior all reduce foreground contention in the restore/project-switch path.
- `Shell.svelte` remains a large UI boundary at 1502 lines, but the recent extraction into `lib/shell/` and `lib/projectSelection.js` does improve the immediate restore regression surface. I did not escalate that file into a top finding because the more urgent issues above were easier to prove with runtime reachability and failing verification.
- No reusable Semgrep run was possible from repo-local configuration. That gap remains worth closing if the team wants repeatable code-quality scans in release audits.
