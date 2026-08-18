# Mesh Windows rollout

Date: 2026-03-13
Author: dev-2

## Objective

Roll out the next Mesh version through Taurhaus, update the Mesh pin first, and verify the Windows build path uses the new bundled Mesh binary.

## Chosen Mesh version

- Previous Taurhaus Mesh pin: `0.2.11`
- New Mesh rollout version: `0.2.12`
- Exact Mesh rollout commit: `fabb518681d6f4336e715ae2a22ed2f3166b4db9`

I chose `0.2.12` because the rollout carries forward the machine-safe `task create` output change while preserving the existing protocol/schema compatibility line (`protocol_version = 1`, `schema_version = 1`).

## Mesh-first update

The shared `/home/user/projects/mesh` checkout was dirty in many unrelated files, so I created a clean rollout worktree at `/tmp/mesh-1255` from committed Mesh `HEAD` and applied only the intended task-create safety change there.

Changes included in the clean Mesh rollout build:

- `task create --json` returns the created `Task` record
- default human output for task creation now includes the subject as `created task #<id>: <subject>`
- Mesh package version bumped from `0.2.11` to `0.2.12`

Focused verification in `/tmp/mesh-1255`:

- `cargo fmt`
- `cargo test --test cli_integration task_create_and_list -- --test-threads=1`
- `cargo test --test cli_integration task_create_json_returns_created_task_record -- --test-threads=1`
- `cargo test --test cli_integration task_create_notifies_lead -- --test-threads=1`

All passed before Taurhaus was repinned.

## Taurhaus repin

Updated Taurhaus to the exact Mesh rollout commit and rebundled the Mesh binary from `/tmp/mesh-1255`.

Updated files:

- `src-tauri/resources/mesh.lock.json`
- `src-tauri/resources/mesh.version`
- `src-tauri/resources/mesh.manifest.json`

Final Taurhaus pin:

- version: `0.2.12`
- protocol: `1`
- schema: `1`
- git commit: `fabb518681d6f4336e715ae2a22ed2f3166b4db9`

## Windows build rollout

Ran:

- `MESH_PROJECT=/tmp/mesh-1255 just build-windows`

Observed results:

- Windows release executable created at `/mnt/d/taurhaus_build/src-tauri/target/release/taurhaus.exe`
- Windows NSIS installer created at `/mnt/d/taurhaus_build/src-tauri/target/release/bundle/nsis/taurhaus_0.5.10_x64-setup.exe`

Artifact timestamps from the run:

- `taurhaus.exe`: `2026-03-13 09:05:10 +0100`
- `taurhaus_0.5.10_x64-setup.exe`: `2026-03-13 09:05:10 +0100`

## Notes

The WSL-side wrapper remained live after the expected Windows artifacts were already present. I confirmed the installer and exe existed on disk, then terminated the lingering wrapper processes manually. This appeared to be a post-build wrapper hang rather than a failed Windows compile or bundle step, because the expected output artifacts were already produced successfully.
