# Mesh Team-Daemon Auto Rotation

**Date:** 2026-03-13  
**Task:** #1265

## Summary

Native Taurhaus Mesh installs now trigger the same immediate bounded Mesh
self-heal pass that the WSL install path already used.

That closes the remaining rollout gap after Mesh binary replacement:

- the installed `~/.local/bin/mesh` binary is still replaced atomically
- the replacement is still verified with `mesh version --json`
- native install now immediately runs the Mesh daemon self-heal pass
- drifted live team-daemons are stopped and re-ensured from the new installed
  Mesh binary during that pass

Result: normal native Mesh upgrades no longer depend on a later Taurhaus startup
or a manual `mesh team-daemon restart` to rotate stale team-daemons.

## Root Cause

The repair implementation already existed in Taurhaus coordination:

- member daemon drift repair in liveness reconciliation
- team-daemon drift repair in `trigger_team_self_heal(...)`
- background team self-heal in `CoordinationState::run_background_self_heal_pass()`

The actual gap was in the native install command path.

Before this change:

- WSL install replaced the Mesh binary and then triggered immediate self-heal
- native install replaced the Mesh binary and stopped after verification

That left native live team-daemons running the old Mesh binary until some later
event happened to touch the team:

- Taurhaus startup background self-heal
- an explicit team-daemon restart
- a later team operation that ensured the daemon

## What Changed

The fix stays on the install command surface in
`src-tauri/src/commands/mesh.rs`.

Changes:

- native install now routes through a small extracted helper,
  `install_mesh_native_at(...)`
- after a successful native binary swap and compatibility verification, that
  helper now invokes the existing Mesh install self-heal path
- install success message formatting was unified so both native and WSL paths
  can report the same daemon-rotation summary shape
- the self-heal result is reduced to the operator-facing fields that matter for
  install reporting:
  - teams reconciled
  - team-daemons ensured

This preserves the existing architecture:

- install still owns binary replacement
- coordination still owns daemon drift detection and daemon repair
- no new daemon lifecycle mechanism was introduced

## Regression Coverage

Added command-layer regression coverage in
`src-tauri/src/commands/mesh.rs`:

- `install_mesh_native_triggers_self_heal_after_successful_install`

That test proves the native install path now:

- installs the binary into the target location
- invokes the post-install self-heal hook
- reports the same rotation summary shape used for install-triggered daemon
  repair

Existing WSL install-path tests remain green, confirming the new shared message
formatting did not regress the older hot-swap path.

## Operational Result

After this fix, manual team-daemon restarts are no longer required as the
standard step after replacing the Mesh binary through Taurhaus on native Linux
or macOS.

Manual restart commands remain valid as fallback operator tools for unusual
states or direct Mesh CLI debugging, but they are no longer required for normal
post-install daemon rotation.

## Remaining Risk

This fix guarantees the install path triggers immediate repair. It does not
change the underlying self-heal scope:

- only teams present in Taurhaus coordination state are scanned
- teams that already sit outside recoverable coordination state still rely on
  the existing fallback/manual recovery tools

That is acceptable for this task because the bug was the missing native install
trigger, not the self-heal implementation itself.
