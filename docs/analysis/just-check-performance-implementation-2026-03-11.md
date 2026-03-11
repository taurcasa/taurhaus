# just check performance implementation

Date: 2026-03-11
Task: #939

## Objective

Implement the safe `just check` speedups identified in task `#937` without reintroducing cargo lock contention or unstable parallel test execution.

## Changes made

1. Removed duplicated Rust test execution from `test-rust-integration`

- Replaced the broad `cargo test --tests -- --test-threads=1` umbrella in `justfile` with explicit integration targets:
  - `coordination_feature_gate`
  - `coordination_integration`
  - `coordination_module_visibility`
  - `coordination_onboarding_linux_e2e`
  - `module_boundary_assertions`
  - `session_pipeline`
- Kept the heavy daemon, watcher, and launcher suites as explicit serialized lib-test commands.

2. Split `lint` into Rust and frontend lanes

- Added `lint-rust` for `cargo clippy --all-targets -- -D warnings`.
- Added `lint-frontend` for the checked-in frontend structural checks.
- Kept `lint` as the compatibility entry point, now delegating to the two split recipes.

3. Reworked `check` into the safe two-lane model

- `just check` now runs:
  - `just fmt`
  - one serialized Rust lane: `lint-rust` then `test-rust`
  - one serialized frontend lane: `lint-frontend` then `typecheck` then `test-frontend`
- The recipe supervises both lanes and fails fast by terminating the sibling lane if either side exits non-zero.
- No concurrent cargo commands are spawned by design.

4. Hardened a frontend smoke test exposed by the new schedule

- Updated `src/lib/components/MeshFlow.test.js` to model the real post-initialize runtime snapshot and click the stable primary add-agent action after the runtime state settles.
- This removed a timing race that only surfaced once frontend tests no longer ran in an artificially idle machine window.

5. Fixed a real command-layer bug uncovered during the rerun

- `src-tauri/src/commands/coordination.rs` now only syncs active-team/project snapshots after initialize if the report actually succeeded through `create_team`.
- This preserves structured validation failures such as `failed_step = validate_configuration` instead of converting them into `"team config not found"` errors.

## Measurements

Baseline from task `#937`:

- Previous `just check`: about `320s` wall clock (inferred from `.check-logs/check-2026-03-11-043822.log`)
- Previous `test-rust-integration`: `128.65s`

After implementation:

- Updated `test-rust-integration`: `111.30s`
- Clean rerun of `just check`: `231.82s`

Measured effect:

- `test-rust-integration`: about `17.35s` faster
- `just check`: about `88s` faster overall
- Relative full-gate reduction: about `27%`

## Why the speedup is safe

- The Rust lane remains fully serialized at the top level.
- The heavy daemon/watcher suites still run one at a time with `--test-threads=1`.
- The only top-level overlap is one cargo-driven lane against one Bun/frontend lane.
- No cargo package-cache or build-directory lock contention occurred in the clean final rerun.

## Residual bottlenecks

- The Rust unit lane is still the dominant cost center.
- The heavy daemon server and daemon client suites remain the longest serialized tail.
- Frontend work is no longer on the critical path in successful runs.

## Follow-up work

1. If more speed is needed, optimize or split the heavy Rust suites before introducing any new concurrency.
2. Keep the two-lane model; do not add parallel cargo jobs against the same workspace.
3. If machine smoothness becomes a problem on shared hosts, consider a modest `CARGO_BUILD_JOBS` cap for the single Rust lane and re-measure.
