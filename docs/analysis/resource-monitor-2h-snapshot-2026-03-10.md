# Resource Monitor 2h Snapshot (2026-03-10)

Window analyzed: `2026-03-10 13:33:03+01:00` to `2026-03-10 15:33:03+01:00`

Data source: `/tmp/taurhaus-resource-monitor-v2.csv`

## Summary

The current build looks materially healthier than the earlier high-watch state.

- `taurhaus-daemon` inotify watches stayed in the `0 -> 3856` range, with the last 15 minutes flat at `3855`. That is still about a `95%` reduction from the old `~73k` level.
- `taurhaus-daemon` steady-state CPU is no longer the old runaway pattern, but it is still the hottest process. Over the last 15 minutes it averaged about `10%` CPU with no upward creep.
- `taurhaus.exe` is calm in steady state: low CPU, stable RSS, stable handle count.
- `mesh` instances are numerous but cheap. There was some PID churn, but no sign of a leak or sustained CPU pressure.

## Per-process summary

| Process | PIDs seen | CPU avg / max | RSS avg / max | Threads avg / max | FDs avg / max | inotify avg / max | Handles avg / max | Last-15m read |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| `taurhaus-daemon` | 5 | `10.45% / 459.32%` | `25.26 / 32.01 MB` | `84.73 / 98` | `228.86 / 270` | `3238.54 / 3856` | `-` | CPU `10.01%`, RSS `30.55 MB`, threads `98`, FDs `265.22`, inotify `3855` flat |
| `taurhaus.exe` | 4 | `0.05% / 3.01%` | `38.55 / 57.41 MB` | `58.51 / 66` | `-` | `-` | `418.48 / 442` | CPU `0.03%`, RSS `37.58 MB`, threads `59.06`, handles `421.33` |
| `mesh` | 12 | `0.05% / 50.03%` | `3.93 / 5.50 MB` | `2.68 / 3` | `10.05 / 11` | `1.68 / 2` | `-` | CPU `0.05%`, RSS `2.99 MB`, threads `2.67`, FDs `10`, inotify `1.67` |

## Process-level observations

### `taurhaus-daemon`

- There was an early restart cluster between roughly `13:36` and `13:46`, visible in the PID sequence: `3490598 -> 3492125 -> 3500177 -> 3500323 -> 3508294`.
- After `3508294` took over at `13:46:04`, the daemon stayed up for the rest of the window.
- The scary `459.32%` CPU spike was a short startup event on PID `3508294` at `14:41:57`, not a steady-state baseline.
- RSS, thread count, FD count, and watch count all grew during warm-up:
  - RSS `8.5 -> 30.02 MB`
  - threads `31 -> 98`
  - open FDs `69 -> 265`
  - inotify watches `706 -> 3855`
- That growth then flattened. In the last 15 minutes:
  - threads stayed pinned at `98`
  - inotify watches stayed pinned at `3855`
  - FDs stayed in a very tight `265-267` band
  - RSS stayed in a tight `29.13-31.77 MB` band

Assessment: no clear leak in the last 15 minutes; most growth is startup convergence plus watch registration, not continuous drift.

### `taurhaus.exe`

- Four PIDs appeared in the window, but only two were meaningful long-lived app runs:
  - `57620` from `13:45:59` to `14:40:56`
  - `54288` from `14:42:37` to the end of the window
- CPU stayed negligible in steady state.
- RSS and handles rose during each app warm-up, then leveled out.
- Last 15 minutes were stable:
  - CPU avg `0.03%`
  - RSS avg `37.58 MB`
  - handles avg `421.33`

Assessment: healthy. No memory or handle leak signal in this window.

### `mesh`

- 12 mesh PIDs appeared over the two-hour window.
- Nine were already present at window start and lived for the entire two hours.
- Three more long-lived mesh PIDs joined around `13:37-13:47` and then stayed up.
- Aggregate mesh CPU stayed trivial despite one-off spikes up to `50.03%` on a single PID.
- RSS stayed small across the fleet, mostly `2.25-5.50 MB` per process.
- FD count and inotify count were flat by design (`5-11` FDs, `0-2` watches depending on process role).

Assessment: there is process churn, but it is bounded and cheap. No leak pattern is visible.

## Anomalies and timing notes

1. Early daemon restart burst
   - The daemon restarted several times in the first ~13 minutes of the window before stabilizing on PID `3508294`.
   - That inflates whole-window averages and explains the noisy daemon CPU max.

2. App restart around `14:42`
   - `taurhaus.exe` switched from PID `57620` to `54288`.
   - Resource levels reset as expected after the restart, then restabilized.

3. Shared spike around `14:41:57`
   - Both `taurhaus-daemon` and at least one mesh process saw sharp short CPU spikes around `14:41:57`.
   - This looks like a coordinated workload event, not a sustained rise.

4. Inotify improvement held
   - The daemon maxed at `3856` watches, not anything close to the previous `~73k` level.
   - The current level is still above the rough `~2900` anecdotal target, but it is stable and dramatically lower than the pre-pruning state.

## Overall health assessment

The current build is in a much better place than the earlier high-watch/high-CPU regime.

- No convincing steady-state memory leak is visible.
- No convincing FD or handle leak is visible.
- Inotify watch usage is dramatically improved and stable.
- `taurhaus.exe` is healthy.
- `mesh` process cost is low.
- `taurhaus-daemon` remains the main process to watch, but the problem has shifted from obvious runaway behavior to startup convergence plus a still-elevated but stable steady-state CPU floor.

If more performance work is needed, the next target remains daemon steady-state CPU rather than memory, watch count, or mesh process count.
