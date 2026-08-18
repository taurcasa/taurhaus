# Windows Build Time Assessment

Date: 2026-03-10
Task: #857
Scope: feasibility assessment only, no implementation

## Executive Summary

`just build-windows` is dominated by Rust release compilation, not by the frontend build.

From recent local build outputs, the current steady pattern is:
- WSL daemon release build: roughly `26s-61s`
- The daemon is built twice in the same recipe path: another `26s-57s`
- Windows frontend build (`bun run build` via Tauri `beforeBuildCommand`): roughly `11s-14s` in the current app build path
- Windows Rust/Tauri release build: roughly `49s-2m33s`, with the common band around `1m30s-1m50s`

The highest-value first moves are:
1. remove the duplicate WSL daemon rebuild inside `just build-windows`
2. add compiler caching (`sccache`) for native Windows Rust builds
3. measure and then trim avoidable Windows-side work (`bun install --frozen-lockfile`, rsync scope, AV interference)

Bun/frontend optimization is a low-priority lane here. The frontend build is too small a slice to materially change end-to-end Windows build time.

## What `just build-windows` does today

Current build flow from [justfile](/home/user/projects/taurhaus/justfile):

1. `install-daemon`
2. `bundle-daemon`
3. `mesh-verify-lock`
4. `bundle-mesh`
5. `sync-windows`
6. Windows `bun install --frozen-lockfile`
7. Windows `cargo tauri build --bundles nsis`

Important detail: `bundle-daemon` depends on `build-daemon`, while `install-daemon` already does its own `cargo build --release --bin taurhaus-daemon`. That means the daemon currently gets compiled twice before the Windows build even starts.

## Evidence Base

I used:
- [justfile](/home/user/projects/taurhaus/justfile)
- [Cargo.toml](/home/user/projects/taurhaus/src-tauri/Cargo.toml)
- [tauri.conf.json](/home/user/projects/taurhaus/src-tauri/tauri.conf.json)
- recent local task outputs under `/tmp/claude-1000/-home-user-projects-taurhaus/tasks/*.output`

Observed environment:
- CPU: `AMD Ryzen 9 5950X`
- cores/threads: `16 / 32`
- Windows build drive: local `NTFS` `D:` volume
- Windows Defender real-time monitoring appears enabled

Notable dependency/build characteristics from [Cargo.toml](/home/user/projects/taurhaus/src-tauri/Cargo.toml):
- `rusqlite` with `bundled`
- `git2` with `vendored-openssl`
- `tantivy`
- Tauri 2 app bundle

These all point toward non-trivial native Rust compile/link cost on Windows.

## Timing Breakdown

## Baseline from recent task outputs

Representative recent runs:

| Run | Daemon build 1 | Daemon build 2 | Frontend build | Windows Rust/Tauri build |
| --- | ---: | ---: | ---: | ---: |
| `bil1c610g` | `26.67s` | `25.96s` | `10.38s` | `49.56s` |
| `bfrhri2tx` | `28.07s` | `27.67s` | `11.49s` | `55.71s` |
| `bnt2hkbb3` | `30.42s` | `28.20s` | `11.25s` | `1m07s` |
| `bn4p54gby` | `41.52s` | `41.33s` | `11.62s` | `1m25s` |
| `bu43ovuox` | `44.78s` | `43.50s` | `12.43s` | `1m38s` |
| `bh4lt5w75` | `46.36s` | `45.88s` | `12.68s` | `1m39s` |
| `bq71r014f` | `1m01s` | `56.93s` | `14.45s` | `2m33s` |

## Stable pattern

Across the current output set:
- frontend Vite production build: median about `12.49s`
- common Windows Rust/Tauri compile band: about `1m30s-1m50s`
- duplicate daemon work alone commonly costs about `55s-95s`

That makes the priority obvious:
- the frontend is not the problem
- duplicated daemon work is real waste
- the Rust compile/link path is the primary optimization target

## Feasibility Assessment by Optimization Area

### 1. Remove duplicate daemon build inside `just build-windows`

Current state:
- `install-daemon` already builds `taurhaus-daemon`
- `bundle-daemon` depends on `build-daemon`, causing another release build

Estimated savings:
- roughly `26s-61s` per Windows build
- likely the single highest-confidence low-risk win

Effort:
- low

Risk:
- low, if the recipe still guarantees that the exact built daemon binary gets bundled

Worth doing:
- **Yes, first**

Why:
- this is not speculative optimization; it is removal of duplicate work already visible in logs

### 2. Add `sccache` for Windows-native Rust builds

Current state:
- no sign of `sccache` integration in the repo build flow

Estimated savings:
- low on clean builds
- high on repeated local developer builds with partial invalidation
- realistic repeated-build improvement could be substantial, especially for the Tauri app target

Effort:
- low to medium

Risk:
- low, mostly operational/setup complexity

Worth doing:
- **Yes, second**

Why:
- this directly attacks the dominant cost center without changing shipped behavior
- especially worthwhile for team-lead and frequent Windows packaging loops

### 3. Instrument per-step timing inside `just build-windows`

Current state:
- timings have to be reconstructed from ad hoc logs
- there is no canonical timing breakdown emitted by the recipe itself

Estimated savings:
- none directly
- very high leverage for avoiding wrong optimization work

Effort:
- low

Risk:
- low

Worth doing:
- **Yes**

Why:
- current evidence is good enough to rank the first steps, but not good enough to responsibly optimize the later ones
- timing output should include at least:
  - WSL daemon build/install
  - mesh bundle steps
  - sync
  - Windows bun install
  - frontend build
  - Windows Rust compile/link
  - NSIS packaging

### 4. Stop running `bun install --frozen-lockfile` on every Windows build unless inputs changed

Current state:
- `build-windows` always runs Windows `bun install --frozen-lockfile`

Estimated savings:
- unknown until measured
- could be small if Bun is mostly no-op and cache-hot
- could still be worthwhile if repeated filesystem validation on Windows is noticeable

Effort:
- medium

Risk:
- medium if dependency freshness becomes ambiguous

Worth doing:
- **Probably yes, but only after timing instrumentation**

Why:
- it is plausible waste, but not yet proven to be a major contributor
- correct shape would be deterministic input-based skipping, not heuristic skipping

### 5. Reduce Windows Defender / AV impact on build directories

Current state:
- Defender real-time monitoring appears enabled
- Windows Rust builds on `target/`, Bun caches, and NSIS staging are common AV hot spots

Estimated savings:
- environment-dependent
- can be meaningful on Windows native Rust builds

Effort:
- low

Risk:
- operational/security tradeoff, not code risk

Worth doing:
- **Yes, as an environment recommendation**

Suggested exclusion candidates:
- Windows build workspace mirror (`D:\taurhaus_build`)
- Cargo target directories
- Bun cache directories
- installed Rust toolchain cache if needed

This should be documented as an opt-in local build-machine optimization, not forced by the app.

### 6. Release profile tuning (`codegen-units`, `lto`, stripping policy)

Current state:
- no custom `[profile.release]` tuning is visible in [Cargo.toml](/home/user/projects/taurhaus/src-tauri/Cargo.toml)

Estimated savings:
- possible, but not guaranteed
- some changes improve compile time at runtime-size/perf cost; others do the reverse

Effort:
- medium

Risk:
- medium to high because this can trade build speed against shipped runtime quality

Worth doing:
- **Maybe later, after measurement**

Why:
- this is the wrong first move unless current profile policy is already explicit and intentionally chosen
- build-time tuning here should be benchmarked, not guessed

### 7. Frontend/Bun optimization

Current state:
- recent Windows Vite builds are usually around `11s-14s`

Estimated savings:
- small in end-to-end build time

Effort:
- low to medium depending on changes

Risk:
- low

Worth doing:
- **Not a priority**

Why:
- even cutting frontend build time in half only saves about `5-7s`
- that does not compete with the `30-90s` class wins available elsewhere

### 8. Tauri-specific packaging changes

Candidates:
- reduce bundle work
- tune packaging assets
- split packaging from compile in local developer loops

Estimated savings:
- unknown

Effort:
- medium

Risk:
- medium because the Windows installer is the real release artifact

Worth doing:
- **Possibly, but after the bigger wins**

Why:
- compile cost still appears to dominate packaging cost
- packaging optimizations should come after compile and duplicate-work fixes

### 9. Faster linker / alternative toolchain ideas

Examples:
- `mold`
- `lld` experiments
- alternate Rust codegen backends for release packaging

Worth doing:
- **No, not first-wave**

Why:
- on this Windows-native build path, these are more invasive and less certain than caching and duplicate-work removal
- they raise toolchain complexity quickly

## Ranked Recommendations

### Tier 1: do now

1. Remove duplicate daemon build from `build-windows`
2. Add `sccache` for Windows native Rust build path
3. Add canonical per-step timing output to `just build-windows`

### Tier 2: do after measurement

4. Skip or cache-gate Windows `bun install --frozen-lockfile` when inputs are unchanged
5. Add documented Defender exclusions for build machines
6. Measure packaging vs compile vs sync cost separately after Tier 1 lands

### Tier 3: only if still needed

7. Release profile compile-time tuning with explicit runtime-size/perf comparison
8. Tauri packaging refinements

### Not recommended as first-wave work

9. Frontend build optimization as the main lever
10. invasive toolchain/linker experiments

## Recommended Follow-up Tasks

1. **Remove duplicate daemon build from `just build-windows`**
- goal: ensure the daemon is compiled once, then installed and bundled from that single artifact

2. **Add per-step timing instrumentation to Windows build recipes**
- goal: emit a stable timing table for each build invocation

3. **Integrate `sccache` into the Windows build path**
- goal: reduce repeated Rust compile time for packaging loops

4. **Measure Windows `bun install` no-op cost and gate it on manifest/lockfile changes if worthwhile**
- goal: eliminate unnecessary Windows-side dependency churn without weakening determinism

5. **Document recommended local Windows build-machine exclusions and cache layout**
- goal: reduce AV/caching overhead on real packaging machines

## Bottom Line

The current Windows build path is not suffering from one mysterious Tauri problem. It is paying for three ordinary things:
- duplicated daemon compilation before packaging
- expensive native Rust release builds on Windows
- some smaller surrounding setup work

The clean first wave is therefore:
- remove duplicate daemon build
- add `sccache`
- add real timing instrumentation

That is the highest-confidence path to materially faster Windows builds without changing product behavior or weakening release correctness.
