# Mesh daemon upgrade migration

Date: 2026-03-07
Owner: developer1
Task: #544

## Short answer

Replacing `~/.local/bin/mesh` does not upgrade already-running mesh daemons.

What changes immediately:
- new `mesh` CLI invocations run the new binary
- any daemon spawned after the replacement uses the new binary

What does not change automatically:
- already-running per-agent daemons keep running the old executable image
- an already-running `team-daemon` keeps running the old executable image
- taurhaus does not currently detect mesh daemon version mismatch and does not restart healthy daemons just because the binary on disk changed

This means a mesh upgrade today is a two-part operation:
1. replace the binary on disk
2. explicitly cycle the running mesh daemons

## What happens today

### 1. Mesh binary replacement only changes future spawns

On the taurhaus side, new mesh daemon spawns always resolve `mesh` from the installed path at spawn time.

Evidence:
- per-agent spawn uses `mesh_cli::mesh_binary_path()`: `src-tauri/src/coordination/runtime.rs:304`
- team-daemon spawn uses `mesh_cli::mesh_binary_path()`: `src-tauri/src/coordination/runtime.rs:351`

Inference:
- once `~/.local/bin/mesh` is replaced, any future daemon spawn uses the new binary
- existing daemons are unaffected until they are stopped and respawned

### 2. Taurhaus mesh version checks look at the installed binary, not running daemons

The mesh install gate compares the bundled version to `~/.local/bin/mesh --version` (or the WSL equivalent). It does not inspect running mesh daemon processes.

Evidence:
- WSL/native install status checks read the installed binary version: `src-tauri/src/commands/mesh.rs:120`, `src-tauri/src/commands/mesh.rs:161`
- `needs_update` is derived from that installed version only: `src-tauri/src/commands/mesh.rs:146`, `src-tauri/src/commands/mesh.rs:249`

Result:
- taurhaus can report “mesh is up to date” while old mesh daemons are still running in memory

### 3. Taurhaus background self-heal repairs missing daemons, not version drift

Taurhaus now runs a background coordination self-heal pass after startup and every 30 seconds.

Evidence:
- background self-heal monitor: `src-tauri/src/startup/mod.rs:590`
- self-heal pass loops teams and calls `trigger_team_self_heal`: `src-tauri/src/coordination/state.rs:123`
- self-heal reconciles liveness and ensures a team-daemon only when runtime indicates something is missing/recoverable: `src-tauri/src/coordination/orchestrator.rs:748`

Important limitation:
- this is a liveness repair path, not a version migration path
- healthy old daemons are left alone

### 4. Mesh `team-daemon restart-all` upgrades member daemons, not the team-daemon itself

The `team-daemon` CLI subcommands are currently:
- `start`
- `stop`
- `status`
- `restart <member>`
- `restart-all`

Evidence:
- CLI definition: `/home/mstie/projects/mesh/src/cli.rs:301`
- command dispatch: `/home/mstie/projects/mesh/src/main.rs:871`

`restart-all` does this for each active member with a `tmuxPaneId`:
- stop the member daemon with SIGTERM, then SIGKILL after 5 seconds if needed
- spawn a new daemon with `std::env::current_exe()`

Evidence:
- `restart-all` iterates members: `/home/mstie/projects/mesh/src/team_daemon.rs:165`
- stop old member daemon: `/home/mstie/projects/mesh/src/team_daemon.rs:217`
- start new member daemon from the currently-running CLI binary: `/home/mstie/projects/mesh/src/team_daemon.rs:227`
- member daemon stop behavior: `/home/mstie/projects/mesh/src/daemon.rs:145`

Result:
- if you run `restart-all` from the new `mesh` binary, restarted member daemons come up on the new version
- the `team-daemon` process itself is not restarted by `restart-all`

### 5. Restarting the team-daemon does not restart member daemons

The `team-daemon` is a foreground process running the idle-monitor loop. It is not the parent/supervisor of member daemons.

Evidence:
- team-daemon loop is standalone: `/home/mstie/projects/mesh/src/team_daemon.rs:26`
- team-daemon stop only stops the team-daemon PID: `/home/mstie/projects/mesh/src/team_daemon.rs:73`

Result:
- restarting `team-daemon` does not automatically cycle any agent daemons
- member daemons can remain on the old version until separately restarted

### 6. Taurhaus daemon hot-swap is hardened; mesh hot-swap is not yet

The taurhaus daemon WSL install path now stages a temp binary, coordinates with a running daemon, atomically swaps it into place, and restarts the daemon only if one was previously running.

Evidence:
- hardened WSL install flow: `src-tauri/src/commands/daemon.rs:482`

By contrast, the mesh WSL install path still does a direct `cp` into `$HOME/.local/bin/mesh` and only verifies `--version` afterward.

Evidence:
- mesh WSL install still uses direct `cp`: `src-tauri/src/commands/mesh.rs:332`

Result:
- the atomic swap work from `#535` covers `taurhaus-daemon`, not `mesh`
- the current Tauri `install_mesh_wsl()` path still has a binary replacement hazard and still does not restart any running mesh daemons

## Current recommended upgrade sequence

This is the safest current sequence for upgrading `mesh` plus taurhaus when teams may already be running.

### Preferred sequence when you can use an atomic mesh install

Use this when running locally with the repo `just install-mesh` recipe, because that recipe installs via temp file + `mv`.

1. Upgrade the taurhaus app / bundle as usual.
2. Upgrade the taurhaus daemon.
   - taurhaus handles this automatically in the hardened WSL path for `install_daemon_wsl()`.
3. Replace the mesh binary on disk using an atomic install path.
   - local/dev: `just install-mesh`
4. Restart the team-daemon from the new binary.
   - `mesh --team <team> --name <operator> team-daemon stop`
   - `mesh --team <team> --name <operator> team-daemon start`
5. Restart member daemons from the new binary.
   - `mesh --team <team> --name <operator> team-daemon restart-all`
6. Inspect the summary and handle skipped members.
   - any member without `tmuxPaneId` is skipped
   - any inactive member is skipped
   - failed members need explicit retry or taurhaus lifecycle action

Why this order:
- step 3 makes the new binary available for future spawns
- step 4 upgrades the team-daemon itself
- step 5 upgrades the member daemons using the new binary via `current_exe()`

### Current taurhaus/Tauri Windows path caveat

If the upgrade is happening through taurhaus’s `install_mesh_wsl()` IPC path, the flow above is not fully safe yet because `install_mesh_wsl()` is still a direct-copy path.

For that path, the current product behavior is:
- copy new mesh binary into place
- verify `mesh --version`
- do not restart team-daemon
- do not restart member daemons

So the current Tauri path is not a complete hot-swap flow.

## What is automatic vs manual today

Automatic today:
- taurhaus-daemon restart on hardened WSL install: yes
- future mesh daemon spawns use the new `mesh` binary: yes
- background self-heal repairs missing mesh daemons: yes

Manual today:
- team-daemon version migration: manual stop + start
- member daemon version migration: manual `restart-all` or explicit targeted restart
- handling skipped members without `tmuxPaneId`: manual
- ensuring a safe mesh binary replacement on the Tauri WSL path: manual or code fix needed

Not automatic today:
- detecting that a healthy running mesh daemon is older than the installed binary
- draining in-flight mesh daemon work before restart
- one-shot “upgrade all mesh daemons to current binary” orchestration

## Edge cases

### Binary replacement while old daemons are running

Expected behavior on Linux/WSL:
- old daemons keep running from their already-executing image
- new spawns use the new binary path

Operational consequence:
- mixed-version mesh daemons are possible until explicit restart/cycling completes

### `Text file busy`

This is still a real risk for the current Tauri `install_mesh_wsl()` implementation because it uses direct `cp` onto the final executable path.

Mitigation available today:
- the repo `just install-mesh` recipe uses temp file + `mv`

Missing mitigation in product:
- `src-tauri/src/commands/mesh.rs` still needs the same atomic-swap treatment that `install_daemon_wsl()` now has

### PID file staleness after restart

Current mesh stop/start flows already handle stale PID files reasonably well.

Evidence:
- team-daemon status/stop clears stale PID files: `/home/mstie/projects/mesh/src/team_daemon.rs:82`, `/home/mstie/projects/mesh/src/team_daemon.rs:132`
- member daemon stop clears stale PID files: `/home/mstie/projects/mesh/src/daemon.rs:153`
- taurhaus team-daemon spawn now validates team PID identity before reuse: `src-tauri/src/coordination/runtime.rs:323`

## Minimal fixes needed

### 1. Harden `install_mesh_wsl()` the same way `install_daemon_wsl()` was hardened

Needed changes:
- stage to temp path
- atomically rename into place
- if desired, record whether a hot-swap migration should run next

Why:
- removes direct-copy replacement risk
- aligns mesh install semantics with the daemon install semantics already shipped

### 2. Add an explicit mesh daemon upgrade orchestrator

Minimum viable behavior:
1. install new `mesh` binary
2. stop old team-daemon
3. start new team-daemon
4. run `team-daemon restart-all`
5. surface skipped/failed members clearly

Why:
- current restart steps are scattered and partially manual
- version migration is a different concern from liveness self-heal

### 3. Optionally add version-aware daemon detection

Potential improvement:
- record daemon version in runtime metadata or query it via a lightweight daemon status contract
- if running daemon version != installed mesh version, trigger controlled restart

Why:
- prevents silent mixed-version steady states
- lets taurhaus report “binary updated, daemons still old” instead of only checking the installed binary

## Practical runbook

### Safe runbook for today

If you control the host and can use the repo tooling:

1. Install/update the new taurhaus app.
2. Update the taurhaus daemon.
3. Update mesh with an atomic install path.
   - `just install-mesh`
4. Restart the team-daemon from the new binary.
   - `mesh --team <team> --name <operator> team-daemon stop`
   - `mesh --team <team> --name <operator> team-daemon start`
5. Restart member daemons from the new binary.
   - `mesh --team <team> --name <operator> team-daemon restart-all`
6. Review output for skipped or failed members.
7. For any skipped active member without `tmuxPaneId`, use taurhaus/member lifecycle actions to recreate or resume that session so a fresh daemon spawn happens from the new binary.

### What not to assume

Do not assume any of the following after only replacing `~/.local/bin/mesh`:
- that running team-daemon is now upgraded
- that running member daemons are now upgraded
- that taurhaus self-heal will restart healthy old daemons just because a newer binary exists
- that taurhaus’s current `install_mesh_wsl()` path gives you the same safety as the hardened daemon install path

## Bottom line

Today, mesh binary replacement is only half of the migration.

The current system is good at:
- using the new binary for future spawns
- repairing missing daemons
- restarting individual member daemons cleanly

The current system is not yet good at:
- upgrading healthy running mesh daemons in place
- making the Tauri-side mesh install path atomic on WSL
- providing a single orchestrated “upgrade all daemons to the new mesh version” flow
