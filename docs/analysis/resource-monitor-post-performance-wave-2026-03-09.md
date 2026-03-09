# Resource Monitor Analysis (Post-Performance Wave)

Date: 2026-03-09
Source data:
- `/tmp/taurhaus-resource-monitor-v2.csv`
- fresh live sample from `scripts/resource-monitor.py`

Context:
- performance wave at commit `a11c347`
- taurhaus `0.5.8`
- mesh `0.2.8`

## Current Snapshot

Fresh two-sample monitor capture at `2026-03-09T19:35:17+01:00`:

- `taurhaus.exe` (`pid 48768`): `0.06%` CPU, `43.17 MB` RSS, `74` threads, `462` handles
- `taurhaus-daemon` (`pid 2161015`): `15.39%` CPU, `73.48 MB` RSS, `67` threads, `175` FDs, `75,558` inotify watches
- `mesh` processes (`13` total):
  - aggregate CPU: `0.00%`
  - aggregate RSS: `64.63 MB`
  - aggregate threads: `37`
  - aggregate FDs: `136`
  - aggregate inotify watches: `24`
  - per-process RSS range: about `3.71 MB` to `6.03 MB`

## Last 10 Minutes

Window end: `2026-03-09T19:34:27+01:00`

### `taurhaus.exe`

- CPU avg/min/max: `0.03 / 0.00 / 0.44%`
- RSS avg/min/max: `47.99 / 42.11 / 49.96 MB`
- threads avg/min/max: `74.23 / 72 / 76`
- handles avg/min/max: `460.08 / 455 / 470`
- RSS drift over window: `42.11 MB` -> `43.05 MB` (`+0.96 MB`)

Assessment:
- healthy
- effectively idle on CPU
- memory is flat
- handle count is stable

### `taurhaus-daemon`

- CPU avg/min/max: `14.99 / 2.30 / 107.61%`
- RSS avg/min/max: `62.90 / 55.50 / 75.18 MB`
- threads avg/min/max: `64.40 / 50 / 67`
- FDs avg/min/max: `167.50 / 128 / 178`
- inotify watches avg/min/max: `69,621 / 49,128 / 75,558`
- RSS drift over window: `56.25 MB` -> `72.57 MB` (`+16.32 MB`)

First five minutes vs last five minutes:

- CPU avg: `13.50%` -> `16.40%`
- RSS avg: `58.23 MB` -> `69.85 MB`

Peak CPU moments:

- `107.61%` at `19:30:22`
- `83.44%` at `19:32:32`
- `63.35%` at `19:32:29`

Assessment:
- this is the main concern
- sustained `~15%` CPU for an apparently idle desktop app is high
- multiple bursts above `60%`, including one above a full core, do not look like quiet-background behavior
- memory is not exploding, but a `+16 MB` rise in 10 minutes after startup is enough to watch closely
- thread and FD counts are elevated but roughly stable; the very high inotify-watch count is likely workload-driven, but it should be treated as intentional only if the project set really justifies `~75k` watches

### `mesh` processes

Aggregate per-timestamp over the last 10 minutes:

- process count avg/min/max: `12.15 / 12 / 13`
- CPU avg/min/max: `2.16 / 0.00 / 34.42%`
- RSS avg/min/max: `59.48 / 58.63 / 64.63 MB`
- threads avg/min/max: `36.15 / 36 / 37`
- FDs avg/min/max: `132.59 / 132 / 136`
- inotify watches avg/min/max: `24 / 24 / 24`
- aggregate RSS drift: `58.63 MB` -> `64.63 MB` (`+6.00 MB`)

First five minutes vs last five minutes:

- CPU avg: `1.74%` -> `2.65%`
- RSS avg: `58.63 MB` -> `61.15 MB`
- process count avg: `12.00` -> `12.43`

Assessment:
- acceptable
- there are many `mesh` processes, but each is small and currently idle
- aggregate RSS around `60-65 MB` across `12-13` processes is not alarming
- occasional aggregate CPU spikes happen, but the baseline is low and the current sample is `0%`

## Overall Read

What looks good:

- `taurhaus.exe` itself is behaving well
- `mesh` `0.2.8` does not show a resource regression from the JSONL projection work
- no obvious runaway thread or FD growth outside the daemon

What looks problematic:

- `taurhaus-daemon` is the only likely regression candidate after the performance wave
- current daemon CPU (`15.39%`) is too high for an idle-ish steady state
- daemon CPU spikes above `100%` suggest periodic expensive work rather than one-time startup cost
- daemon RSS is still climbing upward over the observed window, even if not at leak-grade velocity yet

Recommended follow-up:

1. Inspect which daemon loop is firing around the `19:30` and `19:32` spikes.
2. Correlate the spikes with runtime session snapshot, Codex transcript binding, and watcher/projection activity in `taurhaus.log.jsonl`.
3. Re-run the monitor after a longer fully idle soak (30-60 minutes). If daemon RSS keeps climbing at the same slope, treat it as a memory-growth regression.
