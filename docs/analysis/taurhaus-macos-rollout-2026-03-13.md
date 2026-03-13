# Taurhaus macOS Rollout with Mesh 0.2.12

## Objective

Run the macOS rollout lane for Taurhaus and record the exact Mesh version and commit bundled into the macOS artifacts.

## Result

`just build-macos` did not complete. The rollout stopped at the remote Mesh compatibility gate before daemon installation, Mesh installation, resource bundling, and final macOS app packaging.

## Exact Mesh Identity Observed

Expected bundled Mesh identity from Taurhaus lock/manifest:

- version: `0.2.12`
- protocol_version: `1`
- schema_version: `1`
- git_commit: `3ec0e241b89b257a7dea2fdf40529a33e254b3f4`

Observed remote built Mesh identity from the macOS build host:

```json
{
  "version": "0.2.12",
  "git_commit": "unknown",
  "git_dirty": false,
  "build_time_utc": "2026-03-13T16:52:15Z",
  "protocol_version": 1,
  "schema_version": 1
}
```

The compatibility gate failed on the commit field only:

```text
✗ Remote mesh compatibility mismatch:
  - git_commit: lock='3ec0e241b89b257a7dea2fdf40529a33e254b3f4' installed='unknown'
```

## Artifact State

No new macOS Taurhaus rollout artifact was produced by this run because the recipe exited before the bundling and `cargo tauri build` stages.

## Root Cause

The Mesh build itself succeeded on macOS, but the built binary did not carry the expected git commit metadata. Based on the `build-macos` recipe, this is consistent with the Mesh source being rsynced to the Mac builder with `.git` excluded before the remote `cargo build --release --bin mesh` step, leaving the binary with `git_commit = "unknown"` instead of the locked commit.

## Evidence

- `src-tauri/resources/mesh.lock.json`
- `src-tauri/resources/mesh.manifest.json`
- `just build-macos`
- `ssh m1@62.210.195.235 "zsh -ilc '~/projects/mesh/target/release/mesh version --json'"`

## Follow-up

This rollout run is a clean fail-stop. The next step is to adjust the macOS Mesh build path so the built Mesh binary retains the locked git commit metadata, then rerun `just build-macos`.
