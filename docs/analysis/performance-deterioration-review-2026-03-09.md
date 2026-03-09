# Performance Deterioration Review — 2026-03-09

Task: `#826`

Review rule used for this pass:
If a performance commit made any user-visible behavior less correct, less fresh, or less capable, it is marked `YES` even if the tradeoff may still be acceptable overall.

## Summary Table

| Task | Commit | Deteriorates user-visible functionality? | Short reason |
| --- | --- | --- | --- |
| `#816` | `2db79a2` | `YES` | Idle session freshness is reduced after the scanner drops into 2s steady-idle cadence. |
| `#817` | `45473bd` | `NO` | Replaces fixed startup sleep with readiness polling; behavior is preserved and startup becomes more correct. |
| `#818` | `8f9beb3` | `YES` | Project bootstrap now lands critical data first and defers README/relationships/commits, so overview content can appear later. |
| `#819` | `6439e90` | `YES` | Live-status requests no longer self-heal stale team presence on demand, so Mesh status can stay wrong longer. |
| `#820` | `bde9be5` | `NO` | In this tree it only adds coverage for the deferred-loading behavior; no new runtime deterioration was introduced here. |
| `#821` | `7ce85fe` | `YES` | Codex idle/classification now prefers cached runtime transcript bindings, which can make attribution stale or wrong for a few seconds when bindings drift. |
| `#822` | `a59b345` | `YES` | Language support and highlighting coverage were intentionally reduced; large files now fall back sooner. |
| `#825` | `985784b` | `YES` | Foreground project and watcher reconciliation now prefer cached state, so UI truth can lag real session/project changes. |

## Detailed Review

### `#816` — `2db79a2`
`perf(daemon): split display-session liveness from expensive idle classification`

What changed:
- `SessionActivityHub` now keeps the main active cadence at `500ms`, but once all sessions have been stably idle it switches to a steady-idle cadence of `2s`.
- Full idle classification is no longer performed every cycle in that state; it uses a `FastIdle` mode and only forces a full pass every fourth steady-idle cycle.
- The fast path reuses previous display sessions and relies on lightweight process/tmux checks before falling back to a full rescan.

Deteriorates user-visible functionality? `YES`

What is worse for the user:
- After a team has been idle long enough to enter the steady-idle path, visible activity-state freshness is worse.
- New activity that is not caught by the light fast-idle checks can take up to the next full pass to appear, instead of being reclassified on the previous 500ms cadence.
- In practice this means sidebar/session activity can stay `idle` briefly after work resumed.

### `#817` — `45473bd`
`perf(startup): replace fixed 2s daemon reconnect sleep with readiness polling`

What changed:
- Removed the unconditional `sleep(Duration::from_secs(2))` during startup daemon reconnect.
- Startup now polls for actual daemon readiness and proceeds as soon as the daemon is reachable.

Deteriorates user-visible functionality? `NO`

What is worse for the user:
- None.
- This change removes artificial startup delay without weakening correctness; it waits for readiness instead of sleeping blindly.

### `#818` — `8f9beb3`
`perf(frontend): eliminate duplicate startup project selection`

What changed:
- Removed the second delayed startup `selectProject(...)` call.
- Startup selection now applies the project optimistically and loads only the critical data first.
- README, relationships, and commits are moved into a deferred second phase.

Deteriorates user-visible functionality? `YES`

What is worse for the user:
- Initial project bootstrap is no longer one fully-populated load.
- README, relationships, and recent commits can now arrive later, so the overview can appear partially populated immediately after selection.
- This is a real user-visible tradeoff even if the faster critical render is desirable overall.

### `#819` — `6439e90`
`perf(coordination): take reconciliation off the live-status request path`

What changed:
- Removed `reconcile_team_presence_for_live_status(...)` from the live-status request path.
- Live status now maps stored roster/runtime state directly instead of self-healing stale presence while serving the request.

Deteriorates user-visible functionality? `YES`

What is worse for the user:
- Mesh/runtime status can stay stale longer.
- A member that should have been corrected on demand can remain incorrectly shown as attached/healthy/offline until the background reconciliation path catches up.
- So the user can see wrong team presence at the moment they ask for fresh live status.

### `#820` — `bde9be5`
`perf(frontend): split project selection into critical and deferred loading`

What changed:
- In the current tree, this commit only adds test coverage for the deferred-loading behavior.
- The runtime behavior tradeoff was already introduced by the preceding frontend selection change.

Deteriorates user-visible functionality? `NO`

What is worse for the user:
- None from this commit itself.
- It validates the staged-loading behavior, but does not newly change runtime behavior in this tree.

### `#821` — `7ce85fe`
`perf(daemon): reduce Codex classifier cost with runtime transcript binding`

What changed:
- Codex idle detection now prefers Taurhaus runtime transcript attachments before doing broader project-scoped session discovery.
- Added a `5s` runtime attachment cache.
- When one authoritative managed attachment is found, the classifier can short-circuit to that binding.

Deteriorates user-visible functionality? `YES`

What is worse for the user:
- If the runtime attachment is stale or recently changed, Codex activity can be attributed to the wrong member/session for a short window.
- The new `5s` cache increases that staleness window.
- User-visible effect: wrong idle/active state, wrong member attribution, or wrong transcript binding can persist briefly after pane/session changes.

### `#822` — `a59b345`
`perf(frontend): reduce markdown/Shiki/Mermaid loading and rerender cost`

What changed:
- Narrowed language support to a small allowlist instead of broad bundled coverage.
- Added hard size cutoffs for markdown and code highlighting.
- Added cache-first rendering paths and more aggressive Mermaid gating.

Deteriorates user-visible functionality? `YES`

What is worse for the user:
- This is confirmed deterioration.
- Syntax highlighting coverage is reduced because support was narrowed from broad bundled coverage to roughly two dozen explicit languages.
- Large markdown/code files now lose rich highlighting sooner because of the hard line/character cutoffs.

### `#825` — `985784b`
`perf(backend): cache-first foreground lookup and watcher reconciliation`

What changed:
- Foreground project lookup now prefers cached tmux/session/project-path state before falling back to a fresh session listing.
- `list_cli_sessions_impl(...)` can now return last-known-good cached sessions after daemon failure.
- Periodic watcher reconciliation can reuse cached project/settings snapshots instead of always reloading from the DB.

Deteriorates user-visible functionality? `YES`

What is worse for the user:
- Foreground project indication can be stale because cached focus/session/project mappings are now accepted before fresh truth is fetched.
- After daemon failure, session lists can show stale last-known-good state instead of forcing a fresh local scan immediately.
- Periodic activity-watch reconciliation can lag project/settings changes until the cache is refreshed by another path.

## Bottom Line

Out of the 8 reviewed performance commits:
- `6` are `YES`
- `2` are `NO`

The main deterioration pattern is consistent:
- most of the wins came from replacing fresh truth with staged loading, cached state, or reduced scan/classification frequency
- those changes do improve latency/CPU, but several of them also make the UI less fresh or less fully populated for some window of time
