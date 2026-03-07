# Mesh daemon availability and reliability audit

Date: 2026-03-07
Owner: developer1
Task: #524

## Findings

### 1. Critical: Windows coordination daemon PID handling is not WSL-safe

The coordination runtime writes and reads mesh daemon pidfiles from the host-side mirrored `.claude` tree, but on Windows it verifies and kills those PIDs with native `tasklist` and `taskkill`. The mesh/team daemons themselves are started through WSL-facing mesh commands, so the recorded PIDs are not reliably host Windows PIDs.

Evidence:
- Host-side pidfile locations: `src-tauri/src/coordination/runtime.rs:1015`
- Windows passes WSL `~/.claude` into mesh CLI: `src-tauri/src/coordination/runtime.rs:1052`
- Team daemon startup trusts pidfile liveness through `tasklist`: `src-tauri/src/coordination/runtime.rs:317`
- Team daemon stop uses `taskkill /PID`: `src-tauri/src/coordination/runtime.rs:492`
- Generic PID liveness on Windows uses `tasklist`: `src-tauri/src/coordination/runtime.rs:1131`
- Generic PID termination on Windows uses `taskkill`: `src-tauri/src/coordination/runtime.rs:449`
- Windows daemon identity matching is currently stubbed to `Ok(true)`: `src-tauri/src/coordination/runtime.rs:1063`

Impact:
- Duplicate-daemon cleanup can target the wrong process on Windows.
- Team daemon stop can silently fail to stop the real WSL daemon.
- Startup/reuse checks can treat unrelated Windows processes as valid daemon ownership.
- Availability is unreliable specifically on the main supported Windows deployment path.

Status: broken

### 2. High: per-agent mesh daemons no longer self-heal during steady-state runtime

`reconcile_team_liveness()` still contains the write-on-drift restart/adopt/cleanup logic for per-agent mesh daemons, but the live team status and project mesh snapshot IPC paths no longer call it. After startup, the remaining startup reconciliation only clears stale runtime metadata; it does not restart missing daemons.

Evidence:
- Live status now fast-reads only: `src-tauri/src/commands/coordination.rs:437`
- Project mesh snapshot now fast-reads only: `src-tauri/src/commands/coordination.rs:713`
- Restart/adopt logic still lives in liveness reconcile: `src-tauri/src/coordination/orchestrator.rs:523`
- Daemon restart branch inside reconcile: `src-tauri/src/coordination/orchestrator.rs:691`
- Startup reconciliation only clears stale runtime state: `src-tauri/src/coordination/orchestrator.rs:472`
- Startup stale-pid cleanup marks `SessionDead` but does not respawn: `src-tauri/src/coordination/orchestrator.rs:775`

Impact:
- If a live pane loses its per-agent mesh daemon after initialization, the app can remain degraded indefinitely.
- Recovery depends on an explicit lifecycle action such as resume-team, resume-member, add-member, or reinitialize.
- The hot-path performance fix removed the only autonomous repair loop, but nothing replaced it off the hot path.

Status: broken

### 3. High: team daemon has no independent health monitor or restart path

The team daemon is best-effort started during initialize/add/resume flows, but there is no equivalent of the app-wide `daemon_health_check()` for the team daemon. If the team daemon dies after a successful start, no background monitor restarts it.

Evidence:
- Initialize ensures team daemon once: `src-tauri/src/coordination/pipelines/initialize.rs:193`
- Add-member ensures team daemon once: `src-tauri/src/coordination/pipelines/members.rs:170`
- Resume-member ensures team daemon once: `src-tauri/src/coordination/pipelines/members.rs:466`
- Resume-team ensures team daemon once after member loop: `src-tauri/src/coordination/orchestrator.rs:435`
- Best-effort wrapper only does a spawn attempt: `src-tauri/src/coordination/orchestrator.rs:1042`
- Team daemon stop is explicit teardown only: `src-tauri/src/coordination/orchestrator.rs:1060`
- Global app daemon does have a periodic monitor/restart loop: `src-tauri/src/daemon_lifecycle.rs:500`

Impact:
- Team-level automation can silently disappear after a crash or kill.
- Runtime reliability depends on users hitting a later lifecycle action that happens to call `spawn_team_daemon()` again.
- This is a structural availability gap, not a transient implementation defect.

Status: broken

### 4. Medium: WSL daemon install path is not hardened for live binary replacement

The Tauri-side Windows install path copies directly to `$HOME/.local/bin/taurhaus-daemon`, chmods it, and verifies `--version`. It does not coordinate with a running daemon process, use a temp file + atomic rename, or perform a stop/restart handshake inside the command itself.

Evidence:
- Direct `cp` into final destination: `src-tauri/src/commands/daemon.rs:505`
- No stop/restart coordination inside the IPC install path: `src-tauri/src/commands/daemon.rs:482`

Impact:
- This is vulnerable to live-replacement races and platform-specific `Text file busy` style failures.
- Reliability depends on callers using a higher-level recipe that restarts the daemon externally.

Status: fragile

## What is currently robust

### 1. App-wide daemon bootstrap and recovery are coherent

The global taurhaus daemon has a clear startup bootstrap and an ongoing health loop that reconnects first, then restarts, then respawns watches and reseeds git status.

Evidence:
- Background startup bootstrap: `src-tauri/src/startup/daemon.rs:8`
- Periodic reconnect/restart loop: `src-tauri/src/daemon_lifecycle.rs:500`

### 2. Coordination daemon startup verifies a live pidfile before accepting success

Per-agent and team daemon spawns do not trust the launcher child PID alone. They wait for the daemon pidfile and confirm the recorded PID is live before reporting success.

Evidence:
- Spawn waits for pidfile verification: `src-tauri/src/coordination/runtime.rs:905`
- Per-agent daemon spawn uses pidfile resolution: `src-tauri/src/coordination/runtime.rs:300`
- Team daemon spawn uses pidfile resolution: `src-tauri/src/coordination/runtime.rs:317`

This is a sound design choice. The weakness is the Windows PID interpretation behind it, not the verification pattern itself.

## Recommended follow-up tasks

1. `#527` Fix Windows coordination runtime WSL daemon PID liveness and termination handling.
2. `#528` Add a background per-agent mesh daemon health reconciliation loop outside the live-status hot path.
3. `#529` Add team daemon death detection and background self-heal.
4. `#530` Harden `install_daemon_wsl()` with temp-file swap and running-daemon coordination.

## Overall assessment

The global app daemon lifecycle is in reasonable shape. The mesh coordination daemon lifecycle is not yet equally reliable.

The two main gaps are:
- Windows PID handling assumes host-native processes for daemons that are effectively WSL-managed.
- Hot-path performance work removed the only autonomous per-agent repair path, and there is still no team-daemon monitor.

That means the current system can look healthy at initialization time but drift into a degraded steady state with no autonomous recovery, especially on Windows.
