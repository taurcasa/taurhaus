# Taurhaus Usage Health Review

## Date

- 2026-03-13

## Evidence Reviewed

- Current resource monitor output: `/tmp/taurhaus-resource-monitor-v2.csv`
- Current Windows production log: `C:\Users\mstie\AppData\Roaming\com.taurhaus.dev\taurhaus.log.jsonl`
  - WSL path: `/mnt/c/Users/mstie/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl`

## Current Resource Snapshot

Monitor tail reviewed through `2026-03-13T15:23:08+01:00`.

### Last 15 minutes

#### `taurhaus-daemon`

- CPU: min `0.35%`, median `17.34%`, p95 `24.77%`, max `31.89%`
- RSS: min `25.96 MB`, median `26.96 MB`, p95 `27.96 MB`, max `28.71 MB`
- Threads: median `23`
- Open FDs: median `36`
- Inotify watches: median `717`, max `718`
- Latest sample (`15:23:07+01:00`): `1.41%` CPU, `26.96 MB` RSS, `23` threads, `37` FDs, `702` watches

#### `taurhaus.exe`

- CPU: min `0.00%`, median `0.04%`, p95 `0.15%`, max `0.25%`
- RSS: min `34.70 MB`, median `35.53 MB`, p95 `42.55 MB`, max `42.55 MB`
- Threads: median `56`, p95 `59`
- Handles: median `582`, max `591`
- Latest sample (`15:23:08+01:00`): `0.03%` CPU, `35.19 MB` RSS, `59` threads, `585` handles

#### `mesh`

- CPU: min `0.00%`, median `0.00%`, p95 `0.40%`, max `2.03%`
- RSS: min `4.75 MB`, median `5.97 MB`, p95 `7.79 MB`
- Threads: median `3`
- Open FDs: median `11`
- Inotify watches: median `4`

## Windows Log Findings

Latest log reviewed through `2026-03-13T14:20:43.162Z`.

### Warning/error slice in the last two hours

Only four warning-class records were present; there was no recurring error storm, crash loop, or sustained stall pattern.

1. `2026-03-13T13:43:03.308Z`
   - `startup.daemon_connect.deferred`
   - backend warning
   - fast-path daemon connect deferred after `2032 ms`

2. `2026-03-13T13:43:05.716Z`
   - `daemon.rpc.failed`
   - backend warning
   - method: `get_runtime_session_snapshot`
   - status: `error`
   - duration: `0 ms`

3. `2026-03-13T13:43:06.648Z`
   - `frontend.console.received`
   - frontend warning
   - message: startup viewport sync failed because `plugin:window|set_size` is not allowed by ACL

4. `2026-03-13T13:43:18.426Z`
   - `frontend.console.received`
   - frontend warning
   - message: Shiki fell back to plain rendering because `powershell` was not loaded

Additional lock-wait records in this slice were `DEBUG` only and had `wait_ms = 0`, so they do not indicate real contention.

## Assessment

### What looks healthy

- No current `taurhaus.exe` CPU, memory, or handle runaway is visible.
- No current `mesh` process explosion or sustained CPU problem is visible.
- The Windows production log is mostly quiet after startup; there is no live cascade of backend failures.

### What still stands out

- `taurhaus-daemon` remains the dominant background resource consumer.
- Its current profile is stable, but a median `17.34%` CPU over the last 15 minutes is still materially higher than the UI process and worth continued scrutiny.
- The latest Windows log still contains hidden startup warnings that are easy to miss in the UI:
  - daemon fast-path connect defer / one failed runtime snapshot RPC
  - forbidden window-size sync ACL call
  - missing `powershell` Shiki language registration

## Conclusion

There is no evidence of a current live Windows stall, crash loop, or resource runaway. The present state is broadly healthy, with one clear ongoing background cost and a few low-volume startup warnings.

## Follow-up Need

Yes. A follow-up fix task is warranted, but it should be narrow:

1. Remove or fix the Windows-only viewport sync call that currently fails ACL checks at startup.
2. Load/register `powershell` in the Shiki pipeline or explicitly downgrade that path so the fallback is expected rather than logged as a warning.
3. Keep daemon CPU under review; if users still report “idle but warm” behavior, the next fix target remains daemon steady-state scanning rather than the UI process.
