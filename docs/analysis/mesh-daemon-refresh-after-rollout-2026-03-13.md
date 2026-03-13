# Mesh Daemon Refresh After Rollout

**Date:** 2026-03-13
**Task:** #1256

## Summary

There are two different daemon layers in the current rollout path, and they
refresh differently:

- `taurhaus-daemon` is Taurhaus's own backend daemon binary
- Mesh runs separate long-lived processes for:
  - each member daemon: `mesh daemon --pane ...`
  - the team daemon: `mesh team-daemon start ...`

The important result is:

- rebuilding or reinstalling the Taurhaus app bundle does **not** by itself
  rotate already running Mesh daemons
- Mesh daemon rotation happens only when the **installed Mesh CLI binary**
  changes and a restart path or self-heal path runs against it
- `taurhaus-daemon` refresh is a separate concern and currently depends on
  explicit install or restart behavior

## Current Refresh Paths

## 1. Taurhaus daemon (`taurhaus-daemon`)

### Explicit install path

`just install-daemon` is the only explicit refresh path for the installed
`taurhaus-daemon` binary in normal local workflow.

Current behavior:

- build release daemon
- detect whether `taurhaus-daemon` is running
- stop it
- atomically replace `~/.local/bin/taurhaus-daemon`
- restart it if it had been running before

That means a real daemon binary refresh only happens when the install recipe
runs.

### App startup behavior

At app startup, Taurhaus bootstraps its backend connection through
`src-tauri/src/startup/daemon.rs` and `src-tauri/src/daemon/launcher.rs`.

Important behavior:

- if the daemon is not connected, Taurhaus tries to start it from the installed
  daemon path
- on native Linux startup, Taurhaus also validates whether the already running
  daemon process is using the expected installed binary and will terminate and
  restart it if the running executable is stale or deleted
- on Windows/WSL, that startup staleness check does **not** currently perform
  the same running-binary identity validation for the WSL daemon

Operational consequence:

- Linux native startup can evict a stale `taurhaus-daemon` if the installed
  daemon has already been updated
- Windows/WSL startup does not currently give the same automatic stale-binary
  rotation guarantee
- if Taurhaus is rebuilt or reinstalled without also refreshing the installed
  daemon binary, the already running `taurhaus-daemon` keeps serving from the
  old install

## 2. Mesh member and team daemons

### How Taurhaus launches them

Taurhaus coordination launches Mesh daemons from the installed Mesh CLI path,
not from the Taurhaus app bundle resource directly.

Current runtime launch points:

- member daemon spawn uses `mesh daemon --pane ...`
- team daemon spawn uses `mesh team-daemon start ...`

Both go through `coordination/runtime.rs`, which resolves the Mesh binary path
with `mesh_cli::mesh_binary_path()`.

That path is:

- `~/.local/bin/mesh` on Linux and macOS
- WSL `$HOME/.local/bin/mesh` on Windows

So the long-lived Mesh daemon identity is tied to the **installed Mesh CLI**,
not to whatever Mesh binary happens to be bundled inside the current Taurhaus
build.

### Binary drift detection

Taurhaus coordination already has explicit Mesh binary drift detection:

- `process_uses_current_mesh_binary(pid)` compares the running process identity
  with the installed Mesh binary identity
- member daemon reconciliation restarts a running member daemon when the binary
  identity has drifted
- team self-heal checks whether the team daemon is using the current installed
  Mesh binary, stops it if drifted, and ensures a fresh one is started

This means Taurhaus can repair already running Mesh daemons, but only relative
to the installed Mesh CLI path.

## What Happens In Real Rollout Scenarios

## Scenario A: Taurhaus app rebuild only

Example: local rebuild, new desktop bundle, or Tauri reinstall without
refreshing `~/.local/bin/taurhaus-daemon` or `~/.local/bin/mesh`.

What happens:

- running `taurhaus-daemon` keeps running the previously installed daemon binary
- running Mesh member daemons keep running the previously installed Mesh binary
- running Mesh team daemon keeps running the previously installed Mesh binary

What does **not** happen:

- the app bundle does not automatically overwrite installed `mesh`
- the app bundle does not automatically overwrite installed `taurhaus-daemon`
- already running Mesh daemons are not rotated just because the app bundle
  changed

Conclusion:

- app rebuild alone is not a daemon refresh

## Scenario B: `just install-daemon`

What happens:

- `taurhaus-daemon` is explicitly stopped
- the installed daemon binary is replaced
- the daemon is restarted if it was previously running

What does **not** happen:

- Mesh member daemons are unaffected
- Mesh team daemon is unaffected

Conclusion:

- this refreshes only the Taurhaus daemon layer

## Scenario C: Mesh install through Taurhaus on Windows/WSL

The WSL Mesh install path is the most complete current rotation flow.

What happens:

- Taurhaus copies the bundled Mesh binary into WSL `~/.local/bin/mesh`
- before replacement, it kills running Mesh member daemons and team daemons if
  they are present
- after install, if any Mesh daemons had been running, Taurhaus triggers a
  background Mesh self-heal pass
- that self-heal re-establishes healthy member daemons and ensures the team
  daemon is running again from the newly installed Mesh binary

Conclusion:

- this is already a real daemon rotation path for Mesh on Windows/WSL

## Scenario D: Mesh install on native Linux or macOS

The native install path currently copies and verifies the installed Mesh binary,
but it does **not** explicitly trigger the same immediate daemon self-heal that
the WSL install path does.

What happens:

- `~/.local/bin/mesh` is replaced
- verification runs against `mesh version --json`

What does **not** happen immediately:

- running member daemons are not proactively stopped by the install command
- running team daemon is not proactively stopped by the install command

How they do get refreshed:

- explicit Mesh restart commands can rotate them if run through the newly
  installed Mesh binary
- Taurhaus startup self-heal can rotate them after startup
- the background self-heal monitor can rotate them on its next pass

Conclusion:

- native Mesh install updates disk immediately, but live daemon refresh is
  deferred unless another restart or self-heal path runs

## Scenario E: Taurhaus restart after Mesh install

On app startup Taurhaus starts the coordination self-heal monitor with:

- initial delay: 5 seconds
- recurring interval: 30 seconds

That background pass:

- reconciles member daemon liveness
- restarts drifted Mesh member daemons against the current installed Mesh binary
- stops a drifted team daemon and ensures a fresh one is running

Conclusion:

- once the installed Mesh binary has changed, Taurhaus startup plus background
  self-heal is enough to rotate stale Mesh daemons for active teams
- but that still depends on the installed Mesh path having been updated first

## What This Means For Rollout

The current rollout boundary is:

- Taurhaus bundle version
- installed `taurhaus-daemon`
- installed `mesh`
- already running Mesh member daemons
- already running Mesh team daemons

These do **not** all refresh together automatically.

### Required refresh facts

1. A new Taurhaus app build does not automatically refresh the installed Mesh
   CLI.
2. A new Taurhaus app build does not automatically refresh the installed
   `taurhaus-daemon`.
3. Mesh daemon rotation uses the installed Mesh CLI identity, not the bundled
   resource identity.
4. Existing Mesh daemons only rotate once:
   - an explicit restart happens, or
   - Taurhaus self-heal runs after the installed Mesh CLI has changed.

## Recommended rollout rule

For a real rollout that changes Mesh or daemon behavior, treat refresh as a
three-step operation:

1. refresh the installed `taurhaus-daemon` if the backend daemon changed
2. refresh the installed `mesh` binary if Mesh runtime behavior changed
3. run an immediate Mesh self-heal or explicit daemon restart so active
   member/team daemons rotate now, not later by chance

## Recommended operational sequence

### Windows/WSL rollout

Preferred sequence:

1. install the new Mesh binary in WSL
2. let the built-in WSL install flow kill old Mesh daemons
3. run the post-install self-heal immediately
4. separately refresh `taurhaus-daemon` if that binary changed

### Linux or macOS rollout

Preferred sequence:

1. install the new Mesh binary
2. explicitly cycle Mesh daemons, or trigger self-heal immediately
3. refresh `taurhaus-daemon` separately if it changed

## Bottom Line

Already running Mesh daemons are **not** refreshed by a Taurhaus rebuild or
reinstall alone.

They refresh only when the installed Mesh CLI changes and a restart or
self-heal path actually runs against that installed path.

`taurhaus-daemon` is separate again: it only rotates when its own install or
restart path runs. So a safe rollout must treat:

- Taurhaus app bundle refresh
- Taurhaus daemon refresh
- Mesh CLI refresh
- Mesh daemon rotation

as related but distinct steps, not one implicit operation.
