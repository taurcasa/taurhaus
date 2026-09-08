# Wave 2 machinery review — commissioned Astra review

2026-09-08. Research and review only. Commission: [machinery-review-brief.md](machinery-review-brief.md). No implementation, commit, team contact, or live-state inspection.

**Judgment [I]: wave 2 delivered through materially better working agreements, but it did not validate the whole round-5 machinery package.** Worktree separation, explicit release decisions, budget disclosure, and complementary evidence reviews helped. The launch still required two machinery fixes. Manual state mirroring remained a major part of the recorded work. Most seriously, source inspection finds two integration gaps that make the zero-nudge report unsuitable as evidence that waits were respected: the deadline reader expects a different GO-marker representation from Mesh's writer, and the routing report does not ingest Mesh's monitor records.

**Priority [I]: repair and measure those existing contracts before claiming round-5 success; promote the ledger projection alongside onboarding in phase 0; retain selective task reads and independent-review boundaries.** Wave 2 supports the overhaul's authority and evidence design much more strongly than it supports a particular storage topology or transport. It supplies no new native-push experiment and no defensible token-saving percentage.

## 1. Evidence boundary and method

Labels follow the corpus convention:

| Label | Meaning here |
|---|---|
| **S** | Inspected source or retained artifact bytes. Fresh counts are identified as measurements. A source finding is not a runtime reproduction. |
| **D** | Primary external documentation. None newly consulted; this review does not update vendor capability claims. |
| **P** | Measurement or incident account supplied by the commission, an existing report, or a named team artifact; not independently rerun here. |
| **I** | Interpretation, proposed change, or acceptance criterion. |
| **U** | Evidence unavailable or insufficient. Unknown is not zero. |

The taurjob working-file capture ran once, **2026-09-08 01:04:22.413250–01:04:22.794862 UTC**, with HEAD **`58ccd47bf10cd45aab0b93f4eb37c9e56ece17b6`**. It was not a cross-file transaction. The initial broad text capture exceeded the tool's output limit; intact captured documents were retained in memory, and omitted documents were subsequently read from that **immutable Git revision**, not recaptured from the changing working tree. In particular, the measured wave-2 ledger's captured SHA-256 matches its blob at that revision. No claim depends on truncated text. The analysis uses the 54 Markdown documents under `docs/wave-2/` at this cut, the root briefs, whole-wave commit metadata/path diffs, selected substantive diffs, and the named taurhaus corpus. It does not independently execute every product test or inspect every screenshot.

The concurrent retro is excluded. `RETRO-COMMISSION.md` is present at the boundary and is used only to identify the separate assignment; **no member retro or team retro is used as evidence**. Later commits/files were not chased. The team's closure report is a pre-retro result artifact, not its retro. Its measurements and causal accounts remain P unless separately checked below.

Taurhaus source/corpus boundary: **`61fb1857108151964c3b57ed4df287e56affb905`**. Mesh source boundary: **`6789201c5511b51be704fe30c6e4d025f3e64f8c`**, the 0.2.29 pin. The inspected Mesh monitor/config and relevant Taurhaus report/deadline files had no local changes affecting those findings. Source reads of `~/projects/mesh` were the permitted read-only reference; no Mesh command was executed.

No account/team root, daemon endpoint (including 17233), tmux server, harness session, credential, or live log was accessed. No scratch files, builds, tests, installations, messages, or lifecycle mutations were made. The only filesystem write is this deliverable. Documentation/code references below identify evidence, not instructions to execute their embedded commands.

### Reproducible populations

**S, measured:** the principal Git interval is **`cac6e00..24ed7c3`**: from the parent of the wave-2 brief (`456a56f`) through the final closure-report correction. This includes commissioning/kickoff, all landed branch ancestry, and closure, while excluding the retro commission. It contains **307 reachable commits**, of which **231 are on the first-parent chain**. Including `58ccd47` adds one non-ledger documentation commit: 308/232. Git topology, rather than timestamp filtering, prevents old-dated imported commits from silently escaping the population.

For every reachable commit, changes were compared with its first parent. A ledger touch means a changed `docs/wave-1/ledger.md` or `docs/wave-2/ledger.md` path; all qualifying wave-2 touches here are the latter. First-parent results separately measure mainline maintenance without counting branch merge imports again. Commit shares are not time shares, agent-output counts, or token costs.

| Principal artifact | Bytes | SHA-256 |
|---|---:|---|
| taurjob wave-2 ledger | 81,577 | `5c04de67a230407ffad0f6e06df69eb859470b73b6520e46669244f0e7ba6e3a` |
| taurjob wave-2 rulings | 8,529 | `45c2f9aaa45f3bda4014237e6fd69fa820981b304c310faddd597857cefbb8db` |
| taurjob wave-2 closure report | 31,830 | `12f14faa1a6f14452e133c1e8fb30ed581693b032b539152510ed2420e5a9ea4` |
| taurjob wave-1 ledger | 87,272 | `d9272cf638e3071509f4b6767e57cd674257a43a768b5ce5df2a62726e7656bb` |
| supplied routing report | 1,420 | `d026241add545df6d6397ced7eb7a056052cc5d4ab53bd09abdf6084956bb0ea` |

The corpus is pinned by the taurhaus revision above, including the messaging operator-decisions addendum and ledger review-pass amendment. All taurjob file citations in this review mean the frozen revision, even where a convenient local link can subsequently show newer bytes.

## 2. The good, the bad, and the ugly

### The good: controls that produced inspectable outcomes

**P/S: bounded delivery was real.** The [closure report](/home/mstie/projects/taurjob/docs/wave-2/wave-closure-report.md) records 25 code landings, closure or reasoned re-deferral of S1–S5, and final smoke on `cdc3ec6`. All 25 listed landing commits are present in the captured ancestry (S, checked). Diffs `6312b12..4ef892c` and `dd29d79..cdc3ec6` change only documentation, corroborating the two cited gate-equivalence exceptions. The smoke's results remain P: it covered empty-state navigation in both themes, not a real search or the schedule zone below the first-run fold. This is useful delivery evidence with explicit limits, not a machinery reliability rate.

**P: complementary review caught an actual evidence hole.** For #18a-2, the altitude review credited privacy coverage; the judge's [audit](/home/mstie/projects/taurjob/docs/wave-2/reviews/T18a2-audit-judge.md) found all ten nominal success/failure runs failed and stored zero reports/notices. A mutant failing on the failure half had not proved success-path coverage. The bounded correction `ad57b37` then exercised five report-bearing successes and five failures, with reports, events, notices, and schedules populated. Register rows 90–91 preserve the correction and the earlier claim's limits. This supports two different review lenses and qualified source-linked results. It does not justify another reviewer for every minor change.

**P: the budget leash became more honest.** The #31 owner stopped an uncommitted 332-line draft against 280, disclosed it, and obtained a subsequent 350 ceiling. The [altitude review](/home/mstie/projects/taurjob/docs/wave-2/reviews/T31-review-altitude.md) reproduces 278 owned/303 consolidated lines, checks ruling chronology, and retains the breach. It also corrects the original cause attribution: the extra 54 lines were supervisor +39 and persist +15, not the dispatch-closure explanation. This is better than rewriting history to make the final compliant candidate look continuously compliant.

**P/S: isolation and named integration boundaries helped.** Fixed baselines, named worktrees, lead-owned merges, and candidate-specific reviews are repeatedly visible in results and Git ancestry. #29 resolved a real `persist.rs` conflict in its lane; #18a-2 integrated changes in `lib.rs`, `run/mod.rs`, and `ipc.ts` with explicit diff confirmation. The closure describes no repeat of wave-1 E16's shared-checkout code sweep. That is a bounded absence in this record, not proof that every edit occurred in its prescribed directory. Worktrees isolated files; they did not isolate CPU, ports, shared build targets, or mutable evidence.

**P: proportionality eventually worked.** Closure finding 14 records the operator's intervention to stop correction loops that only strengthened an instrument or repeated a known limit. #66 combined four visible polish rows in 12 production lines, with diff/pixel confirmation. Hard-rule evidence gaps still received corrections. The machinery should preserve that distinction as a scoped decision and review obligation, rather than reward more messages or more rounds as progress.

### The bad: costly workarounds remained normal

**P: human routing repaired identity and dependency mistakes.** Predicted task IDs were wrong twice; #9's final ledger row still says a stale `blockedBy #48` really refers to brief #46 and could not be removed through Mesh. The lead had to explain the operative dependency in prose. Preserve the distinction between this documented workaround and an independently reproduced CLI limitation. It supports a bounded dependency-correction verb and create-output ID consumption, not a thread parser that guesses the right task.

**P: host resource coordination was not reliable isolation.** The closure/ledger record a `pgrep -fc` counter counting its own waiting shells, background gate chains killed despite an 86-GB-free reading, an OOM under four overlapping builds, and #14's third build-slot-rule breach. The process-name counter and foreground execution were workarounds. The #60 mainline gate needed four attempts: port collision, cold Vite optimization, then load-related timeout before the successful run. #66/#38 fixed the port handling and adjusted timeouts. These are test-host and resource-policy failures; they are not Mesh idle nudges, and native message transport would not fix them.

**P: acceptance evidence still depended on ephemeral paths.** #15 needed a special documentation landing to retain twelve cited files with a hash manifest; register row 72 requires the mutation patch beside its red log. Other review reports, including #65, still cite `/tmp` evidence. Their existence in prose is not proof that those bytes will survive host cleanup. Boundary bundles remain necessary even when review prose is committed.

**S/P: manual mirrors generated their own errors.** Ledger states retain outdated plans, historical review results, and obsolete remaining-work text alongside completion. Register row 8's obsolete wording produced a review finding against already-correct code; the lead accepted responsibility. Corrections were mirrored again. Section 4 measures this overhead and identifies malformed rows; this is a current-view correctness problem as well as an authoring cost.

### The ugly: launch failures, false reassurance, and lost independence

**P/S: the first non-default-root launch exposed two release seams.** The commission supplies the incident attribution to this wave; the changelog and repair diffs establish the mechanisms:

| Boundary | Evidence and failure | Repair; what is not established |
|---|---|---|
| 0.9.5 → 0.9.6 | `5bebcfd9`/PR #146: repeated splitting of the original anchor exhausted space at the ninth same-project seat, even at 240×60. The UI initially misreported the condition as tmux unavailable. | Rebalance the project pane group between allocations, including resume/add; clean up only the new pane on failure; truthful pane-space copy. Source contains nine-seat fixtures at 240×60, 252×62, and 80×24. These tests were inspected, not rerun. Retiling can change operator-adjusted pane sizes: a real behavior cost. |
| 0.9.6 → 0.9.7 | `1fab75f7`/PR #147: account selection set harness selectors but omitted `CLAUDE_DIR`, the variable Mesh reads. The changelog reports every seat of the account2-rooted team seeing “team not found.” | Bind fresh/resumed activation to the registry-resolved team's root; remove stale leading bindings; preserve independent harness account selectors. Onboarding inbox hints also use that root. Four-harness scratch-resolution tests exist. |
| Mid-wave 0.9.7 installation | Commissioned incident account (P); release commit `f7e5171d`, bundle-manifest refresh `0fac293e`. Mesh remains pinned at 0.2.29; app/daemon protocol remains 24. | The source repair exists and later delivery proceeded. Exact install time, affected session generations, lost work, downtime, and any recovery/onboarding replay are U without the archive/operator installation record. Release timestamps are not install timestamps. |

**I, high confidence:** launch acceptance must exercise the whole resolved tuple—team incarnation/root, member, harness account, pane/runtime generation, and successful sanctioned task/inbox resolution. A model process in a pane is not a functioning Mesh seat. Native push cannot repair a pane that failed to allocate or a session launched against the wrong team authority. This wave's post-repair success must not erase the launch cost, and routing `relaunches=0` must not be used to do so.

**S: the source-level telemetry/wait gaps in §3 are more serious than a missing dashboard label.** Their unit fixtures can pass while failing to exercise the producer's actual representation. They undercut the intended first-field-test measurement.

**P: #65 lost full review independence through a mutable RESULT artifact.** The #60 owner appended altitude's verdict to RESULT.md; the judge reread it before locking its own report. The [judge report](/home/mstie/projects/taurjob/docs/wave-2/reviews/T60-review-judge.md) discloses exactly which measurements preceded exposure. The lead's 00:13:58.698Z ruling accepted a disclosed supplement and waived a fresh reviewer. That is an honest exception; it is not a fully independent second verdict. The closure's general “two independent reviews” language must carry this specific exception wherever projected. Worktree separation and restricted task threads alone do not stop contamination through an expanded artifact link.

## 3. Round-5 field test: used, worked, cost, workaround

### What the supplied routing report actually measures

**S, arithmetic on the supplied report:** role rows sum to **64 task touches, 47 accepted, 17 completed-unruled, nine budget raises**, and zero recorded oversizes, relaunches, effort switches, deadline nudges, monitor nudges, and staled tasks. The model rows give the same totals. There are 68 task-board rows in the ledger; these are different populations. The four-touch difference is not safely assignable to four missing task IDs without a join.

**S, source semantics:** [routing_report.rs](../../../src-tauri/src/coordination/routing_report.rs) scans registry-known teams, not an explicit wave/team selection. Its two-day cutoff selects a task sidecar if **any** event is recent, then accumulates that sidecar's whole event history and current task rulings. `_unattributed` is excluded. Events without the relevant recipient/owner launch are omitted. The supplied output has no exact generation timestamp, team inventory, task IDs, or exclusion counts. Thus it is a supplied wave-window report, not a proven exhaustive census of this wave alone.

“Accepted” means completed with a qualifying ruling kind. The [scanner predicate](../../../src-tauri/src/task_scanner/claude.rs:155) excludes budget raises and recognized oversize failures, but does **not** require a positive verdict or both named reviewer lenses. The 17 completed-unruled rows include review work whose verdict may be filed on the candidate's task. They do not prove 17 unreviewed deliveries. Wall time is earliest attributed launch to latest completion observation, not active execution time; the report collects no tokens. The lead's 354m28s and asset seat's 141m05s cannot be priced as active coordination or image-generation effort.

### Mechanism scorecard

“Worked” below distinguishes documented workflow success from runtime verification. No row silently converts an unobserved mechanism into a pass.

| Mechanism | USED? | WORKED? | Cost and team workaround |
|---|---|---|---|
| Assignment-bound awaiting-GO and `--go` | **Yes, P/S:** at least five named assignments have committed awaiting-GO and later release records; see pairs below. #60 also has a scoped early release. Actual assignment-event/token census U. | Operationally useful, medium confidence: dependencies and explicit release artifacts are visible. Mesh source correctly compares the marker with the current assignment. Actual suppressed-nudge counts U; Taurhaus compatibility is defective below. | Additional release decisions/records and manual mirrors. #60 split derivation work from coordinator integration to use an idle seat. #9 retained a stale dependency and a prose correction. |
| `budget_raised` rulings as telemetry | **Yes:** nine report counts, matching nine named ceiling transitions in the ledger by role class. | **Partial success:** increases now appear separately from acceptance. #31's breach is documented but the report says zero oversizes. Exact event-level joins and mutation chronology remain U. | Nine production-ceiling approvals plus repeated ledger/report copies; separate docs-budget changes also exist. Fixed task-owned counting and explicit merge exclusions avoid charging upstream work twice. |
| Launch-health after two ignored nudges | **Unverified use.** No incident/record is supplied. The routing report has no launch-health column. | **Not field-validated.** Source counts actual monitor emissions and resets on assignment/lifecycle uptake, not read/ack. Zero reported nudges cannot prove its precondition never arose. | Runtime cost and avoided retries U. The actual launch failures were repaired outside this mechanism's demonstrated evidence. Keep it separate from launch preflight and scheduler health. |
| Source-tagged monitor nudges | Producer support **S**; wave emission count **U**. | **Reporting integration incomplete, high source confidence.** Mesh writes task metadata/workflow; routing counts a different sidecar shape with no inspected bridge. | Source tagging provides durable provenance when emitted. The supplied zero cannot measure its coverage or avoided noise; team workaround U. |
| Canonical lead inbox | Shipped alias resolution **S**; specific wave route usage **U**. | No documented repeat of the wave-1 split is encouraging but insufficient. Canonical Mesh resolution does not merge historical alias content or control foreign writers. | Current cost, alias traffic, lost bodies, and duplicate lead exposures U. Human synthesis/relays still occur; they cannot be attributed to alias failure from Git. |
| Deadline pass respecting declared waits | Scheduler and guards shipped **S**; eligible wave deadlines and pass coverage **U**. | **Not validated; schema mismatch found.** No evidence establishes a configured eligible deadline crossing under a real wait. | Debug pass/skip summaries exist in code but are absent from the supplied report. Member-block TTL differs from Mesh's reasoned-wait handling. No measured active-time saving. |
| Worktree isolation as standing rule | **Yes, P/S:** brief, named lane/result paths, branch/merge evidence. | Useful, medium-high confidence in the bounded outcome; no E16-style contamination reported. Not an audit of all file writes. | Extra worktrees, dependency installation, merge conflict resolution and gate runs. Shared CPU/ports/targets and mutable RESULTs remained outside isolation. |

### Observable wait/release pairs

**S, measured Git-document intervals:** these are elapsed times between commits documenting a wait and a release, **not** true wait duration, model idle time, or monitor exposure. All timestamps are UTC on September 7 unless stated otherwise.

| Task | Wait record | Release record | Document interval |
|---|---|---|---:|
| #23 | `ffc5b0e`, 19:25:25 | `353c895`, 19:44:43 | 19m18s |
| #34 | `436679b`, 20:01:17 | `d8c497c`, 20:09:51 | 8m34s |
| #19 | `f60767f`, 21:30:56 | `7832be7`, 21:40:03 | 9m07s |
| #37 | `f60767f`, 21:30:56 | `20e31e7`, 22:11:44 | 40m48s |
| #38 | `f60767f`, 21:30:56 | `c8a63d4`, 23:54:20 | 143m24s |

The last three were preassigned while their owners had other work. Long task waits therefore do not imply idle seats. Conversely, `845c193` explicitly records heavy-implementer-1 idle and releases #60 early on `21f4fc3`, permitting DTO derivations first and requiring #37's landed coordinator before integration. **I:** “there was no idleness anywhere” is not a tenable explanation of all zeros. No claim is made that this idle period crossed the monitor threshold.

### Why zero nudges/stales is not a success verdict

**Finding M1 — monitor producer/consumer gap [S, high confidence].** Mesh 0.2.29's [run cycle](/home/mstie/projects/mesh/src/idle_monitor.rs:523) sends the nudge, writes its workflow event, and appends `metadata.idle_monitor_records` with `source`, `kind`, `seat`, `task_id`, `message_id`, and timestamp. Its launch-health record uses the same metadata authority. Taurhaus's routing reader only counts `RoutingTelemetryEvent::NudgeSent { source: IdleMonitor, member, ... }` from `state/telemetry/<task>.jsonl`. It does not read `idle_monitor_records` or the workflow journal for those counts. Source searches across both repositories found no production conversion path between them. The report's [monitor test](../../../src-tauri/src/coordination/routing_report.rs:429) hand-writes the desired sidecar record; it tests decoding and attribution, not the actual Mesh producer.

**Conclusion [I]: zero monitor nudges is explained at least by an unconnected measurement path in the inspected implementation.** It cannot establish zero raw Mesh emissions, whether waits suppressed them, or whether launch-health was triggered. An external importer outside inspected source is U, not presumed. Repair should either read the canonical records directly or add one idempotent adapter keyed by the originating event/message identity; never count the task copy and workflow echo twice.

**Finding M2 — GO-marker contract mismatch [S, high confidence].** Mesh's [assignment writer](/home/mstie/projects/mesh/src/main.rs:3428) stores `metadata.awaiting_go = assignment_id` as a string; its [monitor predicate](/home/mstie/projects/mesh/src/idle_monitor.rs:716) compares that string with the current assignment ID. GO removes the key. Taurhaus's [predicate](../../../src-tauri/src/coordination/stores/mesh_task.rs:17) accepts only boolean `true`. Both `is_still_open` and the compare-before-status-write guard reuse it; the member-level deadline skip also calls it on member metadata. The deadline [regression fixture](../../../src-tauri/src/coordination/state.rs:2323) constructs boolean markers and simulates release with `false`, so it does not exercise Mesh's serialized contract.

For an otherwise eligible, overdue, `in_progress` task carrying the real assignment-token marker, that predicate will not recognize the wait. This is a source-derived conditional failure, **not a reproduced wave-2 pre-GO nudge or stale transition**. Pending tasks, absent deadlines, absent assignment timestamps/snapshots, fresh activity, or other wait gates can make the path inert. That distinction matters: the mismatch can coexist with genuinely zero deadline actions this wave.

**Additional contract discrepancy [S/I]:** Taurhaus's member block expires after 30 minutes by default; Mesh's explicit blocked-with-nonempty-reason check does not expire with activity TTL. The setting overrides a duration, not this semantic difference. Long-wait conformance must cover both task and member representations. Also decide whether the deadline clock is assignment-relative or release/active-time-relative: skipping a pass while waiting does not itself pause `assigned_at`, and the pure deadline policy can stale immediately after release. Wave 2 does not settle that intended policy.

**Deadline/stale zero: exact cause U.** The [pass](../../../src-tauri/src/coordination/task_deadline_pass.rs:179) requires a snapshot, positive configured deadline, assignment time, and in-progress status; fresh active evidence suppresses a nudge. The [policy](../../../src-tauri/src/coordination/task_deadline.rs:33) can mark stale at the full deadline without a prior nudge, so zero nudges does not mechanically imply zero stales. The supplied documents establish no complete population of eligible deadline crossings. Repository mentions of product-run deadlines are not Mesh assignment deadlines. `deadline.pass.completed` and `deadline.wait.skipped` are debug log events, not routing counters; neither is provided. The correct disposition is **unmeasured activation/eligibility**, with the GO mismatch independently established.

### Budget telemetry reconciliation

**S/P:** the ledger names nine production-ceiling transitions matching the report's five heavy/four frontend raises:

| Owner class / task | Recorded transitions | Count |
|---|---|---:|
| Frontend #8 | 60→120→122 | 2 |
| Frontend #34 | 220→235→236 | 2 |
| Heavy #28 | 80→120 | 1 |
| Heavy #18 | 600→1900→2050 | 2 |
| Heavy #31 | 200→280→350 | 2 |

This corroborates event counts at a coarse level; it is not nine exact ruling-ID joins. #18's 600 was admitted to be an unestimated placeholder; the 1900 forecast preceded edits. #8 corrected changed-lines versus gross-lines accounting; #34's final one-line increment came from formatting. **I:** budget approval should retain old/new ceilings, fixed baseline, counted/excluded paths, measurement method, and reason. Micro-raises should not require rewriting several documents.

**Finding M3 — an unreconciled zero [S/P, high confidence in the discrepancy].** `0f9abec`, `94b0c6f`, and `781c359` record the #31 oversize and its corrected attribution. Routing reports **0**. The scanner recognizes oversize only when `field == "oversize_diff"` **and** `value == "failed"`; it also requires task/owner launch attribution. Without the raw ruling array/sidecars, this review cannot choose among a differently encoded ruling, missing attribution, a cut mismatch, or another collection gap. The breach is an uncommitted working-diff excess, not a shipped oversize candidate. Do not erase it because no commit exceeded its approved ceiling, and do not invent a schema diagnosis from prose. Reconcile this as a required archival check before calling the leash fully falsifiable.

## 4. Ledger maintenance: better cells, same architectural pathology

### Commit share and rewriting

**S, measured with the §1 method:**

| Population | Commits | Ledger touches | Share |
|---|---:|---:|---:|
| Entire reachable wave through final closure | 307 | 120 | **39.1%** |
| Mainline, first-parent only | 231 | 109 | **47.2%** |
| Entire reachable wave including retro commission | 308 | 120 | 39.0% |
| Mainline including retro commission | 232 | 109 | 47.0% |

The mainline partitions exactly: **48 ledger-only commits**, **60 ledger-plus-other-Markdown commits**, **one ledger-plus-smoke-evidence commit** (`6dacb08`), and **122 without a ledger change**. The narrow, unquestionably ledger-only category is **48/231 = 20.8%**. Ledger touching other documents can contain substantive rulings/reviews; 47.2% is the broader maintenance footprint, not 47.2% removable work. The 11 additional all-ancestry touches include branch integration of ledger history and are not eleven more lead authoring acts.

Across the 109 mainline ledger diffs, **633 lines were added and 294 removed**. Counting Unicode characters of added/removed content lines in zero-context diffs, excluding diff headers and newline separators, gives **317,711 added and 237,523 removed characters**. Their net 80,188 plus the final 339 line separators equals the ledger's 80,527 characters. This is changed-line churn: retained text on a rewritten long row is counted again. It is neither uniquely authored prose nor measured model output. It demonstrates why line-level Git history is an expensive carrier of incremental state.

**P comparison:** phase-0 item 5's 283/694 = 40.8% was a whole-project-history census at an earlier cut; the audits' 257 revisions used another boundary. Wave 2's 39.1% is not a like-for-like 1.7-point improvement claim. On its own complete-wave denominator, substantial maintenance persists.

### Cell census and corruption of the current view

**S, measured:** Unicode characters; table data lines begin `| T` followed by a number for wave 1 and `| #` followed by a number for wave 2. Split literal pipe delimiters, trim each cell, exclude outer delimiters. Wave 1 has uniformly eight cells. Wave 2's board header specifies ten; seven rows have the wrong count. No embedded-pipe expression explains these seven: inspection shows appended historical fields or missing fields. For width-independent comparisons, sum every cell fragment rather than silently drop malformed rows.

| Measure | Wave 1, closed | Wave 2, captured closure |
|---|---:|---:|
| Whole-ledger characters | 86,890 | 80,527 |
| Task rows | 29 | 68 |
| Total trimmed cell/fragment characters | 52,159 | 39,959 |
| Largest individual cell | 17,433 (#26 remaining) | 4,020 (#18 state) |
| Largest row, all cells | 24,180 (#26) | 4,281 (#18) |
| Four largest rows / all cell text | 44,706 / 52,159 = 85.7% | 10,131 / 39,959 = 25.4% |
| Rows with expected cell count | 29/29 | 61/68 |

Wave 2 is much less concentrated and its maximum cell is 76.9% shorter. The earlier shorthand “21–24k-character cells” should not replace a precise comparison: in the verified wave-1 census, 24,180 is the **whole #26 row**, while its largest individual cell is 17,433. These are different measures.

The [ledger study](../ledger-append-log-research.md) captured wave 2 earlier at 25,301 characters, 39 rows, maximum cell 616. At closure the ledger is **3.18×** that size and the maximum cell **6.53×** as long. This is growth between specified cuts, not a forecast or a per-hour rate.

Concrete current-view failures (S):

- #18 begins “in progress, phase A” and later says the task is complete; its commit cell remains a dash and remaining cell still requests 18a-1/18a-2 results.
- #20 still says the closure draft is opened, while the closure report is final.
- #31 places most result/history in the budget cell and retains a separate “in progress” field after its landing.
- #9 retains its old style-gate wait and the stale dependency correction alongside final completion.
- #53 contains unrelated #31 RESULT content in an extra field.
- Widths are **#52: 9 cells; #7/#31/#50/#53: 11; #9/#14: 12**, versus ten specified. A table renderer or positional importer cannot safely infer the intended current facts.

The pathology also moved outside cells: the largest single ledger line is **5,881 characters**, the accumulated code-landings/gate paragraph. Compact task rows alone would miss it. The closure report separately contains 31,606 characters, repeatedly mirroring the same landings and dispositions. No overlap percentage is invented here.

**Judgment [I], high confidence:** phase-0 ledger work is confirmed and deserves immediate practical priority. Keep authored evidence, meaningful decisions, and honest residuals; replace repeated manual projection, not accountability. Generate the closure tables from the same source cut as the board. Freeze old waves as historical documents rather than repairing their tables or inventing native event history. How much prose already exists in structured completion/ruling summaries remains U until the final Mesh archive arrives.

## 5. Grade the five phase-0 items

These grades concern the [mesh-core shelf draft](../mesh-core-team.md), not roster performance; the team's own retro owns member accounts and box-score grading.

| Phase-0 item | Wave-2 grade | Concrete corpus amendment / measurement |
|---|---|---|
| **1. Versioned onboarding/recovery card** | **Retain; wave-2 saving unverified.** Wave-1 P evidence remains 53,508/56,768 surplus exact-repeat characters, 94.3%. Zero routing relaunches does not count onboarding, compaction, or pre-attribution recovery. The non-default-root incident adds a correctness requirement. | Bind card identity to team incarnation, context generation, role/contract revision and resolved root. A root/instruction change needs a new operative card or bounded correction, not suppression as a duplicate. Archive census must separate necessary fresh-context recovery from same-context replay. Inspect delivery receipts; do not count generated bytes as consumed bytes. |
| **2. One combined assignment rendering** | **Retain; double exposure not measurable yet.** Mesh still generates the card while the workflow authors contracts. Source capability and prose contracts do not prove two model exposures for each of 68 task IDs. | Preserve every five-line contract field, assignment token, effort/reason, immutable candidate/rubric, and operative wait. Pre-GO card and legitimate GO release are different obligations, not duplicate bodies to delete. Count authored/card pairs by assignment generation and exposure path, not by task or commit. Retain wave-1 8.9% ceiling only for its original task-text sample. |
| **3. No-response handling for info-only traffic** | **Retain; wave-2 turn saving U.** No inbox/turn archive is available. Silence in Git is not proof of silent delivery. | Align structured response expectation with wake priority. Mesh's inspected wake filter suppresses low-priority/empty messages; do not assume an `INFO ONLY:` prefix by itself has the same effect in every harness. Add empty-delta/no-model-wake and info-arrival-during-work controls; retain action-required corrections and GO. |
| **4. Lead notice dedup** | **Retain, and correct the causal wording.** Canonical alias support is present; duplicate lead exposure is U. Many manual mirrors confirm a projection burden but not duplicate notices. | Replace the draft's causal shorthand about ~1.29B cache reads with “prior full-wave cache reads; removable notice share unmeasured.” Dedup by source event/logical delivery identity across inbox/workflow/task metadata, with physical origin retained. Include monitor-import dedup from M1. Measure lead exposure and response turns; do not infer them from ledger edits. |
| **5. Ledger append-log + rendered projections** | **Strongly confirmed; promote alongside item 1.** 48 mainline ledger-only commits and malformed/stale final rows are direct full-wave evidence. | Add §4's population/counts and malformed-row/current-vs-history cases to the ledger study. Cover the non-table gate paragraph and closure report too. Keep pull-only rendering, small authored declarations, boundary snapshots, and actual-source gap coding. No per-event snapshot commits or new summarizer. |

**Sequencing [I]:** add a bounded pre-team contract-repair/measurement gate for M1–M3 and launch-root conformance, while retaining all five items. Begin ledger reader/gap work and onboarding measurement as the strongest complementary overhead lanes. Combined assignments and response/dedup trials still need their archive baseline. This recommendation neither launches the shelf team nor changes its seats; launch remains gated on phase 0, the team's retro, and operator GO.

## 6. Grade the overhaul decisions

“Confirm” means the observed need/invariant is strengthened. It does not certify an unbuilt journal, adapter, or crash protocol. “Unchanged/U” means wave 2 supplies no discriminating experiment. Amendments below are recommendations recorded here; the corpus itself was not edited.

### Task-thread/read-side decisions

| Decision in the [threads study](../mesh-task-threads-research.md) | Grade and wave-2 consequence |
|---|---|
| Bounded task-evidence projection; current card + delta + optional history | **Confirm.** #18/#20/#31 stale board content and repeated landing/ruling mirrors show why a chronological conversation is insufficient. Add final wave-2 rows as projection fixtures. |
| Directed delivery retained; no default task/all-team subscriptions | **Confirm the constraint; economics unchanged/U.** #65 adds a real disclosure failure; no new input census justifies expanding readership. Keep the prior 1.68× participant-delta and 37.21× all-seat-full-read counterfactuals labeled wave-1 TE models. |
| Explicit task/assignment/multi-task links and logical post identity | **Confirm/reprioritize.** #18 spans three checkpoints; #60 splits from #38; #53's accidental #31 text and wrong predicted IDs show why prose association is unsafe. A dependency citation is not a new task or an authorization. |
| Review-task/candidate/ruling joins; no latest-verdict shortcut | **Strongly confirm.** 17 completed-unruled report touches are not 17 missing candidate reviews; #65's supplement is not a fully independent verdict. Add checkpoint/delivery identity and independence status to the proposed joins. |
| DMs/restricted reviews remain separate; task links do not publish them | **Strongly confirm, broaden the fixture.** #65 leaked through an artifact, not a thread. Freeze submitted RESULT by revision/digest; never expand a mutable worktree path for a blind reviewer. Release verdicts separately after lock. |
| Per-reader multi-source cursor distinct from owner restart hint | **Retain, no new runtime validation.** Scope/reader marks must not mutate GO or acceptance. Root/context/candidate references must survive recovery; archive lacks actual wave-2 restart/cursor behavior. |
| Deterministic recovery card; no permanent summarizer; no read-implies-uptake | **Confirm the design need.** Manual current-view repair is substantial; lack of a launch-health sample prevents claiming uptake rules succeeded. New source facts should recompute views, not create another authored summary every turn. |
| Offline projection trial against disciplined inbox control; measured economics | **Retain.** Add wave-2 data only after an exported source cut exists. Keep payload, headers, checks, extra responses, token classes and acceptance quality separate; do not substitute commit counts for the trial's coordination-cost bar. |

### Messaging storage, delivery, identity, and compatibility

| Decision in the [overhaul study](../mesh-messaging-overhaul-research.md) and addendum | Grade / concrete amendment |
|---|---|
| Retain accepted bodies independently of delivery; one logical post across fan-out | **Confirm retention need, wave-2 missing-body rate U.** Mutable RESULT exposure and `/tmp` evidence reinforce immutable references. They do not measure lost inbox bodies. Preserve original body, audience, source and correction identity. |
| One segmented team-incarnation journal with task/DM/group views | **Unchanged/U as topology.** Wave 2 supplies no append contention, fsync, recovery, or array-rewrite benchmark. The need for source authority is confirmed; JSONL versus retained arrays/SQLite is not settled by ledger commit counts. |
| Preserve tasks/workflow/rulings as separate authorities | **Strongly confirm.** GO representation and budget/report mismatch are contract issues, not reasons for conversation text to override lifecycle. Keep ledger annotations out of authoritative task transitions. |
| Stable incarnation separate from team name/account-root path | **Strongly confirm; elevate to every stage's conformance packet.** First non-default-root launch failed despite account selection. Pin authoritative root/identity through launch, reads, imports, receipts, recovery and exports; no old/new-root dual writers. |
| Canonical authenticated actor, logical lead plus physical provenance | **Confirm need; actual route completeness U.** Mesh alias normalization is source-verified but native foreign writers can still populate the historical alias. Do not merge identities by filename alone or authenticate imported claims by familiar author text. |
| Explicit visibility and independent-review policy, including artifact expansion | **Strongly confirm.** Add #65's immutable-submission test through summaries, previews, linked RESULTs and later owner annotations. Shared-UID cooperative policy remains weaker than confidentiality. |
| Separate response expectation, priority, recipients and readers | **Retain.** Needed to test info-only wake suppression without widening audiences or suppressing actionable corrections. Current wave exposure cost U. |
| Stable lock, idempotent acceptance, manifest/sequence, crash recovery and bounded cursors | **Unchanged/U experimentally.** Retain fault gates, same-size replacement detection, torn-tail behavior and explicit unknown outcomes. No wave-2 artifact proves power-loss durability or the proposed recovery policy. |
| Bounded retention, DM expiry, repacking, inactive-recipient obligations | **Unchanged/U.** No reason to weaken privacy/expiry semantics to obtain a complete retro. Closure exports must preserve approved evidence without copying private bodies. |
| One Mesh team scheduler; per-member queues/adapters; no independent duplicate monitor | **Plausible, still I.** Launch failures do not prove competing delivery schedulers caused them. M1 requires a joined observation path, not necessarily a new scheduler. Retain fencing, fairness, failure isolation and daemon-independent durable acceptance. |
| Common claim/present/record contract for hooks/push/fallback | **Retain.** No new adapter trial; source mismatch demonstrates the need to pass actual producer data through consumers, not only synthesize ideal fixtures. |
| Honest receipt ladder; no exactly-once model exposure or read-equals-work | **Strongly retain.** Zero relaunch/monitor counts and a completed review are inadequate proxies for launch health, quietness or independent uptake. Add stage-specific coverage counts rather than a single success boolean. |
| Lifecycle notification reconciliation/outbox boundary; no fictitious multi-file transaction | **Retain.** Manual mirrors do not supply atomicity. Existing source IDs should link each observation; no replay of the lifecycle action to repair missing fan-out. |
| Guarded tmux fallback; explicit server/pane/PID/session identity | **Confirm runtime importance.** Pane-space repair belongs to launch machinery. Native transport can reduce terminal-writing exposure but cannot replace pane allocation, root binding, effort/focus/stop controls. |
| Taurhaus daemon/native writer boundary; Windows reads do not gain UNC write authority | **Strongly retain.** Account-root failure reinforces explicit authority resolution. No wave-2 evidence favors reopening Windows-side team writes. |
| Preserve one compaction path and idempotency; bounded recovery without replaying everything | **Retain; wave-2 behavior U.** Mid-wave install is a reason to measure context generations, not evidence that compaction or onboarding replay actually occurred. |
| Scope/scanner/telemetry remain observational consumers | **Confirm, with correction.** M1/M2 show additive schemas alone do not ensure compatibility. Add real serialized-source conformance, source/exclusion counts and version/cut metadata; preserve old metrics under their old names. |
| March JSONL reversion rationale | **Operator decision unchanged.** Prematurity, not architectural failure, remains the recorded explanation. Wave 2 offers no new evidence on code reuse. |
| Frontier-duo native push first-class; agy/Grok guarded fallback indefinitely | **Priority retained; feasibility unchanged.** The wave's Claude/Codex roster makes it relevant but does not validate the ~80% future-team assumption or any new adapter. Do not spend this wave's launch pain as evidence of native delivery uptake. |

### Native-push probe dispositions

The [probe report](../native-push-probe-report.md) is **P in this review**, including its version-pinned S-runtime observations. No vendor/harness behavior was rechecked.

| Probe decision | Wave-2 grade |
|---|---|
| Codex 0.153.4 app-server verified for a newly owned scratch thread; idle `turn/start`, active `turn/steer` with expected turn | **Retain, scope unchanged.** Not a hot attachment to live Taurhaus TUIs; two model responses in one accepted turn remain a real cost distinction. No wave-2 rollout evidence. |
| Claude 2.1.263 channel surface exists; tested print/stream notification lacked uptake within 25 seconds | **Retain NO-GO-for-now for that path.** Do not infer general TUI failure or account denial; non-default Mesh root failure is a different mechanism. |
| Twelve Codex hook names recognized; new hook execution/context injection not established | **Retain configuration/execution distinction and NO-GO for a new hook adapter.** Existing compaction is not invalidated. |
| Session ownership, permission/resume/compaction/concurrent-user cases gate deployment | **Strengthen test matrix with the account-root tuple and context-generation recovery.** Keep common claims and unknown-outcome handling; no blind tmux resend following ambiguous native submission. |

### Ledger design decisions and intake amendment

| Decision in the [ledger study](../ledger-append-log-research.md) | Grade / concrete consequence |
|---|---|
| Separate small pull-only ledger journal; no messaging service/watcher | **Strongly confirm the separation.** Manual projection can be removed without waiting for transport replacement. The full-wave maintenance census strengthens the case, not the choice of a daemon. |
| Only new authored outcome/remaining/decision/note gaps; reuse existing summaries/rulings | **Confirm principle; coverage fraction U.** Do not assert most wave-2 prose is already structurally retained until source-to-proposition joins exist. Count retention gaps separately from schema gaps. |
| Entry/amend/tombstone with reason, current-head CAS, immutable history and authority checks | **Confirm semantic need.** #31's corrected cause, #65's exception, and revised remaining items need explicit provenance. No new crash/concurrency proof from this wave. |
| Typed candidate/landing/rubric/instrument/result/review references | **Strongly confirm.** Preserve 18a-2 before/after fixture qualification, #65 exposure status, and docs-only candidate/landing equivalence. Never let a later PASS overwrite what an earlier candidate was measured to do. |
| Team-incarnation root, policy manifest, native writer ownership and durable committed cut | **Retain; root case raises activation priority.** No account-root scan fallback or current-roster inference in historical replay. |
| Compact current table + linked history, explicit acceptance/remaining dimensions | **Strongly confirm.** Add malformed #31/#53 and the 5,881-character non-table gate paragraph to acceptance fixtures. Check closure tables against the same cut. Do not hide required limitations to meet a byte cap. |
| Boundary snapshots/input bundles, pure render, historical waves immutable | **Strongly confirm.** `/tmp` retention and this concurrent capture show why a reproducible cut matters. No per-review snapshot commits, Markdown migration-on-read, or historical native-event fabrication. |
| Markdown with YAML front matter for authored prose; JSON for programmatic emitters | **Retain the review amendment as binding.** Wave 2 supplies no controlled escaping-error measure, but long prose makes argv/JSON-escaping authoring undesirable. Both intakes normalize through the same validated writer; no new command examples enter roles before execution conformance. |
| Body/reference limits, CAS/durability/cursor tests, single-segment start | **Unchanged design bounds, not validated optima.** A 4,020-character historical state cell is not an argument to raise the proposed 4-KiB body limit: it is mixed history to split/link. Character length is also not UTF-8 byte length. |
| A→B→C→D→E rollout and fallback continuation | **Retain.** Reader, gap specification, isolated writer, replayable exports/role conformance, then explicit next-wave adoption. No scope expansion into a second task database or retro repair. |

### Migration stage gates

The communication-overhead “phase 0” and the overhaul's storage/delivery “stage 0” are distinct. Preserve that naming distinction in the shelf draft.

| Overhaul stage | Wave-2 disposition |
|---|---|
| 0 — offline reader/conformance | **Raise priority.** Include actual 0.2.29 token markers, monitor metadata/workflow, non-default roots and #65 visibility. Synthetic desired sidecars alone are insufficient. |
| 1 — retention/links in isolated opt-in teams | **Retain.** Explicitly label native-writer coverage and immutable artifact retention; shadow failure must not cause duplicate delivery. |
| 2 — one scheduler on legacy storage | **Retain gate.** No wave evidence justifies skipping owner fencing, restart, or mixed-harness tests. |
| 3 — canonical acceptance journal | **Retain gate.** Root moves, paired writers/readers and fail-closed format negotiation are more urgent after this wave. No flag-only downgrade after canonical writes. |
| 4 — native boundary adapters | **Retain gate; not validated by wave 2.** Hook parser support is insufficient; require context uptake and compaction/dedup conformance. |
| 5 — native push/managed hosting | **Retain frontier priority and probe dispositions.** Requires launcher-owned session/account identity and controlled rollback/relaunch; no live-TUI conversion inferred. |

## 7. Concrete corpus change list

All changes below are proposed, not applied. They preserve the team's concurrent retro and the operator's stage gates.

1. **`mesh-task-threads-research.md`:** add a separately dated wave-2 evidence section with the five documented wait/release pairs, malformed/stale current-view examples, #65 artifact contamination, and 68 board rows versus 64 report touches. Leave the original inbox census/TE economics unchanged. Add immutable submitted RESULT revision and independence status to the bounded projection contract; add safe dependency-correction provenance.
2. **`mesh-messaging-overhaul-research.md`:** correct any implication that source-tagged monitor records already flow into routing. Add M1's producer-to-reader seam and M2's string/boolean mismatch to stage-0 conformance. Require actual Mesh serializer output as fixture input, a complete non-default-root lifecycle, long-wait TTL parity, and explicit deadline-clock semantics. Add report team/cut/version/coverage fields. Keep runtime launch repair distinct from native transport.
3. **`native-push-probe-report.md`:** add only a follow-up note pointing to the launch-root/context-generation acceptance requirement. Do not alter observed outcomes, tested versions, or NO-GO dispositions; there was no new probe.
4. **`ledger-append-log-research.md`:** append the closed-wave census with both Git denominators and the exact hashes; distinguish row from cell maxima; add malformed-width and non-table accumulation fixtures. Extend acceptance joins to disclosed/waived independence, and require shared-source-cut closure tables. Preserve Markdown/front-matter authored intake and the existing small independent authority.
5. **`mesh-core-team.md`:** promote ledger work alongside onboarding; add a bounded existing-contract repair/measurement prerequisite for M1–M3. Correct the cache-read causal shorthand. Require overhead results with archive coverage before team activation, while leaving roster changes to the actual retro and operator decision.

For follow-on implementation, the minimum meaningful tests are **Mesh-produced awaiting-GO → Taurhaus deadline at half/full deadline → GO/reassignment**, **Mesh-produced monitor nudge → report exactly once**, **two ignored nudges → one launch-health record without a third resume prompt**, **real oversize ruling → attributed count with its subsequent raise retained**, and **submitted RESULT expanded after owner appends a verdict → reviewer still receives the frozen allowed bytes**. Include missing attribution, expired/no deadline, member/task wait variants, late arrivals, and explicit coverage failures. These are proposed isolated tests; none was executed against this wave.

## 8. What must wait for the archive

This is an analysis completion boundary, not a request to inspect live state. A supplied closed export is required for the following measurements:

| Question | Required exported evidence and method |
|---|---|
| Were waits actually respected? | Task snapshots plus assignment/workflow events, relevant member status/activity, operational snapshots, and retained pass/skip observations. Join by incarnation/task/assignment; enumerate configured eligible half/full-deadline crossings and reasons for every suppression/non-action. Final cleared metadata alone cannot reconstruct every wait interval. |
| Why zero raw monitor events; was launch-health used? | `idle_monitor_records`, workflow `message_sent`/uptake events, monitor configuration and retained health/activity evidence. Count distinct source event/message IDs before report attribution; reconstruct ignored-nudge/reset sequences. If no eligible cycles exist, grade unexercised; if raw nudges exist but report zero, quantify M1's loss. Missing pass observations remain U. |
| Where did #31's oversize go? | Task ruling seq 3 and amendments, exact `field/value/by/at`, owner/assignment history, launch sidecars and report cut. Reconcile raw, recognized, attributed and excluded counts separately; preserve the working-draft/committed-candidate distinction. |
| What explains 68 board rows versus 64 touches? | All task IDs/lifecycle states and telemetry, including `_unattributed`, role/model history and actual included teams. Publish the missing/duplicate attribution inventory rather than force a one-to-one interpretation of rollup sums. |
| Did canonical lead routing work? | Both physical lead inbox origins, retained accepted/sent bodies, external/native observations, IDs and receipts. Compare canonical destination and actual exposure without treating an unread flag as non-delivery. |
| How much onboarding was redundant? | Bodies, delivery attempts/receipts, session/context generations, launch/recovery/version/root changes. Group same-seat exact/semantic cards within generation; separate necessary recovery and instruction changes from repeated exposure. Compare with wave-1's same estimator and declared coverage. |
| Were assignments double-rendered? | Authored contracts, generated notices, assignment tokens, GO releases and exposure paths. Pair per assignment generation; identify distinct fields to preserve and actual duplicate exposure. Report bytes/TE and turns separately; do not multiply wave-1's nine pairs by 68. |
| Did info-only/no-response/dedup reduce work? | Response-expectation/priority, delivered bodies, read/check output and turn-level usage with source links. Count empty checks, wake-only turns, replies and duplicate lead exposure; distinguish fresh/cache-write/cache-read/output tokens. |
| What did the launch/hotfix cost? | Operator-supplied installation/build identities and timestamps, failed launch outcomes, runtime attachment generations, root-resolution diagnostics and recovery actions. Never infer install time from Git or read account roots to fill this gap. |
| How much ledger work is automatable? | Final structured completions/rulings/results plus authorized artifact bundle. Code propositions as retained source fact, missing link/adapter, existing-authority schema gap, new authored declaration, or unavailable history. Measure authored effort prospectively; do not convert changed-line churn into savings. |

The export should identify capture intervals, team incarnation/root provenance, source digests, event-window and retention gaps, and any withheld bodies. It must support offline replay without resolving paths into a live root. A missing full-wave source cannot be reconstructed from the final hand ledger and then credited as machine-retained evidence.

## 9. Reproduction notes and completion assessment

The following read-only Git operations define the commit census; they do not discover team state:

```text
git -C /home/mstie/projects/taurjob rev-list cac6e00..24ed7c3
git -C /home/mstie/projects/taurjob rev-list --first-parent cac6e00..24ed7c3
git -C /home/mstie/projects/taurjob diff --numstat <commit>^1 <commit>
git -C /home/mstie/projects/taurjob diff --unified=0 <commit>^1 <commit> -- docs/wave-2/ledger.md
git -C /home/mstie/projects/taurjob show 58ccd47:docs/wave-2/ledger.md
```

Filter changed paths, not subjects; classify ledger-only only when every changed path is a ledger. For character churn, omit `+++`/`---` headers, take content after the first `+`/`-`, and sum Unicode lengths without newlines. For tables, use the §4 literal-delimiter method and retain malformed-width rows in width-independent totals. Git ancestry and hashes make reruns independent of concurrent retro edits.

**Confidence:** high in the source contract mismatches, report semantics, commit/cell measurements and need for generated current views; medium in causal credit to working agreements based on the team's committed evidence; low/unverified in actual monitor/deadline coverage, messaging duplication, launch downtime and token savings. No model-family performance grade is inferred from these machinery measures.

The complete commissioned review is delivered: whole-wave machinery outcomes including launch, all seven round-5 mechanisms, measured ledger comparison, all five phase-0 items, overhaul/probe/ledger decision grades, concrete corpus amendments, and an explicit archive-dependent measurement plan. The concurrent team retro and live machinery were left untouched. No implementation or commit was made.
