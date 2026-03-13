# Taurhaus Windows Rollout

**Date:** 2026-03-13
**Task:** #1269

## Commit Used

- Taurhaus commit: `f4d754514f5d4fafa8cbcb36e0943c02815b4273`

## Commands Run

From `/home/mstie/projects/taurhaus`:

1. `just check`
2. `MESH_PROJECT=/tmp/mesh-1255 just build-windows`
3. `MESH_PROJECT=/tmp/mesh-1255 just install-windows`

## Result

The requested quality gate and Windows rollout completed successfully.

- `just check` passed cleanly after one small clippy fix in
  `src-tauri/src/commands/mesh.rs`
- Windows build completed successfully from the current Taurhaus tree
- Windows install completed successfully

## Built Windows Artifacts

Release executable:

- path: `/mnt/d/taurhaus_build/src-tauri/target/release/taurhaus.exe`
- size: `29343232`
- timestamp: `2026-03-13 16:42:47 +0100`

NSIS installer:

- path: `/mnt/d/taurhaus_build/src-tauri/target/release/bundle/nsis/taurhaus_0.5.10_x64-setup.exe`
- size: `17908149`
- timestamp: `2026-03-13 16:42:47 +0100`

## Installed Windows Application

Installed executable:

- path: `/mnt/c/Users/mstie/AppData/Local/taurhaus/taurhaus.exe`
- size: `29343232`
- timestamp: `2026-03-13 16:42:20 +0100`

Installed bundled Mesh version file:

- path: `/mnt/c/Users/mstie/AppData/Local/taurhaus/resources/mesh.version`
- value: `0.2.12`

## Mesh Rollout Status

No additional **Windows install-side** Mesh action is required for this rollout.

The installed Windows app now carries Mesh `0.2.12`, which matches the current
Taurhaus pin.

Separate from this Windows rollout, the earlier Linux-side cross-project Mesh
audit still found already-running Mesh daemons on old executable inodes after
the shared `~/.local/bin/mesh` upgrade. That is a live daemon rotation follow-up,
not a Windows bundle/install defect.

## Additional Notes

During `just build-windows`, the helper path that refreshes the WSL
`taurhaus-daemon` reported:

- `⚠ Daemon did not restart — start it manually`

That warning did not block the Windows build or install output for this task.
