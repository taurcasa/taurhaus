# Resource Monitor Assessment — 2026-03-11

Primary data source: `/tmp/taurhaus-resource-monitor-v2.csv`

Supporting references:
- [inotify-instance-audit-2026-03-10.md](/home/mstie/projects/taurhaus/docs/analysis/inotify-instance-audit-2026-03-10.md)
- [daemon/watch.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/watch.rs)
- [daemon/compaction.rs](/home/mstie/projects/taurhaus/src-tauri/src/daemon/compaction.rs)
- [session_scanner/compaction_watcher.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/compaction_watcher.rs)
- [session_scanner/compaction_extractor.rs](/home/mstie/projects/taurhaus/src-tauri/src/session_scanner/compaction_extractor.rs)
- [startup/watchers.rs](/home/mstie/projects/taurhaus/src-tauri/src/startup/watchers.rs)
- [fs/watcher.rs](/home/mstie/projects/taurhaus/src-tauri/src/fs/watcher.rs)

## Executive summary

Current runtime health looks good.

- The old inotify-instance exhaustion shape is gone.
- The current daemon is stable at `7` inotify instances and `714` watch descriptors.
- System-wide inotify usage is no longer close to the kernel limit:
  - instances: `64 / 512` (`12.5%`)
  - watches: about `31.5k / 524,288` (`~6.0%`)
- `taurhaus-daemon` is still the dominant CPU consumer, but in this capture it is steady rather than runaway:
  - median CPU about `12.7%`
  - median RSS about `25.7 MB`
  - threads flat at `21`
  - open FDs flat at `33`
- `taurhaus.exe` looks stable:
  - median CPU `0.02%`
  - median RSS `42.2 MB`
  - threads `57-58`
  - handles `583-586`
- `mesh` daemons also look healthy:
  - most long-lived processes sit at `2` inotify instances / `2` watches with very low CPU

No code fix was required from this assessment. The current runtime evidence does not show missing watches, instance leakage, or a fresh stability regression after `#908 -> #921`.

## Scope and capture window

The CSV spans:

- first sample: `2026-03-10T21:48:59+01:00`
- last sample: `2026-03-11T01:33:25+01:00`

Rows by process family:

- `mesh`: `83,362`
- `taurhaus-daemon`: `4,657`
- `taurhaus.exe`: `4,476`

This window includes both:

1. pre-fix daemon runs that still showed the old duplicated-watch pathology
2. the current post-fix daemon/app pair that settled into the new healthy shape

So the correct reading is not the whole-window average. It is the latest steady-state segment.

## 1. Inotify watch-count assessment

### Current answer

`714` watches is plausible for the current watch architecture. It does not look like watches are missing.

### Why the current `714` is not suspicious

The latest stable daemon PID in the capture is:

- `taurhaus-daemon` PID `3139878`
- lifetime in capture: `2026-03-11T00:57:02+01:00` -> `2026-03-11T01:33:25+01:00`

Its inotify stats are flat for the entire segment:

- instances: exactly `7`
- watches: `714-715`, ending at `714`

Direct `/proc/3139878/fdinfo/*` inspection shows those `7` inotify file descriptors break down like this:

| FD | Watch count | Likely owner |
| --- | ---: | --- |
| `21` | `634` | shared daemon project/tasks watcher |
| `23` | `63` | recursive team-topology watcher |
| `6` | `10` | transcript extractor watcher |
| `12` | `2` | team signal watcher |
| `19` | `2` | team signal watcher |
| `9` | `2` | team signal watcher |
| `13` | `1` | team signal watcher |

That sums to `714`.

This is the important result:

- one large shared watcher now owns the ordinary project-tree watch set
- the remaining `80` descriptors are the fixed compaction/topology infrastructure
- the old failure mode was many duplicated ordinary watcher instances
- that duplication is absent here

### Relation to the current project set

The live Windows DB snapshot shows:

- `20` total projects
- thresholds:
  - `active_days = 4`
  - `recent_days = 12`
- `17` active projects
- `0` recent projects
- `3` stale projects

So the daemon currently needs to watch only the active WSL projects plus the `.claude/tasks` path and compaction infrastructure.

A rough filesystem walk without `.gitignore` pruning produced `4,184` candidate directories across the `17` active WSL projects. That intentionally overcounts because it includes directories that the actual watcher logic prunes away. The live daemon’s real shared watcher count of `634` directories is therefore not “too low”; it is exactly what we should expect once:

1. activity thresholds narrow the watched set
2. the shared watcher collapses duplicate registrations
3. `.gitignore`/pre-pruning removes a large amount of subtree noise

### Comparison against the old broken shape

The first daemon in the capture, PID `726119`, still shows the old pathological state:

- `66` inotify instances
- `3,284` watches

That is the pre-fix duplicated-listener era described in [inotify-instance-audit-2026-03-10.md](/home/mstie/projects/taurhaus/docs/analysis/inotify-instance-audit-2026-03-10.md).

The current steady state is:

- `7` instances
- `714` watches

That is the exact kind of reduction the earlier fixes were meant to achieve.

## 2. Performance assessment

### Current daemon (`taurhaus-daemon` PID `3139878`)

Steady-state metrics for the latest daemon PID:

| Metric | Min | Median | Max | Last |
| --- | ---: | ---: | ---: | ---: |
| CPU % | `1.56` | `12.67` | `33.62` | `14.42` |
| RSS MB | `17.0` | `25.72` | `29.26` | `27.97` |
| Threads | `21` | `21` | `22` | `21` |
| Open FDs | `33` | `33` | `39` | `34` |
| Inotify instances | `7` | `7` | `7` | `7` |
| Inotify watches | `714` | `714` | `715` | `714` |

Assessment:

- CPU is still non-trivial, but it is stable, not runaway.
- Memory is stable.
- Thread count is flat.
- FD count is flat.
- Inotify counts are flat.

This does not look like a leak or churn pattern.

### Current app (`taurhaus.exe` PID `27664`)

Steady-state metrics for the latest app PID:

| Metric | Min | Median | Max | Last |
| --- | ---: | ---: | ---: | ---: |
| CPU % | `0.0` | `0.02` | `2.99` | `0.05` |
| RSS MB | `23.46` | `42.16` | `54.12` | `35.18` |
| Threads | `11` | `58` | `64` | `57` |
| Handles | `306` | `586` | `602` | `583` |

Assessment:

- CPU is effectively idle most of the time.
- RSS has a normal interactive range and ends below the median.
- Thread and handle counts are stable.
- Nothing here suggests the recent task-management work left the app in a degraded runtime state.

### Mesh daemons

Long-lived mesh daemons mostly sit at:

- `2` inotify instances
- `2` watches
- RSS around `2-6 MB`
- CPU near zero except brief spikes

There are many short-lived mesh PIDs in the full window, but the long-lived active ones look healthy. This matches earlier mesh resource audits.

## 3. Stability assessment after the recent fix wave

The recent sequence `#908 -> #921` touched:

- compaction signal handling
- watcher architecture
- daemon watch ownership
- task query/request paths
- UI refresh behavior

The resource monitor data suggests the system is now in a healthier state than before those fixes.

### Evidence for improved stability

1. **No return to high instance pressure**
- latest daemon: `7` instances
- latest system-wide user total: `64 / 512`
- this is far from the old `123 / 128` failure mode

2. **No watch-descriptor runaway**
- latest daemon: flat at `714`
- no monotonic growth across the final `~36` minutes of the capture

3. **No FD/thread leak in the daemon**
- FDs remain around `33-34`
- threads remain at `21`

4. **No app-process thrash in the current segment**
- app CPU low
- handles stable
- RSS not trending upward

### What the full window still shows

The full capture contains many daemon/app PIDs because the system was being rebuilt, restarted, and reinstalled during the troubleshooting session. Those restarts are real, but they should not be interpreted as current runtime flakiness by themselves. The latest stable pair is the better signal for “is the current build healthy?”

My conclusion from the monitor data is:

- the current build looks materially healthier than the earlier runs in the same file
- the earlier large spikes are historical evidence of the fixed problems, not evidence that they are still present

## 4. Actionable findings

### Runtime health

No runtime fix is justified from this CSV alone.

The data does **not** support:

- missing daemon watches
- fresh inotify instance leakage
- fresh watch-descriptor leakage
- daemon FD/thread runaway
- app handle/thread runaway

### One remaining diagnostics gap

There is still an observability mismatch:

- the resource monitor and `/proc` show the live daemon at `7 / 714`
- but recent `inotify.telemetry` entries in `taurhaus.log.jsonl` only show the early empty-plan shape:
  - `daemon_listener_connections = 0`
  - `logical_watch_subscriptions = 0`
  - `physical_watch_registrations = 0`
  - `process_local_inotify_watch_descriptors = 80`

That means the current structured telemetry is not capturing the live post-plan steady state as clearly as the raw resource monitor does.

This is a diagnostics problem, not a runtime health problem. It does not justify an emergency fix in this task, but it is worth tracking separately because it makes future watch-health investigations harder than they should be.

## Conclusion

The current system state looks healthy.

- `714` daemon watches is not evidence of missing watches.
- It is the expected post-fix shared-watcher shape for the current active project set.
- The daemon’s current `7` instances / `714` watches are a major improvement over the earlier `66` / `3,284` broken state.
- System-wide inotify pressure is now low.
- The latest daemon/app pair shows stable CPU, memory, thread, FD, and handle behavior.

So the evidence-based answer for `#922` is:

- **watch health:** healthy
- **performance/stability:** healthy enough, no new concrete regression indicated by this monitor capture
- **code changes needed:** none from this assessment
- **follow-up worth tracking:** improve daemon inotify telemetry so logs reflect the live post-plan counts, not just the early startup state
