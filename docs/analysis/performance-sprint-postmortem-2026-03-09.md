# Performance Sprint Postmortem — 2026-03-09

Task: `#827`

Related review:
- [performance-deterioration-review-2026-03-09.md](/home/user/projects/taurhaus/docs/analysis/performance-deterioration-review-2026-03-09.md)

## Outcome

The performance sprint did not hold as a safe optimization pass.

Most of the attempted wins came from one of these patterns:

- reducing scan frequency
- deferring data that used to be loaded immediately
- preferring cached live state over fresh truth
- narrowing rendered capability to reduce cost

Those changes did reduce work, but several of them also made the product less correct, less fresh, or less complete from the user’s perspective. That means they were not clean optimizations for Taurhaus. They were functionality trade-offs.

The sprint will be largely reverted. Two changes should survive the revert because they are clean wins.

## Changes That Survive

### `#817` — `45473bd`
`perf(startup): replace fixed 2s daemon reconnect sleep with readiness polling`

Why it survives:

- It removes an artificial fixed delay.
- It does not reduce correctness, freshness, or completeness.
- It waits for real daemon readiness instead of sleeping blindly.

What it improves:

- faster startup when the daemon is already ready early
- more correct startup timing under variable daemon boot conditions

Why this is a real optimization:

- user-visible behavior is preserved
- only the cost and latency of delivery improve

### `#820` — `bde9be5`
`perf(frontend): split project selection into critical and deferred loading`

Why it survives:

- In the current tree this commit is test coverage only.
- It does not introduce new runtime behavior.
- It documents and validates already-existing behavior rather than changing it.

What it improves:

- preserves coverage around the staged-loading behavior already present elsewhere
- remains useful after the revert because it protects the tested contract

Why this is safe:

- there is no new runtime trade-off in this commit itself

## Changes Reverted Or Marked As Failed

### `#816` — `2db79a2`
Failed because:

- it reduced activity freshness after the scanner dropped into the slower steady-idle path
- sessions could remain visually idle after work resumed

### `#818` — `8f9beb3`
Failed because:

- it turned first-project bootstrap into a partially populated load
- README, relationships, and commits could appear later instead of landing together

### `#819` — `6439e90`
Failed because:

- it removed request-path self-heal from live mesh status
- stale presence could remain visible longer

### `#821` — `7ce85fe`
Failed because:

- it preferred cached runtime transcript bindings in the hot path
- stale bindings could briefly produce wrong Codex attribution or wrong active/idle truth

### `#822` — `a59b345`
Failed because:

- it intentionally reduced supported highlighting coverage
- it added hard size cutoffs that remove highlighting for larger content

### `#825` — `985784b`
Failed because:

- it preferred cached session/foreground/watch state over fresh truth
- foreground and session state could lag reality

Detailed per-commit reasoning for all 8 reviewed changes is captured in:
- [performance-deterioration-review-2026-03-09.md](/home/user/projects/taurhaus/docs/analysis/performance-deterioration-review-2026-03-09.md)

## Why The Sprint Failed

The core mistake was treating “less work” as equivalent to “better performance.”

That is not the standard Taurhaus needs.

In this product:

- live state must stay live
- status must stay accurate
- overview data must stay complete
- rendering capabilities must not be silently reduced just to save work

The sprint repeatedly crossed that line:

- slower polling replaced real freshness
- deferred loading replaced full initial availability
- stale caches replaced current truth
- reduced renderer support replaced capability

Those are product trade-offs, not pure optimizations.

## What Future Developers Should Keep

Keep these lessons:

1. Remove fixed waits when readiness can be observed directly.
2. Keep tests that prove behavior, even when the implementation behind them is reverted.
3. Prefer architectural simplification over sampling less often.

## What Future Developers Must Not Repeat

Do not call it an optimization when the user can observe any of these:

- data arrives later than before
- state is less fresh than before
- live status is less accurate than before
- supported rendering/features are narrower than before

If any of that happens, the change is a product trade-off and must be treated as one explicitly.

## Bottom Line

The surviving wins are narrow:

- `#817`: real startup optimization
- `#820`: test coverage only

Everything else from the sprint is either being reverted or should be treated as failed performance work because it paid for lower CPU or faster paint with visible product deterioration.
