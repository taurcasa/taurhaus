# Taurhaus macOS Rollout Metadata Fix

## Objective

Fix the macOS build path so the bundled Mesh binary carries the pinned Mesh 0.2.12 git commit metadata, then rerun the Taurhaus macOS rollout.

## Root Cause

The macOS build recipes synced `/home/mstie/projects/mesh` to the remote Mac with `.git` excluded:

- `build-macos`
- `build-macos-intel`
- `build-macos-universal`

Mesh derives its embedded `git_commit` in `build.rs` via `git rev-parse HEAD`. Without `.git` on the remote builder, the first remote Mesh build embedded `git_commit = "unknown"`.

After restoring `.git` to the remote sync, the already-built remote Mesh binary still remained stale because Cargo did not rebuild it automatically from the prior metadata-less output. The recipe therefore also needed to force a clean remote Mesh rebuild.

## Fix

Updated `justfile` so all macOS Mesh sync/build lanes now:

- sync the Mesh repository with `.git` included
- run `cargo clean` in `~/projects/mesh` before rebuilding Mesh on the remote Mac

That guarantees the remote Mesh binary regenerates version metadata from the actual pinned commit before Taurhaus bundles it.

## Rerun Result

`just build-macos` completed successfully after the fix.

Critical compatibility gate during the rerun:

```text
✓ Remote mesh compatibility matches lock (0.2.12)
```

Final packaged macOS outputs:

- remote app: `/Users/m1/projects/taurhaus/src-tauri/target/release/bundle/macos/taurhaus.app`
- remote dmg: `/Users/m1/projects/taurhaus/src-tauri/target/release/bundle/dmg/taurhaus_0.5.10_aarch64.dmg`
- local copied dmg: `/home/mstie/projects/taurhaus/builds/macos-aarch64/taurhaus_0.5.10_aarch64.dmg`

## Exact Bundled Mesh Identity

Verified from the packaged app bundle binary at:

`/Users/m1/projects/taurhaus/src-tauri/target/release/bundle/macos/taurhaus.app/Contents/Resources/resources/mesh`

Output of `mesh version --json`:

```json
{
  "version": "0.2.12",
  "git_commit": "3ec0e241b89b257a7dea2fdf40529a33e254b3f4",
  "git_dirty": false,
  "build_time_utc": "2026-03-13T16:59:33Z",
  "protocol_version": 1,
  "schema_version": 1
}
```

This matches the pinned Taurhaus lock line exactly:

- version: `0.2.12`
- protocol_version: `1`
- schema_version: `1`
- git_commit: `3ec0e241b89b257a7dea2fdf40529a33e254b3f4`

## Files Changed

- `justfile`
- `docs/analysis/taurhaus-macos-rollout-metadata-fix-2026-03-13.md`

## Validation

- `just build-macos`
- `ssh m1@62.210.195.235 "zsh -ilc '~/projects/taurhaus/src-tauri/target/release/bundle/macos/taurhaus.app/Contents/Resources/resources/mesh version --json'"`
