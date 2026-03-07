# Mesh daemon upgrade migration

Date: 2026-03-07
Owner: developer1
Tasks: #544, #558

## Status

This document is now mostly historical.

As of taurhaus 0.5.3 with mesh 0.2.3 hot-swap support, the old manual runbook is obsolete for normal upgrades:
- `install_mesh` performs an atomic binary swap
- taurhaus detects running daemon version drift
- taurhaus automatically cycles drifted daemons through liveness reconciliation and background self-heal

The manual `team-daemon stop/start/restart-all` sequence remains available as an emergency fallback or direct CLI operator tool, but it is no longer the primary upgrade path taurhaus expects.

## Current behavior

### 1. Mesh install now uses a safe replacement path

Taurhaus no longer does a direct overwrite of the installed `mesh` binary.

Current behavior:
- install stages a temp binary
- install atomically swaps it into place
- install verifies the new binary with `--version`
- if mesh daemons were already running, taurhaus immediately triggers a bounded self-heal pass

Result:
- the installed binary on disk is updated safely
- install-triggered daemon migration starts automatically when needed

### 2. Member daemon version drift is detected automatically

During coordination liveness reconciliation, taurhaus verifies that a running member mesh daemon still matches the currently installed `mesh` binary.

Current behavior:
- if the daemon PID is live and still matches the installed binary, taurhaus keeps it
- if the daemon PID is live but its executable identity has drifted, taurhaus terminates it and respawns a fresh daemon for the live pane
- if the runtime daemon PID is stale, taurhaus clears it and repairs state as before

Result:
- healthy-but-old member daemons are no longer left running indefinitely after a mesh upgrade

### 3. Team-daemon version drift is detected automatically

During bounded background self-heal, taurhaus also verifies the running team-daemon against the currently installed `mesh` binary.

Current behavior:
- if the team-daemon matches the installed binary, taurhaus leaves it alone
- if the team-daemon has binary drift, taurhaus stops it and ensures a fresh team-daemon is running
- this happens during background self-heal and during install-triggered self-heal after a mesh upgrade

Result:
- the team-daemon no longer requires a manual stop/start cycle after a normal mesh install

### 4. Manual daemon restart commands are now fallback-only

These commands still exist and remain valid operator tools:
- `mesh --team <team> --name <operator> team-daemon stop`
- `mesh --team <team> --name <operator> team-daemon start`
- `mesh --team <team> --name <operator> team-daemon restart-all`

Use them only when:
- debugging a specific daemon lifecycle problem
- operating the mesh CLI directly outside taurhaus
- recovering from an unusual state that taurhaus self-heal could not repair automatically

They should no longer be documented as the standard post-upgrade path.

## What is automatic today

Automatic today:
- atomic mesh binary replacement during install
- installed-binary verification after swap
- member daemon version-drift detection
- team-daemon version-drift detection
- automatic daemon cycling via install-triggered self-heal and normal background self-heal
- normal liveness repair for missing or stale daemon/runtime state

Usually not needed manually anymore:
- team-daemon stop/start after upgrading mesh
- `team-daemon restart-all` purely to move daemons onto the new binary

Still manual in edge cases:
- recovering members that have no usable `tmuxPaneId` or no recoverable runtime target
- debugging direct mesh CLI lifecycle issues outside taurhaus

## Historical note

The previous version of this document described a manual upgrade sequence where operators had to:
1. replace the mesh binary on disk
2. stop and restart the team-daemon
3. run `team-daemon restart-all` to migrate member daemons

That description reflected the pre-hot-swap state and should now be treated as obsolete.
