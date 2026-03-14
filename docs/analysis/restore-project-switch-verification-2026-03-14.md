# Restore/Project-Switch Verification 2026-03-14

Task: `#1298`  
Epic: `#1294`  
Scope: verify the restore-time project-switch stall follow-up across minimize/restore-adjacent freshness, deferred project loading, daemon recovery, and rapid project switching.

## Result

Bounded verification passed.

- The targeted restore-path regression suites passed.
- `just check-quick` passed on top of the current dirty worktree.
- One remaining helper gap was confirmed and fixed during verification: a superseded debounced selection batch could survive when the next selection reused a hovered project's prefetched in-flight batch.

## Verification Matrix

| Area | Coverage | Evidence | Result |
| --- | --- | --- | --- |
| Background freshness after minimize/restore-style hide/show | `src/lib/shell/events.test.js` | `setupSessionPollingLifecycle` pauses fallback polling when hidden and resumes it when visible again | Passed |
| Daemon recovery state classification | `src/lib/daemonStatus.test.js` | startup/busy/disconnected/reconnecting states still classify correctly for deferred retry behavior | Passed |
| Deferred project-load retry after reconnect | `src/Shell.meshFocus.test.js` | added `retries a deferred project load after daemon reconnects during a project switch` | Passed |
| Rapid switch coalescing | `src/lib/projectSelection.test.js` | existing `coalesces rapid project switches so only the final IPC batch starts` | Passed |
| Hover prefetch reuse on later selection | `src/lib/projectSelection.test.js` | existing `prefetches a project batch and reuses it for the subsequent selection` | Passed |
| Prefetch reuse with an older debounced switch still pending | `src/lib/projectSelection.test.js` | added `cancels a superseded debounced batch when the next selection reuses a prefetched project` | Passed |
| Full repo quick gate | repo quick lane | `just check-quick` | Passed |

## Changes Made

### Regression coverage

- `src/Shell.meshFocus.test.js`
  - updated the Shell selection mock surface to the current deferred-loading entrypoint
  - added a reconnect regression proving retryable daemon-backed section failures stay hidden during recovery and automatically retry once the daemon reconnects
- `src/lib/projectSelection.test.js`
  - added a regression proving a stale debounced selection batch is canceled when the user lands on a hovered project whose deferred batch is already in flight

### Production fix found during verification

- `src/lib/projectSelection.js`
  - added `resolveScheduledSelectionBatchWith(...)`
  - when `loadProjectSelectionData(...)` reuses an in-flight request, any older scheduled batch is now canceled and its waiters are resolved from the reused request instead of firing a stale extra IPC batch later

## Commands Run

```bash
bun run test src/lib/projectSelection.test.js src/Shell.meshFocus.test.js
bun run test src/lib/shell/events.test.js src/lib/daemonStatus.test.js
just check-quick
```

## Remaining Risks

1. There is still no real native window minimize/unminimize integration test. Restore is verified through the visibility/polling path plus daemon-status events, not through a Tauri window harness.
2. The current verification is frontend-heavy. It confirms the deferred project-selection contract and quick gate, but not a real WSL daemon-backed restore under live contention.
3. The new helper regression removed an avoidable extra selection batch, but real restore latency still depends on live provider timing outside unit tests.
