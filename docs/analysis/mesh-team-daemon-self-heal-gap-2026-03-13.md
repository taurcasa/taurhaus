# Mesh Team-Daemon Self-Heal Gap

**Date:** 2026-03-13
**Task:** #1264

## Conclusion

The Mesh team-daemon did not self-heal after the binary upgrade because the
native Mesh install path updates `~/.local/bin/mesh` but does **not** trigger
the immediate post-install self-heal cycle that actually rotates stale live
Mesh daemons.

The repair logic for a drifted team-daemon already exists and is tested. The
gap is the trigger path after upgrade:

- WSL Mesh install: binary swap **plus** immediate daemon-cycle/self-heal
- native Mesh install: binary swap only

That makes the team-daemon depend on some later event to rotate:

- Taurhaus startup background self-heal
- an explicit `mesh team-daemon restart`
- some later team operation that ensures the team daemon

So the failure was not "team-daemon self-heal is broken." The failure was:

- **native Mesh upgrade did not invoke the self-heal path that would have
  repaired the stale team-daemon immediately**

## What The Code Does Today

## 1. Member-daemon restart path

Member daemons have several opportunities to be restarted from the current Mesh
binary:

- direct startup in team initialize/member-add flows
- liveness reconciliation in `reconcile_team_liveness(...)`
- explicit centralized restart paths

The key member repair logic lives in
`src-tauri/src/coordination/orchestrator.rs`:

- if a recorded member daemon PID is running but not using the current Mesh
  binary, Taurhaus terminates it
- if no valid daemon is retained, Taurhaus spawns a fresh member daemon from
  the current installed Mesh binary path

That means member daemons can converge opportunistically whenever a member-level
flow touches them.

## 2. Team-daemon restart path

The team-daemon repair logic also exists:

- `trigger_team_self_heal(...)` checks whether the team-daemon binary has
  drifted
- if drift is detected, it stops the running team-daemon
- it then calls `ensure_team_daemon_running_best_effort(...)` to spawn a fresh
  one

This behavior is covered by tests in `coordination/state.rs`, including the
upgrade-cycle path that expects drifted team-daemons to be stopped and
restarted.

So the repair implementation is already present.

## 3. The install-path asymmetry

The important difference is in `src-tauri/src/commands/mesh.rs`:

### WSL install path

`install_mesh_wsl(...)` does all of the following:

1. copies the new Mesh binary into WSL `~/.local/bin/mesh`
2. kills running Mesh member daemons and team daemons before the swap
3. if Mesh daemons had been running, calls `run_mesh_install_self_heal(app)`

That gives WSL installs an immediate daemon refresh path after binary upgrade.

### Native install path

`install_mesh_native(...)` only:

1. copies the new Mesh binary into `~/.local/bin/mesh`
2. verifies it with `mesh version --json`

It does **not**:

- stop running Mesh member daemons
- stop the team-daemon
- call `run_mesh_install_self_heal(app)`

That is the core gap.

## Why Member-Daemon And Team-Daemon Behavior Diverged

The rollout evidence is consistent with that code split.

From the local upgrade and drift notes:

- the installed Mesh CLI advanced to `0.2.12`
- after that upgrade, the daemon fleet was only partially rotated
- some member daemons had already moved to the new installed binary
- the `taurhaus-team` and `taurmuse-team` team-daemons were still running the
  older Mesh inode

That is exactly what we would expect if:

- the installed Mesh binary changed
- no immediate native post-install self-heal ran
- some member daemons later restarted via separate member-scoped flows
- the team-daemon did not get touched yet

Member daemons have more ways to get refreshed later:

- initialize/add-member/resume flows
- liveness reconciliation tied to active panes
- explicit member restarts

The team-daemon has fewer triggers:

- `trigger_team_self_heal(...)`
- initialize/member-add ensure hooks
- explicit team-daemon restart

So after a native binary swap, stale team-daemons can lag behind even when some
member daemons have already converged.

## Why This Was Visible In The Rollout Timeline

The relevant timeline pattern from the local rollout evidence was:

1. installed Mesh binary changed to the new version
2. immediate shell-level alignment was correct
3. live process alignment was still partial
4. stale team-daemons remained on the older binary identity
5. some member daemons had already moved forward

That pattern points to a **missing immediate repair trigger**, not to a broken
repair implementation.

If the native upgrade path had invoked the same self-heal flow as the WSL path,
the expected result would have been:

- stale team-daemons stopped immediately
- team-daemon re-ensured from the new installed binary
- member daemon repair done in the same same pass

Instead, the upgrade stopped after copying the binary, so the old team-daemon
was left running until a later event would eventually rotate it.

## Recommended Next Step

Make the native Mesh install path mirror the WSL post-install repair behavior.

Smallest honest fix:

- after a successful `install_mesh_native(...)`, invoke
  `run_mesh_install_self_heal(app)` the same way the WSL install path already
  does

Recommended rule:

- if the installed Mesh binary changed successfully, immediately run the Mesh
  daemon self-heal pass on native platforms too

That gives one consistent behavior across platforms:

- install new Mesh binary
- immediately reconcile and rotate stale member daemons
- immediately stop and re-ensure stale team-daemons

## Secondary Hardening

After the native post-install self-heal is added, the next useful hardening is
operator-visible confirmation in the install result:

- number of teams repaired
- number of team-daemons re-ensured
- whether any repair errors occurred

That is already the shape used by the WSL install success message and should be
reused instead of inventing a separate native-only result.

## Bottom Line

The team-daemon did not self-heal after the binary upgrade because the upgrade
path that was actually used only replaced the installed Mesh binary and never
ran the immediate self-heal cycle.

The restart logic for drifted team-daemons already exists. The missing piece is
to **call it after native Mesh installs**, the same way Taurhaus already does
for WSL installs.
