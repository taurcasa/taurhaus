# Windows WSL Daemon Bootstrap Regression — 2026-03-16

## Problem

On Windows, Taurhaus starts and stays usable in local-fallback mode, but it does not bring up `taurhaus-daemon` in WSL anymore.

User-visible symptom:

- the app opens
- `daemon_status` becomes `failed`
- WSL UNC projects fall back to degraded local behavior
- daemon-backed sections like recent commits remain unavailable

This is separate from the Mesh `0.2.17` rollout itself. The bundled Mesh resources install correctly. The failure is specifically in Taurhaus daemon bootstrap on Windows.

## Current Installed State

- Taurhaus app version: `0.5.10`
- Installed Windows bundle resources show Mesh `0.2.17` / commit `9b5303acd73d03b86b26515cedff88d2400a1bba`
- Installed WSL daemon binary exists at `/home/mstie/.local/bin/taurhaus-daemon`
- Installed daemon token exists at `/home/mstie/.local/share/taurhaus/daemon.token`

## Reproduction

1. Install Taurhaus on Windows.
2. Launch the Windows app.
3. Open a WSL-backed project such as `\\wsl$\Ubuntu\home\mstie\projects\taurhaus`.
4. Observe that daemon-backed sections do not recover and the app remains disconnected from the daemon.

## Evidence

### Windows app log

From `C:\Users\mstie\AppData\Roaming\com.taurhaus.dev\taurhaus.log.jsonl`:

- `startup.daemon_phase.started`
- `startup.daemon_connect.deferred` with reason `daemon_unavailable_at_startup`
- `startup.daemon_bootstrap.started`
- repeated `daemon.connection.reconnecting`
- repeated bootstrap message:
  - `Stopping existing WSL daemon on port 17233 before restart`
- eventual frontend state:
  - `daemon_status":"failed"`

Important negative evidence:

- no daemon listener appears on `127.0.0.1:17233`
- no successful `daemon.connection.established`
- no successful `Connected to auto-started daemon`

### Live process state

At failure time:

- no `taurhaus-daemon` process is listening on port `17233`
- `ss -ltnp` shows no listener on `127.0.0.1:17233`

### Manual daemon health

Manual WSL-side daemon start works:

```bash
~/.local/bin/taurhaus-daemon --port 17233 --idle-timeout 600
```

This produced a listener on `127.0.0.1:17233`, which means:

- the daemon binary itself is valid
- the daemon token file is not the immediate blocker
- the failure is in the Windows-side launch/restart path, not in daemon runtime once started

## What Was Ruled Out

### Not the recent Taurhaus feature commits

These recent commits do not touch Windows install/launch scripts:

- `1682edf` — bundled Mesh resource metadata only
- `484bb61` — slash-style WSL UNC path handling only
- `f7ecee2` — clippy cleanup only

### Not a missing daemon binary

Verified:

- `/home/mstie/.local/bin/taurhaus-daemon` exists
- `taurhaus-daemon --help` works

### Not a missing daemon token

Verified:

- `/home/mstie/.local/share/taurhaus/daemon.token` exists

## Related But Separate Issue Already Fixed

The Windows same-version silent reinstall path had a separate bug where the installer could refresh resources but leave the old `taurhaus.exe` in place.

That was fixed in:

- `8501a95` — kill running app before silent Windows install
- `d1e735a` — harden same-version Windows reinstalls

Those fixes got `just install-windows` working again, but they did **not** resolve this daemon bootstrap regression.

## Strongest Current Hypothesis

The break is in Taurhaus's Windows-side WSL launch path for `taurhaus-daemon`.

More specifically:

- Taurhaus reaches restart/bootstrap
- it stops any existing daemon on port `17233`
- but the subsequent Windows-side launch does not leave a live `taurhaus-daemon` process behind

Given that manual WSL launch works, the most likely suspects are:

1. the `wsl.exe`-based spawn path in `src-tauri/src/daemon/launcher.rs`
2. how long-lived `wsl.exe` parent processes are being managed on this host
3. a stale/orphaned `wsl.exe` parent model interfering with new daemon launch

## Relevant Code

- [src-tauri/src/daemon/launcher.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/launcher.rs)
- [src-tauri/src/startup/daemon.rs](/home/mstie/projects/taurhaus/src-tauri/src/startup/daemon.rs)
- [src-tauri/src/commands/daemon.rs](/home/mstie/projects/taurhaus/src-tauri/src/commands/daemon.rs)

Most relevant functions:

- `try_start_daemon_wsl`
- `stop_existing_daemon_wsl`
- `try_restart_daemon`
- `ensure_bundled_daemon_installed`
- `spawn_background_bootstrap`

## Open Questions

1. Does the Windows-side `wsl.exe` launch exit immediately, and if so with what exit code?
2. Is the direct-child `wsl.exe -> taurhaus-daemon` model still reliable on this host?
3. Are existing long-lived `wsl.exe` wrappers from older launches interfering with the restart path?
4. Why do bootstrap logs show the stop step repeatedly but not the expected successful spawn/connect progression?

## Next Investigation Steps

1. Reproduce the exact `wsl.exe` launch command outside the app and capture its exit code from Windows side.
2. Add explicit structured logging around:
   - daemon binary check success/failure
   - `wsl.exe` spawn success/failure
   - `wsl.exe` child PID on Windows
   - whether the port becomes reachable after spawn
3. Verify whether the current long-lived `wsl.exe` strategy still works reliably after install/relaunch on this machine.
4. If needed, redesign the Windows launch strategy so daemon startup is self-healing and not dependent on stale `wsl.exe` wrappers.

## Bottom Line

Current state is unstable because the Windows app can install successfully but still fail to bootstrap `taurhaus-daemon` in WSL.

The daemon binary is healthy.
The bootstrap path is not.
