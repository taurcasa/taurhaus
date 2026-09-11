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
