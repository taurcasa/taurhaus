# Mesh Windows install verification

Date: 2026-03-13
Author: dev-2

## Objective

Install the already-built Taurhaus Windows artifact for the Mesh `0.2.12` rollout, then verify:

- installed Windows executable path
- installed executable timestamp
- bundled Mesh identity in the installed Windows app

## Install step

Ran:

- `MESH_PROJECT=/tmp/mesh-1255 just install-windows`

The expected silent-install target from `scripts/install-windows-silent.ps1` is:

- `%LOCALAPPDATA%\taurhaus\taurhaus.exe`
- resolved WSL path: `/mnt/c/Users/mstie/AppData/Local/taurhaus/taurhaus.exe`

The installed executable exists at:

- `/mnt/c/Users/mstie/AppData/Local/taurhaus/taurhaus.exe`

## Installed executable verification

Installed executable path:

- `/mnt/c/Users/mstie/AppData/Local/taurhaus/taurhaus.exe`

Installed executable timestamp and size:

- `2026-03-13 09:04:50 +0100`
- `29347328` bytes

Installed bundled Mesh files:

- `/mnt/c/Users/mstie/AppData/Local/taurhaus/resources/mesh`
- `/mnt/c/Users/mstie/AppData/Local/taurhaus/resources/mesh.version`

Installed bundled Mesh identity:

- `mesh.version`: `0.2.11`
- `mesh version --json`:
  - version: `0.2.11`
  - git commit: `90824ef002e5feb3258b72ed2cc0cb7d72e8edcf`
  - protocol version: `1`
  - schema version: `1`
  - build time: `2026-03-13T08:01:49Z`

## Comparison against intended rollout

Intended rollout Mesh pin in Taurhaus:

- version: `0.2.12`
- git commit: `fabb518681d6f4336e715ae2a22ed2f3166b4db9`

Current Taurhaus source tree after repin:

- `src-tauri/resources/mesh.version`: `0.2.12`
- `src-tauri/resources/mesh.lock.json`: commit `fabb518681d6f4336e715ae2a22ed2f3166b4db9`

Windows synced build tree also has the new Mesh payload:

- `/mnt/d/taurhaus_build/src-tauri/resources/mesh.version`: `0.2.12`
- `/mnt/d/taurhaus_build/src-tauri/resources/mesh version --json` reports `0.2.12` / `fabb518681d6f4336e715ae2a22ed2f3166b4db9`

However, the built Windows artifacts did not refresh:

- `/mnt/d/taurhaus_build/src-tauri/target/release/taurhaus.exe` timestamp remained `2026-03-13 09:05:10 +0100`
- `/mnt/d/taurhaus_build/src-tauri/target/release/bundle/nsis/taurhaus_0.5.10_x64-setup.exe` timestamp remained `2026-03-13 09:05:10 +0100`

The installed Mesh payload hash also differs from the synced build-tree Mesh payload hash:

- build-tree resource hash: `477577af3e1af696eb8e61bc265d7f6bdb4de5c58a080f715954a68a7494f97c`
- installed resource hash: `d34c08997cbe92326d7331d46db712476a6c4a4520f5b266666d7b167720556e`

## Blocker

Exact blocker:

- the Windows build artifacts that `just install-windows` installs still bundle Mesh `0.2.11`, even though the Taurhaus source tree and synced Windows build-tree resources were updated to Mesh `0.2.12`

This means the Windows install verification for the Mesh `0.2.12` rollout failed: the installed app is still on the previous bundled Mesh identity.

## Notes

The install wrapper and the earlier Windows build wrapper both remained live after the relevant artifacts were already on disk, so I terminated the lingering wrapper processes manually after collecting verification evidence.
