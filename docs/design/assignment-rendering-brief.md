# Research brief: one combined assignment rendering — design

Commissioned by the orchestrator (mesh-next program, phase 0, overhead item
2), 2026-09-08. One Astra researcher/architect (`gpt-6-astra`, reasoning
effort high), read-only, through the research-sweep procedure. Deliverable:
a design document, `assignment-rendering-design.md`, precise enough to
implement as a bounded mesh lane after the operator's go. Research and
design only.

## Assignment

1. **Objective:** replace the two renderings a seat receives per assignment
   today — the lead's authored five-line contract message and mesh's
   generated assignment card — with ONE rendering generated from the task
   record, preserving every contract field, the assignment token, effort
   and its reason, the immutable candidate/rubric references, and the
   operative wait.
2. **Deliverable:** the report at the path the procedure names, sections
   Result / Evidence / Recommendation; a field-preservation matrix; the
   card template; the nine wave-1 pairs re-rendered as evidence; a
   measurement plan with its estimator; open questions; every claim
   labeled S/D/P/I/U with file:line or command output.
3. **First action:** read `docs/design/mesh-task-threads-research.md`
   (the nine-pair measurement, search "nine assignments", and the §11-style
   recipe near "Authored contract index"), the machinery review's item-2
   grade (`docs/design/field-test-wave2/machinery-review-astra.md` §5), and
   `docs/design/mesh-next-adjudication.md` ("Task record as carrier",
   gaps 3, 5, 7); then inventory what mesh 0.2.29 carries and renders:
   `~/projects/mesh/src/cli.rs` (Create/Assign flags), `src/main.rs`
   (`AssignmentContract` ~line 200, `resolve_assignment_contract` ~310,
   the message check that looks for `deliverable:` ~700, the assign
   handler ~2950–3020, `task get` output ~1545–1581).
4. **Completion signal:** the structured summary the procedure requires,
   `status: ok`, with the report path.
5. **Review route:** design; acceptance owner = orchestrator (Fable
   altitude review), decorrelated Opus defect lens; one round.

## Why (measured, P from the threads study on the wave-1 archive)

Within 62 task-scoped bodies there are zero exact duplicates, but nine
assignments appear both as an authored prose contract (15,616 characters)
and a generated card (7,485 characters). One combined delivery that
preserved every contract field and the assignment token would avoid at
most 1,871.25 TE, 8.9% of task text — a ceiling for that sample only. The
two versions are not semantically interchangeable in every field, so blind
deletion is inappropriate. The wave-2 machinery review retains the item and
adds: count pairs by assignment generation and exposure path, not by task;
pre-GO card and GO release are different obligations; preserve
effort/reason, immutable candidate/rubric, and the operative wait. The
wave-2 seats accepted "task record as carrier, contract with creation" 9/9
and want assignment prose rendered from the record with ids at creation;
two seats warned against required fields beyond the five-line contract.

## Ground truth to mine (all read-only)

- The nine pairs: rebuild the inbox row index from the wave-1 archive
  tarball (`~/projects/taurjob/docs/wave-1/mesh-archive/taurjob-team-archive.tar.gz`,
  extract into a tempdir only) exactly as the threads study's recipe does
  (`pairs = {"architect": [(3,6)], "heavy-implementer": [(3,5),(8,7),(11,12)],
  "heavy-implementer-1": [(3,5),(8,7)], "implementer-1": [(3,6),(10,9)],
  "judge-astra-1": [(3,2)]}`; assert 15,616 / 7,485 before using them).
  For each pair, list every field the prose carries, every field the card
  carries, and the fields only one of them carries (S).
- What the record already holds: the archived task snapshots' metadata
  keys (`assignment_id`, `assigned_at`, `assigned_by`, `first_step`,
  `deliverable`, `completion_signal`, `work_kind`, `lane_id`,
  `criticality`, effort keys if present) and the `task_assigned` /
  `assignment_superseded` workflow events.
- The five-line contract, message conventions, awaiting-GO, and the
  optional deadline/effort overrides in `docs/team-delivery-standard.md`.
- How the lead is told to assign today: the bundled lead role texts in
  `~/projects/taurhaus/src-tauri/src/templates/` (grep for `first step`,
  `deliverable`, `completion signal`, `ACTION REQUIRED`) and the wave-2
  kickoff practice (`~/projects/taurjob/docs/wave-2/KICKOFF.md`, quote
  sparingly; the team is standing).
- Taurhaus consumers of these metadata keys (`src-tauri/src/task_scanner/`,
  `src-tauri/src/coordination/stores/mesh_task.rs`) — anything that would
  break if a key were renamed or moved.

## Design dimensions (decide each with reasons; label I)

1. **Field-preservation matrix:** contract line (objective, deliverable,
   first action, completion signal, review route) plus the adjudicated
   additions (accepted base, release condition, packet hash, effort and
   reason, wait state) → today's carrier (record field / prose only /
   both) → proposed record field (existing key, or a new one with its
   justification) → the rendered line. No required field beyond the five
   lines and the existing optional overrides unless evidenced.
2. **The one card:** a deterministic template rendered from the record at
   assignment time, carrying the assignment token; what changes between a
   fresh assignment, a reassignment (`--admin-reason`), and a resumed
   stage; what `mesh task get` shows so the card is never the only copy.
3. **Pre-GO versus GO release:** two obligations, two renderings — what
   each must say, and why a GO release is not a duplicate body.
4. **What the lead stops doing:** no authored contract message; the
   `deliverable:`-in-message check becomes obsolete or inverted (say
   which); the role-text delta is at most a handful of rules and points at
   `mesh task create --help`.
5. **Compatibility:** existing metadata key spellings (`first_step` /
   `firstStep`), taurhaus readers, mesh protocol/schema versioning — is
   the change additive (cite mesh's own version rules) or does it need
   the paired-contract review the overhaul study specifies?
6. **Exposure paths and counting:** inbox notice, `mesh task get`,
   taurhaus Scope; define the unit ("one authored/card pair per assignment
   generation per exposure path") and state what can be counted only from
   the wave-2 archive (U until it exists).
7. **Measurement plan:** bytes/TE and model turns reported separately,
   against the threads study's baseline with its estimator disclosed; the
   wave-1 8.9% stays labeled as its original sample.
8. **Open questions** for the architect charter, each with a safe default.

## Not building

- No implementation, no mesh or taurhaus change, no role-text edit, no
  corpus edit (the studies' next revision folds this document in).
- No transport or delivery change (native push, scheduler, receipts are
  the overhaul study's stages), no shared thread, no subscription.
- No new required assignment fields beyond the five lines and the
  existing optional overrides, unless a gap is evidenced and named as a
  spec-delta.

## Constraints

- Read-only everywhere: `~/projects/taurjob`, `~/projects/mesh`,
  `~/projects/taurhaus`; never `~/.claude`, `~/.claude-account2`,
  `~/.codex`, `~/.gemini`, `~/.grok`, the live daemon (17233) or the
  operator's tmux server. Scratch probing only via the locked binary
  `~/.local/bin/mesh` in tempdir roots (`--claude-dir <tmp>`, empty
  environment) — execute every flag and command you cite (for example
  `task create --first-step --deliverable --completion-signal --effort
  --why`, `task assign --awaiting-go`, `task get --json`) and quote the
  real output.
- Write only under the procedure's scratch/output directory.
- Labels S/D/P/I/U on every claim; measurement separate from judgment;
  confidence stated honestly.
