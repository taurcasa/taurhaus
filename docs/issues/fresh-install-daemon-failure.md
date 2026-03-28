# Fresh Install: Daemon WSL Install Fails Silently

**Reported**: 2026-03-27
**Status**: Open — partially mitigated, remaining Windows-side install failure under investigation

## Problem

On a clean Windows installation (from GitHub release NSIS installer), the FirstRunWizard fails to install the daemon into WSL. The user sees "Could not install the helper service" with no actionable guidance.

## What We Know

### The binary is bundled correctly

The release installer places a real 18MB daemon binary at the correct path:

```
C:\Users\<user>\AppData\Local\taurhaus\resources\taurhaus-daemon
```

### The startup daemon bootstrap was skipped on fresh installs

`src-tauri/src/startup/daemon.rs:45` gates daemon bootstrap on `boot_distro`, which comes from `detect_wsl_distro()` (`src-tauri/src/startup/setup.rs:71`). That function only finds a distro if there are existing projects in the DB with WSL paths. On a fresh install (empty DB), this is `None`, so daemon bootstrap is never attempted.

Mesh install is NOT gated this way — it runs unconditionally at startup and succeeds.

Current tree status:

- `detect_wsl_distro()` now falls back to the default WSL distro via `wsl --list --quiet`, so this specific fresh-install bootstrap gap is addressed.
- The remaining issue is the Windows-side `install_daemon` IPC path failing in-process on some hosts even though equivalent manual WSL copy commands succeed.

### The wizard IPC call fails consistently

When the user clicks "Install" in the wizard, `install_daemon` IPC is called. It fails every time with empty stderr in ~156ms. `check_daemon_install_status` succeeds (208ms) right before the install call.

### Manual copy works

Running the equivalent commands manually from WSL succeeds immediately.

### Cannot reproduce from WSL

Simulating the `wsl.exe` invocation from WSL interop and PowerShell both succeed. The failure only occurs when the Windows-native Tauri app calls `wsl.exe` via `std::process::Command` in the IPC handler context.

## Log Evidence

From `%APPDATA%\com.taurhaus.dev\taurhaus.log.jsonl` (first run, 2026-03-27):

| Timestamp | Event | Result |
|-----------|-------|--------|
| 18:48:42 | startup.mesh_install | succeeded (installed) |
| 18:48:49 | check_daemon_install_status | succeeded (208ms) |
| 18:48:53 | install_daemon | failed (156ms, empty stderr) |
| 18:49:07 | install_daemon (retry) | failed (156ms, empty stderr) |
| 18:50:06 | install_daemon (retry) | failed (157ms, empty stderr) |

No `startup.daemon_bootstrap` events — the bootstrap was skipped entirely.

## Key Files

- `src-tauri/src/commands/daemon.rs` — install logic, WSL script, status checks
- `src-tauri/src/startup/daemon.rs` — startup bootstrap (line 45: distro gate)
- `src-tauri/src/daemon/launcher.rs` — `wsl_command()`, `wsl_shell_args()`
- `src/lib/FirstRunWizard.svelte` — wizard UI
- `src/lib/errorCopy.js` — user-facing error messages
