# Resource Monitor Log Analysis

Source: `/tmp/taurhaus-resource-log.csv`

Window analyzed:
- Start: `2026-03-08T12:17:33+01:00`
- End: `2026-03-08T13:12:19+01:00`
- Duration: `54.8 minutes`
- Samples: `1635` data rows (`109` timestamps at ~30s intervals)

## Executive Summary

The log does not show a clear runaway memory leak, file descriptor leak, handle leak, or restart loop.

The one notable signal is `taurhaus-daemon`:
- `inotify_watches` climbed from `18,973` to a peak of `19,170` (`+197`) in a short burst around `12:25` to `12:35`, then stayed flat.
- `rss_mb` climbed from `22.8 MB` to `26.54 MB` at the end, peaking at `26.79 MB` (`+3.99 MB` peak over start).
- CPU spikes for `taurhaus-daemon` reached `8.07%`, also during that same watch-growth window.

That pattern looks more like bounded workload growth tied to watcher expansion than an obvious unbounded leak. It is still the only thing in the sample worth follow-up if the expected steady-state watch count should have remained constant.

## Process-by-Process Findings

### `taurhaus-daemon` (`pid 3673048`)

- Lifetime: stable for the full capture, no PID change
- RSS: `22.8 MB -> 26.54 MB`, min `22.23 MB`, max `26.79 MB`
- RSS slope: about `+0.056 MB/min`
- Threads: constant at `20`
- Open FDs: oscillated narrowly between `53` and `55`
- inotify watches: `18,973 -> 19,170`, peak `19,170`
- Watch slope: about `+3.64 watches/min` across the full sample, but the actual growth happened in one short burst and then flattened
- CPU: mostly low single digits, peak `8.07%`

Largest watch increases:
- `12:25:09 -> 12:25:39`: `+36`
- `12:31:14 -> 12:31:45`: `+30`
- `12:33:47 -> 12:34:17`: `+35`

Largest RSS jumps:
- `12:35:48 -> 12:36:19`: `+3.31 MB`
- `12:37:50 -> 12:38:20`: `+1.75 MB`
- `12:39:21 -> 12:39:52`: `+1.50 MB`

Interpretation:
- No evidence of thread leak
- No evidence of FD leak
- No restart behavior
- The watch-count increase is real, but it stabilizes
- RSS growth tracks the same period and then plateaus in a narrow `~24.5 MB` to `26.8 MB` band

This should be treated as a possible watcher-reconciliation or project-scan expansion effect, not yet as a confirmed leak.

### `taurhaus.exe` (`pid 26796`)

- Lifetime: stable for the full capture, no PID change
- RSS: `48.61 MB -> 33.43 MB`, min `33.05 MB`, max `53.55 MB`
- RSS trend: downward overall, not leak-like
- Threads: constant at `71`
- Handles: constant at `443`
- CPU: extremely low, peak `0.12%`

Interpretation:
- Healthy
- No memory leak signal
- No handle leak signal
- No thread growth

### `mesh` processes (`13` stable PIDs)

Tracked PIDs:
- `1514963`
- `2126015`
- `2126793`
- `2127090`
- `2127455`
- `2127826`
- `2128142`
- `2128468`
- `3647139`
- `3697252`
- `3697396`
- `3697589`
- `3697822`

Common behavior:
- All `13` PIDs were present for the full capture window
- No mesh PID changes or restarts during the sample
- Most RSS values were effectively flat
- Typical RSS per process stayed around `3.25 MB` to `5.5 MB`
- Threads were constant at `3` for most processes, `1` for two lighter processes
- Open FDs were constant (`11` on most, `5` on the lighter processes)
- inotify watches were constant (`2` on most, `0` on the lighter processes)
- Per-process CPU peaks were around `1%`; aggregate mesh CPU peaked at `8.19%`

Only small deviation:
- `mesh pid 3647139` rose from `4.5 MB` to `5.1 MB`, peaking at `5.54 MB`
- That is still small in absolute terms and not supported by FD/thread/watch growth

Interpretation:
- Healthy
- No sign of leak or churn
- The number of mesh processes is notable but stable; this looks intentional rather than pathological

## Restarts / Lifecycle Anomalies

No process restarts were observed inside the capture window:
- `taurhaus-daemon`: one PID throughout
- `taurhaus.exe`: one PID throughout
- `mesh`: all `13` PIDs were stable throughout the full sample

## Leak Assessment

### Memory leak

No clear unbounded memory leak is visible in this window.

What is visible:
- `taurhaus-daemon` grows by roughly `3.7 MB` net and about `4 MB` peak-over-start during a watcher-growth burst
- `taurhaus.exe` trends downward
- `mesh` is effectively flat

Conclusion:
- No confirmed leak
- `taurhaus-daemon` merits follow-up only if watcher count was expected to remain constant

### CPU spikes

Notable sustained activity is limited to `taurhaus-daemon`:
- `12:25:39`: `8.07%`
- `12:31:45`: `7.58%`
- `12:35:48`: `7.24%`
- `12:26:40`: `6.46%`
- `12:19:34`: `5.44%`

These are spikes, not a high sustained baseline. The daemon otherwise sits roughly around `2%` to `4%`.

### File descriptor / handle leak

No leak signal:
- `taurhaus-daemon` FDs stayed in a tight `53` to `55` range
- `taurhaus.exe` handles stayed flat at `443`
- `mesh` FDs were flat

### inotify watch growth

This is the main anomaly:
- `taurhaus-daemon` went from `18,973` to `19,170`
- Peak: `19,170`
- Growth concentrated in a short interval, then stable afterward

That is not an unbounded trend in this sample, but it is the clearest candidate for a watcher-lifecycle bug if those additional watches should have been cleaned up.

### Thread growth

No thread leak signal:
- `taurhaus-daemon`: constant `20`
- `taurhaus.exe`: constant `71`
- `mesh`: constant `1` or `3`, depending on process

## Bottom Line

The system looks broadly healthy over this 55-minute capture.

Nothing in the log shows:
- a runaway RSS leak
- unbounded FD growth
- handle growth
- thread growth
- restart churn

The only item worth flagging is bounded growth in `taurhaus-daemon` watcher count, accompanied by a modest RSS increase and brief CPU spikes. That looks like a watcher expansion event rather than a classic leak, but it is still the best candidate for a real bug if the daemon should have returned to its earlier watch count after the underlying activity finished.
