# Rust Build Cleanup

Date: 2026-03-07
Scope: Rust projects under `~/projects/`

## Current pressure

Current `target/` footprint under `~/projects/` is about `165 GiB` by file size.

Largest target directories:

- `~/projects/taurhaus/src-tauri/target`: `110G`
- `~/projects/taurchess/src-tauri/target`: `15G`
- `~/projects/taurball/src-tauri/target`: `14G`
- `~/projects/mesh/target`: `2.2G`
- `~/projects/ledger/target`: `1.4G`

Cargo registry/git caches are much smaller:

- `~/.cargo/registry/cache`: `309M`
- `~/.cargo/registry/index`: `119M`
- `~/.cargo/git/db`: `18M`

Conclusion: the real problem is `target/`, not Cargo download caches.

## Why the previous cargo-sweep approach missed most of the space

The old approach was based on `cargo sweep --time`.

Investigation results:

1. `cargo-sweep --time` does **not** age every file by file mtime.
2. It walks `.fingerprint/`, uses fingerprint access time, and keeps/removes fingerprint-linked artifacts from a limited set of paths.
3. The upstream source explicitly skips `incremental/`.
4. It also does not treat the whole target directory as one stale unit, so large recent artifacts in active profiles keep a lot of space alive.

Relevant `cargo-sweep` behavior from the installed source:

- `last_used_time()` reads fingerprint entry `metadata().accessed()`
- `remove_not_built_with_in_a_profile()` cleans `build/`, `deps/`, `.fingerprint/`, and some top-level hashed outputs
- it explicitly comments that `incremental` is not tracked by fingerprint and skips it

That explains why a large share of the 165G was not being flagged.

## Where the space actually is

Examples from the biggest projects:

### taurhaus `src-tauri/target/debug`

- `deps`: `61G`
- `incremental`: `43G`
- `build`: `2.2G`
- `.fingerprint`: `38M`
- large top-level outputs also exist (`libtaurhaus_lib.a`, binaries, `.so`, `.rlib`)

### taurball `src-tauri/target/debug`

- `deps`: `7.2G`
- `incremental`: `4.6G`
- `build`: `661M`
- `.fingerprint`: `23M`

### ledger `target/debug`

- `deps`: `813M`
- `incremental`: `199M`
- `build`: `406M`
- `.fingerprint`: `13M`

The main miss was stale content inside `deps/`, `incremental/`, `build/`, and `.fingerprint/`.

## Is directory mtime reliable?

Only at the right level.

- `target/debug` or `target/release` directory mtime is **not** reliable for staleness. One new compile updates the profile tree and makes the whole directory look fresh.
- The immediate child entries inside `build/`, `deps/`, `incremental/`, and `.fingerprint/` are much more useful. Their own mtimes track when that specific artifact bucket was last written.

So the cleanup policy should be:

- do **not** judge staleness from `target/debug` or `target/release`
- do judge staleness from each direct child inside the cleanable subdirectories

## What is safe to delete by age?

Safe to delete by age because Cargo will regenerate them:

- `target/debug/build/*`
- `target/debug/deps/*`
- `target/debug/incremental/*`
- `target/debug/.fingerprint/*`
- `target/release/build/*`
- `target/release/deps/*`
- `target/release/incremental/*`
- `target/release/.fingerprint/*`

Do **not** delete automatically in this script:

- whole `target/debug` or `target/release` roots based only on parent-directory age
- `target/*/bundle/` outputs
- top-level final binaries/libraries in the profile root (`target/debug/<app>`, `*.rlib`, `*.so`, `*.a`) unless a separate explicit policy is added
- anything outside `target/debug|release/{build,deps,incremental,.fingerprint}`

This keeps the cleanup bounded to known-regenerable compiler outputs.

## What actually reclaims bulk space?

Using the custom subdirectory-entry policy, approximate reclaimable space is:

- older than `2` days: about `90.0G`
- older than `3` days: about `61.8G`
- older than `5` days: about `47.5G`
- older than `7` days: about `11.0G`

That is why the new default should be a custom mtime-based cleanup, not `cargo-sweep`.

## Recommendation

Adopt a custom script with these defaults:

- root: `~/projects`
- threshold: `2` days
- target only direct child entries inside:
  - `build/`
  - `deps/`
  - `incremental/`
  - `.fingerprint/`
- support dry-run and timestamped logs
- schedule daily at `08:30` local time with a user-level systemd timer or cron

Reasoning:

- `2` days reclaims substantially more space before WSL disk pressure becomes disruptive
- the current workspace scan shows about `90.0G` reclaimable at `2` days, versus about `61.8G` at `3` days
- it avoids deleting the entire profile root
- it reclaims stale incremental state, which `cargo-sweep` misses entirely

## Script vs daemon

Keep this as a script plus scheduler, not a long-running daemon.

Why:

- the cleanup job is periodic, not event-driven
- a user-level `systemd` timer or cron entry is simpler and more reliable than a process that must survive for days
- a daemon would add state, crash recovery, locking, update, and observability concerns without improving the core cleanup decision

A daemon only becomes justified if cleanup later needs:

- disk-pressure-triggered policy changes
- app-visible status/history
- per-project exemptions managed through taurhaus UI
- coordination with active builds or other storage subsystems

## Implemented helper scripts

This repo now includes:

- `scripts/rust-cleanup.sh`
- `scripts/rust-cleanup-install.sh`

### Cleanup wrapper

Default run:

```bash
./scripts/rust-cleanup.sh
```

Dry-run preview:

```bash
./scripts/rust-cleanup.sh --dry-run
```

More conservative threshold:

```bash
./scripts/rust-cleanup.sh --days 5
```

The script:

- scans `target/debug` and `target/release`
- only touches stale entries inside `build/`, `deps/`, `incremental/`, and `.fingerprint/`
- never deletes outside those subdirectories
- writes a timestamped log under `~/.local/state/rust-cleanup/`

### Scheduling helper

Install daily user-level systemd timer:

```bash
./scripts/rust-cleanup-install.sh --install-systemd-user
```

Print a cron alternative:

```bash
./scripts/rust-cleanup-install.sh --print-cron
```

## WSL2 VHDX note

The WSL2 recommendation from the earlier research still stands:

- enable `sparseVhd=true`
- keep a scheduled `wsl --shutdown` + VHD compact path for deterministic reclaim when needed

That is separate from the Rust `target/` cleanup problem.
