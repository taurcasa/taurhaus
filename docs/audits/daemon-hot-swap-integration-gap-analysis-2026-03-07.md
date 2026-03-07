# Daemon hot-swap integration gap analysis (2026-03-07)

## Scope

Task `#555` asks for full integration coverage across taurhaus + mesh for the daemon hot-swap lifecycle:

1. Version drift detection with real `/proc`-backed executable identity checks
2. Atomic `install_mesh_wsl` with daemon cycling end-to-end
3. Team-daemon `restart-self` lifecycle
4. Full upgrade cycle: old binary running -> install -> self-heal detects drift -> daemons cycle -> message delivery still works
5. Partial failure recovery after upgrade
6. Sandboxed `HOME` task/message delivery

## Current in-tree coverage

### Scenario 1: Version drift detection

Partially covered.

- `taurhaus`: `coordination::orchestrator::tests::liveness_reconcile_restarts_running_daemon_when_binary_has_drifted`
- `taurhaus`: `coordination::runtime::process_uses_current_mesh_binary` implementation is exercised indirectly by the orchestration regression

Gap:
- The taurhaus regression uses the recording runtime double, not a real daemon process whose executable identity is read from `/proc/<pid>/exe`.
- We still need a realistic process-level integration test that runs an older mesh binary, swaps the installed binary, and proves the real identity check flips from current -> drifted.

### Scenario 2: Atomic install with daemon cycling

Partially covered.

- `taurhaus`: `commands::mesh::tests::install_mesh_wsl_script_uses_atomic_swap_and_emits_daemon_cycle_markers`

Gap:
- This validates script structure only.
- It does not run real daemons, invoke `install_mesh_wsl`, or verify that old processes stop and replacement daemons come up on the new binary.

### Scenario 3: Team-daemon restart-self

Partially covered.

- `mesh`: `team_daemon_restart_self_replaces_running_daemon_pid`

Gap:
- Current mesh CLI coverage checks PID replacement and pidfile cleanup.
- It does not verify member daemons survive/useful communication continues after self-cycle.

### Scenario 4: Full upgrade cycle

Not covered end-to-end.

Signals present:
- `taurhaus`: `coordination::orchestrator::tests::trigger_team_self_heal_cycles_stale_team_daemon_and_restarts_drifted_member_daemon`
- `taurhaus`: `coordination::state::tests::background_self_heal_pass_cycles_stale_team_daemon_for_active_team`

Gap:
- These are still runtime-double/state tests.
- No test currently runs real mesh daemons, performs an actual binary swap, triggers background self-heal, and verifies that message delivery still works after the cycle.

### Scenario 5: Partial failure recovery

Not covered.

Gap:
- No integration case currently simulates install success + restart failure followed by recovery on a later self-heal pass.

### Scenario 6: Sandboxed HOME task/message delivery

Partially covered on the mesh side.

- `mesh`: CLI integration tests around sandboxed `HOME` and `MESH_SYSTEM_HOME_OVERRIDE` already exist (`tests/cli_integration.rs`)

Gap:
- Need taurhaus-driven integration coverage showing the installed/runtime path still resolves correctly when taurhaus launches the lifecycle under sandboxed agent `HOME`.

## Blockers

1. `#553` is still in progress, so the final full-upgrade and recovery flows are moving underneath this task.
2. I need a mesh binary/workspace state containing `#542/#543/#552` to run realistic end-to-end tests. Coordination request sent to `mesh-expert`.
3. Existing test coverage is spread across taurhaus runtime/orchestrator/state tests and mesh CLI integration tests; the missing part is a joined lifecycle harness, not more isolated unit cases.

## Proposed implementation split

### Taurhaus integration additions

1. Add a process-backed coordination integration test that:
   - stages an "old" mesh binary under a temporary home
   - starts real member/team daemons from that binary
   - swaps `~/.local/bin/mesh` to a different inode
   - proves taurhaus drift detection marks the running daemon stale based on real executable identity

2. Add an install/self-heal integration test that:
   - invokes `install_mesh_wsl` or a platform-local equivalent under a temp-root harness
   - verifies daemon cycle markers are emitted
   - verifies next status/self-heal pass repairs the team and member daemons

3. Add a failure-recovery integration test that:
   - forces restart failure after a successful binary swap
   - verifies the next self-heal pass recovers the team automatically

### Mesh integration additions

1. Extend `tests/cli_integration.rs` with a restart-self survivability case:
   - start team-daemon + member daemon
   - run `restart-self`
   - verify new team-daemon PID and functional member operations afterward

2. Add a hot-swap-friendly binary/version fixture approach so taurhaus can test old-vs-new executable identity without hand-editing compiled artifacts.

## Immediate next step

Wait for `mesh-expert` to provide the exact mesh commit/build artifact for `#542/#543/#552`, then implement the process-backed taurhaus + mesh integration harness against that binary.
