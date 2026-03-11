# just check performance investigation

Date: 2026-03-11
Task: #937

## Objective

Identify how to make `just check` materially faster without reintroducing resource thrash or reducing determinism.

## Data collected

Host capacity:

- `nproc` -> `32`

Observed full-gate run:

- Successful run log: `.check-logs/check-2026-03-11-043822.log`
- Approximate wall-clock for the full gate: about `5m20s`
  - inferred from the log filename timestamp (`04:38:22`)
  - Vitest started at `04:43:25`
  - Vitest duration was `17.35s`

Direct timing probes:

- `just fmt` -> `1.61s`
- `just typecheck` -> `7.18s`
- `just lint` -> `13.97s`
- `just test-frontend` -> `20.09s`

Key durations extracted from the successful `just check` log:

- Rust unit lane (`cargo test --lib --bins ...`) -> `49.21s`
- Rust integration/system umbrella (`cargo test --tests -- --test-threads=1`) -> `128.65s`
- `coordination_integration` target inside that lane -> `7.94s`
- `coordination_onboarding_linux_e2e` target inside that lane -> `16.54s`
- `session_pipeline` target inside that lane -> `1.39s`
- Heavy lib daemon server suite -> `38.63s`
- Heavy lib daemon event listener suite -> `8.23s`
- Heavy lib daemon client suite -> `27.10s`
- Heavy lib daemon launcher suite -> `8.38s`
- Frontend Vitest lane -> `17.35s`

## Current critical path

`just check` is fully serialized today:

1. `just fmt`
2. `just lint`
3. `just typecheck`
4. `just test`

`just test` is also serialized:

1. `test-rust-fast`
2. `test-rust-unit`
3. `test-rust-integration`
4. `test-frontend`

In practice, the critical path is dominated by Rust test execution, not frontend checks:

- frontend quality work is roughly `40s` total (`lint` frontend portion + `typecheck` + `test-frontend`)
- Rust testing alone is well over `4 minutes`
- `fmt` is negligible

## What is intentionally serialized today

These guardrails appear justified and should stay serialized:

- Heavy Rust suites use `--test-threads=1`
  - This is appropriate for daemon, watcher, socket, and process-lifecycle tests.
- The heavy daemon/watcher suites are split into separate commands
  - This limits blast radius and avoids the worst test-level interference.
- `just check` writes a single log and exits on first failure
  - This is useful for reproducibility and operator clarity.

## Where the real bottlenecks are

### 1. Rust test duplication is the biggest cost

The largest single cost in the current gate is not frontend work and not formatting/linting. It is repeated Rust test execution.

The strongest signal is `cargo test --tests -- --test-threads=1` taking `128.65s` while still running the lib unit test binary (`src/lib.rs`) in the log. That means the integration/system lane is not limited to just standalone integration crates in practice; it replays a large amount of test work that already ran in `test-rust-unit`.

This is the highest-impact optimization target.

### 2. Top-level Rust parallelism causes lock contention, not speed

When Rust-heavy commands were run in parallel earlier in this task, cargo reported:

- `Blocking waiting for file lock on package cache`
- `Blocking waiting for file lock on build directory`

That is the exact shape of resource thrash we want to avoid. Parallelizing multiple cargo commands against the same workspace/target directory is not a safe speedup.

### 3. Frontend work is cheap enough to overlap with Rust

The entire frontend lane is short compared with Rust:

- `typecheck`: `7.18s`
- `test-frontend`: `20.09s`
- structural frontend lint inside `just lint`: low tens of seconds total with clippy included

Frontend work is a good candidate to overlap with Rust because it does not compete for cargo locks or the Rust target directory.

## What is safe to parallelize

### Safe

- One Rust lane and one frontend lane at the same time
  - Example: run the entire frontend quality lane in parallel with the Rust quality lane.
- Internal compiler/test runner parallelism inside a single tool invocation
  - cargo and Vitest already know how to use multiple cores internally.

### Unsafe or low-value

- Multiple cargo commands at once against the same workspace
  - causes build/package-cache lock contention
  - duplicates compilation
  - creates noisy, bursty CPU and IO usage without shortening the true critical path
- Splitting the heavy daemon/watch/process suites into concurrent top-level jobs
  - these are exactly the suites most likely to regress under concurrency

## Recommended safe concurrency model

Recommended model:

- Top-level concurrency: `2` lanes
  - `rust lane`
  - `frontend lane`
- Internal concurrency:
  - let cargo/Vitest use their own worker models
  - if the team wants an explicit cap for smoother machine behavior, cap the single Rust lane to a modest build parallelism like `4-8` jobs, not `4-8` separate top-level commands

In other words:

- yes, the host has enough headroom for more than one core
- no, we should not turn that into many concurrent `just`/cargo jobs
- the safe way to use more headroom is:
  - one cargo-driven lane
  - one Bun/Vitest-driven lane
  - optional modest cargo job cap if machine smoothness matters more than absolute peak throughput

## Ranked recommendations

### 1. High impact, low risk: remove duplicate Rust test execution

Change `test-rust-integration` so it runs only the intended integration/system targets instead of the broad `cargo test --tests` umbrella that is replaying lib tests.

Why this is first:

- it attacks the biggest measured bottleneck
- it does not require introducing new concurrency
- it improves both wall-clock time and machine stability

Likely direction:

- replace `cargo test --tests -- --test-threads=1` with explicit `cargo test --test ...` invocations for the actual integration crates
- keep the existing dedicated heavy lib suites as separate serialized commands

### 2. Medium impact, low risk: split `check` into Rust and frontend lanes and run them together

After `fmt`, run:

- Rust lane:
  - `just lint-rust`
  - `just test-rust`
- Frontend lane:
  - `bun run lint`
  - `bun run typecheck`
  - `bun run test`

This should recover most or all of the frontend time from the critical path without touching the fragile daemon/watcher serialization.

### 3. Medium impact, medium risk: split `lint` into `lint-rust` and `lint-frontend`

Right now `just lint` mixes:

- `cargo clippy`
- frontend structural checks

That layout makes safe parallel scheduling harder. Splitting them is a prerequisite for a clean two-lane `check`.

### 4. Low impact, low risk: keep `fmt` first and serial

`fmt` is only `1.61s`. There is no reason to complicate it.

### 5. Low impact, medium risk: consider a modest cargo build-job cap only if needed

If the team observes that a single cargo lane is still too bursty on shared machines, use a modest cap such as `CARGO_BUILD_JOBS=8` for the Rust lane.

This is not the first optimization to take. The current dominant waste is duplicate work, not insufficient CPU usage.

## Proposed future structure

One reasonable target shape:

1. `just fmt`
2. start frontend lane and Rust lane in parallel
3. wait for both

Frontend lane:

- `bun run lint`
- `bun run typecheck`
- `bun run test`

Rust lane:

- `cargo clippy --all-targets -- -D warnings`
- `cargo check --tests`
- `cargo test --lib --bins ...`
- explicit integration targets only
- explicit heavy daemon/watcher suites only

## First implementation step

First change to make:

- rewrite `test-rust-integration` to replace the broad `cargo test --tests` invocation with explicit integration test targets and re-measure `just check`

Reason:

- highest measured payoff
- no concurrency risk
- reduces duplicated Rust execution before touching scheduler structure

## Bottom line

`just check` is slow mainly because the Rust lane is doing too much repeated work, not because the machine lacks available cores.

The safe optimization path is:

1. remove duplicated Rust test execution
2. then overlap the frontend lane with the Rust lane
3. do not run multiple cargo commands in parallel against the same workspace
