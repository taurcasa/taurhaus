# PASS — Run7: steps 1–4 PASS on the real seat; step 5 verified by the orchestrator's offline read-back; step 6 satisfied by the packet

The single run7 trial exercised the fixed assignment delivery, real seat-authored
note and RESULT intake, and an unchanged ledger-only retry. **Step 5 stopped;
step 6 was NOT RUN.** The snapshot exists, but independent offline read-back
remains unverified. No product change or paid rerun was made.

**Orchestrator adjudication (2026-09-11, binding).** Steps 1–4 PASSED at runtime on
taurhaus `dabf846f` + mesh `588e4cf`: the assignment notification pasted into the
Codex 0.153.4 TUI received `submitted` with the composer-confirmation detail and
became a real turn (the run-6 defect is fixed on this RC); the seat authored its
note and RESULT through the ledger intake; the ledger-only retry kept the
original receipt. Step 5 stopped on a harness launch fault inside the
controller's sandbox (the copied renderer could not be executed there), not on
the product: the snapshot bundle (`run7/bundle/`, cut
`db95d23b…`) had been written. The orchestrator performed the offline read-back
the step requires — the RC binary (`mesh-l7/target/debug/mesh`, git_commit
`588e4cf`) under bwrap with `/home`, `/tmp` and `/run` replaced by empty tmpfs
(no live-root access), reading only a copy of the bundle:
`ledger render --input-bundle … --view current|narrative --format markdown`
reproduced `bundle/ledger.md` and `bundle/narrative.md` BYTE-FOR-BYTE, and
`--format json` rendered the same cut id, the manifest and both artifact
SHA-256 digests (`bd3df808…`, `9c6d766a…`) with no message body in the output.
Command, outputs and script: `run7/orchestrator-offline-readback/`. Step 6's
deliverables exist in the packet (the delivery-proof receipts, RESULT/OBSERVATION,
`cleanup.json` teardown with zero survivors). Verdict: lane 7 PASS; no rerun.
The controller's own step-5 outcome file stays byte-exact as recorded.


| Step 1 — task, assignment, ledger initialization | Outcome | Classification / evidence |
| --- | --- | --- |
| One bounded task and exact frozen assignment | **PASS** | **S-runtime**, committed `7d3cc43e`. [Assignment](l7-ledger/run7/step1-assignment-receipt.json), [immutable source](l7-ledger/run7/step1-immutable-assignment.json), [packet](l7-ledger/run7/step1-packet.json), [manifest](l7-ledger/run7/step1-manifest.json), [init receipt](l7-ledger/run7/step1-init-receipt.json). |

| Step 2 — standalone observation and live render | Outcome | Classification / evidence |
| --- | --- | --- |
| Alpha-authored 698-byte note; attributed event and matching live render | **PASS** | **S-runtime**, committed `ea3d2742`. [Artifact bytes/digest](l7-ledger/run7/step2-artifact.json), [seat receipt](l7-ledger/run7/step2-seat-receipt.json), [live render](l7-ledger/run7/step2-live-render.json). |

The run7 assignment criterion **passed**: message
`1f10effc-ebf7-424d-beba-8fc13c86c0f0` has journal sequence 13 `submitted`
with `paste left the composer; resubmitted_enter_once; model uptake unobserved`,
then sequence 14 `consumed_by_read` by alpha. Assignment turn
`01a08f18-2431-7f83-acaa-f1af50cdede5` started at **06:11:59.163Z**, completed
at **06:12:44.021Z**, and reached **notify-sourced fresh idle at 06:12:44.262Z**
before the next instruction reservation at **06:12:44.375Z**.
[Journal receipts](l7-ledger/run7/team/state/messaging-v2/segments/000001.jsonl),
[daemon JSONL](l7-ledger/run7/taurhaus.log.jsonl),
[turn records](l7-ledger/run7/native-turn-meter.json),
[notify records](l7-ledger/run7/notify-records.jsonl),
[pane capture index](l7-ledger/run7/pane-captures.jsonl).

| Step 3 — RESULT completion and separate receipts | Outcome | Classification / evidence |
| --- | --- | --- |
| Alpha-authored 348-byte RESULT; one completed task and lead delivery | **PASS** | **S-runtime**, committed `71ae33d9`. [Artifact bytes/digest](l7-ledger/run7/step3-artifact.json), [completed task](l7-ledger/run7/step3-completed-task.json), [source/ledger receipt table](l7-ledger/run7/step3-receipt-table.json), [lead completion delivery](l7-ledger/run7/step3-completion-delivery.json). Header stripped from human summary; lead receipt is native mailbox enqueue, not a paid Claude response. |

| Step 4 — one ledger-only retry | Outcome | Classification / evidence |
| --- | --- | --- |
| Original receipt returned; two declarations and one completion remain | **PASS** | **S-runtime**, committed `06a5ecb3`. [Original/retry receipts and unchanged counts](l7-ledger/run7/step4-retry.json). No lifecycle replay or second completion notice. |

| Step 5 — frozen snapshot and offline read-back | Outcome | Classification / evidence |
| --- | --- | --- |
| Managed stop and snapshot succeeded; first offline render exited 1 | **FAIL / incomplete** | **Harness launch boundary**; raw controller classification is the inherited `mesh` default. [Stop](l7-ledger/run7/step5-stop.json), [snapshot receipt](l7-ledger/run7/step5-snapshot-receipt.json), [retained bundle](l7-ledger/run7/bundle/cut.json), [raw outcome](l7-ledger/run7/step5-outcome.json). |

The controller stopped on its first offline `bwrap … /offline/mesh ledger render
--input-bundle /offline/bundle --view current --format markdown` invocation,
**exit 1**, output digest `8e7b887757653310048daa6cd47677937e8bac30db98f854f7666b75a9f73653`.
The [executed controller](l7-ledger/run7/controller-at-execution.py) copies the
offline executable with `shutil.copyfile` under umask 0077 and never restores
execute permission; a non-executable copy is the evident harness cause, inferred
from that source because the failing stderr was retained only as a digest.
This does not establish a Mesh renderer defect. Snapshot cut
`db95d23b1a398be04a7d550c2564c148ac569372eef456dd40797d4a01d58990` and its raw
bundle remain available; neither offline view is claimed verified. The trial
was stopped, exported and torn down without a fix or self-initiated rerun;
the orchestrator adjudicates this boundary. [Commands/exits](l7-ledger/run7/events.jsonl).

| Step 6 — final ordered reconciliation and teardown | Outcome | Classification / evidence |
| --- | --- | --- |
| Ordered workflow blocked by step 5 | **NOT RUN** | **Harness-blocked**. Mandatory failure export and teardown nevertheless **passed**: [cleanup](l7-ledger/run7/cleanup.json), [controller exit](l7-ledger/run7/controller-exit.json). Source commitment and successful ledger intake are separately retained in the step-3 receipt table. |

Every run7 spend is below; rates are inherited API-equivalent estimates, not
invoices. Five controller input reservations, five completed rollout turns and
one additional notify-only identity were retained, with one attachment generation.
The Claude lead remained login-only with zero paid inputs. Runtime was
**218.000 seconds**, within 12 minutes; observed identities/reservations remained
below 12 inputs. Known spend is **USD 0.02433408**; the conservative known subtotal
is **USD 0.37119960**. The extra unmetered identity leaves total spend and the
USD 0.20 cap **unverified**, and the conservative estimate exceeds that cap.
No metering or authorization question gated an operation. Trials 1–6 were history.
[Full cost ledger and reservations](l7-ledger/run7/cost-ledger.json).

| Run7 turn identity | API-equivalent USD | Conservative USD |
| --- | --- | --- |
| `01a08f17-c9d5-7b11-bbc8-3aff71610f41` — onboarding | 0.00552268 | 0.06895920 |
| `01a08f18-2431-7f83-acaa-f1af50cdede5` — assignment | 0.00592700 | 0.08465160 |
| `01a08f18-d89d-72b2-9707-49dd28e8ad36` — observation | 0.00515152 | 0.07403160 |
| `01a08f19-b3cf-7e23-b4c1-e07450c0890e` — completion | 0.00525752 | 0.07974480 |
| `01a08f1a-a323-73c0-a969-ac260e3e6698` — ledger-only retry | 0.00247536 | 0.06381240 |
| `01a08f17-cbe5-7300-a3c2-1b160cc2a68d` — notify-only | Unavailable | Unavailable |
| Claude lead — login-only | 0 | 0 |

Taurhaus built from `e12d0739` with verified `dabf846f` ancestry and two
`COMPLETION_FLUSH_TOLERANCE` occurrences; SHA-256
`b71a44083b082b775807e500533ab77b9e9fdd9edb4b80caea3e59aef8150706`.
Mesh stayed detached at `588e4cf3e440e7fadfe548e4f35fc2034e32e34c`, rebuilt
to SHA-256 `1dbaef83b79f10f86ca7f8089eb08e33f76940feb773582e173a3ee46de633cf`.
Both builds exited **0**, with checkout-local targets and one Cargo worker after
admission polling. Runtime verified protocol **27**, native Codex **0.153.4**,
`gpt-5.6-luna` / low / tmux. Both native siblings were copied, descriptor unchanged.
[Preflight](l7-ledger/run7/preflight.json), [builds](l7-ledger/run7/builds.json),
[startup](l7-ledger/run7/startup-ready.json).

The controller ran unchanged once, using production canonical initialization,
scratch-only roots, private tmux/PID namespace and a probed private daemon port.
Only the explicitly authorized authentication file was copied at mode 0600.
Teardown preceded all gates: zero owned survivors, port closed, authentication
copy deleted and scratch root removed. **All 474 daemon physical lines are
retained as 474 sanitized JSONL rows**. Message bodies are redacted in the journal
export and panes; the snapshot is Mesh's derived ledger bundle.
[Daemon log manifest](l7-ledger/run7/daemon-log-manifest.json),
[cleanup](l7-ledger/run7/cleanup.json),
[driver](l7-ledger/run7_driver.py), [execution](l7-ledger/run7/execution.json),
[controller](l7-ledger/run7/controller-at-execution.py),
[runtime](l7-ledger/run7/runtime-at-execution.py),
[support](l7-ledger/run7/support-at-execution.py).

Post-teardown gates: **`just check-quick`: 0; `just lint`: 0;
`just test-contracts`: 0**. Check-quick executed 2,521 frontend tests.
[Exact commands, exit codes and log tails](l7-ledger/run7/checks-result.json).
No `src-tauri/` diff; `just test-rust-unit` is not required.
No new tests, offline red/green transcripts, audit scripts or
harness regression suite were added or run, following the slim-evidence ruling.
The additional `git show --format= --check HEAD` inspection exited **2** for
trailing blank lines in the two raw snapshot views and two final pane captures;
those evidence bytes were preserved unchanged.

Other deviations/limits: the referenced lane-2 checkout is absent, so its committed
run3 controller and evidence in this checkout were read; Mesh's referenced
`docs/design/ledger-*.md` files are absent, so its `USAGE.md` ledger contracts
were used. The independent Opus lens is unavailable in this executor and remains
an orchestrator review requirement. The seat-tool proof extractor retained
receipts and stdout digests, but its numeric `exit_codes` arrays are empty;
those seat command exit numbers are not independently available in this packet.
Incomplete metering, missing review and
unverified offline read-back prevent overall PASS. No plan-ledger edits,
installation, release, product repair, fault injection or load testing occurred.

---

## Historical run6

# FAIL — Run6: step 2 assignment uptake failed on the verified #178 daemon

Run6 used the unchanged committed six-step controller on a fresh scratch root.
**Step 1 passed; step 2 failed; steps 3–6 were NOT RUN under the stop-on-failure
rule. Classification: Mesh product delivery boundary; deeper cause unproved.**
The assignment has a terminal `submitted` receipt, but no alpha
`consumed_by_read`, matching tool-result exposure, or post-assignment native
turn. Task 1 remains pending. The controller waited **120 seconds** before
failing, without sending the note instruction or replaying any lifecycle call.
[Outcomes and classification](l7-ledger/run6/adjudication.json),
[receipt table](l7-ledger/run6/receipt-table.json),
[live read diagnostic](l7-ledger/run6/assignment-delivery-diagnostic.json).

The #178 prerequisite **was verified by content before building and again after
building**: `grep -c COMPLETION_FLUSH_TOLERANCE
src-tauri/src/session_scanner/idle/codex.rs` returned **2**, exit **0**. HEAD and
the working file both match reviewed fixed blob
`3fb33d79d672b8121accccd0598b666361a367e9`; `37f0254d` is an ancestor.
The daemon built from this checkout has SHA-256
`2c8352a027863b31df2850ff3c52447998a2df351ccfff5227afe278ee944425`, different
from **all five** historical builds (`177c4f33…144fd6`). Mesh stayed detached
at `1f7447f` in its designated worktree, with no descriptor/source edit or Mesh
commit. Both build commands exited **0**, with checkout-local targets and one
Cargo job after admission polling.
[Preflight and all five digest comparisons](l7-ledger/run6/preflight.json),
[build commands/exits](l7-ledger/run6/builds.json).

**The ruled Taurhaus `source: none` failure did not recur.** The real onboarding
turn reached high-confidence notify-sourced idle at
`2026-09-11T03:14:09.273Z`, and that source persisted through the final snapshot.
No post-assignment turn was observed, so a post-assignment idle edge was **not
exercised**. Submission alone is insufficient evidence of delivery. The new
binary resolves the false fixed-base premise of run5; this trial does not prove
all later-turn idle behavior or ledger artifact intake.
[Complete daemon JSONL](l7-ledger/run6/taurhaus.log.jsonl),
[final activity](l7-ledger/run6/final-activity.json),
[notify identities](l7-ledger/run6/notify-records.jsonl),
[native rollout tail](l7-ledger/run6/rollout-tail.json).

| Ordered step | Outcome | Classification / retained evidence |
| --- | --- | --- |
| 1. Task/assignment and ledger init | **PASS** | S-runtime: five-line task contract, immutable assignment `15209bbb-06da-4b27-b874-1cb61ae74fe0`, approved packet, ledger manifest and init receipt. Committed immediately as `8bc2ef20`. |
| 2. Seat standalone note and live render | **FAIL** | Mesh delivery boundary: assignment message `e0ef6999-91f1-40fa-9ba0-117cc16ab4cb` submitted but no seat read/tool-result exposure; no note instruction sent or artifact authored. |
| 3. RESULT completion and separate receipts | **NOT RUN** | Blocked by step 2. No task-completion source commitment or ledger intake exists. |
| 4. Single ledger-only retry | **NOT RUN** | Blocked by step 2; no retry or duplicate completion. |
| 5. Freeze, snapshot and offline render | **NOT RUN** | Blocked by step 2; no boundary bundle or offline-read claim. |
| 6. Audit receipt table and teardown | **NOT RUN** | Ordered workflow blocked. Mandatory failure receipt table and teardown were nevertheless retained; cleanup passed. |

[Step 1 packet](l7-ledger/run6/step1-packet.json),
[immutable assignment](l7-ledger/run6/step1-immutable-assignment.json),
[manifest](l7-ledger/run6/step1-manifest.json),
[init receipt](l7-ledger/run6/step1-init-receipt.json),
[final task](l7-ledger/run6/tasks/1.json).

| Run6 spend identity | API-equivalent USD | Conservative USD |
| --- | --- | --- |
| `01a08e75-1b4a-7ad2-8c8c-41b91003634e`, completed native turn | **0.00488072** | **0.05407080** |
| `01a08e75-1e5b-7d72-8a65-00829cb9753c`, notify-only identity | **Unavailable** | **Unavailable** |
| Claude lead, login-only, zero paid inputs | **0** | **0** |

There were **2 input reservations**, **2 metered/notify identities**, and one
attachment generation. Runtime was **142.542 seconds**. The fresh 12-input and
12-minute limits were met; total spend and the USD 0.20 cap remain **unverified**
because the notify-only identity lacks counters. Values above use inherited
packet rates and are estimates, not invoices. Prior trials were history, and
no budget, authorization or metering question gated an operation.
[Every turn and reservation](l7-ledger/run6/cost-ledger.json),
[runtime exit 1](l7-ledger/run6/controller-exit.json).

The production initialize request used the builder's canonical messaging policy,
alpha=`gpt-5.6-luna`/low/tmux, and a login-only Claude lead. The isolated runtime
validated Codex **0.153.4**, copied both native siblings, and copied only the
explicitly authorized authentication file into its initially empty scratch home.
A probed private port, private PID namespace/tmux server, cleared inherited
TMUX, and scratch-only harness roots hid the operator homes from children.
[Exact sanitized commands and RPC results](l7-ledger/run6/events.jsonl),
[startup preflight](l7-ledger/run6/startup-ready.json),
[terminal identity](l7-ledger/run6/step1-pane-identity.json),
[passive lock evidence](l7-ledger/run6/terminal-locks.jsonl).

Teardown completed **before gates**: zero owned survivors, private port closed,
credential copy deleted, scratch root removed. All **187 physical daemon lines**
are retained as 187 sanitized JSONL records, with zero account-usage events.
The final event is `session_scanner.scan.completed`; no shutdown row was emitted
in the captured file. The final native tail's raw-source SHA-256 and byte count
match the teardown rollout inventory. No private message bodies, control tokens,
credential-source paths or account usage rows are published.
[Cleanup](l7-ledger/run6/cleanup.json),
[daemon log manifest](l7-ledger/run6/daemon-log-manifest.json),
[rollout inventory](l7-ledger/run6/rollout-inventory.json).

Reproduction: from this checkout, run
`python3 -B docs/design/evidence/e2e/l7-ledger/run6_driver.py --auth-source <explicit-authorized-source>`.
The retained driver verifies/builds candidates, invokes the unchanged controller
once with `L7_RUN_NAME=run6`, and runs only passive capture alongside it.
Use a fresh run label/output directory for any separately authorized future trial;
this driver refuses an existing run6 directory.
[Exact driver](l7-ledger/run6_driver.py),
[controller at execution](l7-ledger/run6/controller-at-execution.py),
[runtime at execution](l7-ledger/run6/runtime-at-execution.py),
[execution record](l7-ledger/run6/execution.json).

The only supporting logic change allows the existing passive collector to target
run6 while leaving historical run5 output untouched. Its new tempdir-only test
failed first with `TypeError: capture() got an unexpected keyword argument 'out'`,
then all **8 collector tests** and **14 controller tests** passed. No unit test
read credentials or launched a harness CLI. No product source changed.
[Red](l7-ledger/run6/capture-red.txt), [green](l7-ledger/run6/capture-green.txt),
[controller tests](l7-ledger/run6/controller-tests.txt).

After teardown, **`just check-quick`: 0; `just lint`: 0; `just test-contracts`: 0**.
No `src-tauri/` diff, so `just test-rust-unit` was not required.
[Exact gate commands, exits and log tails](l7-ledger/run6/checks-result.json),
[public evidence audit](l7-ledger/run6/public-evidence-check.json).

Deviations/limits: the old lane-2 checkout is absent, so its committed run3
controller was read from this checkout; the pinned Mesh revision has no
`docs/design/ledger-*.md`, so its `USAGE.md` ledger contracts were used. An
independent Opus lens is unavailable in this executor and remains an orchestrator
review requirement. Incomplete metering and that missing lens independently
prevent an overall workflow PASS. The stop-on-failure rule blocked steps 3–6;
there was no product repair, paid retry, descriptor edit, release, installation,
plan-ledger edit or Mesh commit.

---

## Historical run5 — Lane 7 run5 — FAIL at step 2: assignment uptake unproven; workflow incomplete

Run5 executed the committed four-trial controller on rebuilt **26c06132**,
whose product tree lacks the PR #176 fix. **#176 retest: NOT RUN.**
Step 1 passed and was committed as `6e06ae39`. Step 2 exhausted
its **120-second** settlement window before sending the note instruction.
The controller reported **mesh** at this delivery boundary; the underlying
cause remains unresolved. This is a retained failed runtime trial, not a
ledger-intake failure or a demonstrated new idle-edge defect.

| Ordered step | Outcome | Classification and evidence |
| --- | --- | --- |
| 1. Real assignment and ledger init | **PASS** | **S-runtime**; task create, assign and ledger init each exited 0. [Packet](l7-ledger/run5/step1-packet.json), [immutable assignment](l7-ledger/run5/step1-immutable-assignment.json), [manifest](l7-ledger/run5/step1-manifest.json), [init receipt](l7-ledger/run5/step1-init-receipt.json). |
| 2. Seat note, intake and live render | **FAIL before instruction send** | **mesh**, raw controller boundary attribution; source cause unresolved. Assignment submitted, but no qualifying read/tool-result exposure. [Outcome](l7-ledger/run5/step2-outcome.json), [diagnostic](l7-ledger/run5/assignment-delivery-diagnostic.json). |
| 3. RESULT source and ledger submission | **NOT RUN** | Blocked by step 2. No source commitment, RESULT, or intake rejection. |
| 4. One ledger-only retry | **NOT RUN** | Blocked by step 2. Zero ledger retries and no lifecycle replay. |
| 5. Frozen snapshot and offline render | **NOT RUN** | Blocked by step 2. No artifacts or boundary bundle. |
| 6. Full receipt reconciliation | **NOT RUN** | Required artifact receipts unavailable. Failure export and teardown separately **PASS**. [Receipt table](l7-ledger/run5/receipt-table.json). |

Task `1` retains assignment `82389a66-5af4-4353-88f5-b5f32af3ebae` and remains
**pending**. Ledger `d95221c0-cd88-442e-be13-e47d79e2cee1` binds that exact
assignment. Onboarding message `3630ea73-7754-42a7-86de-8a59c3ac402f` had both
submission and `consumed_by_read` from alpha before the assignment. Assignment
message `3e76b819-7fdd-4d7c-9f69-f65f0707addb` had a submitted receipt, with no
qualifying consumption proof. No subsequent send occurred.
[Commands, RPCs and exits](l7-ledger/run5/events.jsonl),
[journal](l7-ledger/run5/team/state/messaging-v2/segments/000001.jsonl),
[task](l7-ledger/run5/tasks/1.json).

The daemon observed the real seat's native completion and reported fresh
`source: notify`, `state: idle` throughout the failed assignment wait. Its
last retained activity has output age **93 seconds**. No captured activity row
has `source: none`; therefore this trial does not demonstrate the specific
post-turn decay described in the fifth-trial ruling. The passive capture has
40 complete rows and one completed turn, with private text omitted. Its original
source digest, byte length and capture time were not recorded; it cannot be
bound to the final file digested in `rollout-inventory.json`. The original scratch
file was deleted at teardown, so these missing fields cannot be recovered.
Two notify identities are retained.
[Final activity](l7-ledger/run5/final-activity.json),
[notify records](l7-ledger/run5/notify-records.jsonl),
[native tail](l7-ledger/run5/rollout-tail.json),
[adjudication](l7-ledger/run5/adjudication.json).

**Base correction (review round 1):** ancestry verification passed, but
`26c06132` (#177) reverted `9617ea6e` (#176)'s `codex.rs` fix. The tested file
equals `106f06c7`'s blob `8f665c637b73d7380681a557bfb2426c79e48c56`, rather than
the fixed blob `3fb33d79d672b8121accccd0598b666361a367e9`. The entire `src-tauri/`
diff from `106f06c7` to `26c06132` is empty. Run5's daemon SHA-256 is
`177c4f333aeaef73ae672109ee97c731ad50a3e166c244d93fe715ec5f144fd6`, identical
to trials 1–4 despite the successful rebuild. The intended fixed-base retest
and its #176 failure criterion were therefore **NOT RUN**.

The orchestrator's ruling reattributes the historical run-4 boundary to
Taurhaus's #176 defect; run5 does not validate that attribution on a fixed base.
Earlier harness defects and raw observations remain recorded below.
**Separate product/release handoff to the orchestrator:** main at `26c06132`
lacks #176. Restore and verify the fix upstream before any sixth trial; this
evidence lane makes no product change. Replace ancestry-only admission with
`python3 -B docs/design/evidence/e2e/l7-ledger/gates.py --check-base` before
building or launching a future trial. It compares both HEAD and working-file
content to #176 and currently exits **1**, correctly rejecting this base.
An intentionally different upstream fix needs a reviewed content pin.
[Post-review content verification](l7-ledger/run5/preflight.json).

Runtime exited **1** after **143.990 seconds**. Teardown verified **zero owned
survivors**, a closed private port, and removal of the scratch root and copied
credential. All **189 physical daemon lines** are retained as 189 sanitized
JSONL records. The last event is `session_scanner.scan.completed`; no shutdown
record was captured. No account usage rows were present.
[Exit](l7-ledger/run5/controller-exit.json), [cleanup](l7-ledger/run5/cleanup.json),
[log manifest](l7-ledger/run5/daemon-log-manifest.json),
[complete daemon log](l7-ledger/run5/taurhaus.log.jsonl).

| Run5 spend | API-equivalent USD | Conservative USD |
| --- | --- | --- |
| `01a08e41-57a6-7410-b76d-60380a8ee2d4` — completed native turn | **0.00407104** | **0.05322840** |
| `01a08e41-59c9-79c1-a1f0-9d925800219d` — notify-only identity | **Unavailable** | **Unavailable** |
| Claude lead — login-only, zero paid inputs | **0** | **0** |

There were **2 observed identities**, **2 input reservations**, and one seat
attachment generation. Run5 used its fresh **12 inputs / USD 0.20 / 12 minutes**
budget; historical trials did not gate it. The input and runtime bounds were
met. Total spend and dollar-cap compliance remain **unverified**, because the
notify-only identity has no retained counters. These are inherited rate
estimates, not invoices. Workflow implementer/reviewer spend is separately
owned by the orchestrator and unavailable here. Metering gated no lifecycle
operation, and no paid retry or authorization question followed the failure.
[Every spend](l7-ledger/run5/spend-summary.json),
[original controller meter](l7-ledger/run5/cost-ledger.json).

`just build-daemon` and the pinned Mesh build both exited **0**, using the
respective checkout-local targets with one build job. Cargo admission used
30-second polls and waited only while at least three Cargo processes were
already running. Other lanes later started additional Cargo processes; none
was signalled. Merge-base verification succeeded for `26c06132` but did not
verify that #176's content survived. Mesh remained
`1f7447f` with no source or descriptor edit. The private runtime used protocol
**27**, Codex **0.153.4**, **gpt-5.6-luna / low**, both native Codex siblings, the
canonical production messaging policy, a login-only lead, and scratch-only
roots in a private PID namespace/tmux server.
[Build commands and exits](l7-ledger/run5/builds.json),
[preflight](l7-ledger/run5/preflight.json),
[candidate](l7-ledger/run5/candidate.json); binary SHA-256 digests are in the
`binary` rows of [events](l7-ledger/run5/events.jsonl).

The [controller](l7-ledger/run5/controller-at-execution.py),
[runtime](l7-ledger/run5/runtime-at-execution.py), and
[helpers](l7-ledger/run5/support-at-execution.py) ran unchanged. The additional
[passive collector](l7-ledger/run5_capture.py) only reads the controller-recorded
scratch root and retains native identity evidence; it never sends, writes live
state, or changes settlement. Two offline privacy tests failed first because
the collector was absent, then passed; all **14 existing controller tests**
also passed. This adds no product regression fix.
[Red](l7-ledger/run5/capture-red.txt), [green](l7-ledger/run5/capture-green.txt),
[controller checks](l7-ledger/run5/controller-green.txt).

Post-teardown gates: **`just check-quick` 0**, **`just lint` 0**,
**`just test-contracts` 0**. No `src-tauri/` diff was made, so
`just test-rust-unit` is inapplicable.
[Gate results](l7-ledger/run5/checks-result.json),
[Cargo admission](l7-ledger/run5/gate-cargo-polls.jsonl),
[execution record](l7-ledger/run5/execution.json),
[public evidence validation](l7-ledger/run5/public-evidence-check.json).
The byte-identical alpha runtime alias is recorded in
[deduplication](l7-ledger/run5/deduplication.json).

Review round 1 verified all five findings. The collector now publishes both
outputs by temporary-file replacement and records capture wall time, byte length
and SHA-256 from the same bytes used to parse each rollout. These are prospective
repairs; the original run5 tail is unchanged. Five added offline tests reproduced
the failures before repair; all seven collector/preflight/gate-driver tests now
pass. Fixtures use temporary roots and mocked gate/Git calls, with no live CLI
or credential access.

The original gate wrapper was not retained. The parameterized
[gate driver](l7-ledger/gates.py) now reproduces its run/output/log selection and
cleanup precondition: `python3 -B docs/design/evidence/e2e/l7-ledger/gates.py
--run-name run5 --log-dir .check-logs/l7-ledger-run5`. The review rerun uses
`--log-dir .check-logs/l7-ledger-run5-review --output-dir
.check-logs/l7-ledger-run5-review` to preserve historical results; its results
are appended to the [execution record](l7-ledger/run5/execution.json).
Review gates passed: `just check-quick` **0** (2,521 frontend tests),
`just lint` **0**, `just test-contracts` **0** (68 assertions). The first contracts
attempt exited **101**: its repository scan mistook a passing test-name string
in an old ignored run5 JSON gate output for a retired tool literal. That output
was preserved byte-for-byte under a `.log` extension, as were review result
files, and all three gates were rerun successfully. No product/test-source fix
was made. Exact exits, timings, Cargo admission polls and log digests are in the
execution record. Gate children exited and were reaped; no new runtime cleanup
was needed. No `src-tauri/` diff exists, so the Rust unit gate is inapplicable.

This fix round starts no paid trial: additional seat inputs **0**, seat spend
**USD 0**. It does not change the six-step outcomes or recover missing spend.

Deviations and limits: the designated lane-2 checkout no longer exists; its
committed run-3 controller was read in this checkout. The pinned Mesh checkout
contains no `docs/design/ledger-*.md`; its `USAGE.md` ledger contract was read.
The prescribed unchanged controller stopped at the first failed boundary, so
steps 3–6 were not executed. Native tail capture is sanitized identity/hash
evidence, not public model/message text. One notify-only cost is unavailable.
The original trial lacked an independent Opus lens; this fix round addresses
the Opus round-1 findings supplied by the orchestrator. No workflow PASS, release approval,
product change, Mesh commit, descriptor change, or plan-ledger edit is claimed.

## Historical fourth trial (superseded attribution; retained verbatim)

# Lane 7 — FAIL at step 2: assignment delivery unavailable; workflow incomplete

The fourth explicitly requested trial passed and committed step 1. Step 2
stopped before sending the note instruction because assignment delivery did not
settle within the **120-second** wait. Its raw classification is **mesh** (the
controller's delivery-boundary attribution); the underlying product or harness
cause is **unresolved**, not a demonstrated Mesh defect. No paid retry followed.

| Ordered step | Outcome | Classification / evidence |
| --- | --- | --- |
| 1. Create/assign task; initialize ledger | **PASS**, commit `5ebc205a` | **S-runtime**. [Immutable assignment](l7-ledger/run4/step1-immutable-assignment.json), [packet](l7-ledger/run4/step1-packet.json), [manifest](l7-ledger/run4/step1-manifest.json), [receipt](l7-ledger/run4/step1-init-receipt.json). |
| 2. Seat note, intake and live render | **FAIL before instruction send** | **mesh**, raw controller attribution; cause unresolved. [Outcome](l7-ledger/run4/step2-outcome.json). |
| 3. RESULT source and ledger submission | **NOT RUN** | Blocked by step 2; no source commitment or intake rejection. |
| 4. Ledger-only retry | **NOT RUN** | No ledger or lifecycle retry. |
| 5. Snapshot and offline views | **NOT RUN** | No artifacts or snapshot. |
| 6. Receipt reconciliation | **NOT RUN** | Receipts explicitly unavailable; failure export and teardown separately **PASS**. [Receipt table](l7-ledger/run4/receipt-table.json). |

Task `1` has frozen assignment `27d55b82-876f-4569-82eb-d6f135362b85`.
Ledger `9f50c8af-98ee-4c16-a3f5-505555ee01f4` binds that assignment to the
scratch project. Task creation, assignment and ledger init each exited **0**.
Onboarding `b18eac5a-e47e-455b-9754-1e948a14a190` had submitted and explicit read
receipts plus fresh idle before assignment. Assignment message
`9bd337c7-3529-4d0a-9872-7d6428cacf6a` had a **submitted** receipt but no explicit
read receipt. The native diagnostic found neither its assignment UUID nor its
canonical message ID in any captured tool-result row. The final task remained
**pending**, with no accepted/started timestamp, while the daemon reported idle.
No subsequent send was made. This is an unproven delivery, not a claim of a
`stage: pending` row or proof that submission equals consumption.
[Commands and exits](l7-ledger/run4/events.jsonl),
[journal](l7-ledger/run4/team/state/messaging-v2/segments/000001.jsonl),
[native row identities and hashes](l7-ledger/run4/native-assignment-diagnostic.json),
[task](l7-ledger/run4/tasks/1.json),
[final activity](l7-ledger/run4/final-activity.json).

Runtime exited **1** after **139.156 seconds**. Teardown verified **no survivors**,
a closed private port, and removal of the scratch root and credential copy.
All **180 physical daemon lines** were retained as 180 sanitized records.
[Exit](l7-ledger/run4/controller-exit.json), [cleanup](l7-ledger/run4/cleanup.json),
[log manifest](l7-ledger/run4/daemon-log-manifest.json),
[complete daemon log](l7-ledger/run4/taurhaus.log.jsonl).

| Run-4 turn | Known API-equivalent USD | Conservative USD |
| --- | --- | --- |
| `01a08e2e-0f65-7852-a472-899deeebb7d1` — 43,773 input, 36,864 cached, 507 output | **0.00272748** | **0.05313600** |
| `01a08e2e-1175-7d03-9902-46ed51d64976` — notify-only | **Unavailable** | **Unavailable** |
| Claude lead — login-only | **0** | **0** |

This trial has **2 observed turn identities**, **2 input reservations**, and one
seat attachment generation. Per the operator's explicit fresh-budget instruction,
its admission uses this trial's **12 inputs / USD 0.20 / 12 minutes**; historical
trials remain reported separately. No lifecycle action was gated by metering.
Actual total spend and dollar-cap compliance remain **unverified** because the
notify-only identity lacks counters. Across four trials: **9 observed identities**,
**7 input reservations**, known **USD 0.01830316**, conservative known
**USD 0.29244720**, plus **four unmetered identities**. These are inherited
API-equivalent estimates, not invoices; workflow spend is separately owned by
the orchestrator. [Every recorded spend](l7-ledger/run4/cumulative-spend.json),
[run meter](l7-ledger/run4/cost-ledger.json).

The fresh-run admission regression failed first, then all **14 offline controller
checks passed**. Assignment instructions now explicitly request inbox read/mark
before accept/start. Executed controller commit: `4cc54f41`.
[Red](l7-ledger/fresh-run-red.txt), [green](l7-ledger/fresh-run-green.txt),
[exact controller](l7-ledger/run4/controller-at-execution.py),
[exact runtime](l7-ledger/run4/runtime-at-execution.py),
[exact helpers](l7-ledger/run4/support-at-execution.py).
The unchanged binaries use Taurhaus product base `106f06c7`, Mesh `1f7447f`,
protocol 27, Codex 0.153.4 and gpt-5.6-luna / low.
[Candidate](l7-ledger/run4/candidate.json), [startup](l7-ledger/run4/startup-ready.json).

Post-teardown gates: `just check-quick` **0**, `just lint` **0**,
`just test-contracts` **0**. No `src-tauri/` diff, so the Rust unit gate does not
apply. [Gate results](l7-ledger/run4-checks-result.json),
[Cargo admission](l7-ledger/gate-cargo-polls.jsonl),
[evidence validation](l7-ledger/run4/public-evidence-check.json).
The independent Opus lens remains unavailable. Later required steps and complete
metering remain missing, so this workflow is incomplete. Earlier limitations
below remain historical; no product, Mesh source, descriptor or plan ledger changed.

## Historical third trial

# Lane 7 — INCOMPLETE: step 1 verified; harness delivery check blocked continuation

The third operator-requested trial completed the audit's step 1. Its controller
then timed out at an additional delivery check before recording that success.
The raw timeout is retained; the adjudication below follows the audit's actual
step boundaries. No product defect is established. No artifact intake or offline
read-back occurred, so the lane does **not** meet the PASS criterion.

| Ordered step | Outcome | Classification / evidence |
| --- | --- | --- |
| 1. Create/assign one task; initialize ledger with frozen assignment | **PASS** | **S-runtime**: all three Mesh commands exited 0; immutable assignment, packet, manifest and initialization receipt agree. [Adjudication](l7-ledger/run3/adjudication.json), [assignment](l7-ledger/run3/step1-immutable-assignment.json), [manifest](l7-ledger/run3/step1-manifest.json), [receipt](l7-ledger/run3/step1-init-receipt.json). |
| 2. Seat note, intake and live render | **NOT RUN** | **Harness** continuation boundary failed: delivery predicate did not recognize assignment exposure. No note was requested. |
| 3. RESULT source completion and ledger intake | **NOT RUN** | Blocked by the failed boundary; no source commitment or intake rejection. |
| 4. Ledger-only retry | **NOT RUN** | No completion or ledger retry. |
| 5. Snapshot and offline read-back | **NOT RUN** | No snapshot or artifact bytes to compare. |
| 6. Source/ledger reconciliation | **NOT RUN** | Required receipts unavailable; mandatory failure export and teardown separately **PASS**. |

Task `1` has immutable assignment `3f9dd5c7-84d8-4dc5-90b6-a3c13bcd714a`,
assignment event `43ddfdce-c0b1-47e1-85f0-9b35063c59d4`, and canonical message
`122964b1-1fc0-4cb6-8119-ae6a3bc5eb9f`. Ledger
`182cf4d8-2b3f-4503-b3b1-67c4c9f295ea` binds that assignment and the scratch repo
root. Packet digest: `544da14523b5524eda9979768adb44f9c223dc42b9b37196674394b8e5b01816`.
The five contract fields are retained in the exact task-create command;
message bodies in journal and task snapshots are redacted.
[Commands and exits](l7-ledger/run3/events.jsonl),
[packet](l7-ledger/run3/step1-packet.json),
[workflow events](l7-ledger/run3/team/state/workflow_events.jsonl).

The timeout reason, “seat did not accept/start frozen assignment,” is inaccurate:
alpha accepted at `01:43:32.892Z` and started at `01:43:56.443Z`, and the daemon
subsequently reported fresh idle. The extra predicate required either an explicit
read receipt or the **canonical message ID** in a rollout tool-result row after
transport submission. Assignment transport was submitted, but no explicit read
receipt or canonical ID appeared. An assignment UUID was observed in one native
tool-result row during diagnosis; that alone is not proof of the whole delivered
card. The row was not exported before teardown. Retained evidence therefore does
**not** establish the spec's alternate card proof, and no subsequent send occurred.
[Raw controller outcome](l7-ledger/run3/step1-outcome.json),
[actual task](l7-ledger/run3/tasks/1.json),
[attributed runtime](l7-ledger/run3/final-runtime-sessions.json),
[journal](l7-ledger/run3/team/state/messaging-v2/segments/000001.jsonl).

Runtime exited **1** after **172.441 seconds**. The private namespace was torn
down: **no survivors**, private port closed, scratch root and credential copy
removed. The complete daemon source was retained as **268 sanitized records from
268 physical lines**, including the final line. No paid restart followed this
failure. [Exit](l7-ledger/run3/controller-exit.json),
[cleanup](l7-ledger/run3/cleanup.json),
[log manifest](l7-ledger/run3/daemon-log-manifest.json),
[complete daemon JSONL](l7-ledger/run3/taurhaus.log.jsonl).

Executed source was evidence commit `0915c1fd`, with unchanged product base
`106f06c7`, Mesh `1f7447f`, protocol 27 and the previously built binaries.
The actual native Codex runtime remained 0.153.4, gpt-5.6-luna / low. Startup
completed the onboarding submitted/read/fresh-idle gate before assignment.
[Candidate](l7-ledger/run3/candidate.json),
[startup](l7-ledger/run3/startup-ready.json),
[executed controller](l7-ledger/run3/controller-at-execution.py),
[executed runtime](l7-ledger/run3/runtime-at-execution.py),
[executed helpers](l7-ledger/run3/support-at-execution.py).

| Run-3 turn | Known API-equivalent USD | Conservative USD |
| --- | --- | --- |
| `01a08e21-f8cc-70b2-b777-51a45cf109c3` — 44,311 input, 32,768 cached, 443 output | **0.00349556** | **0.05370480** |
| `01a08e22-302b-7132-8050-8a822f46ae91` — 88,396 input, 80,128 cached, 784 output | **0.00419696** | **0.10701600** |
| `01a08e21-fb15-7083-815d-d6f69fb4dc35` — notify-only | **Unavailable** | **Unavailable** |
| Claude lead — login-only | **0** | **0** |

Run 3: **3 observed turn identities**, **2 input reservations**, one seat
attachment generation; known subtotal **USD 0.00769252**, conservative known
subtotal **USD 0.16072080**, plus one unmetered identity. Across all three trials:
**7 observed identities**, **5 input reservations**, known **USD 0.01557568**,
conservative known **USD 0.23931120**, plus **three unmetered identities**.
The conservative estimate exceeds USD 0.20; actual total spend and dollar-cap
compliance remain unverified. Metering did not gate a lifecycle operation.
All rates are inherited estimates, not invoices. Workflow implementer/reviewer
spend remains separately owned by the orchestrator.
[Every recorded turn](l7-ledger/run3/cumulative-spend.json),
[run meter](l7-ledger/run3/cost-ledger.json),
[native counter excerpts](l7-ledger/run3/native-turn-meter.json).

The controller fix is offline only: it now accepts an exact accepted card body
in a native tool-result row after a transport receipt, decodes nested output
wrappers, and retains only proof identities/digests. A lone assignment UUID or
model-prose echo is insufficient. Step 1 is recorded immediately after its
required exports; the next send still waits for delivery and fresh idle.
The regression test failed first, then all **13 controller checks passed**.
This does not retroactively prove run 3's unexported card or later steps.
[Red](l7-ledger/card-delivery-red.txt),
[green](l7-ledger/card-delivery-green.txt).

Post-teardown gates for this continuation all **PASS**: `just check-quick`
**0** (2,521 frontend tests), `just lint` **0**, `just test-contracts` **0**.
All **13 offline controller tests** passed separately. No `src-tauri/` diff exists,
so `just test-rust-unit` is not required. Cargo admission was sampled before each
gate, with checkout-local targets and one Cargo build job per command.
[Gate exits and timings](l7-ledger/run3-checks-result.json),
[Cargo admission](l7-ledger/gate-cargo-polls.jsonl),
[public evidence check](l7-ledger/run3/public-evidence-check.json).

Remaining deviations: the extra harness delivery check stopped this trial; its
full native card row was not retained; total cost is unverified and the cumulative
conservative estimate exceeds the cap; the independent Opus lens is unavailable.
The requested artifact/snapshot PASS remains unestablished. The older missing
controller-worktree/design-file and run-2 log-retention limitations are recorded
below. All owned runtime processes are gone; no product or Mesh source changed.
 The independent Opus
evidence lens remains unavailable in this executor. Earlier deviations and
historical results below remain part of the record; their “no further trial”
statements describe the end of those earlier turns, before the next explicit
operator continuation. No plan ledger or product source was changed.

## Historical second trial

# Lane 7 continuation — FAIL at step 1: harness assignment-ID lookup

The authorized second trial passed startup onboarding, created task `1`, and
assigned it to alpha. It then stopped **before ledger init**: task assign returned
`delivery_id`, while controller `66df97a5` searched only `legacy_id`. That lookup
raised `StopIteration` (an empty exception string). This is a **harness defect**;
the raw default Mesh classification is corrected by the
[run-2 adjudication](l7-ledger/run2/adjudication.json).

| Step | Run-2 outcome | Classification |
| --- | --- | --- |
| 1. Task/assignment and ledger init | **FAIL** | Harness; task created and assigned, ledger init not attempted. |
| 2. Standalone note intake | **NOT RUN** | Blocked by step 1. |
| 3. RESULT completion/intake | **NOT RUN** | No source completion or ledger rejection occurred. |
| 4. Ledger-only retry | **NOT RUN** | No retry. |
| 5. Snapshot and offline render | **NOT RUN** | No snapshot. |
| 6. Receipt reconciliation | **NOT RUN** | Failure teardown independently **PASS**. |

The recorded assignment ID is `d9c0fd46-012f-4de8-8ca7-8fb5708d3d06`.
Its returned delivery ID `4cac5cb3-a379-42bd-a8e0-13aa62b54470` maps to canonical
message `5d55fb71-f8b6-4d0c-84ed-fb32ed0a1f74`. The offline regression failed first,
then all **11 tests passed** after matching the actual delivery ID.
[Red](l7-ledger/assignment-delivery-red.txt),
[green](l7-ledger/assignment-delivery-green.txt),
[executed controller](l7-ledger/run2/controller-at-execution.py).
No further paid trial followed this failure.

Runtime exited **1** after **18.590 seconds**. Teardown reports **no survivors,
closed private port, removed scratch root and removed credential copy**.
[Exit](l7-ledger/run2/controller-exit.json), [cleanup](l7-ledger/run2/cleanup.json).
The binary builds, protocol 27, model/effort and descriptor are unchanged from the
first run; startup uses the corrected read predicate and private roots.
[Startup evidence](l7-ledger/run2/startup-ready.json),
[candidate](l7-ledger/run2/candidate.json),
[commands and binary digests](l7-ledger/run2/events.jsonl).

**Log-retention limitation:** this run's daemon source had 69 physical lines;
68 complete JSON records were exported. The unparsed physical line was not
retained, and its content is unavailable after teardown. It cannot be assumed
to be an empty line or a complete event. Complete-source retention is therefore
**unverified**, an additional evidence deficiency.
[Manifest](l7-ledger/run2/daemon-log-manifest.json),
[retained daemon JSONL](l7-ledger/run2/taurhaus.log.jsonl).

| Run-2 turn | Known API-equivalent USD | Conservative USD |
| --- | --- | --- |
| `01a08e1c-a901-7293-94de-136cad81ad0f` — 44,522 input, 22,016 cached, 431 output | **0.00545872** | **0.05394360** |
| `01a08e1c-ab41-7551-b212-03c5d70f5ca4` — notify-only | **Unavailable** | **Unavailable** |
| Claude lead — login-only | **0** | **0** |

Across both trials: **4 observed turn identities**, **3 controller input
reservations**, known subtotal **USD 0.00788316**, conservative known subtotal
**USD 0.07859040**, plus **two unmetered identities**. Total dollar spend and the
USD 0.20 cap remain unverified. Metering did not gate lifecycle operations.
[Run-2 meter](l7-ledger/run2/cost-ledger.json),
[counter excerpts](l7-ledger/run2/native-turn-meter.json),
[cumulative spend](l7-ledger/run2/cumulative-spend.json).

Post-teardown continuation gates all **PASS**: `just check-quick` **0**,
`just lint` **0**, `just test-contracts` **0**. Check-quick executed 2,521
frontend tests; contracts executed 68 Rust assertions. All 11 controller
checks passed separately. No product source changed, so the additional Rust
unit gate does not apply. [Gate results](l7-ledger/run2-checks-result.json).
The green controller fixes were committed; no numbered runtime step completed.
The independent Opus evidence lens remains unavailable in this executor.

## Historical first trial

# Lane 7 — FAIL / unavailable at startup: harness read-receipt predicate

**Step 1 failed before task creation; steps 2–6 were not run.** The seat did
receive and explicitly read onboarding, and the daemon reported fresh idle.
The controller discarded the read receipt because it required `recipient`,
where Mesh's read receipt identifies the actor with `reader_name: alpha`.
This is a **harness defect**, not evidence of a Taurhaus or Mesh defect.
The original controller timeout classified it as Taurhaus; the
[offline adjudication](l7-ledger/adjudication.json) preserves and corrects that
classification. No product change or second runtime trial was made.

| Ordered step | Outcome | Classification / evidence |
| --- | --- | --- |
| 1. Create/assign one task; initialize ledger with its frozen assignment | **FAIL before these operations** | Harness startup predicate; initialization of the team succeeded, but the readiness wait timed out after 120 seconds. [Raw outcome](l7-ledger/run/step1-outcome.json), [adjudication](l7-ledger/adjudication.json). |
| 2. Seat writes standalone note; ledger entry; live render | **NOT RUN** | Blocked by step 1; no artifact or declaration. |
| 3. Seat submits RESULT; source completion and ledger receipts | **NOT RUN** | No lifecycle submission, no source commitment and no intake rejection. |
| 4. Ledger-only retry; identical receipt; no second completion | **NOT RUN** | No retry of any kind. |
| 5. Freeze writer; snapshot; offline current/narrative render | **NOT RUN** | No snapshot or offline read-back claim. |
| 6. Source/ledger reconciliation and export | **NOT RUN** | Required receipt comparison unavailable; [receipt availability table](l7-ledger/run/receipt-table.json). Mandatory failure teardown separately **PASS**. |

The live controller exited **1**, after **124.362 seconds** of runtime, below
the 12-minute cap. It made only the initialization/onboarding input reservation;
no assignment or subsequent send occurred. This run does not establish the lane's
ledger PASS criterion. [Controller exit](l7-ledger/run/controller-exit.json).

## Runtime and receipt evidence

Taurhaus product source was `106f06c7`; the exact executed controller commit was
`64f3d81c`. The designated Mesh worktree remained at `1f7447f`, without descriptor
or source edits. The private daemon answered protocol **27**. Copied native Codex
**0.153.4** ran **gpt-5.6-luna / low**, confirmed by the scratch rollout context.
Both native siblings were copied. The Claude lead was login-only with zero paid
inputs. [Candidate and binary digests](l7-ledger/run/startup-preflight-summary.json),
[private ping](l7-ledger/run/ping.json), [turn evidence](l7-ledger/run/native-turn-meter.json).

Canonical initialization used the builder's actual `DEFAULT_CANONICAL_POLICY`,
lead=`claude/tmux`, alpha=`codex/tmux`, and production
`coordination.initialize_team`. Runtime identity, root, attachment generation,
private socket/pane/PID/start ticks and attributed activity were captured in
[initial runtime](l7-ledger/run/step1-runtime.json),
[pane identity](l7-ledger/run/step1-pane-identity.json),
[operation result](l7-ledger/run/step1-operation.json), and
[final activity](l7-ledger/run/final-activity.json).

For onboarding message `a0ac01e9-82a6-4cb7-b925-444a63f53480`:

- Transport: journal sequence **6**, `stage: submitted`, recipient `alpha`.
- Explicit read: sequence **7**, event `09bb75a9-825b-4a2c-ae5b-0cf48dc4122a`,
  `kind: consumed_by_read`, `reader_name: alpha`, `reader: alpha@l7-ledger`.
- The executed predicate returns **false** over these captured rows and idle;
  the corrected offline predicate returns **true**. This is an offline replay,
  not a rerun or a green runtime step.

The full sanitized [journal segment](l7-ledger/run/team/state/messaging-v2/segments/000001.jsonl)
and [commands/RPCs](l7-ledger/run/events.jsonl) distinguish acceptance, transport
and explicit read. Message bodies are redacted. Pane captures contain at most
60 lines and redact card text. Complete native message/tool-result rows were not
exported; the retained native excerpts cover metering and model identity only.
No claim relies on a prompted reply or on a `stage: pending` row.

## Every recorded spend

Fresh run caps were **12 Codex inputs / USD 0.20 / 12 minutes**. The inherited
meter conservatively records **two turn identities**, one attachment generation,
and one controller input reservation. One turn has full token counters; another
notify-only identity has no retained token counters. **Total dollar spend and
the USD 0.20 cap are therefore unverified**, not zero or assumed within budget.
Metering did not block initialization, the receipt wait, or teardown.

| Turn identity | Evidence | API-equivalent USD | Conservative USD |
| --- | --- | --- | --- |
| `01a08e08-9b9d-7d61-a236-b9ab623301cd` | Completed rollout turn: 20,287 input, 10,752 cached input, 252 output tokens | **0.00242444** | **0.02464680** |
| `01a08e08-9df0-70f3-86be-c40886758378` | Notify-only identity; no corresponding retained rollout counter | **Unavailable** | **Unavailable** |
| Claude lead | Login-only; no model input | **0** | **0** |

Rates inherited from the specified integration/messaging controller are
USD 0.20 / 0.02 / 1.20 per million input / cached input / output tokens.
The conservative calculation prices all recorded input/output at USD 1.20/M.
These are API-equivalent estimates, not invoices. The known subtotal is
USD **0.00242444**, with an additional unmetered identity. Implementer/reviewer
spend is outside seat accounting and belongs to the orchestrator.
[Cost ledger](l7-ledger/run/cost-ledger.json), [counter excerpts](l7-ledger/run/native-turn-meter.json).

## Isolation, teardown, checks and deviations

All runtime homes, project/data/temp roots and sockets were scratch-only;
`TMUX` was absent. Bubblewrap hid operator homes and provided a private PID
namespace; the private daemon listened on probed port **46863**, not 17233.
Exactly one authorized authentication file was copied at mode 0600. No credential
source path, auth contents, member control token, installation ID or account
usage row is in the evidence. Complete daemon row order was retained with private
fields sanitized: **162 source rows / 162 exported rows**, including shutdown.
[Daemon JSONL](l7-ledger/run/taurhaus.log.jsonl), [row-count manifest](l7-ledger/run/daemon-log-manifest.json).

Teardown stopped only owned processes using PID/start-tick checks and namespace
ownership. **No survivors; port closed; scratch root removed; credential copy
removed.** [Teardown audit](l7-ledger/run/cleanup.json).

Offline tests failed first, then passed. The live-schema regression specifically
failed `test_live_read_receipt_schema_uses_reader_name` with `False is not true`;
all **7 tests pass** after the one controller fix. Its `// Regression:` comment
names `030980a7`. [Red](l7-ledger/reader-schema-red.txt), [green](l7-ledger/reader-schema-green.txt).
The original helper is preserved as [support-at-execution.py](l7-ledger/run/support-at-execution.py);
[current controller](l7-ledger/controller.py), [runtime machinery](l7-ledger/runtime.py),
and [tests](l7-ledger/controller_test.py) remain reviewable. No paid rerun followed.

Builds: initial `just build-daemon` exited **101** because the fresh worktree
lacked the ignored Tauri resource placeholder. `just ensure-tauri-resources`
exited **0**, followed by `just build-daemon` **0** and the designated Mesh
`cargo build --bin mesh` **0**. This prerequisite retry preceded all paid input.
[Build ledger](l7-ledger/builds.json), [initial failure](l7-ledger/daemon-build-initial-red.txt).
Cargo used checkout-local targets and one build worker; machine-wide admission
waits only at three or more Cargo processes, with 30-second polls and a 30-minute
queue deadline.

All required gates ran **after teardown** and now pass:

| Command | Initial exit | Final exit | Result |
| --- | --- | --- | --- |
| `just check-quick` | 127 | **0** | Rust test compilation, Svelte check, 150 frontend files / 2,521 tests passed. |
| `just lint` | 127 | **0** | Frontend and repository structure/workflow guards passed. |
| `just test-contracts` | **0** | **0** | 68 tests passed: 15 renderers, 20 harness conformance, 33 module boundaries. |

The two initial 127 exits were missing `svelte-check` / `knip` in this fresh
checkout. `bun install --frozen-lockfile` exited **0**; only those two failed
gates were retried. The final lint retry waited in 30-second polls while three
other Cargo processes were present. No product-file edit was needed.
[Initial gate results](l7-ledger/checks-result.json),
[successful retries](l7-ledger/gate-retries.json),
[Cargo admission observations](l7-ledger/gate-cargo-polls.jsonl),
[final audit](l7-ledger/final-audit.json).
No tracked `src-tauri/` diff exists, so `just test-rust-unit` is not required.

Other deviations: the historical lane-2 worktree is absent, so its versioned
run-3 controller/evidence in this checkout were used. The Mesh RC lacks the
referenced `docs/design/ledger-*.md`; USAGE.md and its implementation/tests were
read instead. Preparatory test/controller commits precede the numbered audit;
no numbered step was green, so no green numbered-step commit exists. No plan
ledger rows were edited. An independent **Opus evidence review is not available
in this executor** and remains for the orchestrator; the workflow is incomplete
without it. No product fix, Mesh commit, descriptor edit, release, install,
fault injection, load test or lifecycle replay occurred.
