# Performance Improvement Guidelines

Companion references:

- [performance-sprint-postmortem-2026-03-09.md](/home/mstie/projects/taurhaus/docs/analysis/performance-sprint-postmortem-2026-03-09.md)
- [performance-deterioration-review-2026-03-09.md](/home/mstie/projects/taurhaus/docs/analysis/performance-deterioration-review-2026-03-09.md)

## Standard

In Taurhaus, a performance improvement is only valid if user-visible behavior stays effectively identical.

That means:

- same correctness
- same freshness
- same completeness
- same supported capability

Only these things may improve:

- latency
- CPU cost
- memory cost
- I/O cost

If the user can tell that the product is less live, less complete, less accurate, or less capable, it is not an optimization. It is a behavior trade-off.

## Core Rules

### 1. Reduce algorithmic cost, not product quality

Real improvement means changing the cost of the work itself.

Good:

- `O(n^2)` to `O(n log n)`
- one authoritative lookup instead of repeated broad scans
- eliminating duplicate full loads
- avoiding repeated parse/render work for identical immutable input

Bad:

- run the same expensive logic less often and accept staler answers
- skip classifications and call it optimization
- defer data the user previously had immediately and call it faster

Rule:

Going from `O(n^2)` to `0.7 * O(n^2)` by making the product update less often is not a real win.

### 2. “Do less work” is not a strategy by itself

Reducing work is only valid when the removed work was redundant or provably unnecessary.

Good:

- remove fixed startup sleeps when readiness polling is available
- remove duplicate project bootstrap loads
- remove duplicate internal fanout behind an unchanged boundary event

Bad:

- poll less often when the user expects real-time truth
- stop reconciling live state on the path that promises live state
- delay README, relationships, or commits if the previous behavior was full immediate availability and the delay is user-visible

Rule:

If the saved work was serving a real user-facing guarantee, removing it is feature degradation.

### 3. Caching is valid only when staleness is harmless

Caching is good for immutable or functionally pure outputs.

Good cache targets:

- markdown source -> rendered HTML
- code source + language + theme -> highlighted HTML
- parsed static metadata for immutable inputs

Bad cache targets:

- foreground project identity
- live session lists
- team presence / attachment truth
- active vs idle truth
- any state where the user expects “now,” not “recently”

Rule:

If cached staleness can change what the user believes is happening right now, the cache is unsafe unless there is a hard guarantee that it cannot be stale in that context.

### 4. Preserve identical user-visible behavior

Every performance change must pass this test:

- same data
- same timing guarantees that matter to the user
- same accuracy
- same supported feature surface

Allowed:

- same result, delivered faster
- same truth, computed cheaper

Not allowed:

- smaller language support set
- hard rendering cutoffs that remove prior capability
- slower update cadence that makes live state visibly stale
- partial content now, full content later, unless that was already the accepted product behavior

### 5. Desktop-app context matters

Taurhaus is a desktop app, not a web page fighting network and bundle-delivery constraints.

Implications:

- tiny bundle wins are usually low-value compared to correctness and capability
- aggressively narrowing local rendering features to save small amounts of memory or parse time is usually the wrong trade
- web-performance patterns should not be imported blindly

Example:

- shaving a small local desktop bundle cost by dropping broad Shiki language coverage is not a win if the user loses expected syntax highlighting

### 6. The shutoff test

Use this test on every proposed optimization:

If you turned this change off, would the user notice anything except speed or resource usage?

If the answer is `yes`, it is not a pure optimization.

Examples of failure:

- “the sidebar would show active state sooner”
- “the mesh status would be more accurate”
- “the README would already be there”
- “the code block would still be highlighted”

Those are product behaviors, not performance-only side effects.

## Good vs Bad Patterns In This Codebase

### Good: readiness-based daemon bootstrap

Pattern:

- replace a fixed startup sleep with readiness polling

Why it is good:

- preserves exact behavior
- removes artificial delay
- improves correctness under variable startup timing

Reference:

- surviving sprint win `#817`

### Good: caching immutable rendered content

Pattern:

- cache markdown/code rendering when the input content and rendering parameters are identical

Why it is good:

- output is deterministic for the same input
- stale cache is not a semantic problem because the cache key fully defines the result

Good condition:

- cache key includes the source plus all rendering-affecting inputs

### Good: removing duplicate internal work

Pattern:

- remove duplicate startup loads or duplicate internal fanout when both paths produce the same user result

Why it is good:

- user-visible behavior remains the same
- total cost drops because redundant work disappears

### Bad: reducing scan frequency for live state

Pattern:

- slowing live session/activity scanning to cut CPU

Why it is bad:

- live state becomes less fresh
- user can observe stale idle/active truth

Examples:

- scanner cadence reductions that leave sessions visually idle after work resumed

### Bad: cache-first foreground or session truth

Pattern:

- prefer cached session/foreground state before fresh truth

Why it is bad:

- foreground project and session state are live facts
- stale results are user-visible and misleading

Examples:

- cache-first foreground lookup
- stale last-known-good session lists used where current truth is expected

### Bad: deferring overview data to make switching feel faster

Pattern:

- load only a “critical subset” and defer the rest, when users previously got a fully populated overview immediately

Why it is bad:

- the UI becomes faster by being less complete
- this is feature reduction disguised as latency improvement

Allowed only if:

- the deferred data was already non-blocking by product definition
- or the user-visible behavior is explicitly redesigned and accepted as a product trade-off

### Bad: reducing renderer capability to lower cost

Pattern:

- smaller language allowlist
- hard size cutoffs that disable highlighting

Why it is bad:

- feature coverage is reduced
- the user directly loses capability

Examples:

- markdown/code highlighting regressions from narrowed Shiki support

## Review Checklist For Future Performance Work

Before landing a performance change, answer all of these:

1. What exact work became cheaper?
2. Is the algorithm better, or is the same algorithm just running less often?
3. Can any live truth now be stale longer than before?
4. Can any data arrive later or more partially than before?
5. Can any rendering/feature capability be narrower than before?
6. If the optimization were disabled, would the user notice anything except speed/resource usage?

If questions 3, 4, 5, or 6 reveal a product-visible difference, stop calling it a pure optimization.

## Required Validation Standard

Performance work must validate both:

- resource improvement
- behavior preservation

Required evidence:

- before/after timing or CPU evidence
- explicit review of user-visible behavior
- regression coverage for the preserved behavior where practical

For live-state paths, that means checking:

- freshness
- correctness
- completeness
- attribution accuracy

For frontend work, that means checking:

- rendered completeness
- capability coverage
- absence of partial/stale UI states unless intentionally designed

## Decision Rule

Approve the change only if all of the following are true:

- the measured cost is lower
- the user-visible contract is unchanged
- the implementation is simpler or at least defensible

Reject or redesign the change if it wins performance by weakening the product contract.

That is the standard Taurhaus should hold going forward.
