# Compaction Architecture Review — 2026-03-09

## Verdict

The compaction pipeline is **mostly coherent downstream**, but **not yet fully coherent end-to-end**.

What is coherent today:
- transcript boundary extraction writes canonical signal records to a per-team signal log
- signal watchers consume from the append-only log with persisted offsets and replay recovery
- downstream processing resolves a managed member, applies freshness/idempotency gates, and writes the reinjection card to the member inbox
- Claude and Codex delivery paths are now clearly separated at the final delivery hop

What is still inconsistent:
- the upstream "which Codex transcripts are active and should be watched" problem is still maintained by steady polling in the daemon
- a second daemon polling loop independently scans display sessions for activity export
- that means the pipeline is not purely event-driven; it is better described as **event-driven delivery built on poll-driven session discovery**

That split is the main reason the architecture still feels partially incremental rather than fully converged.

## Findings

### 1. Two daemon-owned 500ms session authorities remain the main architectural inconsistency

Files:
- `src-tauri/src/daemon/compaction.rs`
- `src-tauri/src/daemon/session_activity.rs`

Severity: medium

The downstream compaction chain is based on canonical records and watcher replay, which is a good architecture. But upstream state is still discovered by two separate daemon loops:

- `daemon/compaction.rs` keeps a `500ms` runtime-session refresh so it can diff the active Codex set and update the extractor
- `daemon/session_activity.rs` keeps its own `500ms` display-session scan when any session is active, and also exports activity snapshots on every cycle

These are both legitimate consumers, but they are separate polling authorities over the same underlying session world.

Implications:
- higher daemon CPU than an event-driven design should need
- duplicated tmux/process scanning logic in steady state
- architecture reads as "event-driven after discovery" rather than one unified event-driven pipeline

Recommendation:
- move toward one authoritative daemon session snapshot source
- have compaction active-set maintenance and activity export subscribe to that shared snapshot instead of each owning a scan loop
- if that is not done soon, treat the current model explicitly as a transitional architecture, not the final one

### 2. `compaction.signal_consumed` was emitted twice for the same signal

Files:
- `src-tauri/src/session_scanner/compaction_watcher.rs`
- `src-tauri/src/coordination/compaction_processor.rs`

Severity: low

Before this review, the watcher emitted `compaction.signal_consumed` after processing a record, and the processor emitted the same event again at the start of `process_signal_at(...)`.

That duplicated observability for one logical consumption event and blurred ownership of the signal lifecycle.

Cleanup landed:
- kept `compaction.signal_consumed` emission in the watcher
- removed the duplicate emission from the processor

Why this is the right boundary:
- the watcher owns the durable signal-log offset and replay lifecycle
- the processor should own resolution/delivery outcomes, not log-consumption bookkeeping

### 3. Codex inbox rendering name had drifted from reality

Files:
- `src-tauri/src/coordination/reinjection.rs`
- `src-tauri/src/coordination/compaction_processor.rs`

Severity: low

`render_codex_inbox_text(...)` no longer rendered generic text; it serialized a structured JSON payload that is appended into the member inbox.

That was stale naming from an earlier mental model and made the code read as if the Codex path were still text-first.

Cleanup landed:
- renamed it to `render_codex_inbox_payload(...)`
- updated the processor and tests to match

### 4. The extractor is architecturally correct, but the file is too dense

File:
- `src-tauri/src/session_scanner/compaction_extractor.rs`

Severity: medium

The extractor now does all of these in one file:
- service lifecycle
- notify watcher loop
- transcript watch reconciliation
- runtime transcript synchronization
- boundary parsing
- pair normalization
- signal emission
- persisted extractor state
- diagnostics
- tests

The behavior is mostly coherent, but the implementation density is high enough that future changes are likely to create more local patches.

Recommendation:
- do not rewrite it during this release cycle
- after stabilization, split it into at least:
  - service/watch management
  - JSONL boundary parsing + normalization
  - persisted extractor state

That is a maintainability recommendation, not a correctness blocker.

### 5. Error handling is mostly consistent, but watcher/processor boundaries still flatten detail

Files:
- `src-tauri/src/session_scanner/compaction_watcher.rs`
- `src-tauri/src/coordination/compaction_processor.rs`

Severity: low

The overall pattern is reasonable:
- extractor failures emit structured failure events and keep going
- watcher loops warn on notify failures and preserve replay recovery
- processor records `skipped`, `stale`, `failed`, and `injected` outcomes into per-member state and logs
- inbox corruption fails closed and quarantines the file

The remaining weakness is that watcher-side processing converts processor failures into a plain `String`, which loses some typed structure at the watcher boundary.

This is acceptable for now because the structured delivery events are preserved elsewhere, but it is not ideal if watcher-side recovery policy becomes more complex.

Recommendation:
- leave as-is for now
- if the watcher needs richer retry/escalation logic later, give the processor a typed error surface rather than a string

### 6. Test coverage is good at the component level, but one end-to-end regression is still missing

Severity: low

The pipeline now has solid targeted coverage for:
- extractor replay/offset behavior
- watcher replay recovery
- processor delivery outcomes
- inbox corruption handling
- Claude hook skip/delivery paths
- duplicate paired-boundary suppression in the extractor

What is still missing:
- a full end-to-end regression asserting that a paired Codex boundary written through the extractor/watcher/processor chain results in exactly one terminal inbox delivery

Current coverage proves the extractor emits one canonical signal in the split-pass case, which is the critical fix, but it does not yet assert the complete downstream effect in one test.

Recommendation:
- add that only if duplicate-delivery regressions reappear
- current coverage is acceptable for now

## Documentation And Comment Accuracy

Code comments and module docs are mostly aligned with current behavior.

What is accurate:
- watcher is described as event-driven with replay recovery
- inbox store describes file-backed inbox delivery correctly
- reinjection code describes post-compaction payload composition accurately
- Claude hook bridge comments match the current compact-hook flow

What needed correction:
- the stale Codex renderer name (`render_codex_inbox_text`) implied text-first delivery; this review corrected it

Non-code note:
- `docs/analysis/compaction-fresh-deploy-2026-03-09.md` is now historical evidence, not current behavior, because duplicate paired-boundary delivery has since been fixed

## Cleanup Landed In This Review

Code cleanup applied:
- removed duplicate `compaction.signal_consumed` emission from the processor
- renamed `render_codex_inbox_text(...)` to `render_codex_inbox_payload(...)`

No larger refactor was attempted in this review.

## Design Decisions To Discuss Separately

1. Should daemon compaction and daemon session-activity continue as separate polling authorities, or should they converge onto one shared daemon session snapshot service?

2. Is the current extractor/watcher/processor split considered the final architecture, or is the extractor expected to be decomposed after this release window?

3. Do we want richer typed watcher/processor failure contracts, or is string-level watcher failure reporting sufficient because structured delivery events already exist?

## Bottom Line

This is **not** a pile of random band-aids anymore.

The downstream half of the pipeline is coherent:
- canonical signal log
- replay-capable watcher
- delivery processor with persisted outcomes
- inbox/Claude delivery split by tool

But the upstream half is still partly transitional:
- duplicated daemon polling remains the main architectural inconsistency
- that inconsistency also matches the known daemon CPU problem

So the honest assessment is:
- **delivery architecture: coherent**
- **session-discovery architecture: still mixed / transitional**
- **cleanup state after this review: improved, but one larger unification decision remains**
