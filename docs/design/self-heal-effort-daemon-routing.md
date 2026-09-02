# B1b — self-heal and effort passes move into the daemon

The closing background-pass slice of [`coordination-daemon-routing.md`](coordination-daemon-routing.md) (phase B1; the deadline pass moved at protocol 15 and is the worked example). After this slice, no coordination background pass runs in the app process, and B3 — the writer-boundary assertion — becomes reachable.

Grounded by three research passes (app-side pass anatomy, daemon config surface, seam design-space). Their decisive finding: **the config seam is far narrower than assumed.** The only app-owned inputs these passes consume are the two serialized fields the protocol 16–19 intents already carry — `cli_commands` (four per-tool base-command structs) and `tmux_layout`, both from the app's SQLite settings row. Everything else is host-local and the daemon already derives it via the shipped `prepare_daemon_launch_inputs_for_tools` path: selector dirs, managed-Codex hook trust, the notify executable, `CliVersions`, and the pane-shell alias probe (which on Windows *only* the daemon can run — the app already delegates via `resolve_launch_base`). The daemon never gains a database: SQLite over 9p is the disease this migration cures, and protocol v9's history (a daemon guessing the settings DB path) is the standing warning.

## The design: hybrid, split by config dependence and trigger shape

**1. Self-heal core — daemon-scheduled, config-free.** Liveness reconcile, quarantine, orphan cleanup, and team-daemon ensure (`trigger_team_self_heal`) consume no launch settings at all. They move exactly the way the deadline pass did: a second arm in the daemon scheduler beside `DeadlineScheduler`, running on `CoordinationState::for_process_default()`. Telemetry moves from `startup.self_heal.*` to a daemon `self_heal.pass.completed/failed` family with duration and summary fields — the deadline migration's lesson (commit 34fdeead) that tracing-only reporting is invisible in production JSONL applies verbatim.

**2. Effort sweep (`BackgroundSweep`) — daemon-scheduled, reading a pushed in-memory snapshot.** New method `coordination.put_launch_settings { version, cli_commands, tmux_layout }`; the app pushes it post-commit from `update_settings` and on every daemon (re)connect; the daemon holds highest-version-wins in memory. Version = the app's settings-save counter. The shipped follow-up widens the sweep to TaskChanged semantics: every 30 s cycle may start an owed switch as well as retry a recorded failure, closing starvation when the app task edge never fires. The per-team scan and three-attempt budget remain the bounds.

- **Skip-when-absent, never default**: a daemon that has never received a push does not render relaunches. Rendering from `CliCommandSettings::default()` is the memorialized `claude2` regression — a stock-default relaunch moved a member off the account its alias pinned. Skipping loses nothing: the retry is durable in the runtime record's `effort_resume_failure`, and the app's first connect unblocks it. One bounded `effort.sweep.awaiting_settings` record, not a per-cycle warn.
- **No disk persistence.** On Windows the daemon's data dir is drvfs (`/mnt/c/...`), so a persisted snapshot would revive exactly the cross-filesystem semantics this phase exists to kill. The daemon's 600 s idle shutdown bounds the missing-snapshot window to minutes; persistence becomes the natural increment only if daemon lifetime is later extended.
- Staleness semantics are benign by construction: settings change only while the app runs, the app pushes immediately on change, and a pass firing inside the reconnect window renders from the previous *committed* document — the same exposure today's pass has reading the DB a moment before the operator clicks Save. Settings saves are whole-document, so no torn state is observable.

**3. Task-arrival effort trigger (`TaskChanged`) — stays app-driven as a self-contained intent.** The edge detector is the app's task-scan signature diff over its own DB; it becomes `coordination.apply_task_effort { project_path, cli_commands, tmux_layout }` on the established accept-then-poll run-registry shape, carrying fresh config per invocation (the protocol 16–19 pattern). The daemon does not learn to watch the tasks dir or duplicate that edge detector. The app trigger remains the earliest path; bundled mesh 0.2.28 holds a mismatched notice on `appliedEffort`, and the widened daemon sweep is the bounded fallback that starts an owed switch when the app edge is absent.

The daemon-side sweep calls the member-activation resume pipeline that already lives in the daemon (protocols 17/18) — no pipeline code moves, only its caller. The alias probe keeps its contract (resolve only when a member will actually be relaunched; idle passes never probe a shell) because the daemon-side pass calls the local resolver at the same point.

## Wire contract

- `PROTOCOL_VERSION` 20 → 21 (20 is the in-flight stop_member retirement; the two slices must not share a bump — each lands atomically via the exact-match gate and repair flow).
- New methods: `coordination.put_launch_settings` (fire-and-ack, no run), `coordination.apply_task_effort` + `_status` (run-registry shape, shared registry).
- App-side deletions with removal pins: `spawn_coordination_self_heal_monitor` and the app-side pass entry points (`run_background_self_heal_pass*`, `apply_task_effort_after_task_change`'s in-process execution), mirroring how the deadline arm was deleted at 15.

## Consequences to say out loud

- **Daemon lifetime**: with the app closed, the daemon idles out after ~10 minutes, so "app-closed self-heal" is a ≤10-minute tail — B1b inherits B1a's answer (passes run only while the daemon lives) and deliberately does not change the idle timeout. Extending daemon lifetime while teams are live is a separate, later decision; this design leaves it a clean increment (add persistence to the snapshot, nothing else changes).
- **B3 preview** (scoped by this research): after B1b, the remaining app-side writers under `teams_dir()` are the task-scan operational-snapshot sync (needs SQLite → becomes an RPC carrying prepared snapshots, as initialize already does), the live-status presence/active-team writes, and the WSL-native hook/CLI processes (which stay — they are not the 9p problem). That is the B3 retirement list; the assertion lands when it is empty of app-process entries.

## Deliberately not building

- Any daemon read of the app database — never.
- Snapshot persistence to disk (revisit only with a daemon-lifetime change).
- Daemon-side task watching / a second `TaskChanged` edge detector.
- A work queue drained by the app — runtime `appliedEffort` plus assignment state already exposes owed work to the daemon sweep, and app-side execution would reinstate the 9p writers.
- Changes to the effort state machine itself (attempt budget, held-task selection, pre-check-before-stop all move verbatim).

## Tests

- Deadline-pass-parity tests for the new scheduler arm: `self_heal.pass.completed/failed` emitted; the app-side monitor is gone (removal pins).
- Sweep-without-snapshot: no relaunch rendered, one `effort.sweep.awaiting_settings` record, retry state untouched; push then sweep → a new owed switch or retry renders from pushed settings (assert the pushed base, not a default, reaches the render — the `claude2` regression pin).
- Version monotonicity: an older push never overwrites a newer snapshot.
- `apply_task_effort` intent round-trip on the run registry; protocol pins for 21; existing effort pipeline tests green untouched (the state machine is unmoved).
- Daemon suites single-threaded in gates.

## Sizing and order

A taurhaus `feature-pr` lane, **after the protocol-20 lane merges** (both touch `protocol.rs`, the run registry, and `commands/coordination.rs`). Architecture-bearing per the review doctrine: the lane gets the Fable altitude lens beside the two Opus lenses.
