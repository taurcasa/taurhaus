# Session Stale Presence Implementation

## Objective

Implement stale-but-retained session presence across short daemon gaps so Taurhaus no longer treats the last known session snapshot as either fully live or fully gone during a transient daemon interruption.

## State Model

The implementation now distinguishes two session-presence freshness states:

- `live`: the snapshot came from a fresh daemon or polling refresh
- `stale`: the snapshot is retained from the last known good refresh after the daemon bridge reported a gap

This is intentionally a freshness flag layered on top of the existing session `state` (`active` / `idle`), not a replacement for it.

That means a retained active session is now represented as:

- execution state: `active`
- presence freshness: `stale`

instead of being flattened into either:

- still fully live, which was misleading, or
- cleared entirely, which was too destructive for short gaps

## Coherent Path

### 1. Session store owns freshness annotations

Changed:

- `src/lib/sessionStore.svelte.js`

New behavior:

- all fresh snapshots stamp each session with:
  - `_presenceStatus = 'live'`
  - `_presenceStale = false`
  - `_presenceUpdatedAt = <timestamp>`
- new `markSessionPresenceStale()` rewrites the currently retained snapshot in place as:
  - `_presenceStatus = 'stale'`
  - `_presenceStale = true`

This keeps the stale/live distinction attached to the canonical frontend session data instead of scattering it across components.

### 2. Shell applies stale transition on daemon gap

Changed:

- `src/Shell.svelte`

New behavior:

- when `daemon-status` is anything other than `connected`
  - `sessionBridgeLive = false`
  - `markSessionPresenceStale()` is called
- when a fresh `sessions-updated` payload arrives
  - `sessionBridgeLive = true`
  - `applyDaemonSessionUpdate(...)` clears stale state by stamping the new snapshot as `live`

This keeps the stale transition aligned with the actual daemon bridge lifecycle.

### 3. Polling fallback can restore freshness

No special-case UI logic was added for polling.

Because `startPolling()` already resumes when the bridge is not live, any successful polling refresh now naturally clears stale presence through the same `applySessions(...)` path in the session store.

That is the correct behavior:

- failed polling during a daemon gap keeps the retained stale snapshot
- successful polling upgrades the snapshot back to fresh

## Visual Behavior

Changed:

- `src/lib/sessionIndicator.js`
- `src/lib/SidebarProjectList.svelte`
- `src/app.css`

New semantics:

- stale retained presence uses explicit stale styling instead of active/idle styling
- sidebar row tint softens from live tint to a weaker retained tint
- single-session indicators and hover badges expose stale presence via:
  - `session-pill-stale`
  - `text-info-300`
  - aria labels that include `retained stale`
- grouped mesh-team indicators now surface a `stale` tone when the retained team snapshot is stale

This keeps presence visible during short gaps while making it clear that the snapshot is not current.

## Why This Is Better

The old model had an honesty problem:

- retaining the old session icons made them look fresh
- clearing them immediately made short daemon gaps look like sessions vanished

The new model resolves that tradeoff directly:

- short daemon gaps retain operator context
- the retained context is visibly marked as stale
- any fresh session refresh clears the stale state automatically

## Verification

Ran:

- `bunx vitest run src/lib/sessionStore.test.js`
- `bunx vitest run src/lib/sessionIndicator.test.js`
- `bunx vitest run src/lib/Sidebar.component.test.js src/Shell.meshFocus.test.js`
- `bun run typecheck`

Results:

- all targeted tests passed
- `svelte-check` reported `0 errors` and `0 warnings`

## Touched Files

- `src/Shell.svelte`
- `src/app.css`
- `src/lib/SidebarProjectList.svelte`
- `src/lib/sessionIndicator.js`
- `src/lib/sessionStore.svelte.js`
- `src/Shell.meshFocus.test.js`
- `src/lib/sessionIndicator.test.js`
- `src/lib/sessionStore.test.js`

## Remaining Risk

This slice intentionally stays on the frontend/session-state boundary.

It does not change:

- backend daemon recovery logic
- daemon-status production rules
- long-gap UX policy beyond the retained stale treatment

If the next issue is that stale presence persists too long after a bridge failure, the next adjustment should be a bounded freshness timeout or a richer stale timestamp presentation, not a return to hard-clearing the snapshot.
