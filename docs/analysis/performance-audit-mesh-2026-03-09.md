# Performance Audit: mesh Binary and mesh Daemons

Date: 2026-03-09

## Scope and Method

Data sources:
- Resource monitor CSV: `/tmp/taurhaus-resource-monitor-v2.csv`
- Code-level review of `/home/mstie/projects/mesh`
- Targeted local CLI timings against `/home/mstie/projects/mesh/target/release/mesh`

Method notes:
- The CSV contains `194,764` rows total, with `172,284` rows for `mesh` across `65` unique PIDs.
- Process-role attribution is partly inferred from runtime signatures plus source structure:
  - probable per-agent daemons: `3` threads, `11` open FDs, `2` inotify watches
  - probable team-daemon/background orchestrators: `1` thread, `5` open FDs, `0` inotify watches
- CLI timings below are median values from repeated `/usr/bin/time` runs on synthetic datasets. They are useful for slope and memory-growth analysis, not as microbenchmark-grade absolute latency claims.

## Measured Baselines

### Long-lived mesh processes from the live monitor CSV

Probable per-agent daemons (`3` threads / `11` FDs / `2` watches):
- `19` long-lived PIDs
- CPU: `p50 0.0%`, `p95 0.39%`, max observed `19.68%`
- RSS: `p50 4.71 MiB`, `p95 6.50 MiB`
- Interpretation: idle baseline is low, but there are short spikes during event handling

Probable team-daemon/background mesh processes (`1` thread / `5` FDs / `0` watches):
- `4` long-lived PIDs
- CPU: `p50 0.0%`, `p95 0.39%`, max observed `4.43%`
- RSS: `p50 3.75 MiB`, `p95 5.25 MiB`
- Interpretation: idle baseline is also low; no evidence of sustained busy-spin

Short-lived CLI processes:
- Mostly `1` thread with `3` to `5` FDs and no watches
- This matches the expected one-shot CLI command model

## CLI Command Profiling

Median local timings with the release binary:

| Operation | Dataset | Wall | User | Sys | Max RSS |
|---|---:|---:|---:|---:|---:|
| `mesh send bob hello` | empty recipient inbox, `3` members | `0.01s` | `0.00s` | `0.00s` | `3.5 MiB` |
| `mesh send bob hello` | recipient inbox `10,000` messages, `3` members | `0.02s` | `0.00s` | `0.00s` | `9.5 MiB` |
| `mesh send bob hello` | recipient inbox `100,000` messages, `3` members | `0.14s` | `0.05s` | `0.07s` | `65.5 MiB` |
| `mesh send bob hello` | empty recipient inbox, `200` members | `0.01s` | `0.00s` | `0.00s` | `3.8 MiB` |
| `mesh read --json` | inbox `100` messages | `0.00s` | `0.00s` | `0.00s` | `3.3 MiB` |
| `mesh read --json` | inbox `10,000` messages | `0.01s` | `0.00s` | `0.00s` | `8.8 MiB` |
| `mesh read --json` | inbox `100,000` messages | `0.09s` | `0.04s` | `0.05s` | `56.5 MiB` |
| `mesh who --json` | `3` members | `0.00s` | `0.00s` | `0.00s` | `3.3 MiB` |
| `mesh who --json` | `200` members | `0.00s` | `0.00s` | `0.00s` | `3.5 MiB` |
| `mesh who --json` | `1,000` members | `0.00s` | `0.00s` | `0.00s` | `4.0 MiB` |
| `mesh task assign 1 --owner bob` | `10` tasks | `0.02s` | `0.00s` | `0.00s` | `3.5 MiB` |
| `mesh task assign 1 --owner bob` | `1,000` tasks | `0.01s` | `0.00s` | `0.00s` | `3.5 MiB` |
| `mesh task assign 1 --owner bob` | `10,000` tasks | `0.02s` | `0.00s` | `0.00s` | `3.5 MiB` |
| `mesh tasks --all --json` | `10` tasks | `0.00s` | `0.00s` | `0.00s` | `3.3 MiB` |
| `mesh tasks --all --json` | `10,000` tasks | `0.11s` | `0.02s` | `0.08s` | `8.5 MiB` |

Key measured takeaways:
- Member-count growth is currently cheap for `who` and `send` at the tested scales.
- Inbox-size growth is the dominant CLI scaling problem.
- `task assign` stays flat with task count because it updates one task file, not the whole task directory.
- `tasks --all` scales with task count because it scans and parses every task JSON file.

## Findings

### High: inbox operations are full-file parse and rewrite hot paths

Evidence:
- `read_inbox()` reads and parses the whole JSON array: `src/inbox.rs:43-56`
- `append_message()` reads, parses, appends, and rewrites the whole JSON file: `src/inbox.rs:63-74`
- `cmd_read()` loads the whole inbox before filtering or slicing: `src/main.rs:494-563`
- `cmd_send()` appends through the same path: `src/main.rs:393-423`

Measured impact:
- `send` grows from `3.5 MiB` RSS on an empty inbox to `65.5 MiB` on a `100,000` message inbox
- `read --json` grows from `3.3 MiB` RSS at `100` messages to `56.5 MiB` at `100,000` messages
- Wall time remains acceptable in the benchmark, but the memory slope is steep and will amplify lock hold time under concurrency

Why it matters:
- Every append rewrites the mailbox, so larger inboxes increase both latency and write amplification.
- This is the main CLI scaling limiter in the current design.

Recommended improvements:
1. Move inbox storage to an append-only journal plus a compacted read view.
2. If the format must stay JSON-array compatible, add aggressive inbox rotation or archival once size thresholds are crossed.
3. Keep hot-path appends away from rewriting already-delivered history.

### Medium: every task-directory change wakes every agent daemon and triggers a full task scan

Evidence:
- Each agent daemon watches the entire team task directory: `src/daemon.rs:713-716`
- On a task event, it calls `check_tasks()`: `src/daemon.rs:741-745` (event loop) and `src/daemon.rs:615-633`
- `check_tasks()` calls `list_tasks()` which scans and parses every task JSON file in the directory: `src/tasks.rs:19-66`

Measured impact:
- `mesh tasks --all --json` rises from `3.3 MiB` / `0.00s` at `10` tasks to `8.5 MiB` / `0.11s` at `10,000` tasks.
- `task assign` itself stays cheap, but one assign can still wake all agent daemons and make each of them rescan the task directory.

Why it matters:
- This is an N-daemon multiplier, not just a single-process cost.
- In larger teams, one task mutation can fan out into many redundant directory scans.

Recommended improvements:
1. Stop driving assignment detection from full task-directory rescans.
2. Reuse the task-mutation journal as the daemon trigger and consume only new entries.
3. If full scans remain, add per-owner filtering or cache task mtimes to avoid reparsing unchanged files.

### Medium: the team-daemon is light at idle, but it still does a fixed 1 Hz wake loop plus a full idle-monitor pass every 30 seconds

Evidence:
- `team-daemon` wakes once per second regardless of activity: `src/team_daemon.rs:48-80`
- Every idle-monitor cycle rereads config and all task files before iterating members: `src/idle_monitor.rs:187-197`
- The member loop loads per-member snapshots and runtime files and may touch marker files: `src/idle_monitor.rs:210-270`, `src/idle_monitor.rs:534-565`, `src/idle_monitor.rs:605-616`

Measured impact:
- Long-lived non-watch mesh processes still show `p50 0.0%` and `p95 0.39%` CPU, so this is not currently a burn-the-CPU problem.
- The issue is avoidable wakeups and repeated file I/O, not runaway idle cost.

Why it matters:
- The current baseline is acceptable, but the loop is structurally poll-driven.
- As teams and task counts grow, the 30-second full-team rescan becomes more expensive even if the 1-second sleep loop itself is cheap.

Recommended improvements:
1. Replace the 1 Hz loop with sleep-until-next-idle-poll or event-assisted wakeups.
2. Cache the last-seen task/config state across idle cycles.
3. Avoid per-member file rereads when snapshots or runtime files have not changed.

### Low: many nominally read-oriented CLI commands still rewrite `config.json` for implicit activity tracking

Evidence:
- `record_activity_implicit()` rewrites member activity in config: `src/main.rs:1563-1569`, `src/config.rs:115-128`
- It is called from `send`, `read`, `ack-status`, `tasks`, `task get`, `task create`, `task update`, and `task assign`: `src/main.rs:423`, `src/main.rs:566`, `src/main.rs:619`, `src/main.rs:1165`, `src/main.rs:1201`, `src/main.rs:1222`, `src/main.rs:1257`, `src/main.rs:1299`

Why it matters:
- This turns otherwise read-heavy or one-shot commands into config write traffic.
- It also means some commands do repeated config work: membership check(s) first, then an activity rewrite at the end.

Measured impact:
- Member-count growth is still cheap in current tests, so this is not urgent.
- It is still unnecessary write amplification and lock contention risk.

Recommended improvements:
1. Rate-limit implicit activity writes per member.
2. Move activity heartbeats to a separate append-only journal or lightweight per-member state file.
3. Reuse one config load in hot commands instead of rereading it for each membership check.

### Low: team-config reads are not the current bottleneck

Evidence:
- `list_members()` is a whole-config read, but the tested slopes remain flat: `src/config.rs:92-94`
- `cmd_who()` simply reads config and serializes active members: `src/main.rs:864-884`

Measured impact:
- `who --json` stayed effectively at `0.00s` from `3` members through `1,000` members.
- Max RSS only moved from `3.3 MiB` to `4.0 MiB`.

Why it matters:
- This is not where optimization effort should go first.

Recommended improvements:
- None urgent. Keep the current model until team sizes or config payloads grow materially.

## Hot Path Summary

Most CPU- and I/O-relevant code paths today:
1. Inbox append/read on large mailboxes: `src/inbox.rs:43-74`
2. Agent-daemon task rescans after any task directory event: `src/daemon.rs:615-633`, `src/tasks.rs:19-66`
3. Team-daemon idle-monitor polling loop: `src/team_daemon.rs:48-80`, `src/idle_monitor.rs:187-270`
4. Implicit activity rewrites after common CLI operations: `src/main.rs:1563-1569`, `src/config.rs:115-128`

## Improvement Recommendations by Expected Impact

1. Replace inbox JSON-array rewrites with append-only or rotated mailbox storage.
2. Replace per-daemon full task rescans with journal-driven or incremental assignment detection.
3. Make team-daemon idle monitoring sleep until next useful work instead of waking every second.
4. Decouple implicit activity tracking from full `config.json` rewrites.
5. Leave config/member enumeration alone until team sizes grow beyond current ranges.

## Bottom Line

No critical performance defect is visible in current live data. Idle mesh daemons are generally well-behaved and small.

The performance risks are structural and will show up first under growth, not under current idle load:
- large inboxes
- many task files
- many agent daemons all reacting to the same task mutation

If only one optimization gets funded, it should be inbox storage. That is the clearest measured slope in both latency and RSS.
