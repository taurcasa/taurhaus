# Research brief: no-response handling for info-only traffic, and lead notice dedup — design

Commissioned by the orchestrator (mesh-next program, phase 0, overhead
items 3 and 4), 2026-09-08. One Astra researcher/architect (`gpt-6-astra`,
reasoning effort high), read-only, through the research-sweep procedure.
Deliverable: a design document, `info-only-and-notice-dedup-design.md`,
precise enough to implement as bounded mesh and taurhaus lanes after
review. Research and design only. The MEASUREMENT of both items is gated
on the M1–M3 contract fix and the wave-2 archive; the design is not.

## Assignment

1. **Objective:** (3) an `INFO ONLY:` notice must not cost a model turn —
   the structured response expectation and the harness wake policy agree,
   an empty delta check is free, and action-required corrections and GO
   still wake; (4) the same event visible through inbox, workflow event,
   and task metadata reaches the lead as ONE notice with several sources,
   deduplicated by source event / logical delivery identity with physical
   origin retained — including the monitor records the M1 fix now imports.
2. **Deliverable:** the report at the path the procedure names, sections
   Result / Evidence / Recommendation; the inventory of today's wake and
   notice paths (S, file:line); the response-expectation ↔ wake-policy
   contract; the dedup identity rule and its fixtures; the role-text
   delta; the measurement plan with its estimator; open questions; every
   claim labeled S/D/P/I/U.
3. **First action:** read the machinery review's item-3 and item-4 grades
   (`docs/design/field-test-wave2/machinery-review-astra.md` §5, and the
   "Separate response expectation" and "Honest receipt ladder" rows of
   §6), the overhaul study's message-accepted and visibility/routing rows
   (`docs/design/mesh-messaging-overhaul-research.md`, search
   "response_expectation", "Low-priority/info traffic remains available
   without a wake", "deduplicate a monitor event"), and the delivery
   standard's message conventions; then inventory mesh's wake filter.
4. **Completion signal:** the structured summary the procedure requires,
   `status: ok`, with the report path.
5. **Review route:** design; acceptance owner = orchestrator (Fable
   altitude review), decorrelated Opus defect lens; one round.

## Why

Item 3 (threads memo's read-economics note): a notice that wakes a model
into generating a reply is pure overhead; the delivery standard already
has the prefix vocabulary (`ACTION REQUIRED:` / `INFO ONLY:` ending "no
response needed"), but the wave-2 review found that mesh's wake filter
suppresses only low-priority or empty messages — an `INFO ONLY:` prefix by
itself is not guaranteed to have the same effect in every harness, and
"info arriving during work" has no defined behavior. Item 4: wave 1
recorded prior full-wave lead cache reads with the removable notice share
UNMEASURED (the "~1.29B" causal shorthand is retracted); wave 2 confirms
a projection burden through many manual mirrors but not duplicate
notices — so the design must define dedup by source event identity and
measure lead exposure and response turns, never infer them from ledger
edits. Both items retain action-required corrections and GO as wakes.

## Ground truth to mine (all read-only)

- Mesh (`~/projects/mesh`): `src/daemon.rs` wake suppression (~line 1527:
  assignment / low-priority / empty → suppressed, telemetry
  `all_messages_empty_or_low_priority`), the low-priority nudge writer
  (~1854–1883), `src/cli.rs` `send`/`broadcast --priority urgent|low`,
  `src/watch.rs`, `src/idle_monitor.rs` (`format_auto_nudge_message`,
  `format_task_notice_header`, `send_nudge`), the workflow `message_sent`
  echo and `metadata.idle_monitor_records` (the M1 seam), the lead
  notices mesh generates on lifecycle events (completion, review request,
  block) — find every writer that addresses the lead.
- Taurhaus: `src-tauri/src/coordination/task_deadline_pass.rs` (nudge and
  stale notices), `coordination/backend/claude.rs` and
  `templates/types.rs` (where the prefixes are rendered or required), the
  M1 reader on branch `fix/telemetry-contracts` in
  `~/projects/taurhaus-contractfix/src-tauri/src/coordination/routing_report/mesh.rs`
  (message-id dedup of monitor records vs workflow echoes — reuse its
  identity rule, do not invent a second), `docs/operations/telemetry-contracts.md`
  there, and the compaction reinjection path (`coordination/compact_hook.rs`)
  for "info arriving during work".
- Harness behavior, as documented in the corpus (P/D only — no new probe):
  `docs/design/native-push-probe-report.md` and the overhaul study's
  per-harness rows on hooks, wake, and Stop continuation.
- The archive: the wave-1 inbox bodies' `INFO ONLY:` share, which of them
  drew a reply (a message from the recipient citing it), and the lead
  inbox's repeated-event exposure (same task event via notice + workflow
  echo) — from `~/projects/taurjob/docs/wave-1/mesh-archive/` (extract
  into a tempdir only); label what cannot be decided U.

## Design dimensions (decide each with reasons; label I)

1. **The contract:** a structured `response_expectation` (action / info)
   carried on the accepted message (overhaul study field), how it is set
   (explicit flag, derived from the prefix, or both — say which wins on
   disagreement), and how it maps to wake policy per harness (native
   hook, inbox read, tmux injection) so an info notice is available
   without a wake and an empty delta check costs no model turn.
2. **Info arriving during work:** what happens to an info notice while
   the seat is busy — queued, folded into the next earned read, never
   injected mid-turn; and what remains a wake regardless (assignment, GO,
   an action-required correction, a stop).
3. **Dedup identity for lead notices:** the logical event identity
   (source authority + event id / message id / ruling seq / assignment
   generation) across inbox notice, workflow echo, and task metadata; the
   monitor-record import from M1; what is rendered once with its sources
   listed; what is never merged (distinct ID-less messages, foreign
   imports); fixtures for each.
4. **Receipts:** how "not woken" is recorded honestly (the receipt
   ladder), so silence is never read as delivery failure or as uptake.
5. **Role-text delta** under the attention-budget rule: the prefixes
   stay; at most a handful of rules change; point at the flag's `--help`.
6. **Compatibility:** additive to mesh protocol/schema 1 or not (cite
   mesh's versioning rules); what the taurhaus scanner/report must keep
   reading unchanged.
7. **Measurement plan** (gated): empty checks, wake-only turns, replies
   to info notices, duplicate lead exposure — counted from the wave-2
   archive with fresh / cache-write / cache-read / output tokens kept
   separate; the estimator disclosed; what is measurable on wave 1 now.
8. **Open questions** with safe defaults.

## Not building

- No implementation, no code or role-text edit, no corpus edit.
- No transport replacement, scheduler, native adapter, or journal — the
  overhaul study's stages own those; this design must work on the
  current inbox + tmux + hook paths and survive the future ones.
- No acknowledgment duty of any kind (the adjudication's explicit
  condition).

## Constraints

- Read-only everywhere: `~/projects/taurjob`, `~/projects/mesh`,
  `~/projects/taurhaus`, `~/projects/taurhaus-contractfix`; never
  `~/.claude`, `~/.claude-account2`, `~/.codex`, `~/.gemini`, `~/.grok`,
  the live daemon (17233) or the operator's tmux server. Scratch probing
  only via the locked binary `~/.local/bin/mesh` in tempdir roots
  (`--claude-dir <tmp>`, empty environment); execute what you cite (for
  example `send --priority low`, `read`, and the daemon's suppression
  path if it can run against a tempdir root without tmux).
- Write only under the procedure's scratch/output directory.
- Labels S/D/P/I/U on every claim; measurement separate from judgment;
  confidence stated honestly.
