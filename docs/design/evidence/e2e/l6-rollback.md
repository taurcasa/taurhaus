# L6 rollback — run5 IN PROGRESS (step 4 PASS)

## Run3 result (2026-09-11)

**Recorded runtime FAIL at step 3, classified as a harness-predicate stop.**
The lead successfully stopped the canonical owner and downgraded with B pending.
The next assertion failed: `required resulting format 1 not observed`.
That expectation came from the superseded **run2** ruling and its helper, imported
unchanged into run3; the binding **third-run** ruling imposes no format-1 value.
Mesh `3015cb0` publishes the documented legacy `messaging_format: 0` and
`delivery_owner: members` during downgrade. This trial did not measure the
subsequent ownership command, marker removal, fresh C delivery or full reconciliation.
The review correction takes the explicitly offered **no-rerun** option: preserve
this trial, correct its classification, and keep steps 4–6 unmeasured. No product
change or additional paid attempt was made.

The controller exited **1** after **70.10 seconds**. Steps 4–6 were not run because the
controller treated its inherited assertion as a failed step. Mandatory teardown passed.
All required gates subsequently exited **0**. The original execution could not
launch its independent Opus lens; this correction addresses the supplied Opus
round-1 findings. No full workflow PASS or release approval is claimed.

| Ordered run3 step | Outcome | Classification and evidence |
| --- | --- | --- |
| 1. Initialize; deliver/read A; observe B pending during ordinary work | **PASS** | S-runtime. Production canonical initialization, attributed activity, one A submission/read/reply, B accepted with no begun transport and scheduler opportunity. [Pending boundary](l6-rollback/run3/run/step1-pending-boundary.json). |
| 2. Lead stops owner, waits for exit/marker, downgrades with B pending | **PASS, operation only** | S-runtime. Stop exit 0; owner exits, operator marker exists; downgrade exit 0; B is projected under its original logical ID with disposition `pending`, `read: false`. This does not assert the required resulting format/ownership boundary. [Commands and report](l6-rollback/run3/run/step2-command.json). |
| 3. Settle; retry once if temporarily refused; verify committed format boundary | **FAIL** | Harness-predicate stop. No refusal or retry occurred. Ordinary work settled with attributed idle; format 0 was rejected by the inherited run2 format-1 assertion. Run3 does not require 1. Config already says `members`; the explicit ownership command remains untested. [Outcome](l6-rollback/run3/run/step3-outcome.json), [analysis](l6-rollback/run3/analysis.json). |
| 4. Explicit members handoff, request/report/epoch boundary, marker removed last | **NOT RUN** | Blocked by step 3. No `team delivery --owner members` call, no handoff request and no `delivery_owner_changed` event. Marker remains. |
| 5. Start guarded RC member executor; account for B; send/reply C | **NOT RUN** | Blocked by step 3. Controller never starts the executor or sends C. An RC member executor attributed to Taurhaus delivered B during step 3; that observation is not substituted for step 5. |
| 6. Explicit read/ack and reconciliation across both boundaries; export/teardown | **NOT RUN** | Final read/ack sequence and two-boundary reconciliation were not attempted. Failure export and teardown separately passed. |

### Owner-stop product observation

**Expected #179 named skip observed; owner restart not observed.** At
`04:15:41.573Z`, the private daemon emitted `coordination.team_daemon.skipped`
with `reason: owner_stopped_by_operator`. Its subsequent self-heal passes reported
`team_daemons_ensured: 0`, `owner_ensure_refused: 1`. The census retained **75 samples**,
maximum adjacent interval **0.5694 seconds**, with no owner after the stop.
The pre-stop sample identifies host PID **1620584**, start ticks **32064476**;
Mesh's private-namespace stop output identifies PID **196**. The operator marker
records epoch **2**, lead, and `04:15:37.060062861Z`.

The observation window ends at the step-3 failure: step 4 was not reached.
The marker survives downgrade and teardown export. The complete sanitized daemon
JSONL contains **235 rows**, including shutdown; it is not an event-filtered extract.
[Daemon JSONL](l6-rollback/run3/run/taurhaus.log.jsonl),
[owner census](l6-rollback/run3/run/owner-census.jsonl),
[pre-stop identity](l6-rollback/run3/run/step2-owner-before.json).

A **Taurhaus-started member executor** (host PID **1628609**, start ticks
**32067925**) was observed after downgrade. This starter attribution is inferred
from the retained `mesh daemon --pane %2 --team l6-rollback-run3 --name alpha
--claude-dir …` argv matching Taurhaus's `spawn_mesh_daemon_at_root`
([system.rs](../../../../src-tauri/src/coordination/runtime/system.rs), lines 279–300)
and the private namespace's actors: the controller never reached its executor
start, leaving the private Taurhaus daemon as the coordinator that could spawn it.
No parent PID was retained, so this is source/argv attribution rather than a
recorded parent-child edge. **Taurhaus product observation:** that executor
injected B while `owner-stopped.json` was still present and before the explicit
handoff command was attempted.
B received one legacy `tmux_injected` row at `04:15:41.600Z`, then alpha explicitly
read B and replied `B-2c3955c5` at `04:16:14.479Z`. This is not a restarted team owner.
The exact process, workflow, native tool-result and reply rows are retained in
[analysis](l6-rollback/run3/analysis.json).

### Identity and boundary accounting

| Marker | Original logical message ID | Delivery/projection ID | Final observed disposition |
| --- | --- | --- | --- |
| A-943bd6e0 | `36328545-20b9-40d4-af7b-4614dd34d94f` | `713839d3-91e4-4398-87bc-532827e3c23a` | One canonical submission; alpha explicit read and assistant reply. Downgrade reports `consumed_by_read`; retained legacy projection remains read. No replay observed. |
| B-2c3955c5 | `b026d303-9ac7-46d8-987f-9b34a52c19bb` | `55e21660-dc01-4ae4-914c-0b186cf5ee65` | Accepted/pending while working, scheduler opportunity observed, no canonical submission. Downgrade preserves pending body/ID and unread state. Later one legacy submission, alpha tool-result exposure/read and assistant reply. |
| C | — | — | Never sent; fresh legacy delivery criterion untested. |

Canonical history is identical before downgrade, immediately afterward and at
failure export. The authority marker is `transition: complete`, with legacy cut
`5f7ae87c5a1228e236077cbac00cd977a5bd05b7e442a5e4b23a4e72c1325b09`.
The format operation advances epoch **2 → 3**, but produces no separate ownership
boundary event. The downgrade report and rollback compatibility projection are
retained; step 4's durable handoff requirements remain untested.
**Untested source-level observation:** Mesh's
`/home/mstie/projects/mesh-l6/src/delivery/ownership.rs:395-404` takes the same-owner
path when owner is already `members` and no handoff file exists: it verifies
`rollback.json`, calls `clear_owner_stop()`, and returns `Ok(())` without creating
a new handoff or epoch. This is a mechanism to examine in a future execution,
not an observed ownership-command failure. Mesh `USAGE.md:74-81` documents
stop → downgrade → members handoff.
[Boundary snapshot](l6-rollback/run3/run/step2-after-format.json),
[RC source excerpt](l6-rollback/run3/mesh-downgrade-source.txt).

### Candidate, harness and spend

Checkout stayed on `feat/e2e-l6-rollback`; execution started at `50a36d848858294d9c7b87060aee0bece411b21c`,
with product source identical to merged main **33443fe79b8e835a69ef3edf2bece38a9f75646a**.
`COMPLETION_FLUSH_TOLERANCE` occurs **2** times. No rebase was needed because that
base is already an ancestor. Mesh HEAD and `release/overhaul-rc` both resolve to
**3015cb0fda5328d1f8c793b528275e6205683208**, with no source or descriptor diff.

`just build-daemon` used this checkout's `src-tauri/target`; the Mesh build used
`/home/mstie/projects/mesh-l6/target`. Both exited 0. Cargo admission polled the
required machine-wide predicate and found fewer than three processes at each
admission; `CARGO_BUILD_JOBS=1`. Binary digests and command tails are in
[builds](l6-rollback/run3/builds.json); census in
[cargo-census](l6-rollback/run3/cargo-census.jsonl).

The private daemon used port **45937**, protocol **27**. Codex **0.153.4**, model
**gpt-5.6-luna**, effort **low**, ran in one real tmux seat `alpha`; lead was the
real Claude login-only seat, with no Claude model turns. Production initialization
used the builder's canonical `messaging` policy and creation-time `delivery: tmux`.
The scratch bin held both native Codex siblings. Only the explicitly authorized
account-b `auth.json` was copied, at mode 0600, into an otherwise empty scratch
CODEX_HOME. The copy was deleted. No token/auth contents or installation IDs are
retained. Bubblewrap hid operator homes and used private PID/tmux namespaces;
all harness, data, temporary and project roots were disposable. Inherited TMUX
was absent. No descriptor edit, fault injection, install, release or product edit.

| Spend/input | Observed metered USD | Notes |
| --- | --- | --- |
| Onboarding: `01a08ead-2da2-7221-8383-af06dcdc4ac8` | 0.00161804 | Completed metered turn. |
| A: `01a08ead-51fb-7963-ae25-6dc7ccb5d00a` | 0.00427332 | Completed metered turn, including tool calls. |
| Ordinary response plus B processing: `01a08ead-959d-7d33-9153-e6b632e86d3a` | 0.00506960 | B was injected during this active turn; native usage is not separable into an independent B cost. |
| Notify-only ID: `01a08ead-2f79-7cf2-8276-6de193ec9722` | Unknown/not independently billed here | No corresponding native `task_started`; not counted as an additional model turn or free input. |
| Claude lead | 0 model inputs | Login-only; no Claude model spend observed. |
| **Total** | **0.01096096** | **4 inputs**, **3 native turn IDs**, one attachment/context generation; below 10 inputs and $0.20 metered. |

The four inputs are onboarding, A, ordinary work and the legacy B presentation.
All three native turns completed with token usage. The ledger's deliberately
inflated all-token sensitivity estimate is **$0.2554164**, not measured cost or the
metered budget metric. Rates are the inherited Luna packet rates ($0.20/M input,
$0.02/M cached input, $1.20/M output); figures are API-equivalent estimates, not invoices.
Every individual usage delta is retained in the [cost ledger](l6-rollback/run3/run/cost-ledger.json).
Metering did not gate a lifecycle operation. Implementer spend is externally
metered and unavailable here; no independent reviewer was launched.

### Red-first checks, gates and cleanup

The new offline guards first failed against the inherited controller: **4 failures**
(order, metering wait, missing pending-input checks, missing owner census), then
**2 behavioral failures** (actual `team-daemon start` process selection and
`reader_name: alpha` receipt recognition). All **6 now pass**, without reading
credentials or starting CLIs. Regression comments name the historical controller
commits. [Tests](l6-rollback/run3/controller_test.py),
[first red](l6-rollback/run3/red.txt), [predicate red](l6-rollback/run3/predicates-red.txt),
[green](l6-rollback/run3/green.txt). The live assertion failed in the inherited
harness predicate; no product fix was attempted in this evidence lane.

| Exact command from checkout root, after teardown | Exit |
| --- | --- |
| `just check-quick` | 0 |
| `just lint` | 0 |
| `just test-contracts` | 0 |

[Gate result](l6-rollback/run3/checks-result.json). No `src-tauri/` or `src/` diff;
`just test-rust-unit` was therefore not required. No full `just check` was run.
[Cleanup](l6-rollback/run3/run/cleanup.json) records no survivors, closed port,
removed auth copy and removed scratch root. No foreign process was signalled.
Pane excerpts are bounded to 60 lines; duplicate files resolve through the export
manifest. A single benign initialization observer error involved an absent
activity-path value; it did not abort a wait or remove daemon rows.

### Deviations and reproduction

- The specified lane-2 checkout no longer exists (read-only git command exited 128).
  Its versioned run3 controller/evidence is retained in this checkout; the run3
  controller reuses the lane-6 run2 derivative with the required stop, pending and
  metering corrections. No other Taurhaus checkout was modified.
- The run2 format-1 expectation was incorrectly carried into run3. The binding
  third-run ruling does not impose it. The review fixes run3's helper call to
  select the RC's legacy format 0, while preserving run2's default expectation.
  No rerun was performed; steps 4–6 remain NOT RUN after the harness-predicate stop.
- The owner observation covers stop through failure, not a completed handoff.
  The named operator-stop skip is observed; no `rollback_pending` claim is made.
- The original execution recorded independent Opus review as unavailable:
  [review availability](l6-rollback/run3/review.json). This fix addresses the
  supplied Opus round-1 findings; no new reviewer or live seat was launched.

Original executed controller: commit `22643c6f`'s
`docs/design/evidence/e2e/l6-rollback/run3/controller.py`.
The current [controller.py](l6-rollback/run3/controller.py) includes the review's
format selection and credential-log corrections and has not been run live.
[Controller provenance](l6-rollback/run3/controller-provenance.json) retains the
original run's hashes; they do not describe the corrected helper/controller.
From this checkout, the controller accepts `--auth-source "$AUTHORIZED_SOURCE"`
(the spec's authorized account-b file only) and an optional `--out NEW_DIRECTORY`.
The output must be new; this is reproduction documentation, not permission to
restart the paid run. Exact RPCs/commands/exits/digests are retained in
[events](l6-rollback/run3/run/events.jsonl). Its raw `stopped` row preserves the
original erroneous runtime classification; the step outcome and analysis carry
the review correction. The only event redaction removes the credential digest
from `auth_copy`, retaining the source label, file list and mode.
The historical `run3/analyze.py` reproduces the **superseded** interpretation;
do not use it to overwrite corrected `analysis.json`. It remains unchanged to
respect this fix's named-file boundary.

## Run3 review correction verification (2026-09-11)

All four supplied findings were verified and addressed; none was skipped.
The five new offline tests live in the named `run2/run2_rules.py` file to respect
the requested file boundary. Run them from the checkout root:

```sh
python3 -B docs/design/evidence/e2e/l6-rollback/run2/run2_rules.py -v
python3 -B docs/design/evidence/e2e/l6-rollback/run3/controller_test.py -v
```

Red: the new suite exited **1** with **3 failures and 1 error**: credential
fingerprint retained, stop classified `mesh`, missing run3 format selection,
and unsupported per-run format argument. A direct call with a synthetic valid
format-0 boundary also exited **1** with the exact recorded assertion,
`required resulting format 1 not observed`. Green: **5/5 new tests** and
**6/6 existing controller tests** passed, both commands exit **0**. These checks
read only source/retained evidence or disposable fixtures, never credentials,
and invoke no CLI. The tests preserve run2's format-1 requirement while checking
run3's format 0, completed transition, verified digest and unchanged history.
Regression comments identify `22643c6f`, `011c2738` and `b840d330`.

The correction's gates were rerun after the original teardown; no live trial
was restarted. Original PID/start-tick identities were checked again: **no
survivors**, and `/tmp/th-l6-5f67mhpk` remains absent. Cargo admission used the
specified `pgrep` predicate and found **0 / 1 / 2** existing processes before the
three respective gates, so no waiting was needed. Each gate used
`CARGO_BUILD_JOBS=1` and this checkout's `src-tauri/target`.

| Exact review gate from checkout root | Exit |
| --- | --- |
| `just check-quick` | 0 |
| `just lint` | 0 |
| `just test-contracts` | 0 |

Gate logs and admission census are local under `.check-logs/l6-run3-review/`.
No `src-tauri/` diff was introduced; the Rust unit gate was not required.
The complete original daemon JSONL, cost ledger and outcomes of unexecuted steps
are retained unchanged. **Additional trial inputs/spend: 0 / $0**; historical
run3 remains **4 inputs / $0.01096096 estimated metered**, with each turn listed
above. Implementer cost remains externally metered and unavailable here.

The accepted no-rerun alternative is the material limitation: steps 1–2 remain
S-runtime PASS (step 2 operation only), step 3 remains a recorded **harness FAIL**,
and steps 4–6 remain **NOT RUN**. The corrected predicate is offline-verified;
marker removal, durable handoff/epoch behavior, fresh C and two-boundary
reconciliation have no new runtime evidence. The original analyzer is preserved
within the named-file constraint and must not overwrite the corrected analysis.

## Historical runs 1–2 (retained unchanged)


Review correction: step 1 passes; step 2 fails on a non-quiescence refusal.
The original controller continued and stopped at the step-3 readiness poll.
All three inputs finished and were metered; teardown and all required gates pass.

Run2 follows the **2026-09-11 amendment** in `docs/design/e2e-coverage-audit.md`,
section 6, read from the orchestrator’s main checkout: format first, ownership
second. The exact run2 controller and new sidecars live under
[l6-rollback/run2/](l6-rollback/run2/). Run1 below remains historical evidence.

## Historical run1 — ownership-first contract observation

The mandated ownership-first rollback is refused by Mesh `310144d` while the
team remains canonical: **`delivery: canonical_downgrade_required`**, exit **1**.
Step 1's busy-deferral corner is **UNPROVED**; steps 3–6 were not run. This is a failed operational rollback
under the audit, despite safe preservation of B. No product change, reordered
rollback, forced flag edit, descriptor edit, Mesh commit, install, or release.

| Ordered audit step | Outcome | Classification and observed evidence |
| --- | --- | --- |
| 1. Initialize; deliver/read A; observe B pending during ordinary work | **UNPROVED** | **Harness evidence gap.** Initialization and A's delivery/read are S-runtime verified. B was accepted while working, but the recorded heartbeat predates acceptance and no retained pending obligation names B. Scheduler opportunity and busy deferral are unproved; the PASS recorded in `82e2655b` is superseded. |
| 2. Lead requests `team delivery --owner members` | **FAIL** | **Mesh product contract.** Exit 1, `canonical_downgrade_required`; permanent prerequisite refusal, not a named temporary quiescence refusal. No handoff request, rollback compatibility file, epoch change, or ownership boundary. B intact. |
| 3. Settle/retry once; verify one committed ownership boundary | **NOT RUN** | Stopped at permanent step-2 failure as required; no retry or ownership-transfer claim. |
| 4. `team format --legacy --quiescent`; reconcile B | **NOT RUN** | No downgrade command or reconciliation report. Resulting format remains **2**, owner **team**. |
| 5. Guarded RC member executor; account for B; fresh C reply | **NOT RUN** | No member executor started, no C created, no legacy-delivery claim. |
| 6. Explicit read/ack; reconcile A/B/C; export and teardown | **NOT RUN** | No rollback read/ack or reconciliation. Mandatory failure export/teardown independently **PASS**. |

[Controller outcome](l6-rollback/run/controller-exit.json),
[step-2 command and exit](l6-rollback/run/step2-command.json),
[exact commands/RPCs/results](l6-rollback/run/events.jsonl).
The live controller exited **1** after **24.921 seconds**. All positive waits
provided 90–120 second deadlines; they could finish early on observed success.
Busy/lock refusals have bounded retries; this permanent prerequisite refusal is
not transient and was not retried.

## What the rollback refusal preserved

The exact command, executed as the registered lead inside the private namespace:

```sh
/tmp/th-l6-qgmptfx3/home/.local/bin/mesh team delivery --owner members \
  --claude-dir /tmp/th-l6-qgmptfx3/claude --team l6-rollback --name lead
# exit 1: error: IO error: delivery: canonical_downgrade_required
```

`/home/mstie/projects/mesh-l6/src/delivery/ownership.rs:278–281` rejects
`Members` when `format::supported(..., 2) == 2`, before lead authorization,
quiescence checks, handoff creation, or epoch fencing. This explains the
observed refusal. Changing the command order would violate this lane's ordered
steps, so no alternate rollback was attempted.

| Identity / disposition | Observed value |
| --- | --- |
| Team incarnation | `19889ba6d58a872128b635b97967711ec220ee32b0bb7a2d7938a597ea8f7042` |
| Epoch / owner / format before and after refusal | **2 / team / 2** |
| Handoff request / rollback compatibility / ownership-change events | **Absent / absent / 0** |
| Alpha native session | `01a08d8f-8beb-7731-96ef-a3f92eb457f4` |
| Attachment / context generation | **1 / "0"** |
| Socket / pane / pane PID / start ticks | `/tmp/th-l6-qgmptfx3/tmux/tmux-1000/default` / `%2` / **135** / **30192684** (private namespace) |
| A marker / logical ID | `A-c79df580` / `d94184ac-d1e6-48e7-a17a-65ac2ed1fb9c` |
| A delivery ID / disposition | `d8a7d12f-6211-4407-8dc6-25015f435672` / **one submitted receipt + consumed_by_read** |
| B marker / original logical ID | `B-cd1489b7` / `cbb72528-371a-4bc0-90b6-1a150c0b318d` |
| B delivery ID | `244196ad-289f-46be-940e-0b984a60e1bd` |
| B legacy mapping allocated at acceptance | `62ded8e8-480d-4880-823c-332c9f514cfe`; this is not evidence of legacy projection or delivery |
| B final disposition | **Accepted, unread; no attempt_started, submitted, or read receipt.** Busy deferral is unproved because scheduler opportunity was not demonstrated. |
| C | Not created |

B's purported pending observation at `2026-09-10T23:03:31.537Z` used production activity
`likely_working`, observed at `23:03:31.085Z`. It was **acceptance without a
receipt while working**, only 204 ms after acceptance at `23:03:31.3336Z`.
The owner's heartbeat was `23:03:30.8987Z`, before acceptance, with no
`pending_since` or `last_defer_reason`; neither retained pending obligation
names B. This does not prove that the owner had an opportunity to defer B.
The busy-deferral corner is therefore unproved, and step 1 cannot be called
PASS. B's logical and delivery identities remain in the journal after refusal.
The corrected controller requires a heartbeat at or after this acceptance and
keeps polling within the existing 90-second deadline. The original runtime
sidecars, including the unsupported step-1 PASS, remain historical evidence;
this report supersedes that classification without rewriting observations.
The step-2 boundary snapshot was byte-identical to the step-1 snapshot and was
deduplicated with an explicit alias.
[Pending observation](l6-rollback/run/pending/cbb72528-371a-4bc0-90b6-1a150c0b318d.json),
[boundary snapshot](l6-rollback/run/step1-pending-boundary.json),
[A transport/read history](l6-rollback/run/A-history.json),
[complete canonical journal](l6-rollback/run/team/state/messaging-v2/segments/000001.jsonl),
[final runtime](l6-rollback/run/final-runtime.json).

## Candidate and isolation

Taurhaus product **`a7e6db7e484290542ac35ead468f9809968bb4f0`**, branch
`feat/e2e-l6-rollback`; private daemon ping confirmed **protocol 27**, version
0.9.7. Mesh built only in `/home/mstie/projects/mesh-l6` at
**`310144de1f7939e7fbb2d80ab42ebd00580ce15d`**. This guarded RC still reports
numeric version 0.2.29; it is not the excluded stock 0.2.29 executor.
There was no hosted seat and no descriptor edit. Native Codex reported
**0.153.4**, with actual launch **gpt-5.6-luna / low**.

| Copied binary | SHA-256 |
| --- | --- |
| Taurhaus daemon | `5ca8150fe8c15d3bf770a51b7eaf7b94be7b18b0bd56f82c0521d500563aa21e` |
| Mesh RC | `e3ddbd2184433aa08889f3fcc3bdc33443057f2d7fa7e819e97facd494e22991` |
| Codex | `56ef98ab4032d317ab26e9b5e5a175650717351edb16ed9cde0cb6d1734d62da` |
| codex-code-mode-host | `3e85d67471825f73d02ff5f7e047ca1f6ca8caa3f59e4c6e8d9ca6ca7302cb45` |

Runtime HOME, all harness/config/data/temp roots, project, and tmux socket were
scratch-only. Bubblewrap hid operator homes and supplied a private PID namespace;
TMUX was absent. The private daemon used probed port **47513**, never 17233.
Exactly the operator-authorized auth file was copied into empty CODEX_HOME with
mode **0600**, never logged/exported, then removed. The Claude lead stayed in its
unauthenticated startup/theme screen and took **zero model turns**.
Production `coordination.initialize_team` used the builder's actual canonical
messaging policy and creation-time `delivery: tmux` for both seats.
[Candidate](l6-rollback/run/candidate.json), [ping](l6-rollback/run/ping.json),
[initialization](l6-rollback/run/step1-operation.json),
[command-capable instructions](l6-rollback/run/scratch-AGENTS.md).

## Every observed spend and the metering limit

| Input / turn ID | Input / cached / output tokens | API-equivalent USD |
| --- | --- | --- |
| Onboarding — `01a08d8f-9547-7c20-ad72-6dd2e5545982` | 20,448 / 6,912 / 297 | **0.00320184** |
| A delivery — `01a08d8f-bda8-7692-a25d-ea84be176fa9` | 42,048 / 36,096 / 262 | **0.00222672** |
| Ordinary B-work — `01a08d8f-dcf1-70e0-90f9-4428484aa469` | **Unknown**; still active at mandatory failure teardown | **Unknown, not zero** |
| B pending reservation | No terminal submission or model turn | **0 observed** |
| C / recovery / retries / compaction | Not run | **0** |
| Claude model inputs | 0 | **0** |
| Implementer / independent reviewer | Enclosing orchestrator accounting | Not exposed to this lane |

**Known spend: $0.00542856 plus the unmetered ordinary turn.** The conservative
priced-subset estimate is $0.075666; it is not a total bound. Therefore the
**$0.20 total cap is unverified**, not claimed satisfied. The observed input cap
is respected: **3 submitted inputs, 3 rollout turn IDs, 4 reservations including
unsent B, one attachment generation**, against 10 allowed. A separate native
notify-only ID is retained and is not counted as another model input.
No metering predicate blocked the ownership command or any lifecycle operation.
No additional paid attempt was made.

The [cost ledger](l6-rollback/run/cost-ledger.json) retains all five usage rows,
individual deltas and turn identities. Rates are inherited packet estimates:
$0.20/M uncached input, $0.02/M cached input, $1.20/M output; not invoices.
The native transcript export precedes namespace shutdown, so it cannot establish
an eventual final billing row for the interrupted turn. This is an evidence
limitation, separate from the proven Mesh refusal.

## Export, teardown, tests and gates

The **complete emitted daemon JSONL (101 rows)** is retained, including rows
emitted during shutdown; no event-selection or tail filter. Stderr records
`Received shutdown signal` and `taurhaus-daemon shut down cleanly`. The journal,
workflow rows, native transcript, receipt/activity snapshots and passive lock
samples are retained. All pane excerpts are at most 60 lines. The deterministic
packaging pass trims trailing blank pane lines and aliases byte-identical
snapshots; complete streams are retained separately. Standalone command logs
remove trailing whitespace only, with hashes in
[the normalization record](l6-rollback/command-log-normalization.json); command
and exit JSON records retain their original output excerpts.
[Daemon JSONL](l6-rollback/run/taurhaus.log.jsonl),
[daemon stderr](l6-rollback/run/daemon-stderr.txt),
[manifest](l6-rollback/export-manifest.json), [packaging script](l6-rollback/pack.py).

Teardown used only owned PID/start-tick identities and the daemon's normal
SIGINT shutdown. The namespace reaped its descendants. **No scratch daemon,
team/member executor, tmux server, Codex, code-mode host or Claude survives;
port closed, auth copy removed, scratch root removed.** A separate read-only
audit verified all 69 retained files, 13 aliases, complete JSONL rows, bounded
panes, absence of credential-shaped data/operator-home paths in runtime exports,
and no surviving recorded process identity.
[Cleanup](l6-rollback/run/cleanup.json), [audit](l6-rollback/final-audit.json).

**22 offline tests pass**: six lane tests plus 16 inherited shared-harness and
credential checks. Initial red: transient retry, duplicate/unverified boundary,
and changed independent message state. Additional reds: the inherited
object-only parser raised `JSONDecodeError` on a generated legacy array; the
meter omitted a legacy executor input (`2 != 3`). All tests use synthetic values,
tempdirs and mocks; none launches a CLI or reads real harness credentials.
The parser regression comment names introducing controller commit `57c8ff36`.
No product regression was fixed.
[Original red](l6-rollback/red.txt), [array red](l6-rollback/legacy-array-red.txt),
[meter red](l6-rollback/legacy-meter-red.txt), [green](l6-rollback/green.txt).

| Exact root command | Exit | Outcome |
| --- | --- | --- |
| `just check-quick` | **1 initial; 0 retry** | Initial SessionHistory failures; unchanged exact-command retry: **2,519 tests pass**, Rust compile and typecheck pass |
| `just lint` | **0** | Rust/frontend/workflow/recipe lint pass |
| `just test-contracts` | **0** | **68 tests pass** (15 renderer, 20 harness, 33 boundary) |

[Exact gate results](l6-rollback/gates-result.json). The initial quick gate had three
SessionHistory assertion failures and one `transformCallback is not a function`
unhandled rejection. A focused run passed all 28 tests (exit 0), then the exact
full quick gate passed without product changes. Both runs are retained; no
flaky test was hidden or relabeled as a product fix.

No `src-tauri/` diff, so conditional `just test-rust-unit` does not apply.
The initial `just build-daemon` exited **101** because ignored `resources/mesh`
was absent. `just ensure-tauri-resources` exited **0**; the retry and Mesh build
both exited **0**. `bun install --frozen-lockfile` exited **0**. Cargo admission
was polled before every build/gate, waiting only if at least three Cargo
processes existed, with 30-second polls and a 30-minute bound. Targets remained
checkout-local; no gate overlapped the paid window.

## Reproduction and deviations

From this checkout root, with a new output directory and separately tracked
remaining run budget:

```sh
python3 -B -m unittest discover -s docs/design/evidence/e2e/l6-rollback -p '*_test.py'
just ensure-tauri-resources
python3 -B docs/design/evidence/e2e/l6-rollback/checks.py build
python3 -B docs/design/evidence/e2e/l6-rollback/controller.py --auth-source "$AUTHORIZED_SOURCE" \
  --out docs/design/evidence/e2e/l6-rollback/run-review
# Only after teardown:
python3 -B docs/design/evidence/e2e/l6-rollback/checks.py gates
```

`run-review` must not exist; choose another fresh directory for a later trial.
The default remains `run`, with the same must-not-exist guard. The historical
`pack.py` and `audit.py` target the original `run` packet, not the new output.
`AUTHORIZED_SOURCE` is the exact standing-authorized single file pinned in
[preflight.py](l6-rollback/preflight.py), shared by the standalone preflight and
[controller](l6-rollback/controller.py); there is no home fallback. Reproduction
starts at step 1 and stops at any failed step; it cannot skip ahead to steps 3–6.

- The prescribed L2 worktree was absent (Git exit 128). Its run-3 controller was
  recovered read-only from local commit `2690352e`; the same commit's run-5
  corrections supplied the mandated shared startup/submission/delivery rules.
  No other Taurhaus checkout was changed.
- Mesh's `docs/design/delivery-stage2b-brief.md` identifies itself as a membership
  addendum and says the full brief is absent. The separate
  `docs/analysis/delivery-stage2-s2b.md` exists and was read during this fix round;
  it is an implementation/isolated acceptance packet, not the full brief. The
  original lane read the addendum, `docs/analysis/journal-stage3-storage.md`,
  `USAGE.md`, and rollback source contracts. No alternate rollback order was
  inferred as permission.
- Build-resource preparation/retry was necessary; no tracked product change.
- The first quick gate failed in existing SessionHistory tests; focused diagnosis
  and one unchanged full retry passed. Lint and contracts each passed once.
- Step 2's permanent Mesh refusal stopped steps 3–6. The dollar cap remains
  unverified because the ordinary input was interrupted before final metering.
- Independent Opus evidence review belongs to the enclosing small-change
  workflow and did not execute inside this implementer lane. No review approval
  or complete workflow PASS is claimed.

## Review fix round

All five supplied findings were confirmed and addressed in the named harness
and report files; no product or original runtime sidecar changed. Four new
offline regression tests name introducing commit `82e2655b`. Before the fixes,
the 26-test suite exited **1** with four failing subtests and two errors:
working activity admitted missing/stale owner heartbeats, standalone preflight
rejected the authorized pin (78), and output selection was unavailable.
After the fixes the same suite exited **0**, with **26 tests passing**, including
equal/newer heartbeat acceptance, submission exclusion, shared pin enforcement,
CLI output selection/default, and existing-evidence preservation. Credentials
were mocked; no real harness or credential access occurred in these tests.

No paid rerun occurred: **0 new Codex/Claude inputs, $0 new seat spend**, and no
daemon, member executor, tmux server or model seat was started. Historical spend
remains $0.00320184 + $0.00222672 + one unknown ordinary turn; its total cap
remains unverified. Step 1 is unproved (harness); step 2 remains FAIL (Mesh);
steps 3–6 remain NOT RUN. The dead boundary bindings and unused controller state
were removed. Replaying the retained B observation through the corrected
predicate returned no pending evidence, as required.

| Fix-round exact root command | Exit | Observed result |
| --- | --- | --- |
| `just check-quick` | **0** | Rust compile/typecheck and all **2,519** frontend tests pass |
| `just lint` | **0** | Rust/frontend/workflow/recipe lint pass |
| `just test-contracts` | **101 initial; 0 retry** | Retry passes all **68** contracts (15 renderer, 20 harness, 33 boundary) |

The initial contracts run scanned its own output under `.check-logs/l6-review`
and flagged its printed retired-tool test name. Moving only these generated
logs to excluded `src-tauri/target/l6-review/` resolved the harness artifact
collision; the exact command passed unchanged. Both attempts are retained in
that local ignored directory; the original committed gate sidecars are unchanged.
Cargo admission found zero existing Cargo processes before each gate. Every
gate used this checkout's `src-tauri/target`, one build job, and ran after the
historical teardown, with no paid window. No tracked `src-tauri/` change occurred,
so `just test-rust-unit` was not required. This fix round adds no live rollback
coverage and leaves the original unknown spend explicitly unresolved.



## Run2 — amended format-first trial, 2026-09-11

**FAIL at step 2; no completed rollback.** Only step 1 passes. Mesh returned
`team owner already holds lifetime lock`, an `Owner::acquire` refusal at
`src/delivery/store.rs:351`, after `members_quiescent` had already passed at
`src/journal/transition.rs:55–56`. This is neither of Mesh's named quiescence
refusals (`quiescent_required; exclude all producers and native consumers` or
`quiescent required; member executor ...`). It does not qualify for the ruling's
one temporary-quiescence exception. The lifetime lock is independent of seat
activity: owner PID **217**, process start **30526237**, was still heartbeating at
**00:00:47.144924578 UTC**, after the format command and settle window.

The original controller incorrectly marked step 2 PASS and continued. The
ordinary Codex turn completed, but the subsequent **90.240-second** readiness
poll never established attributed fresh idle. The permitted format retry was
**never issued**, so temporariness was never demonstrated; no second command
result is claimed. Step 2 is a failed operational rollback under the audit's
permanent-refusal rule, not a proved temporary refusal. The later step-3
Taurhaus readiness observation remains valid; its deeper cause is unproved.
[Owner epoch](l6-rollback/run2/run/team/state/delivery/epoch.json),
[owner heartbeat](l6-rollback/run2/run/team/state/delivery/health-alpha.json).

This run follows the **2026-09-11 amendment** in section 6 of
`/home/mstie/projects/taurhaus/docs/design/e2e-coverage-audit.md`, read at the
binding absolute path: format first, ownership second. Run1 above is retained
as the historical ownership-first contract observation, with its own budget.

| Amended ordered step | Outcome | Classification / S-runtime evidence |
| --- | --- | --- |
| 1. Initialize; deliver/read A; observe B pending during ordinary work | **PASS** | S-runtime: production canonical initialization; one A terminal receipt, explicit read and assistant reply; B accepted without receipt while working, with owner heartbeat **after** acceptance. |
| 2. Lead requests `team format --legacy --quiescent` with B pending | **FAIL — non-quiescence refusal** | Mesh: exit **1**, `team owner already holds lifetime lock`. B intact, no begun/submitted receipt; format **2**, owner **team**, epoch **2** unchanged. No legacy projection or reconciliation report was produced. |
| 3. Settle; retry once if refused; verify committed format boundary | **FAIL** | Taurhaus activity readiness: 90.240 seconds / 348 runtime samples did not establish attributed fresh idle. Native turn completed and was metered; delivery sidecar remained working, then uncertain. Retry and format-boundary assertion were not reached. |
| 4. Lead requests ownership `members`; durable handoff, report, epoch boundary | **NOT RUN** | Blocked by the failed rollback (step 2 refusal, then step 3 readiness stop). No ownership command, durable rollback request, report, or members boundary. |
| 5. Guarded RC member executor; B accounting and fresh C reply | **NOT RUN** | Blocked by the failed rollback (step 2 refusal, then step 3 readiness stop). No member executor start; C never created. |
| 6. Explicit read/ack; cross-boundary A/B/C reconciliation; export/teardown | **NOT RUN** | Read/ack and rollback reconciliation not reached. Mandatory failure export/teardown independently **PASS**. |

[Outcome](l6-rollback/run2/run/controller-exit.json),
[format command](l6-rollback/run2/run/step2-command.json),
[settle analysis](l6-rollback/run2/step3-analysis.json),
[complete commands/RPCs and exits](l6-rollback/run2/run/events.jsonl).
Runtime: **125.983 seconds**, controller exit **1**, one initialization, no
paid restart, recovery, compaction, or automated model-turn retry.

### Run2 pending proof and preserved identities

B was accepted at **23:59:17.025803963 UTC**, sampled with production working
activity at **23:59:17.428832675**, and owner heartbeat at
**23:59:17.950301702**. The pending observation was captured at
**23:59:17.989 UTC**, with `last_defer_reason: pending: activity not freshly idle`.
Unlike run1, the heartbeat follows this acceptance; the predicate never accepts
a `stage: pending` row. No B transport/read receipt or native exposure existed.
The immediately preceding command snapshot still had B unbegun.

| Fact | Observed identity / disposition |
| --- | --- |
| Team / incarnation | `l6-rollback-run2` / `640a339a973c0d762946d00205b514974dd607004fbd504046da27843ed877ac` |
| Alpha session / attachment / context | `01a08dc2-7165-7742-85a8-a6094661515f` / **1** / **"0"** |
| Socket / pane / pane PID / start ticks | `/tmp/th-l6-bo64jqch/tmux/tmux-1000/default` / `%2` / **140** / **30526223** |
| A marker / logical ID | `A-94835941` / `9523f1c8-4c1c-4910-982d-95b1ae8ad4b5` |
| A delivery ID / state | `88b2455e-296e-4703-805a-135d6b6b9d2a` / one submitted receipt, consumed_by_read, assistant reply |
| B marker / original logical ID | `B-7dc78201` / `193725f0-62b0-4c1b-aa94-a243a0373d36` |
| B delivery ID / allocated legacy ID | `77a04119-535d-4e5a-b8ea-3d39082b6858` / `940e4896-40e7-455a-ab9d-90a099e07b06` |
| B final disposition | Accepted, unread, unsubmitted; **zero model inputs/exposures for B**, no legacy projection claim |
| Final format / owner / epoch | **2 / team / 2**; no rollback boundary |

[Pending proof](l6-rollback/run2/run/pending/193725f0-62b0-4c1b-aa94-a243a0373d36.json),
[boundary snapshot](l6-rollback/run2/run/step1-pending-boundary.json),
[A history](l6-rollback/run2/run/A-history.json),
[retained canonical journal](l6-rollback/run2/run/team/state/messaging-v2/segments/000001.jsonl).
The two step-2 snapshots are byte-identical aliases of the step-1 boundary;
[the manifest](l6-rollback/run2/export-manifest.json) records that deduplication.
An allocated legacy ID does not establish legacy projection.

The native ordinary turn completed at **23:59:44.319 UTC**. Later snapshots
reported the matching runtime session `state: idle`, `activity_attribution: none`,
while the delivery sidecar progressed from `likely_working` to `uncertain`.
Its final observation was **00:00:26.431757390 UTC**; it never became fresh idle
for delivery. No activity flag, lock, handoff, freshness rule, or process was
changed to force readiness. [Final runtime snapshot](l6-rollback/run2/run/final-runtime-sessions.json),
[final activity](l6-rollback/run2/run/final-activity.json),
[final pane](l6-rollback/run2/run/final-pane-2.txt).
Source context: Mesh `src/journal/transition.rs` acquires the owner lifetime lock
before downgrade; `src/delivery/store.rs` emits the observed refusal. That
explains why ordinary seat settling cannot release that lifetime lock. The
unexecuted retry is not evidence of temporariness. As a contrast for the
Taurhaus readiness classification, [step-1 readiness](l6-rollback/run2/run/step1-ready.json)
records the same `attributed()` predicate succeeding: matching alpha session/pane,
notify-derived fresh idle and attributed runtime. Both readiness waits now use
`taurhaus`; this labels the observed product state, not a proved root cause.

### Run2 candidate, spend, and isolation

Taurhaus product **a7e6db7e**, unchanged source, built from this checkout with
`just build-daemon` (**exit 0**); run2 started atop evidence commit **e69d468c**.
Mesh detached **310144de1f7939e7fbb2d80ab42ebd00580ce15d**, built with
`cargo build --bin mesh` in `/home/mstie/projects/mesh-l6` (**exit 0**).
No descriptor/source edit, Mesh commit, install, release, or product change.
Private daemon ping: **protocol 27**, version 0.9.7, probed port **46923**.
Actual native Codex **0.153.4**, **gpt-5.6-luna / low**; both native siblings copied.

| Binary | SHA-256 |
| --- | --- |
| Taurhaus daemon | `5ca8150fe8c15d3bf770a51b7eaf7b94be7b18b0bd56f82c0521d500563aa21e` |
| Mesh guarded RC | `c90231406f461568ba4b6766ec2a06d8cfd9702723875a85778d36f6057b060f` |
| Codex | `56ef98ab4032d317ab26e9b5e5a175650717351edb16ed9cde0cb6d1734d62da` |
| codex-code-mode-host | `3e85d67471825f73d02ff5f7e047ca1f6ca8caa3f59e4c6e8d9ca6ca7302cb45` |

| Every paid input / turn ID | Input / cached / output tokens | API-equivalent USD |
| --- | --- | --- |
| Onboarding — `01a08dc2-769b-7d22-be35-d066916195a2` | 70,246 / 60,928 / 767 | **0.00400256** |
| A — `01a08dc2-c795-7770-8cfc-a8fa530854f8` | 44,591 / 26,112 / 274 | **0.00454684** |
| Ordinary B-work — `01a08dc2-eafa-75e1-a6f2-d527c14d5111` | 15,599 / 13,056 / 1,485 | **0.00255172** |
| B pending; C; recovery/retries/compaction; Claude model inputs | No submitted inputs | **0** |
| Total | **3 inputs**, 3 completed native turns, 1 attachment generation | **0.01110112** |

All model turns have final metering. The ledger retains **10 usage deltas**;
conservative all-tokens-at-output-rate cost is **$0.1595544**, also below $0.20.
There are four input reservations, including unsubmitted B. One notify-only
identity is retained separately; it is not counted as another model turn.
Rates remain the inherited packet estimates ($0.20/M input, $0.02/M cached,
$1.20/M output), not an invoice. Implementer/reviewer usage belongs to the
orchestrator's separate accounting and is not exposed to this seat ledger.
[Every usage delta and reservation](l6-rollback/run2/run/cost-ledger.json).

The reused controller provisions scratch-only roots and a private PID namespace,
hides operator homes, removes inherited TMUX, pins binaries, blocks unrelated
CLIs, and starts a private tmux server. One auth file was copied (0600) into
initially empty CODEX_HOME; no credential contents were exported. **Authorization
deviation:** the historical pin targeted the primary `.codex` home, while the
binding spec authorizes `.codex-account-b/auth.json`. The summary explicitly
said the file governs, so it does not override that source. The retained
`auth_copy` event lacks source identity/digest; the billed account cannot be
independently established from this packet. No retrospective credential access
or invented digest was used. The corrected pin follows the spec, an explicit
caller allowlist supports operator-named disposable homes, and future run2
copies record only the parent/basename label and SHA-256 of the copied bytes. Production initialize uses the builder's canonical messaging
policy, one real tmux alpha and one login-only Claude lead (zero model turns).
No fault injection or stress run occurred.

### Run2 export, checks, reproduction, and deviations

The **complete 355-row daemon JSONL**, including shutdown, is retained without
an event-selection filter. Other complete streams: 18 journal rows, 6 workflow
rows, 81 native transcript rows, 996 controller events and 7 passive lock samples.
The read-only post-teardown audit verified **70 retained files**, **13 aliases**,
complete JSONL, sanitized paths/credentials, and pane excerpts of **at most 60 lines**.
**No owned daemon, member/team executor, tmux server, Codex, code-mode host or
Claude survives; port closed; auth copy and scratch root removed.**
[Daemon JSONL](l6-rollback/run2/run/taurhaus.log.jsonl),
[cleanup](l6-rollback/run2/run/cleanup.json), [audit](l6-rollback/run2/final-audit.json).

Five new offline tests first exited **1**: the inherited predicate incorrectly
accepted `stage: pending` (one assertion failure), and four new requirements had
no implementation (four import errors). The run2 predicates then passed all
**5**, while the **26 inherited tests** also passed. Coverage checks working plus
scheduler opportunity, heartbeat timestamp parsing, message-specific pending
obligations, receipt exclusion, exact format-1 committed history,
and assistant reply versus tool echo. The original begun-state test incorrectly
used `receipt/attempt_started`; it did not cover the real
`delivery_attempt/attempt_started` shape. The review fix below adds that coverage.
The original regression comment names **82e2655b**.
Tests use synthetic values only, without CLIs or credential reads.
[Red](l6-rollback/run2/red.txt), [green](l6-rollback/run2/green.txt),
[tests](l6-rollback/run2/run2_test.py), [rules](l6-rollback/run2/run2_rules.py).
No product regression was fixed.

| Exact command from this checkout root, after teardown | Exit | Result |
| --- | --- | --- |
| `just check-quick` | **0** | Rust test compile + typecheck + **2,519 frontend tests** pass |
| `just lint` | **0** | Rust, frontend, workflow and recipe lint pass |
| `just test-contracts` | **0** | **68 tests** pass: 15 renderers, 20 harness, 33 boundaries |

[Gate results](l6-rollback/run2/gates-result.json),
[quick gate](l6-rollback/run2/check-quick.json),
[lint](l6-rollback/run2/lint.json), [contracts](l6-rollback/run2/test-contracts.json).
All first attempts passed; no gate overlapped the paid runtime. Cargo admission
used the required `pgrep -af '(^|/)cargo( |$)'` probe before each command,
30-second polls only if at least three Cargo processes existed, maximum 30 minutes.
Build admission saw **1** then **2** existing Cargo processes; each gate saw **0**.
Each admitted command used one build job and its own checkout's target directory.
Raw gate logs were written under ignored `src-tauri/target/l6-run2-gates` to
avoid the historical self-scanning log collision, then copied/sanitized here.
No tracked `src-tauri/` diff; conditional `just test-rust-unit` did not apply.

Corrected controller: [run2controller.py](l6-rollback/run2/run2controller.py),
reusing the parent support/preflight/rollback modules. The exact controller that
produced the retained runtime is available at commit **d012ab5d**; the current
file contains the review fixes and has not been run live. Reproduction from this
checkout, with a new output directory and a separately authorized fresh budget:

```sh
python3 -B -m unittest discover -s docs/design/evidence/e2e/l6-rollback/run2 -p '*_test.py'
python3 -B docs/design/evidence/e2e/l6-rollback/run2/run2controller.py --auth-source "$AUTHORIZED_SOURCE" --out docs/design/evidence/e2e/l6-rollback/run2/NEW_RUN
```

`AUTHORIZED_SOURCE` is the exact standing-authorized pin in the parent
`preflight.py`; no fallback. The selected output must not already exist.
No paid restart was performed during this task. Packaging/audit used the parent
`pack.py` and `audit.py` with `BASE` set to `run2`; raw complete streams remain
independently retained. The numbered green runtime steps were committed
immediately, before proceeding.

Deviations / limits:

- The specified L2 worktree is absent (Git exit **128**). The inherited one-seat
  controller was reused from run1, whose L2 lineage was recovered read-only from
  local commit **2690352e**; that commit's report was checked again. No other
  Taurhaus checkout was changed.
- The Mesh stage-2b brief is only a membership addendum in this RC. Its USAGE
  contract, the separate stage-2b implementation packet, stage-3 packet, and
  rollback source were read before execution.
- Step 3 stopped on production activity readiness before the allowed retry.
  Native ordinary work completed; no second format result, ownership boundary,
  fresh C or cross-boundary read/ack is claimed. Review identifies step 2 as the
  earlier failed rollback; continuing after it was a controller deviation.
- No independent Opus lens ran inside this implementer lane; it remains the
  enclosing workflow's review obligation. This packet is a failed trial, not
  full workflow PASS or release approval.


### Review correction, 2026-09-11

The supplied Opus review was checked against Mesh RC **310144d** and the
retained artifacts. No product files, descriptors, plan-ledger rows, real
credentials or live seats were touched. This is an offline correction, not a
new run: additional seat inputs **0**, additional metered seat spend **$0**.
Original spend remains onboarding **$0.00400256**, A **$0.00454684**, ordinary
work **$0.00255172**, total **$0.01110112** (estimated API equivalent).

Red-first validation: run2's 10-test suite exited **1** with **13 assertion
failures (including subtests) and 2 errors**: real begun rows were accepted,
owner-lock refusal advanced into step 3, the credential pin/identity was wrong
or absent, and the event writer held no lock. The inherited suite exited **1**
with **1 failure** because the retired pending entry point still returned data.
After correction, **10 run2 + 25 inherited tests pass (both exit 0)**.
Regression comments identify **955df28f** and **82e2655b**. All inputs are
synthetic or mocked; tests invoke no real CLI and read no real credentials.

Pending now rejects `attempt_started`, `outcome_unknown`, `submitted`,
`native_enqueued` and `consumed` for B regardless of event type, including the
canonical `delivery_attempt/attempt_started` shape, and still rejects all
B receipt rows. Other message IDs remain independent. The old support pending
entry points raise with a pointer to `run2_rules.pending`; their obsolete green
assertions were removed. A reentrant lock serializes event writes and shared
observation/identity mutations, with an offline lock assertion rather than a
stress run. No evidence indicates stream corruption in the original run.

Historical `events.jsonl`, `controller-exit.json` and the unfortunately named
`step2-temporary-refusal.json` are unaltered records of the original controller.
Their PASS/temporary labels are superseded by this correction and the annotated
`step2-outcome.json`; original runtime timestamps remain unchanged. The original
export audit is historical, not a validation of these later interpretation edits.

The named build/gate transcripts are trimmed to their final 24 lines; structured
JSON gate records remain intact, and full historical scrollback is recoverable
from **d012ab5d**. This reduces tracked build noise without trimming the complete
daemon or runtime JSONL evidence required by the spec.

The original manifest is retained unchanged within the requested file scope.
A read-only digest comparison found exactly one changed runtime artifact:
`step2-outcome.json`, the annotated review correction above. Its SHA-256 changed
from `d5b5d14d9ef2c40829a2659255ff44dd7a324d596fb5565c4d1dbc43dedd5f3e`
to `0cca2c77fde3ce327aba5333101b4b8ea68aa42cd6a74f44b82fb1da60ba38c1`.
All other retained artifact digests still match the original manifest; all
**996 controller events** and **355 daemon rows** parse as complete JSON.

Review gates ran after the recorded teardown; the original scratch root is
absent. No trial process was started in this fix round. Gate subprocesses
finished and were reaped. Full new transcripts and JSON results are local-only
under `src-tauri/target/l6-review-gates/`; historical gate sidecars above retain
their original results.

| Review gate from checkout root | Exit | Seconds | Result |
| --- | --- | --- | --- |
| `just check-quick` | **0** | 18.65 | Rust test compile, typecheck, 150 files / 2,519 frontend tests pass |
| `just lint` | **0** | 6.743 | Rust, frontend, workflow and recipe lint pass |
| `just test-contracts` | **0** | 5.197 | 68 contracts pass (15 renderers, 20 harness, 33 boundaries) |

Each gate's exact `pgrep -af '(^|/)cargo( |$)'` admission probe returned **1**
(no matches), count **0**; no 30-second waits were needed. Gates used
`CARGO_BUILD_JOBS=1` and this checkout's `src-tauri/target`. No `src-tauri/`
source diff exists, so conditional `just test-rust-unit` does not apply.

Run2 step 1: PASS (S-runtime); see run3/run/step1-outcome.json.

Run2 step 2: PASS (S-runtime); see run3/run/step2-outcome.json.


## Run 4 — fresh trial, stopped before the rollback route

**FAIL at step (a), harness classification.** Production initialization completed,
Codex 0.153.4 / `gpt-5.6-luna` / low reached its composer, and alpha explicitly
read the canonical onboarding card. The inherited controller guard nevertheless
required both a terminal `submitted` receipt **and** the explicit read. This
contradicts the shared rule that alpha's `consumed_by_read` is sufficient. Its
90-second readiness poll expired; the controller exited **1** after **94.00 s**.
The requested complete escape route remains **NOT VERIFIED**.

The raw controller outcome labels this `taurhaus`; that is a harness attribution
error, preserved rather than silently rewritten. Offline inspection finds
`attributed_idle: true`, an attributed ready session, alpha's read receipt and
`inherited_predicate_result: false`. The stricter helper originated in `5e89fb019`.
No product defect is established by this stop. No second paid launch was made,
following the spec's stop-on-failure and no automatic paid-retry rules.

| Ordered run4 item | Outcome | Classification and observed boundary |
| --- | --- | --- |
| (a) Initialize; A read/reply; B pending; lead owner stop and named skip | **FAIL** | Harness startup predicate after successful initialization/onboarding read. A/B never sent; owner-stop command never reached. |
| (b) Quiescent format downgrade with B pending | **NOT RUN** | Blocked by (a); format remains 2 and owner remains team. No run4 format-0/rollback-digest/transition boundary claim. |
| (c) Same-owner members command clears marker | **NOT RUN** | No handoff command, marker removal, new request or epoch claim. |
| (d) RC member executor; B exactly once; fresh C reply | **NOT RUN** | No member executor started; no B/C input or delivery. Executor timing across (b)/(c) is unobserved. |
| (e) Explicit read/ack, A/B/C reconciliation, export and teardown | **NOT RUN** | No A/B/C to reconcile. Mandatory failure export and teardown independently **PASS**. |

The run used five numbered controller steps, matching the fourth-run ruling's
(a)–(e), which supersedes the audit's historical six-step order. The audit's
2026-09-11 format-first amendment and the fourth-run ruling govern the intended
route; earlier run-1/2/3 evidence above remains history, not run4 coverage.

### Candidate and exact controller

This checkout stayed on `feat/e2e-l6-rollback`, tip
`76dcadfa255306cbc5f1b385dc64ce2cb02a0f1d`, with
`git merge-base --is-ancestor ac2bc513 HEAD` exit **0** and no product diff from
`ac2bc513`. `just build-daemon` rebuilt the checkout-local
`src-tauri/target/release/taurhaus-daemon`, exit **0**. Mesh-l6 HEAD and
`release/overhaul-rc` both resolved to `3015cb0`; its checkout-local
`cargo build -j 1` exited **0**. No descriptor edit, Mesh commit, install or
release occurred. Both digests and command tails are in [builds](l6-rollback/run4/builds.json).
Every build/gate admission ran the required Cargo process census; fewer than
three existing Cargo processes admitted this lane with one build job.

The [executed controller](l6-rollback/run4/controller.py) is the run3 controller
adapted for the documented format-0 boundary, same-owner marker commit and
attachment-checked executor reuse. The three new route predicates observed
behavioral reds and then green offline. They were **not reached at runtime**.
The controller is preserved as executed, including the startup helper import;
[provenance](l6-rollback/run4/provenance.json) records its digest. Added readiness
and input-count diagnostic predicates run only offline and do not imply a
re-executed or repaired runtime trial.

Invocation from this checkout: `python3 -B docs/design/evidence/e2e/l6-rollback/run4/controller.py --auth-source "$SOURCE"`,
where SOURCE is the operator-authorized account-b auth.json file named in the
spec. Exactly that file was copied into the otherwise empty scratch CODEX_HOME,
mode 0600, then removed. Both native Codex siblings were copied from the installed
package. Production `coordination.initialize_team` used the builder's canonical
policy, alpha tmux, and one login-only Claude lead. Runtime homes/data/project,
PID namespace, probed port **47743**, and tmux server were private; inherited
TMUX was absent and operator homes hidden from children.

### Readiness, identity and costs

Alpha session: `01a08ed3-d57d-7b10-bcd9-dae8e4344785`; attachment generation **1**,
pane `%2`. Onboarding logical ID `ae64da01-62a5-4622-88dd-2de176556dc6` has one
`consumed_by_read` receipt from alpha. This is actual delivery by the governing
predicate, even though no `submitted` receipt exists. The run sent **no A/B/C**;
no rollback identity table can honestly contain those messages.

[Startup diagnosis](l6-rollback/run4/startup-diagnosis.json),
[analysis and corrected count](l6-rollback/run4/analysis.json),
[raw journal](l6-rollback/run4/run/team/state/messaging-v2/segments/000001.jsonl),
[complete daemon JSONL](l6-rollback/run4/run/taurhaus.log.jsonl), and
[command/RPC stream](l6-rollback/run4/run/events.jsonl) retain the evidence.

**One Codex input, one metered model turn, $0.00467732 API-equivalent estimate.**
The executed transport-only counter incorrectly reported zero inputs because
there was no submitted receipt. Analysis uses the maximum of the native rollout
turn count and transport count, so the actual start is counted. Raw counter
values remain preserved. One notify-only identifier is recorded separately;
it is not a second issued model input and is not represented as free billed
usage. All six measured usage increments belong to turn
`01a08ed3-de6a-7853-b9d0-caed27de972a`:

| Usage timestamp (UTC) | Input / cached / output tokens | API-equivalent USD |
| --- | --- | --- |
| 2026-09-11T04:57:30.679Z | 9031 / 3840 / 184 | 0.00133580 |
| 2026-09-11T04:57:37.867Z | 11324 / 7936 / 268 | 0.00115792 |
| 2026-09-11T04:57:40.568Z | 11996 / 11008 / 100 | 0.00053776 |
| 2026-09-11T04:57:43.692Z | 12432 / 11008 / 124 | 0.00065376 |
| 2026-09-11T04:57:47.458Z | 12606 / 12032 / 141 | 0.00052464 |
| 2026-09-11T04:57:49.254Z | 12986 / 12032 / 30 | 0.00046744 |

Rates are the inherited packet's $0.20 input / $0.02 cached / $1.20 output per
million tokens, estimates rather than invoices. The inherited uncached
upper estimate is **$0.0854664**. Both estimates are below $0.20; one input is
below ten. Lead: login-only, zero paid Claude inputs. Reviewer: none launched,
zero reviewer spend here. Implementer spend is separately owned by the
orchestrator and is unavailable to this lane; it is not claimed zero.

### Teardown, validation and limits

Teardown recorded **zero survivors**, closed private port, removed credential
copy and removed scratch root. The PID/start-time ledger and namespace shutdown
cover daemon, team owner, tmux, Codex and code-mode host. No member executor was
started. [Cleanup](l6-rollback/run4/run/cleanup.json) is complete before gates.
The [export manifest](l6-rollback/run4/export-manifest.json) deduplicates identical
snapshots while retaining all complete stream rows; every pane excerpt is at
most 60 lines. Private credential/account fields are sanitized.

Offline tests: **6 PASS**. Three original route tests first failed for format-1,
fresh handoff and missing existing-executor recognition; the readiness diagnostic
suite first failed importing its absent helpers, then passed. These are harness
assertion tests, not product fixes or a substitute runtime PASS. The executed
startup helper's false result against actual ready/read evidence is retained.

Gates, run after teardown: **`just check-quick` 0; `just lint` 0;
`just test-contracts` 0**. The [gate results](l6-rollback/run4/checks-result.json)
retain exact commands, elapsed times, targets and final log lines. The
[offline export audit](l6-rollback/run4/final-audit.json) passes. No `src-tauri/`
diff, so the conditional Rust-unit gate does not apply.

Deviations: the specified L2 worktree no longer exists, so its versioned run3
controller/evidence was read in this checkout; Mesh's full stage2b brief is
absent from mesh-l6, so the supplied addendum, journal-stage3 storage analysis,
USAGE and authoritative transition/ownership source were read. The inherited
startup predicate violates the shared delivery rule and prevented this run's
route completion. The required independent Opus evidence lens is unavailable
from the tool/model surface; [review status](l6-rollback/run4/review.json) records
that limitation. The overall workflow is **incomplete**, not PASS. No runtime
step was green, so there are no numbered runtime PASS commits for run4.

Run5 step 1: PASS (S-runtime); see run5/run/step1-outcome.json.

Run5 step 2: PASS (S-runtime); see run5/run/step2-outcome.json.

Run5 step 3: PASS (S-runtime); see run5/run/step3-outcome.json.

Run5 step 4: PASS (S-runtime); see run5/run/step4-outcome.json.
