# UNPROVED — run 8: required working-window coverage (harness timing)

Run7: 14 inputs; $0.018016400 metered; 2 unknown-cost inputs; 336.60 seconds. All owned runtime processes and scratch authentication removed. See the [run7 packet](#run7--seventh-attempt-evidence).

## Historical run6 continuation verdict

The authorized continuation committed **steps 1–4 PASS**. Both restart boundaries
were crossed with pending mail on both transports. Step 5 failed because alpha
stopped at a paginated inbox-read result without fetching its marker. Beta received
its marker. The passive owner census passed across restart-self. The first-boundary
beta >=30-second continuous window remains **UNPROVED**: its original turn was
interrupted, and the paced command ran after resume. No full lane PASS is claimed.
**14 counted model inputs; $0.01652596 metered; two unknown-cost inputs; 420.08 s.**
One additional failed startup had no model turn; conservative cap count **15/20**.
All owned processes were removed, all three gates passed, and evidence is retained
under [run6/continuation](#run6-continuation--ordered-restart-evidence).

## Historical run6 initial verdict

Run6 stopped after **180.7 seconds** at beta's startup guard. The daemon's
startup delivery row, the card in host `item/started`, the matching completed
host turn, and host-sourced idle were all present. The controller incorrectly
required the user card in `item/completed` as well. **No baseline was sent and
neither restart was attempted.** This is an observer failure, not a product defect.
All owned processes and credentials were removed; all three gates passed.
**3 counted inputs; $0.00357304 metered; one unknown-cost input; 195.76 seconds runtime.**
See [run6 evidence](#run6--sixth-attempt-evidence-step-1-fails-harness).
Earlier attempts remain historical and do not establish run6 coverage.

## Historical run5 verdict

Run5 stopped after **180.8 seconds** waiting for beta's onboarding read evidence.
Alpha passed the onboarding guard. Beta completed its direct hosted startup turn
and has a runtime `submitted` recovery receipt, but has no `consumed_by_read`
receipt or card in a rollout **tool-result** row. Its card appears in a user-message
row. The guard remained closed: **no baseline sends and no restarts** occurred.
This establishes an unmet harness prerequisite, not a restart-survival result or
a product defect. All three gates passed after verified teardown.
**3 inputs; $0.00238148 metered; one unknown-cost input; 195.43 seconds runtime.**
See [run5 evidence](#run5--fifth-attempt-evidence-step-1-fails-harness).
Earlier attempts remain historical and do not establish run5 coverage.

## Historical run4 verdict

Run4 stopped on a baseline/startup inbox-read race after the required polling
window. Neither restart was attempted. All three required gates passed after
verified teardown. **3 inputs; $0.00302944 metered; one unknown-cost input.**
See [run4 evidence](#run4--fourth-attempt-evidence-step-1-fails-harness).
Earlier attempts below remain historical and do not establish run4 coverage.

## Historical run3 continuation verdict

Both restart boundaries carried pending mail on both transports; every baseline
and backlog ID has exactly one transport receipt and an explicit read. The first
boundary's >=30-second duration was not achieved. Opus round 1 also found that
**step 5's no-overlapping-owners sub-claim is UNPROVED — harness**: the owner
filter matched nothing, with **zero owners in all three samples**. The original
six controller PASS records remain historical observations of its predicates,
not full lane certification. See [continuation evidence](#run3-continuation--six-runtime-checks-completed).
Prior attempts below are historical.

## Historical first attempt and offline continuation

**Step 1 PASS; step 2 FAIL (harness); steps 3–6 NOT RUN.** No product defect or
restart-survival result is established. The controller stopped at the first failed
step, retained evidence, and removed all owned runtime processes and credentials.
Independent Opus review belongs to the invoking small-change orchestrator and is
not claimed by this implementer.

## Historical run1 candidate and scope

Taurhaus `a7e6db7e`, protocol **27**, branch `feat/e2e-l5-restarts`;
Mesh **310144d**, built only in `/home/mstie/projects/mesh-l5`. Its shipped
Codex **0.153.4** app-server descriptor stayed **enabled and unchanged**.
Both native Codex siblings were copied into the private bin; startup checked
`codex --version`. [Binary digests and pins](l5-restarts/candidate.json).

The lane spec's stricter caps govern: **12 Codex inputs, USD 0.25, 15 minutes**.
Actual runtime was **229.38 seconds**, 2026-09-10 22:41:16–22:45:05 UTC.
Both seats used `gpt-5.6-luna`, effort `low`; the unauthenticated Claude lead
remained in first-run setup and took zero paid turns. Total: **6 turns,
7 metered generations, $0.00500816 metered estimate, one unknown-cost turn**.
The metered amount is below the cap; total billed spend cannot be verified from
these records. Unknown cost never blocked an operation. No restart, recovery,
extra model trial, descriptor edit, product change, install or release occurred.

## Historical run1 ordered outcomes

| Step | Outcome / classification | Evidence and spend |
|---|---|---|
| 1. Initialize; complete/read baseline messages | **PASS — runtime**, after a read-only observer correction | Production `coordination.initialize_team` completed with canonical builder policy, lead + alpha tmux + beta app_server. Distinct replies, transport receipts, attributed fresh idle and `consumed_by_read` were proven. 5 turns; $0.00398180 metered plus one unknown-cost startup turn. Committed as `10ba9d72`. |
| 2. Busy turns; capture pending mail on both seats | **FAIL — harness** | Alpha's one confirmed bounded input produced all 80 requested lines. The mixed sidecar/daemon activity predicate rejected the working interval and timed out after **120.7 s**. No backlog marker was sent, and beta's bounded input was not submitted. +1 turn / $0.00102636; cumulative 6 / $0.00500816 plus the same unknown turn. |
| 3. Normally restart Taurhaus; record recovery | **NOT RUN — blocked by step 2** | No SIGINT/restart boundary, replacement PID or resume-member operation. +0 inputs / $0. |
| 4. Deliver backlog; check identity/no replay | **NOT RUN — blocked by step 2** | No backlog IDs exist. Restart identity preservation and baseline replay exclusion are unproved. +0 inputs / $0. |
| 5. Fresh pending mail; Mesh restart-self | **NOT RUN — blocked by step 2** | `mesh team-daemon restart-self` was never invoked. No owner-epoch transition established. +0 inputs / $0. |
| 6. Read/reconcile both restart boundaries | **NOT RUN — blocked by step 2** | Cross-boundary obligation accounting is unproved. Mandatory failure cleanup **passed separately**. +0 inputs / $0. |

[Per-step outcomes](l5-restarts/run/step2-outcome.json),
[exact commands/RPCs and exits](l5-restarts/run/events.jsonl),
[losslessly packed snapshots](l5-restarts/run/snapshots.json).
The snapshot packet maps original filenames to SHA-256-keyed text payloads:
**67 files, 54 unique payloads**. `pack.unpack(packet)` restores exact text.
It includes original initialization/results, bounded panes, paginated inbox and
journal reads, runtime/config/epoch records, receipt snapshots, owner status,
passive `/proc/locks`, observed process identities, and original controller/step
stdout. Complete daemon JSONL remains a separate direct file.

### Step 1 identities and delivery evidence

Team incarnation: `c5e686732f8ad109dfc01d2e4a9357757983347813bd8ee8fa78adef417cb519`.
Delivery owner: team; epoch **2**, namespace PID **2717**, start ticks **30062136**.
Both attachment generations were **1**. Host/kernel identities are retained
separately in the snapshot packet and [owner observations](l5-restarts/run/owner-observations.jsonl).

| Seat | Logical identity | Baseline message / delivery ID | Receipt and reply |
|---|---|---|---|
| alpha, tmux `%2` | `01a08d7b-8878-7071-9fb8-caa350aef74f` | `a3ea3a40-befa-4c75-a399-4a91f874b9aa` / `61ace4c0-c7c3-4617-bab1-3fc4d87f7fb8` | `submitted`; `L5_baseline_alpha_802543`; attributed fresh idle; explicit read |
| beta, app_server `%3` | `01a08d7b-8f23-7310-8a8c-bccfbbe625f7` | `995a7bab-7d0f-4e56-8fed-c1ead45900f0` / `6bc9ee9a-7aff-43d7-b8e4-fa3db4841a1d` | `native_enqueued`; `L5_baseline_beta_d745e9`; attributed fresh host idle; explicit read |

The recorded delivery predicates required receipt **and** attributed fresh idle
**and** explicit read. Replies were independently found in native assistant rows;
acceptance alone was not treated as exposure, consumption or model action.
Journal paging kept filters unchanged and followed the returned cursor until
`done`; the bounded run's pages completed on their first page.

### Why step 2 is a harness failure

The complete daemon log reports alpha's exact session switching to `active`
(source `transcript`) at **22:43:05.486Z**, then to `idle` (source `notify`) at
**22:43:14.986Z**. The observer received **11** `state: active`,
`activity_attribution: attributed` snapshots in that interval. Its additional
sidecar predicate did not admit them, so `send()` was never reached.
The terminal submission was real: the rollout contains the requested 80-line
answer and a new metered turn. [Diagnostic excerpts](l5-restarts/run/step2-diagnostics.json).

Per-poll sidecars were overwritten by the inherited snapshot collector. Thus the
exact failing conjunct cannot be independently reconstructed; the evidence proves
that the daemon reported attributed activity and that the observer missed it.
This is not evidence of a product failure or an absent busy interval. The original
120.7-second failure and controller exit **1** are preserved. No paid retry or
restart followed it, and the retained controller is **not a passing reusable
restart harness**. The backlog and both restart boundaries remain unproved.

## Every measured spend

Rates are the inherited trial packet's API-equivalent estimates, not an invoice:
$0.20 / $0.02 / $1.20 per million uncached input / cached input / output tokens.
Reasoning is included in output, not charged twice. The ordinary-token upper-rate
comparison is $0.09389640; it does not bound the unreported turn.

| Step / seat | Turn ID / generation | Input / cached / output (reasoning) | USD estimate |
|---|---|---|---|
| 1 beta startup | `01a08d7b-94ff-7832-9815-a6c090885bc4` / 1 | 11145 / 6912 / 94 (85) | 0.00109764 |
| 1 alpha startup | `01a08d7b-a152-7083-b5dc-39ce153ccbce` / 1 | 9282 / 6912 / 96 (22) | 0.00072744 |
| 1 alpha startup | `01a08d7b-a152-7083-b5dc-39ce153ccbce` / 2 | 10793 / 8960 / 42 (33) | 0.00059620 |
| 1 ephemeral startup | `01a08d7b-a33f-7ab2-aff4-a220c99a86fd` | Unreported | **Unknown** |
| 1 alpha baseline | `01a08d7c-a71b-7361-875e-aee6cc62c65d` / 1 | 10893 / 9984 / 82 (8) | 0.00047988 |
| 1 alpha baseline | `01a08d7c-a71b-7361-875e-aee6cc62c65d` / 2 | 11773 / 9984 / 182 (168) | 0.00077588 |
| 1 beta baseline | `01a08d7c-ce36-7553-b18c-45f7d2389f4a` / 1 | 11353 / 11008 / 13 (0) | 0.00030476 |
| 2 alpha bounded response | `01a08d7d-2628-7d90-be18-fce8550131dc` / 1 | 11991 / 11008 / 508 (23) | 0.00102636 |

[Full cost ledger](l5-restarts/run/cost-ledger.json),
[rollout usage events](l5-restarts/run/usage-events.json),
[host events](l5-restarts/run/host-events.jsonl),
[native message/tool excerpts](l5-restarts/run/rollout-items.json).
Builds, observer correction, export and gates add zero trial turns.
Implementer/reviewer spend is outside the trial ledger and belongs to the orchestrator.

## Isolation, retention, teardown and deviations

- Reused the messaging run2 controller/actions/support/retention with attempt9's
  normal restart structure. The requested `taurhaus-trial` checkout was absent
  (Git exit **128**); its named attempt9 sources were available in this checkout's
  tracked history and read here. Messaging sources were read via `git show` in
  `/home/mstie/projects/taurhaus-msg`; that checkout was not modified.
- Runtime roots were scratch-only under `/tmp/th-l5-jg_gyq7i`; private PID namespace,
  private tmux server, inherited `TMUX` absent, operator homes hidden, probed private
  daemon port. Both native Codex siblings were used. Only the explicitly authorized
  auth file was copied, mode 0600; credentials and installation/account usage data
  were not retained. No observer connected directly to an app-server socket.
- Startup correction before any baseline input: the first read-only step-1
  observer wrongly required beta's sidecar to contain `state`; authoritative host
  state came from the daemon snapshot. Only that owned observer process was
  terminated (exit **143**), a synthetic regression test failed first, then passed,
  and the observer resumed against the same team without replaying paid input.
  The unmodified paid controller continued throughout. No complete step-1 failure
  or paid-controller relaunch was hidden.
- Step 2 then failed as described above. Later steps were stopped, with no product
  fix and no second paid run. No transient `host member busy` / `lock busy` RPC
  refusal occurred (zero retry episodes). Nonzero owner-health counters are
  retained in the owner status; no general delivery-health claim is made.
- [Cleanup](l5-restarts/run/cleanup.json): no surviving owned process identities,
  private port closed, scratch auth explicitly removed before root removal, root
  removed. Private namespace teardown covered daemon, host child, TUI, tmux and
  Codex descendants. No foreign process was killed. [Waited controller exit](l5-restarts/run/controller-exit.json): **1**.
- [Complete daemon JSONL](l5-restarts/run/taurhaus.log.jsonl): **550 rows**, no event-family
  filtering. Sanitized SHA-256:
  `9b30d2e4f45871ef1de6d2c2a580d01222134e7434135ab7e0f881cf0a438475`.
  Pane captures are at most 60 lines. Duplicate artifacts are interned losslessly;
  opaque reasoning ciphertext and internal model metadata were omitted from
  nonessential rollout excerpts.

## Commands, TDD and gates

Exact source: [controller.py](l5-restarts/controller.py),
[steps.py](l5-restarts/steps.py), [actions.py](l5-restarts/actions.py).
Actual sequence (from this checkout root):

```text
python3 docs/design/evidence/e2e/l5-restarts/build.py
python3 docs/design/evidence/e2e/l5-restarts/finish.py --controller
python3 docs/design/evidence/e2e/l5-restarts/steps.py 1
# Stop only the read-only observer; red-first hosted-state predicate correction.
python3 docs/design/evidence/e2e/l5-restarts/steps.py 1
python3 docs/design/evidence/e2e/l5-restarts/steps.py 2
# Failure invokes controller finally; supervisor waits and records exit 1.
python3 docs/design/evidence/e2e/l5-restarts/gates.py
python3 docs/design/evidence/e2e/l5-restarts/pack.py
```

Offline discovery uses generated data only; no harness CLI or real harness home.
Initial helpers: missing-module red **1** → four tests green **0**. Hosted-state
regression: missing-helper red **1** → five tests green **0**, with a
`// Regression:` comment naming `9fa886ee`. Lossless packing: missing-helper red
**1** → six tests green **0**. These checks do not certify the failed busy predicate.

| Exact command | Exit / result |
|---|---|
| `just ensure-tauri-resources` | **0**, local ignored placeholders |
| `just build-daemon` | **0**, 753.43 s, checkout-local target |
| Mesh `cargo build --bin mesh` | **0**, 32.26 s, designated Mesh target |
| `bun install --frozen-lockfile` | **0**, checkout dependencies |
| `just check-quick` | **0**, 340.25 s; Rust test compilation, Svelte 0 errors/warnings, 150 files / 2,519 tests passed |
| `just lint` | **0**, 75.72 s; Clippy, dependency/workflow/recipe checks passed |
| `just test-contracts` | **0**, 252.38 s; 15 renderer, 20 harness and 33 module-boundary tests passed |
| `just test-rust-unit` | Not required: no `src-tauri/` diff |

Gates run **after teardown**, from the checkout root, inside a fresh credential-free
namespace with real harness CLIs blocked. Cargo preflights use the requested
`pgrep -af '(^|/)cargo( |$)'`; wait only at three or more existing Cargo processes,
30-second polls bounded by 30 minutes; each launched Cargo uses one build job
and this checkout's own target. No full `just check` or concurrent paid/gate run.
[Build results](l5-restarts/build/daemon-build.json), [gate records](l5-restarts/gates/).

## Continuation — observer repair, no paid relaunch

The continuation began from clean `b71ea094`; every previously green result,
including runtime step 1, was already committed. The original runtime packet,
cost ledger and complete daemon JSONL are unchanged. The controller/support as
executed in that run remain recoverable at `b71ea094`.

Commit `c4645d2e` repairs the step-2 observer: both tmux and hosted seats now take
state from the matched, attributed daemon session. The sidecar supplies freshness
evidence and cannot override an active daemon state with missing or old idle
state. Attribution and the 120-second freshness check still apply. Two new
synthetic regression tests (comment naming introducing commit `f95ec193`)
produced four failing assertions, exit **1**; after the repair, all **8 tests**
passed, exit **0**. No real CLI or harness home was used by these tests.
[Red](l5-restarts/continuation/activity-red.json),
[green](l5-restarts/continuation/activity-green.json).

**Runtime step 2 is repaired offline, not rerun; steps 3–6 remain NOT RUN.**
The original team's scratch root and all sessions were removed by mandatory
teardown, so there is no live step-2 continuation to resume. The shared execution
contract explicitly says a controller restart does not reset the lane cap.
Six of the spec's twelve inputs were already consumed. A new complete sequence
needs at least ten more inputs: two seat onboardings, two baseline messages,
two initial bounded turns, two first-backlog deliveries, and two second-backlog
deliveries. This lower bound excludes additional startup, recovery and second
busy-window turns. It already exceeds the remaining six inputs, so no new paid
runtime was launched and no cumulative counter was reset. This is an input-count
constraint, not a refusal based on unknown cost. No authorization question was
asked and no metering check blocked a lifecycle operation.

Continuation adds **0 inputs / $0 metered**; the original six inputs,
$0.00500816 metered and one unknown-cost startup turn remain the lane totals.
No new product process or credential copy was created. The normal isolated gates
were rerun after the existing teardown, with separate output under
`continuation/gates/` so historical gate evidence remains intact.

| Continuation gate | Exit | Seconds |
|---|---:|---:|
| `just check-quick` | 0 | 25.99 |
| `just lint` | 0 | 24.23 |
| `just test-contracts` | 0 | 19.31 |

All three gates passed; 2,519 frontend tests and 68 contract tests passed.
No `src-tauri/` diff, so `just test-rust-unit` remains inapplicable. Gate children
were waited, the gate root removed, and the original 550-row daemon log hash
rechecked unchanged. [Continuation result](l5-restarts/continuation/result.json).


## Run2 second-attempt evidence

**UNAVAILABLE — step 1 PASS; step 2 FAIL (harness observer / backlog unproved); steps 3–6 NOT RUN.**
The repaired controller ran for real from initialization on a fresh scratch root.
It admitted alpha's attributed active state and sent the backlog marker inside that
interval. Its pending predicate then required a journal `stage: pending` receipt;
no such receipt appeared during **120.5 seconds** of polling. The marker proceeded
to one `submitted` receipt and `consumed_by_read`. No retained deferral-health
history proves pending backlog at a restart boundary. This is an evidence/harness
limitation, **not an established Taurhaus or Mesh defect**. The stop-on-first-failure
rule prevented beta's step-2 probe and both restarts. No paid retry was performed.

| Ordered item | Outcome | Classification / evidence |
|---|---|---|
| 1. Initialize, deliver/read both baselines | PASS | S-runtime: tmux `submitted`, hosted `native_enqueued`, replies and `consumed_by_read`; commit `7ba751d3` |
| 2. Busy turns and pending markers | FAIL | Harness: alpha active and marker accepted, but no journal pending receipt; beta probe NOT RUN after first failure |
| 3. Normal Taurhaus shutdown/restart | NOT RUN | Pending-backlog prerequisite unproved; no new daemon PID/start-tick or restart protocol claim |
| 4. Both backlog IDs delivered after restart, no replay | NOT RUN | Step 3 not reached |
| 5. Fresh pending markers and `mesh team-daemon restart-self` | NOT RUN | Step 2 stopped the lane; no owner-change or non-overlap claim across restart |
| 6. Read/account across both boundaries; export/teardown | NOT RUN as numbered acceptance item | Cross-boundary reconciliation absent; evidence export and owned-process teardown completed separately |

### Run2 candidate, identities, and commands

Taurhaus source `a7e6db7e`, protocol **27**, unchanged product code on
`feat/e2e-l5-restarts`; Mesh **310144d**, built in its designated
`/home/mstie/projects/mesh-l5` worktree. The exact 0.153.4 descriptor remained shipped
enabled; no Mesh edit or commit. `just ensure-tauri-resources`, `just build-daemon`,
and the prescribed lane-local Mesh `cargo build --bin mesh` all exited **0**.
Cargo probes saw one other Cargo process and proceeded with `CARGO_BUILD_JOBS=1`;
no three-process wait was needed. Targets remained checkout-local.

The run used both native **Codex 0.153.4** siblings, **gpt-5.6-luna / low**, the real
canonical builder policy, production `coordination.initialize_team`, command-capable
AGENTS.md, and a scratch login-only Claude lead. Private PID/tmux namespaces hid
operator homes. Only the expressly authorized single auth file was copied, mode
0600, into the otherwise-empty scratch Codex home. Hosted observation used only
the daemon transcript RPC. Exact commands, RPCs, results, and binary hashes are in
[events.jsonl](l5-restarts/run2/runtime/events.jsonl) and
[final-audit.json](l5-restarts/run2/final-audit.json).

- Team incarnation: `f98643ab89360862f449e4aaa6806db25a9b493f453168e8230fc8f14d99529f`.
- Alpha: session `01a08d9a-f2c5-79e3-a789-507db57d72b4`, pane `%2`, generation **1**, tmux.
- Beta: session/thread `01a08d9a-f727-79a2-b71a-ec21a4a953db`, pane `%3`, generation **1**, app-server.
- Initial Mesh owner: epoch **2**, namespace PID **2437**, start ticks **30267959**.
- Daemon SHA-256: `4304d1d0350768021c2af8a47ac88e95993cdc1e32c0a1942ee9f73cb1eea906`.
- Mesh SHA-256: `2eac5cf4194a2b4d83edf1f543d76ecea06602007d38f831e85c06a974408203`.

The initial Mesh owner epoch is not a sanctioned restart observation. The complete
[owner observations](l5-restarts/run2/runtime/owner-observations.jsonl) retain
startup behavior without relabeling it step 5.

### Run2 decisive excerpts

Alpha `activity.state.changed` at **23:16:18.853 UTC** changed to `active`, source
`transcript`, attributed to its recorded session. The saved daemon snapshot also
reports `active` / `attributed`. The backlog marker was accepted at
**23:16:20.931 UTC**, message `08e9162a-2c63-4c27-b448-79428dc690fb`, delivery
`34a49618-84b3-435f-bd44-9bba5b02afc8`. Its exact journal rows are retained in
[step2-diagnostics.json](l5-restarts/run2/runtime/step2-diagnostics.json):

- Sequence **16**: `message_accepted`; CLI reports `projection: pending`.
- Sequence **17**: `delivery_attempt`, `attempt_started`, durable claim.
- Sequence **18**: `receipt`, `submitted`, literal paste/harness submit exited 0.
- Sequence **19**: `receipt`, `kind: consumed_by_read`, reader alpha.
- **Zero** `stage: pending` receipts for this ID. Projection pending is not proof
  of transport deferral or a backlog surviving a restart.

The observer failed with `backlog unproved: no pending receipt alpha; polled 120.5s`.
The raw [step2 outcome](l5-restarts/run2/runtime/step2-outcome.json) preserves its
initial unclassified status; the final audit and this report classify it as harness.
The baseline and alpha backlog each have one observed transport exposure; the
unexecuted restart/no-replay claims remain unproved.

### Run2 every spend

Fresh run2 caps were **20 Codex inputs / $0.30 metered / 900 seconds**. Historical
attempt spend was not charged against them. This attempt recorded **7 inputs**, nine
metered generation rows, **$0.00774372 API-equivalent metered**, and **one unknown-cost
notify-only turn**, counted as an input. Metered generations' conservative valuation
is **$0.12560760**, not a bound for the unknown turn. These are token-rate estimates,
not an invoice or a claim that unknown cost is zero. Total setup-through-teardown
runtime was **168.129 seconds**. The runtime ledger's full IDs and token counts are
in [cost-ledger.json](l5-restarts/run2/runtime/cost-ledger.json).

| Turn ID | Seat / input | Metered generation costs (USD) | Turn total (USD) |
|---|---|---|---:|
| `01a08d9a-fd69-7430-adb1-6f26e7994c8b` | beta startup | 0.00107744 | 0.00107744 |
| `01a08d9b-0a02-7890-a86f-98362abb84c1` | alpha startup | 0.00145912 + 0.00059300 | 0.00205212 |
| `01a08d9b-0bdd-7fc1-aa99-c0d66e27275d` | alpha notify-only startup | Unknown | Unknown |
| `01a08d9b-2f93-7f12-9334-e484f7824ac6` | alpha baseline | 0.00048288 + 0.00077988 | 0.00126276 |
| `01a08d9b-76ec-7f90-a239-b740b8b68abb` | beta baseline | 0.00115264 | 0.00115264 |
| `01a08d9b-929f-7a10-99bd-bb2370ca15fe` | alpha bounded response | 0.00102556 | 0.00102556 |
| `01a08d9b-c07d-7621-911f-0851c5dd4d3a` | alpha backlog response | 0.00063196 + 0.00054124 | 0.00117320 |

Claude lead: **0 paid inputs**, login-only. No compaction, steering, lifecycle
restart, resume, or retry input was added. One transient busy refusal was retried
inside the controller's **65-second** window; its exact method, error, and attempt
are in `final-audit.json`. Metering never gated a lifecycle command. Implementer and
reviewer service spend is outside the seat ledger and belongs to the orchestrator.

### Run2 checks, retention, and cleanup

The new offline checks first observed **one failure and one import error**: the old
12-input/$0.25 cap rejected the run2 limit, and the daemon-only busy helper was absent.
After applying the ruling, the same `python3 -m unittest discover -s
 docs/design/evidence/e2e/l5-restarts/run2 -p '*test.py'` passed **10 tests** (eight
retained, two new). [red.txt](l5-restarts/run2/red.txt) and
[green.txt](l5-restarts/run2/green.txt) preserve both results. The busy regression
comment identifies `f95ec193`. This was controller work only; no product fix.

All gates ran from this checkout root, after runtime teardown, under a separate
credential-free home/PID namespace with real harness commands blocked:

| Exact gate | Exit | Observed result |
|---|---:|---|
| `just check-quick` | 0 | Rust format/test compilation, typecheck, 150 frontend files / 2,519 tests passed |
| `just lint` | 0 | Rust/frontend/workflow checks passed |
| `just test-contracts` | 0 | Renderer, harness, and module-boundary contract suites passed |
| `just test-rust-unit` | NOT RUN | No `src-tauri/` diff, so the conditional gate does not apply |

Each gate's Cargo preflight exited **1** (no running Cargo processes). Full exits,
timings and sanitized output are under [run2/gates](l5-restarts/run2/gates).
Both gate children and paid runtime children were reaped. Runtime controller exit
**1** is retained in [controller-exit.json](l5-restarts/run2/runtime/controller-exit.json);
it is the failed step outcome, not a build or cleanup error.
[cleanup.json](l5-restarts/run2/runtime/cleanup.json) records `survivors: []`, private
port closed, auth copy removed before root deletion, root removed, and auth absent.

The **complete sanitized daemon JSONL**, all **428 rows**, remains direct in
[taurhaus.log.jsonl](l5-restarts/run2/runtime/taurhaus.log.jsonl), SHA-256
`21bf4ec07e96db43dbb3d7426f08f5043dfaa8b15fa6a946415a7cb688610f61`.
Other repeated snapshots are losslessly interned: **69 named files / 55 unique
payloads** in [snapshots.json](l5-restarts/run2/runtime/snapshots.json), recoverable
with `pack.unpack`. Pane captures remain bounded to 60 lines. No credential values,
installation IDs, account usage rows, or operator-home paths are retained.

Exact instrument: [controller.py](l5-restarts/run2/controller.py),
[steps.py](l5-restarts/run2/steps.py), [execute.py](l5-restarts/run2/execute.py),
[actions.py](l5-restarts/run2/actions.py), [support.py](l5-restarts/run2/support.py),
plus the run2 build, finish, retention, packing and gate scripts. `execute.py` ran
steps in order and committed step 1 before step 2. Reproduction requires a **new**
evidence directory and separately authorized attempt, not reuse of these artifacts.

Deviations and limits: (1) missing pending-backlog evidence stopped the numbered
lane; (2) the prescribed `taurhaus-trial` reference checkout was absent, so this
lane's committed repaired controller and messaging run2 sources were reused;
(3) an independent Opus review was unavailable in this session and remains for the
orchestrator. No full-workflow PASS or release approval is claimed. No product,
Mesh descriptor, installed binary, or plan-ledger changes were made.


## Run3 — third-attempt evidence

**UNAVAILABLE; no product defect or full restart-survival PASS established.**
The fresh attempt ran once, stopped at its first assertion failure, and used
no paid retry. Raw step-2 checkpoint says PASS for the pending predicate; the
final assessment qualifies that with the subsequently measured duration shortfall.
[Final assessment](l5-restarts/run3/final-audit.json),
[exact controller](l5-restarts/run3/controller.py),
[ordered driver](l5-restarts/run3/steps.py),
[supervisor and checkpoint commits](l5-restarts/run3/execute.py).

Taurhaus product code remains **a7e6db7e**, protocol **27**, on the requested
`feat/e2e-l5-restarts` checkout. Mesh remains **310144d** in the designated
`mesh-l5` worktree: shipped 0.153.4 descriptor **enabled and unchanged**.
`just ensure-tauri-resources`, `just build-daemon`, and the designated Mesh
`cargo build --bin mesh` all exited **0**, with checkout-local targets and
`CARGO_BUILD_JOBS=1`. Binary SHA-256s are in the final assessment and raw trace.
Both native Codex siblings were copied; `codex --version` verified **0.153.4**.
Alpha used tmux; beta used app_server; both used **gpt-5.6-luna / low**.
The Claude lead remained in credential-free first-run setup, with zero observed
paid Claude turns. Production initialize used the builder's canonical policy.

### Ordered outcomes and spend

| Step | Outcome / classification | Evidence and spend |
|---|---|---|
| 1. Initialize and complete/read baselines | **PASS — runtime** | Both distinct baseline replies, one transport receipt each, attributed fresh idle and explicit `consumed_by_read`. 5 inputs; $0.00497156 metered plus one unknown-cost startup turn. Green commit `703163fd`. |
| 2. Busy turns and pending samples | **Pending PASS; duration PARTIAL — harness timing** | Both markers accepted, no submitted/consumed/native_enqueued receipt, both matching sessions attributed `active` at the same sample. +2 inputs; alpha $0.00148456, beta interrupted/unmetered. Green pending checkpoint `8413214e`; later measured alpha duration only 17.492 s. |
| 3. Normal Taurhaus stop/restart and recovery | **FAIL — harness** | Actual SIGINT, new daemon PID/start ticks, protocol 27. Checkpoint asserted against cached old identities. Hosted `stopped:true` was wrongly treated as readable and no supported resume was invoked. +1 alpha backlog input during failure cleanup, cost unknown. |
| 4. Both backlog deliveries; stable identities/no replay | **NOT RUN — blocked by step 3** | No acceptance-window assertion or explicit read. Cleanup happened to observe one alpha `submitted`; beta had no native_enqueued receipt. This incidental evidence is not step 4 PASS. +0 additional inputs / $0. |
| 5. Fresh backlog and Mesh restart-self | **NOT RUN — blocked by step 3** | Zero restart-self commands; no second owner boundary or fresh markers. +0 inputs / $0. |
| 6. Read/reconcile both boundaries | **NOT RUN — blocked by step 3** | No cross-boundary no-loss/no-duplicate verdict. Cleanup is separately proven. +0 inputs / $0. |

The run-3 ruling's fresh caps were **20 inputs / $0.30 metered / 15 minutes**.
Metering did not gate any observer, restart, lifecycle action or teardown.
[All turn/generation rows](l5-restarts/run3/runtime/cost-ledger.json),
[raw token evidence](l5-restarts/run3/runtime/usage-events.json),
[commands, RPC results and timestamps](l5-restarts/run3/runtime/events.jsonl).
The per-step checkpoint costs are observation-time values: step 2's preparatory
checkpoint precedes its inputs, and alpha's bounded-turn usage arrived during
failure cleanup. The causal spend allocation above uses the final ledger.

### Pending sample and actual restart

Runtime: **2026-09-10T23:34:29.216403+00:00 → 2026-09-10T23:35:17.023507+00:00**, **47.807 seconds**, including teardown.

At **23:34:58.837 UTC**, both sessions were attributed `active`. Alpha's
snapshot had only `message_accepted`; beta also had an app-server
`pending: pre_input_failure: IO error: delivery: thread_active` receipt.
The predicate did **not** require a `stage: pending` row. The complete final
journal confirms neither marker had a transport receipt before shutdown.

The controller initiated SIGINT at **23:34:58.841 UTC**, **3.906 ms** after the
retained joint sample. No capture, receipt wait or git commit separated them.
The step-2 commit happened after restart initiation so it could not delay the
boundary. Pre-turn identity/pane checkpoints are explicitly preparatory snapshots;
the separate pending samples are the authoritative boundary evidence.

| Seat | Session / thread | Backlog message ID | Delivery ID |
|---|---|---|---|
| alpha | `01a08dac-3cd3-7100-b8c6-1208790bbd70` | `b58995a3-928d-411f-a954-e6e99f722079` | `2a22bb2a-855c-4023-9236-0fbcd41ad090` |
| beta | `01a08dac-438b-7373-a5ed-0039c754dfc4` | `b5463bda-d143-455d-b35c-b06a3e50ad78` | `821f2712-574a-4f19-b4bc-7ac40b96fd1f` |

Team incarnation remained
`d79b1746ddc491577e13ad39cb25d3c32e160680bf6e81b70741bc5fedf7bbf6`.
Mesh owner epoch stayed **2**, namespace PID **2961**, start ticks **30381396**.
This is distinct from the Taurhaus host-owner PID. The latter changed from
**2712203 / 30380669** to **2720701 / 30383685**, using identical executable,
`--port 29285`, and scratch `--data-dir` arguments; the replacement ping
reported protocol **27**. The old daemon and its owned app-server stopped.
Alpha kept generation **1**; beta retained its logical thread but transitioned
from generation **1** to **2**, with app-server state **stopped**.

The failure was the assertion at `steps.py:146`: `checkpoint()` reused
`identities.json`, which is refreshed only by a `snapshot` action. Its `capture`
action refreshed runtime files but not this process list. Consequently both
step-2 and step-3 identity packets contained the old daemon PID. The controller's
independent `post_daemon_restart` event and `step3-identities.json` show the real
replacement. This is an evidence-instrument failure, not a failed daemon restart.
The inherited controller also treated a successful hosted RPC containing
`{attachmentGeneration:2, orphanProcessId:null, outcomeUnknown:false, stopped:true}`
as “host transcript readable without resume.” No `resume_member` was called.
That is a second harness recovery omission; beta delivery survival is unproved.
No controller repair or second paid replay was performed after the failure.

Both prompts requested **1 to 400, one number per line, then done**, and at least
30 seconds without tools. Alpha nevertheless completed in **17.492 s**; beta
was interrupted by the immediate ordinary shutdown. The requested >=30-second
working duration was not demonstrated. This does not erase the measured pending
sample, but it is an explicit deviation from the third-attempt duration requirement.

During cleanup alpha's same backlog ID received one `submitted` receipt at
**23:35:15.239 UTC**, after restart. Its new turn had no completion before teardown.
Beta retained its accepted obligation and pending diagnostic with no
`native_enqueued` receipt. Each baseline still had exactly one transport receipt
and one explicit-read receipt in the final journal. These are bounded incidental
observations, not proof of step 4 or both-boundary exactly-once delivery.

### Every observed turn and metered spend

Amounts use the inherited packet's API-equivalent token rates
($0.20 / $0.02 / $1.20 per million input / cached / output); they are estimates,
not invoice charges. Unknown turns count as inputs and are never priced at zero.

| Turn ID | Attribution / purpose | Metered generations | USD |
|---|---|---:|---:|
| `01a08dac-4c24-7992-bf8c-64105030d585` | beta onboarding | 1 | $0.00230820 |
| `01a08dac-586e-7e81-b32c-d1144f8dfc3e` | alpha onboarding | 2 | $0.00131504 |
| `01a08dac-5ae5-74a2-b20e-5a4a4d78065e` | notify-only startup; seat unproved | 0 | unknown |
| `01a08dac-6d08-7d73-8fe3-90eb825936e3` | alpha baseline | 2 | $0.00105016 |
| `01a08dac-8796-7cc3-a61c-eb6919043690` | beta baseline | 1 | $0.00029816 |
| `01a08dac-a44a-71a1-92d8-011764cbe849` | alpha 400-line turn | 1 | $0.00148456 |
| `01a08dac-a554-7661-8c90-6e8facc50c77` | beta 400-line turn, interrupted | 0 | unknown |
| `01a08dac-ec14-7d03-9d6e-4b15d006e420` | alpha backlog turn, interrupted by cleanup | 0 | unknown |

**Total: 8 inputs, 7 metered generations, $0.00645612 metered, 3 unknown-cost
turns.** The conservative estimate for the metered subset is **$0.09387120**.
The metered cap passed; complete billed spend remains unverified. Each of the
seven generation token counts and costs is retained in `cost-ledger.json`.
Workflow implementer/reviewer spend is outside this seat ledger and remains
orchestrator-owned; no unavailable review spend is represented as measured zero.

### Retention, cleanup and required gates

The [snapshot packet](l5-restarts/run3/runtime/snapshots.json) losslessly interns
**101 files into 76 unique payloads**. `pack.unpack(packet)` restores exact text,
including commands/exits, message IDs, baseline reads and journal pages, pending
samples, identity/epoch/config snapshots, passive `/proc/locks`, and bounded pane
captures (at most 60 lines each). The primary JSONL/accounting/outcome records
remain direct. The preserved controller is the exact instrument that ran, including
its two identified step-3 faults. Controller stdout and contract-test stdout are retained losslessly as JSON strings
in `run3/controller-console.json` and `run3/gates/gate-test-contracts-output.json`;
this preserves their trailing whitespace without introducing Git whitespace errors.
The raw assertion/exit remain in
[step3-console.txt](l5-restarts/run3/step3-console.txt); the separate final assessment
qualifies rather than overwrites the original step outcomes.

[Complete daemon JSONL](l5-restarts/run3/runtime/taurhaus.log.jsonl): **199 rows**, SHA-256
`4a1c079e1599c388635d45dc28f6082c7ffc0c03078c935050de8af27656f03b`.
No evidence-size abort or omitted daemon tail. Account secrets and operator paths
were sanitized; auth contents, installation IDs and account usage rows are absent.
The explicit authorized auth source was copied alone, mode 0600, into scratch
CODEX_HOME. Both runtime and unpaid gates hid operator homes in private PID/mount
namespaces. Private tmux, harness roots, project, temp/data roots and the probed
nondefault daemon port were used; inherited TMUX was absent. Only the daemon RPC
observed hosted transcripts. No observer connected to the app-server socket.

[Cleanup](l5-restarts/run3/runtime/cleanup.json) confirms **zero survivors**, port
closed, scratch auth explicitly removed before root removal, and root removed.
The parent reaped the controller with exit **1**. All owned daemon, host, TUI,
Codex and tmux descendants were gone; no foreign process was killed. There were
**zero transient busy refusals** in this attempt; the 65-second refusal-retry
branches were available but were not exercised. Positive polls used 120–180-second
windows; the failure was an immediate deterministic cached-identity assertion,
not an early timeout.

| Exact command, after teardown | Exit | Duration |
|---|---:|---:|
| `just check-quick` | **0** | 35.95 s |
| `just lint` | **0** | 38.32 s |
| `just test-contracts` | **0** | 18.66 s |
| Offline controller tests | **0** | 11 tests |
| `just test-rust-unit` | NOT REQUIRED | No `src-tauri/` diff |

[Gate logs and Cargo preflights](l5-restarts/run3/gates/).
Preflight exits were **1, 0, 1**; lint saw one foreign Cargo check and proceeded
within the three-process limit. No process-cap wait was needed. Each gate used
this checkout's target, one Cargo job, credential-free roots and blocked real
CLI wrappers. Gate cleanup reaped its children and removed its scratch root.
No full serialized `just check`, install, release or product edit was performed.

### Red-first verification and deviations

The pending observer test first failed with the old one-argument predicate
(`TypeError`); after implementing accepted-target + no transport receipt +
attributed working, it passed all positive/negative cases. A separate test for
bounded busy refusals first failed because `retry_busy` was absent, then passed.
[Pending red](l5-restarts/run3/red.txt), [busy red](l5-restarts/run3/busy-red.txt),
[11-test green](l5-restarts/run3/green.txt). The pending regression comment names
`92108455`, which inherited the incorrect stage requirement. No product regression
fix is included.

Deviations/limits: (1) the cached identity assertion and stopped-host recovery
omission ended step 3, so steps 4–6 were not run; (2) alpha completed the requested
400 lines in less than 30 seconds; (3) three inputs have unknown cost, with the
metered sum below the fresh cap; (4) the referenced `taurhaus-trial` checkout was
absent, so its committed attempt9 controller/actions/steps/support were read from
this checkout, alongside the available messaging run2 evidence; (5) no Opus model
was callable in this session, so the independent evidence lens remains outstanding
with the invoking orchestrator. No alternate review or product change is claimed.
The step-2 commit followed immediate restart initiation to preserve the ruling's
timing; it did not introduce a pre-restart pause. No plan ledger row was edited.

Reproduction order: `build.py`, offline unittest discovery, `execute.py`, then
(after confirmed teardown) `gates.py`, `audit.py`, and `pack.py`. The packing step
removes duplicate direct files; rerunning the read-only audit requires unpacking
them first. **This is a failed instrument run to inspect, not a certified passing
controller to replay unchanged.**


## Run3 continuation — six runtime checks completed

This fresh isolated run followed the user's instruction to continue. The preceding
run3 runtime had already been destroyed, so continuation could not reuse its team
or pending IDs. Its complete evidence remains unchanged in the parent directory.
The new runtime used the spec's per-attempt **20-input / $0.30 metered / 15-minute**
budget. No product files, descriptor, branch, installed runtime, or plan ledger
were changed. The only Mesh work was the authorized build at **310144d**.

Two controller faults were repaired test-first: `checkpoint()` now requests a
fresh process snapshot, and `stopped:true` from hosted_transcript triggers the
supported `coordination.resume_member` operation. Both regression comments name
`2600f95a`. Tests first produced one stale-PID assertion failure and one missing
helper import error; all **13 tests** then passed. Repair commit **1d01e588**.
[Red](l5-restarts/run3/continuation/red.txt),
[green](l5-restarts/run3/continuation/green.txt),
[exact controller](l5-restarts/run3/continuation/controller.py),
[step driver](l5-restarts/run3/continuation/steps.py),
[supervisor](l5-restarts/run3/continuation/execute.py).

### Ordered outcomes

| Step | Outcome / classification | Commit / evidence |
|---|---|---|
| 1. Initialize and complete/read baselines | **PASS — runtime** | **31c7a8e7**. Canonical builder policy, login-only Claude lead, alpha tmux and beta app_server; both baseline replies, receipts and reads. |
| 2. Pending while working | **Pending checks PASS; duration incomplete — harness** | **429ad5cd**. Both matching sessions attributed active, each marker accepted with no transport receipt; immediate first restart. The first paced commands selected missing `python`, leaving the >=30-second duration unproved. |
| 3. Taurhaus stop/restart and recovery | **PASS — runtime** | **5bf21b68**. SIGINT through the normal shutdown handler, new PID/start ticks, identical arguments and protocol 27; one successful supported beta resume. |
| 4. First backlog and no baseline replay | **PASS — runtime** | **d85a1cb0**. Same logical sessions/team, monotonic generations, both original pending IDs delivered once and explicitly read; no duplicate baseline receipt. |
| 5. Fresh backlog and Mesh restart-self | **Owner exclusion UNPROVED — harness; delivery checks PASS — runtime** | **dc8990f2**. Both seats pending; one restart-self, epoch 2→3, new team-owner PID; both original fresh IDs delivered once and read. The owner-process filter matched nothing: zero owners sampled. |
| 6. Reconcile/read both boundaries | **PASS — runtime** | **b073b016**. All six accepted baseline/backlog targets reconcile to one transport receipt each and explicit read; no lost obligation or duplicate transport exposure in this bounded run. |

The original numbered outcome files record the controller's predicate PASS.
The headline and final assessment qualify step 2's duration and step 5's missing
owner census. The final assessment keeps the original records separately from
its corrected step outcomes; neither is promoted to full spec compliance.
No blanket exactly-once guarantee or release approval is implied. [Final assessment](l5-restarts/run3/continuation/final-audit.json).

### Runtime identities, boundaries and receipts

Initialize → cleanup: **2026-09-10T23:50:50.526157+00:00 → 2026-09-10T23:52:46.424558+00:00**, **115.898 s**.

Candidate: Taurhaus product code **a7e6db7e**, protocol **27**; Mesh **310144d**,
shipped Codex **0.153.4** app-server descriptor **enabled, unchanged**. Both native
Codex siblings were copied and version checked. Both seats used **gpt-5.6-luna / low**.
The final assessment retains every executable SHA-256 and exact initialization.
All builds (`just ensure-tauri-resources`, `just build-daemon`, designated Mesh
`cargo build --bin mesh`) exited **0**, with checkout-local targets and one Cargo job.

Team incarnation:
`b0e31a5a0761796dc24cac95f47f8c41bc5074aa5bdc94f79f9eeb8764591483`.
Alpha session `01a08dbb-35e1-7e53-a54d-e74c5d569e50`, generation **1→1**;
beta thread `01a08dbb-3cc7-7c81-b822-125de6d52f00`, generation **1→3** after
ordinary host stop and supported resume. Both logical identities continued
through the Mesh restart. The replaced Taurhaus host stopped; the new daemon
recovered beta through one `resume_member`, without manual state clearing.

Taurhaus boundary: daemon **2801780 / 30478800** → **2811030 / 30482017**
(host PID / start ticks). Both markers were proven pending at the joint sample;
SIGINT was initiated **3.744 ms** later, before either transport receipt.
The same daemon executable, scratch data root and original port arguments were used.

Mesh boundary: team owner namespace **PID 2648 / ticks 30479457 / epoch 2** →
**PID 5802 / ticks 30484467 / epoch 3**. Restart-self was initiated **0.208 ms**
after the joint pending sample. No receipt wait, capture or commit intervened.
The only owner sequencing evidence is Mesh's own
[`step5-restart.txt`](l5-restarts/run3/continuation/runtime/snapshots.json)
(in the losslessly packed snapshots): `[mesh team-daemon] stopped (PID 2648)` /
`[mesh] restarted team-daemon (PID 5802)`. The passive probe required `run` in
argv, but the real owner used `mesh team-daemon start --team l5-restarts --name lead`.
All three [owner samples](l5-restarts/run3/continuation/runtime/owner-observations.jsonl)
contain `owners: []`; the maximum observed owner count is **0**.
**No-overlapping-owners is UNPROVED, classification: harness.** The epoch change
and CLI stop/start report do not replace the missing process census.

| Marker | Seat | Message ID | Delivery ID | Sole transport receipt |
|---|---|---|---|---|
| baseline | alpha | `06b13aa1-b7f4-4f95-927e-4aaf6ce516dc` | `9066e6ab-33c9-4329-abc1-38bd0747d059` | submitted, 2026-09-10T23:51:05.334451554+00:00 |
| baseline | beta | `bfb931e1-0acf-413d-8786-4363fb460918` | `e2d406f6-5b50-4307-a256-5def08f00fae` | native_enqueued, 2026-09-10T23:51:12.234474299+00:00 |
| taurhaus-backlog | alpha | `bef27735-df81-482f-8d69-0f702caa0049` | `b4c2d404-1b80-4000-bfb3-82d319fdfa93` | submitted, 2026-09-10T23:51:27.800297415+00:00 |
| taurhaus-backlog | beta | `669b0240-3022-4112-bc95-85ffc82e077e` | `db299baf-38ce-4c73-be10-269d76585715` | native_enqueued, 2026-09-10T23:51:36.678426873+00:00 |
| mesh-backlog | alpha | `d402d85e-8aee-4360-a876-d2f55783cb9e` | `f40833d2-2050-49a4-a450-fa036754f860` | submitted, 2026-09-10T23:52:30.394104970+00:00 |
| mesh-backlog | beta | `209ed304-d871-4533-87f1-7e17831f1b5b` | `c9536fe3-f7a8-412c-88c5-1fd5f3fb1341` | native_enqueued, 2026-09-10T23:52:35.253405234+00:00 |

Every marker has an explicit `consumed_by_read` receipt as well as its transport
receipt and observed native reply. The four backlog transport timestamps are
strictly after their respective restart initiation. Pending samples use actual
accepted-target rows and attributed working state; no `stage: pending` row is
required. Canonical journal and unread/mark-read calls preserved their filters
and followed returned cursors until `done`, including empty final inbox reads.

### Duration limitation

The continuation requested one ordinary Python task printing 1–400, one per line,
with 0.1-second intervals, then `done`. This changed the bounded workload from
free-form streaming to a paced tool command; it did not signal, freeze or alter a
product process, write fake activity, or change freshness rules. The first tasks
selected `python`, which was unavailable (exit **127**, retained tool output).
Alpha's first turn ended after **7.593 s**; beta's first turn was interrupted by
the sanctioned shutdown, then its recovered turn also reported missing Python.
Both pending states and the restart crossing were nevertheless directly observed.
The >=30-second duration at the first boundary remains **unproved**.

For the second boundary, both seats chose `python3`, completed the requested task,
and stayed in their turns for **44.955 s (alpha)** and **45.649 s (beta)**.
The executed controller remains available at **1d01e588**, and model tool outputs
are preserved. The review fixes below do not change the executed first workload;
no additional restart was run.

### Every input and spend

| Turn ID | Purpose | Metered generations | USD |
|---|---|---:|---:|
| `01a08dbb-4242-72c0-80f0-a9e2458df810` | beta onboarding | 1 | $0.00104724 |
| `01a08dbb-4ee8-71d2-8879-f23b71640de3` | alpha onboarding | 2 | $0.00255760 |
| `01a08dbb-50be-7712-92a3-7be76fd35c7d` | notify-only startup, attribution unproved | 0 | unknown |
| `01a08dbb-6b61-70e1-9cfc-a2be754aea88` | alpha baseline | 2 | $0.00108956 |
| `01a08dbb-860b-7192-ac52-75df81ea4ee3` | beta baseline | 1 | $0.00059888 |
| `01a08dbb-a381-7473-a868-71d190c54d8b` | alpha first paced task | 2 | $0.00323616 |
| `01a08dbb-a48f-7020-b929-ae5b831461d7` | beta first paced task, interrupted | 0 | unknown |
| `01a08dbb-bb32-7800-a439-148df3c7aadc` | beta supported recovery | 2 | $0.00129540 |
| `01a08dbb-c321-7880-bfb4-f7097dea9334` | alpha first backlog | 2 | $0.00108740 |
| `01a08dbb-e5c4-7662-9d90-d7363cef470f` | beta first backlog | 1 | $0.00048484 |
| `01a08dbc-03ae-77b2-a188-1451c98da694` | alpha second paced task | 4 | $0.00221988 |
| `01a08dbc-04b2-7163-b6e5-7c672ee51c52` | beta second paced task | 3 | $0.00281900 |
| `01a08dbc-b799-7850-8557-398c88861d69` | alpha second backlog | 2 | $0.00126312 |
| `01a08dbc-ca7f-7413-af48-9e94ea42fe3a` | beta second backlog | 1 | $0.00037340 |

**14 observed inputs; 23 metered generations; $0.01807248 metered; two
unknown-cost inputs.** Each generation's input/cache/output tokens and cost are
in [cost-ledger.json](l5-restarts/run3/continuation/runtime/cost-ledger.json) and
the final assessment. No Claude paid turn was observed. Rates are the inherited
packet's $0.20/$0.02/$1.20 per million input/cached/output, not invoice measurements.
The alternative conservative estimate is **$0.35763960** for the metered subset;
it is not the ruling's metered cap measure. The metered sum is below $0.30, while
complete billed spend remains unverified. Unknown turns were counted and never
priced as zero. No lifecycle operation was gated by metering.

Checkpoint observations (inputs / metered USD): step 1 **5 / 0.00529328**;
step 2's preparatory checkpoint **5 / 0.00529328**; step 3 **9 / 0.00852944**;
step 4 **10 / 0.01139708**; steps 5 and 6 **14 / 0.01807248**. These are
observation-time totals; the per-turn table allocates later-arriving usage to its
actual input. Previous run3 spend remains separate history. Implementer/reviewer
spend is orchestrator-owned and not measurable from this seat ledger.

### Gates, retention and cleanup

| Exact command, after teardown | Exit | Duration |
|---|---:|---:|
| `just check-quick` | **0** | 30.94 s |
| `just lint` | **0** | 37.29 s |
| `just test-contracts` | **0** | 21.57 s |
| Offline regression suite | **0** | 13 tests |
| `just test-rust-unit` | NOT REQUIRED | No `src-tauri/` diff |

The Cargo preflights each saw one foreign Cargo process and proceeded under the
three-process limit; none was stopped. Every gate used credential-free isolated
roots, blocked real CLI wrappers, this checkout's target and one Cargo job.
Gate processes were reaped and their scratch root removed. [Gate records](l5-restarts/run3/continuation/gates/).

Controller exit **0**; cleanup verifies **zero survivors**, private port closed,
auth copy explicitly deleted before scratch-root deletion, and scratch root gone.
Both native siblings, all owned hosts/TUIs/Codex processes, daemon and private tmux
server were removed/reaped; no foreign process was killed. The authorized source
was the sole auth file copied into scratch CODEX_HOME, mode 0600, never logged.
Operator homes were hidden from children; runtime and unpaid gates used separate
scratch roots. No observer connected to an app-server socket; hosted observation
used daemon RPC. **Zero transient busy refusals** occurred; the 65-second retry
branches remained available. No load, stress, forced lock, crash injection,
installation, release, product fix or Mesh commit occurred.

[Complete daemon JSONL](l5-restarts/run3/continuation/runtime/taurhaus.log.jsonl):
**440 rows**, SHA-256
`471a7cc833b2a72d33d9c03c60b369698268008f504478c9c99b28833f0c0f5f`.
[Snapshot packet](l5-restarts/run3/continuation/runtime/snapshots.json): **209 files,
133 unique payloads**, losslessly recoverable with `pack.unpack`. It includes
runtime/config/epoch identities, all commands and exits, pending samples,
journals/reads, passive locks and pane captures of at most 60 lines. Complete host
items and rollout/tool output remain available. Controller and contract-test
stdout are preserved as JSON strings to retain exact whitespace. No tokens,
auth contents, installation IDs or account usage rows are included.

Remaining runtime deviations are the first duration failure, the ordinary
paced-Python workload adaptation, two unknown-cost turns, and the inert owner
census. The earlier missing reference checkout limitation is unchanged; its
committed controller files were available in this checkout. Opus round 1 supplied
the independent evidence lens and identified the missing owner coverage.
The document remains **INCOMPLETE**; closing this fix round does not certify the lane.

Reproduction: `build.py`, unittest discovery, `execute.py`; after teardown,
`gates.py`, `audit.py`, `pack.py`, and `verify.py`. Audit precedes packing; unpack
first if re-running it against the retained packet. The exact pacing prompt's
first-boundary limitation must be accounted for before any future full certification.


### Opus round 1 correction (offline; no new runtime)

Used the review's option **(b)**: preserve the runtime, disclose the absent owner
census, and remove the vacuous green assertion from `steps.py` and `audit.py`.
The tested assessment now requires observed owners across both epochs, rejects
overlap, and reports an empty census as **UNPROVED — harness**. The historical
`run` filter is annotated and retained; running this driver unchanged would now
fail the owner-evidence check. A new instrumented runtime is needed to prove that
sub-claim. No runtime rows, original step outcome files, daemon JSONL, or packed
snapshots were rewritten. Source at **1d01e588** records the executed instrument;
current source contains these offline review corrections.

All five findings were verified. The duration finding needs no further change:
the existing limitation stays explicit. The budget helper now gates the actual
paid-submission path using `paid_inputs < 20` and `api_equivalent_usd < .30`;
its tests cover those exact boundaries. The conservative estimate remains
reported separately. The unused `host_poll` toggle was removed; the historical
observer polled through the lifecycle window and recorded zero transient refusals.
The credential source is now one visible literal with a shared, explicit
sanitizer/verifier exception for that **filename only**, alongside the two
allowed worktrees. Credential contents, sibling paths and secret fields remain
redacted; these offline tests never open any real harness home or invoke a CLI.

Red-first verification: **18 tests**, initially **4 failures and 3 errors**
(missing census/path assessment helpers, hidden credential literal, unused budget
helper, and both live cap boundaries accepted by the old helper); subsequently
**18 passed**. Regression comments identify **1d01e588**, **dc8990f2**, and
**d0eacf4d**. This fix round added **zero seat inputs and $0 seat spend**; the
14-input runtime ledger and its two unknown costs above are unchanged.


Fix-round gates, run after the recorded teardown with credential-free scratch
homes, blocked real CLI wrappers, private PID namespaces, and this checkout's
own `src-tauri/target`:

| Exact command | Exit | Duration |
|---|---:|---:|
| `just check-quick` | **0** | 18.39 s |
| `just lint` | **0** | 6.13 s |
| `just test-contracts` (initial) | **101 — harness log placement** | 5.13 s |
| `just test-contracts` (retry) | **0** | 5.13 s |

The initial contract run scanned its own `.check-logs` output and flagged the
retired-tool string in its test name and isolation metadata. Only these newly
created logs were moved beneath `.check-logs/target/`, which that repository scan
excludes; no product or test assertion changed. The exact contract gate then
passed. Local logs remain in `.check-logs/target/l5-opus-round1/` and
`.check-logs/target/l5-opus-round1-retry/`. Each Cargo preflight saw one existing
foreign Cargo process; no wait or foreign-process stop was needed. Both gate
controllers reaped their children and removed their scratch roots.

Offline reassessment and artifact verification exited **0**, retaining **209
packet files / 133 unique payloads**, the same 440-row daemon JSONL hash, zero
scratch runtime survivors, and clean Mesh/product state. The final assessment's
original gate records remain historical; this table records the fix-round reruns.
`just test-rust-unit` was not required: this diff touches no `src-tauri/` files.


## Run4 — fourth-attempt evidence: step 1 fails (harness)

**FAIL at step 1; steps 2–6 NOT RUN.** No product defect, restart-survival result,
or >=30-second working-window claim is established by this attempt. Runtime
was **139.100 seconds**, **2026-09-11 00:32:25.635–00:34:44.735 UTC**. The
controller obeyed the stop-on-failure rule and did not retry a model input.
Independent Opus review remains with the invoking orchestrator.

[Assessed result and every turn](l5-restarts/run4/final-audit.json),
[exact controller](l5-restarts/run4/controller.py),
[ordered step driver](l5-restarts/run4/steps.py),
[commands, RPCs, outputs and exits](l5-restarts/run4/runtime/events.jsonl),
[losslessly packed snapshots](l5-restarts/run4/runtime/snapshots.json).

### Ordered outcomes and classification

| Step | Outcome / classification | Evidence and spend |
|---|---|---|
| 1. Initialize; complete/read baselines on both transports | **FAIL — harness timing** | Canonical initialization succeeded. Alpha's startup inbox read consumed the baseline, and its model replied, but no `submitted`/`native_enqueued` receipt exists for that baseline ID. The receipt predicate polled **120.7 s** before failing. Beta's baseline was never sent. All run4 spend belongs here: **3 inputs / $0.00302944 metered + one unknown-cost input**. |
| 2. Working turns; retain pending markers | **NOT RUN — blocked by step 1** | No paced turns, backlog IDs or measured working windows. +0 inputs / $0. |
| 3. Normal Taurhaus shutdown/restart | **NOT RUN — blocked by step 1** | No restart, new daemon identity or resume-member operation. +0 inputs / $0. |
| 4. Backlog delivery and identity/replay checks | **NOT RUN — blocked by step 1** | No post-restart backlog or replay assessment. +0 inputs / $0. |
| 5. Fresh backlog and Mesh restart-self | **NOT RUN — blocked by step 1** | No restart-self or owner epoch transition. +0 inputs / $0. |
| 6. Reconcile both boundaries and export | **NOT RUN — blocked by step 1** | Cross-boundary accounting unproved. Failure export/teardown passed separately. +0 inputs / $0. |

### Decisive baseline timeline

Alpha logical session **`01a08de1-488a-7650-8eef-c2d1e25a8029`**, tmux `%2`,
generation **1**; beta hosted thread **`01a08de1-4f57-7050-add8-d00258eb416a`**,
pane `%3`, generation **1**. Team incarnation
`a923cf7db5623fdd13c48f6859ad05ae5b8c3575c998f0b1e23b5b9a13f9ce1c`;
delivery owner epoch **2**, namespace PID **3088**, start ticks **30729040**.
Kernel owner identities are retained separately in the census.

1. **00:32:35.926 UTC:** journal sequence 6 records a tmux `submitted` receipt
   for alpha's **startup** message `d5c4f215-23c4-47b7-a8e5-f01e73912fd8`.
2. **00:32:37.490:** sequence 7 accepts baseline
   `2c7d03db-1d91-458e-ad93-92bd1e20a3e2`, delivery ID
   `18315c8b-7840-4e7b-bb90-712f480fb388`, marker `L5_baseline_alpha_713599`.
3. **00:32:38.915:** alpha's startup turn executes
   `mesh read --unread --mark-read --team l5-restarts --name alpha`.
4. **00:32:38.997:** sequence 9 records `consumed_by_read` for the baseline.
5. **00:32:42.232:** alpha replies `L5_baseline_alpha_713599 done`.
6. **00:34:44.445:** the observer fails its separate transport-receipt predicate:
   `delivery receipt/read missing baseline alpha; polled 120.7s`.

The retained journal has **nine rows**. The baseline has one accepted-target
record, one explicit read, and **zero transport submission receipts**. Its
uptake through the earlier startup notice is real, but cannot establish the
required distinct baseline transport delivery. The controller admitted a fresh
idle observation while startup notification work was still settling; the
startup model read then covered the newly accepted baseline. This is a harness
baseline-isolation race, not evidence of lost mail. The raw failure remains
`unclassified pending evidence review`; `final-audit.json` preserves it alongside
the assessed **harness** classification. No product patch or predicate weakening
was applied after the failure.

### Red-first run4 corrections and their coverage limits

The initial discovery run failed **3 of 21 tests**: the retained run3
`step1-identities.json` matched **zero** owners; the paced prompt omitted an
explicit `python3`; the owner observer deduplicated unchanged samples.
The retained owner is kernel PID **2805580**, start ticks **30479457**, running
`mesh team-daemon start --team l5-restarts --name lead`.
A separate red check failed because the full-window census assessor was absent.
After correction, the final discovery command passes **22 tests**, exit **0**:

```sh
python3 -m unittest discover -s docs/design/evidence/e2e/l5-restarts/run4 -p '*test.py'
```

[Initial red](l5-restarts/run4/red.txt), [red exit metadata](l5-restarts/run4/red.json),
[window red](l5-restarts/run4/window-red.txt), [regression tests](l5-restarts/run4/support_test.py),
[final green](l5-restarts/run4/green.txt), [green metadata](l5-restarts/run4/green.json).
Regression comments name the introducing commits. The census now matches
`team-daemon` **and** `start`, retains every sample, and checks <=1-second gaps,
window coverage, at most one owner per sample, and old PID departure before
new-owner delivery. Both bounded prompts explicitly request **python3**, 400
numbers and 0.1-second pacing (40 seconds). None was submitted in this failed run.

The actual startup-only census retained **276 samples**, maximum gap
**0.548539 seconds**, and maximum **one owner** per sample. This proves that the
corrected filter works on live processes; **it does not prove exclusion during
restart-self**, because that boundary was never reached. Measured working
windows for **both seats at both boundaries: NOT RUN**.

### Every input and spend

Both Codex seats used **gpt-5.6-luna / low**. The login-only Claude lead took
**zero paid turns**. These are fresh run4 totals; prior attempts are excluded.
The generation rates are the inherited packet estimates, not invoice amounts.

| Input / generation | Input / cached / output tokens | Metered USD |
|---|---|---:|
| beta turn `01a08de1-5857-7f61-a598-3c3ed1b20a54`, generation 1 | 11,128 / 6,912 / 62 | $0.00105584 |
| alpha turn `01a08de1-6c45-76a0-8a80-a05e8812bcf1`, generation 1 | 9,244 / 3,840 / 95 | $0.00127160 |
| same alpha input, generation 2 | 10,800 / 8,960 / 129 | $0.00070200 |
| notify-only turn `01a08de1-6e01-7e52-90cb-0e7207c3615d` | unavailable; counted as one input | **unknown** |
| **Total: 3 inputs / 3 metered generations** | | **$0.00302944 metered + unknown** |

[Cost ledger](l5-restarts/run4/runtime/cost-ledger.json),
[usage records](l5-restarts/run4/runtime/usage-events.json).
The conservative metered estimate is **$0.03774960**. Input count, metered spend
and runtime satisfy **20 / $0.30 / 900 seconds**; complete billed spend remains
unknown. Unknown cost did not gate a lifecycle operation or cause this stop.
No transient `host member busy` or `lock busy` refusals occurred.

### Candidate, teardown, gates and packet integrity

Taurhaus source remains **a7e6db7e / protocol 27** in this checkout. The
checkout-local `just ensure-tauri-resources` and `just build-daemon`, and the
Mesh **310144d** build in its designated worktree, all exited **0**. The shipped
**0.153.4** descriptor stayed enabled and unchanged. Both native Codex siblings
were copied; scratch startup verified **codex-cli 0.153.4**. Binary SHA-256s,
original daemon arguments, canonical builder policy and explicit scratch roots
are retained in [candidate evidence](l5-restarts/run4/candidate.json) and the
runtime packet. No product, descriptor, install, release or plan-ledger change.

The credential source is represented only as **`<authorized-source>`**. It was
passed explicitly through `L5_CREDENTIAL_SOURCE`; only its `auth.json` was copied,
mode **0600**, into the empty scratch Codex home. No fallback or operator-home
access by trial children. The private PID namespace, private tmux, probed daemon
port and all runtime roots were isolated; no observer opened an app-server
socket. Unpaid gates used credential-free roots and blocked external harness CLIs.

[Cleanup](l5-restarts/run4/runtime/cleanup.json) confirms **no survivors**, closed
port, auth copy removed before scratch deletion, and root removed. Controller
exit **1**, step driver exit **1**. The inherited outer `execute.py` returns
**0** after a handled child failure; packed `execute-exit.json` records that
wrapper behavior explicitly. The child exit and failed outcome govern the verdict.

All exact gates ran **after teardown**, from this checkout root:

| Gate | Exit | Evidence |
|---|---:|---|
| `just check-quick` | **0** | Rust format/test compilation; frontend checks and **2,519 tests / 150 files** passed. |
| `just lint` | **0** | Clippy, frontend dependency checks, workflow syntax and gate guards passed. |
| `just test-contracts` | **0** | Renderer, harness and module-boundary contracts passed. |
| `just test-rust-unit` | **NOT RUN** | No `src-tauri/` diff; the Rust-diff rule does not apply. |

[Gate exits and preflights](l5-restarts/run4/gates/),
[verification](l5-restarts/run4/verification.json). Each Cargo gate preflight used
`pgrep -af '(^|/)cargo( |$)'` and found no Cargo process (exit **1**); build
preflights are retained separately. Every build/gate used one job and the
checkout's own target directory. Gate cleanup passed; no started process remains.

The complete sanitized daemon JSONL is retained directly: **326 rows**, SHA-256
**`1bc979d4c40e72bf40311700680ad600120badf289c2aea51587ce950ea96921`**.
The lossless snapshot packet contains **42 filenames / 35 unique payloads**;
pane captures are <=60 lines. Verification regenerated the run4 record, checked
hashes, sanitation, scratch absence, product-diff absence and a clean Mesh tree.
The run4 heading and red/green sidecars describe this attempt, without carrying
forward prior-run counts or an authorized credential-source literal.

Deviations and remaining limits: step 1 blocked steps 2–6 as required by the
failure rule; the referenced integration attempt9 checkout was absent (`git show`
exit **128**), so the retained run3 controller and read-only messaging run2 sources
were reused; the inherited outer wrapper reports zero despite the recorded child
failure; one input is cost-unknown; independent Opus review is not claimed here.
There was no paid retry, budget/authorization question, or product repair.

## Run5 — fifth-attempt evidence: step 1 fails (harness)

Candidate: Taurhaus **a7e6db7e**, protocol **27**, this branch/checkout;
Mesh **310144d**, its enabled 0.153.4 descriptor unchanged. Both native Codex
siblings were copied into the scratch bin, version checked and hashed. Alpha
used tmux and beta app_server; both used **gpt-5.6-luna / low**. The Claude lead
remained login-only with no paid turn. Production `coordination.initialize_team`
used the builder's canonical policy. [Pins and invocation](l5-restarts/run5/candidate.json),
[exact controller](l5-restarts/run5/controller.py),
[ordered assertions](l5-restarts/run5/steps.py),
[final audit](l5-restarts/run5/final-audit.json).

### Outcomes and classification

| Step | Outcome / classification | Observed result and spend |
|---|---|---|
| 1. Initialize; complete/read baselines | **FAIL — harness** | Initialization succeeded. Alpha's onboarding had submission, explicit read and fresh idle. Beta's direct hosted startup had a runtime submission receipt and completed turn, but lacked the required read/tool-result witness. The 180.8-second guard timed out before any baseline send. **3 inputs / $0.00238148 metered + one unknown-cost input.** |
| 2. Bounded working turns and pending markers | **NOT RUN — blocked by step 1** | No bounded input or pending marker; neither >=30-second window measured. **+0 inputs / $0.** |
| 3. Normal Taurhaus daemon restart | **NOT RUN — blocked by step 1** | No shutdown/restart, replacement PID/start ticks or recovery operation. **+0 inputs / $0.** |
| 4. Backlog delivery, identities and no replay | **NOT RUN — blocked by step 1** | No baseline or backlog IDs exist; restart delivery and replay claims unproved. **+0 inputs / $0.** |
| 5. Fresh pending mail and Mesh restart-self | **NOT RUN — blocked by step 1** | No owner restart. Startup census is retained, but cannot prove exclusion across a restart. **+0 inputs / $0.** |
| 6. Read/reconcile both boundaries | **NOT RUN — blocked by step 1** | No cross-boundary accounting. Required failure teardown passed separately. **+0 inputs / $0.** |

### Exact blocking evidence

Alpha session `01a08df4-f77c-7ec3-8eb2-ca0ca613290c` received onboarding message
`fc317780-9d4d-4941-b275-fef20e10c9ac`. Its journal carries `submitted` and
`consumed_by_read` with `reader_name: alpha`; its rollout contains the card in a
`custom_tool_call_output`. The guard observed attributed fresh idle and opened.

Beta thread `01a08df4-fc12-78b1-b904-513893ecd1da` completed startup turn
`01a08df5-0255-7bc1-9a97-b827140edb4e`. Runtime recovery delivery
`2f4008bb9d88d28f7240d8ed307da967dd88367a63f14eb3e4118b485caf58bb`
has `stage: submitted`, `path: app_server`; this is a real runtime receipt,
not a Mesh journal receipt. Beta has **zero journal onboarding messages**,
**zero tool-result rows**, and its card is a `message` with `role: user`.
The fifth-attempt ruling permits a card in a **tool-result** row as read proof;
that evidence did not appear. The journal-based onboarding guard also assumes
an accepted onboarding message, which this direct hosted path does not create.
No notification, external read, extra model input or lifecycle operation was
used to manufacture the missing witness.

The exact failure is `onboarding lacks submitted and seat read receipts plus
fresh idle beta; polled 180.8s`. That combined assertion does **not** mean beta
lacked a runtime submission or idle state. Step process, execution wrapper and
controller each exited **1**. The packet preserves raw outcomes and separately
classifies the failure as harness in the final audit.
[Step output](l5-restarts/run5/step1-console.txt),
[controller output](l5-restarts/run5/controller-console.txt),
[daemon JSONL](l5-restarts/run5/runtime/taurhaus.log.jsonl),
[receipt/rollout assessment in final audit](l5-restarts/run5/final-audit.json).

### Every input and spend

| Input / turn ID | Metered generations | Metered USD |
|---|---|---:|
| Beta onboarding `01a08df5-0255-7bc1-9a97-b827140edb4e` | 10,997 input / 6,912 cached / 76 output | $0.00104644 |
| Alpha onboarding `01a08df5-12a1-7731-aab6-5132f350ad7b` | 9,277 / 6,912 / 95; then 10,789 / 8,960 / 54 | $0.00072524 + $0.00060980 = $0.00133504 |
| Notify-only `01a08df5-14c2-71d2-9acf-d132ad495df3` | No token-usage generation; counted as one input | **Unknown** |
| **Total: 3 inputs, 3 metered generations** | Fresh caps: <=20 inputs, <=$0.30 metered, <=900 seconds | **$0.00238148 + unknown** |

The conservative estimate for metered generations is **$0.03754560**. These are
token-based estimates, not an invoice; complete billed spend remains unknown.
Unknown cost did not block a lifecycle operation. No baseline, bounded turn,
backlog turn, resume or retry was submitted. Implementer/reviewer metering is
separate and remains with the invoking orchestrator.
[Full ledger](l5-restarts/run5/runtime/cost-ledger.json),
[token and turn events](l5-restarts/run5/runtime/usage-events.json).

### Red, green, gates and retention

Five added offline regressions cover consumed-before-submission delivery,
foreign-reader rejection, onboarding submission/read/idle prerequisites,
pending/in-flight send exclusion and consumed-message pending exclusion.
The inherited logic failed **2 assertions and 3 missing-interface checks**
(exit **1**); repaired run5 logic passed **27 tests** (exit **0**), including
the retained run3 owner identity test and both explicit `python3` pacing probes.
The introducing harness commit is `f95ec193`; run4's `a3d12df4` packet exposed
the receipt race. [Red](l5-restarts/run5/red.txt),
[green](l5-restarts/run5/green.txt), [tests](l5-restarts/run5/support_test.py).
The harness correction was committed before launch as `ca087893`.
No numbered runtime step became green, so no runtime PASS checkpoint was committed.

| Command (checkout root) | Exit | Timing |
|---|---:|---|
| `just ensure-tauri-resources` | 0 | Before trial |
| `just build-daemon` | 0 | Before trial; checkout-local target |
| `cargo build --bin mesh` (designated Mesh worktree) | 0 | Before trial; no descriptor edit or Mesh commit |
| `just check-quick` | **0** | After teardown |
| `just lint` | **0** | After teardown |
| `just test-contracts` | **0** | After teardown |
| `python3 docs/design/evidence/e2e/l5-restarts/run5/verify.py` | **0** | After gates |

Cargo preflight probes are retained in the build/gate sidecars; no probe found
three existing Cargo processes, so no capacity wait was required. Each build/gate
used one Cargo build job. No `src-tauri/` diff exists, so the conditional
`just test-rust-unit` requirement did not apply.
[Build results](l5-restarts/run5/build/daemon-build.json),
[gate results](l5-restarts/run5/gates/gate-check-quick.json),
[verification](l5-restarts/run5/verification.json).

Teardown recorded **no surviving owned processes**, a closed private daemon port,
auth removal before root deletion, and removal of the scratch root. The private
PID namespace contained the daemon, hosted child, TUIs, Codex and tmux server;
no foreign PID was signaled. Gate cleanup also passed. Runtime lasted
**195.427767 seconds**. The complete sanitized daemon log has **420 JSONL rows**,
SHA-256 `92f8a3cf6e4593d1f70048d7df4f59bf93910740378808c2df307f080599e3a6`.
Lossless [snapshots](l5-restarts/run5/runtime/snapshots.json) retain **37 files /
32 unique payloads**, including `onboarding-assessment.json`, final journal,
config, seat identities, private environment, generated configs and pane captures
of <=60 lines. No auth contents, tokens, account usage rows or installation IDs
are retained. [Cleanup](l5-restarts/run5/runtime/cleanup.json).

Deviations/limits: the referenced attempt9 checkout was absent (`git show` exit
128), so run4/run3's retained adaptation and the messaging reference were reused.
Beta's direct startup does not provide the prescribed read witness, leaving the
lane stopped at step 1. The fresh budget was not a blocker. No product, descriptor,
plan ledger, installation or release change was made. Independent Opus evidence
review remains with the invoking orchestrator; this is an implementation/evidence
handoff, not full workflow acceptance.


## Run6 — sixth-attempt evidence: step 1 fails (harness)

The run used Taurhaus product base `a7e6db7e`, protocol **27**, in this checkout
on `feat/e2e-l5-restarts`, with Mesh **310144d** built in the designated Mesh
worktree. The RC descriptor remained enabled and unchanged. Both native Codex
**0.153.4** siblings were copied into the scratch bin; seats used
**gpt-5.6-luna / low**. Production `coordination.initialize_team` received the
builder's canonical messaging policy, alpha `tmux`, beta `app_server`, and a
login-only Claude lead. Binary digests, identities, runtime roots, incarnation,
owner epoch, commands and RPC results are in the
[final audit](l5-restarts/run6/final-audit.json) and
[deduplicated snapshots](l5-restarts/run6/runtime/snapshots.json).

| Ordered step | Outcome | Classification and evidence |
|---|---|---|
| 1. Initialize and complete/read baselines | **FAIL** | **Harness.** Initialization and alpha's onboarding guard passed. Beta's required startup evidence existed, but the observer required an additional user-card `item/completed` event. It timed out after 180.7 s. No baseline send occurred. |
| 2. Working turns and pending markers | **NOT RUN** | Blocked by step 1; no bounded turns, pending markers or >=30 s windows measured. |
| 3. Normal Taurhaus daemon restart | **NOT RUN** | Blocked by step 1; no stop/start or resume operation. |
| 4. Deliver backlog and exclude baseline replay | **NOT RUN** | Blocked by step 1; no backlog or baseline exists to reconcile. |
| 5. Fresh backlog and Mesh restart-self | **NOT RUN** | Blocked by step 1; no owner boundary crossed. |
| 6. Read/mark, reconcile, export, teardown | **NOT RUN** | Cross-boundary accounting unrun; failure-path export and teardown **passed**. |

The exact executed controller and predicates are retained in
[controller.py](l5-restarts/run6/controller.py),
[steps.py](l5-restarts/run6/steps.py), and
[support.py](l5-restarts/run6/support.py). The failure witness is
`onboarding-assessment.json` inside `runtime/snapshots.json`: the
`onboarding.delivery.observed` row has `path: app_server`, `stage: submitted`
and beta's exact recipient key. Host `item/started` contains that same recovery
card, and `turn/completed` identifies its completed turn. The runtime snapshot
reports attributed `idle`, source `host`. No journal read receipt was required
for beta's startup; the new extra `item/completed` requirement was the error.
The executed predicate is preserved without an after-the-fact repair or paid replay.

Four transport-specific synthetic checks first errored against the run5 helper
(`transport` was unsupported; **31 tests, 4 errors, exit 1**). After the helper
change, **31 tests passed, exit 0**. The tests covered hosted startup without a
journal projection, missing startup/completion/idle evidence, hosted native
receipt plus card delivery, and later-send guards. They used a completed user-card
event and missed the real `item/started` case. This limitation is explicit;
the green synthetic checks do not supersede the failed runtime observation.
See [red](l5-restarts/run6/red.txt) and [green](l5-restarts/run6/green.txt).

### Run6 spend and limits

Runtime: **2026-09-11 01:16:58.343527 UTC → 01:20:14.104342 UTC**,
**195.76 seconds**, within the fresh 15-minute cap. Three inputs were counted
against 20; three metered generations total **$0.00357304**, below $0.30 metered.
No controller marker, bounded task, recovery input, or paid retry was submitted.

| Turn / generation | Seat | Metered USD |
|---|---|---:|
| `01a08e0a-1dfb-7e30-80e9-62335f782801` / 1 | beta startup | 0.00101464 |
| `01a08e0a-2fa0-7a31-a173-f660a54ee800` / 1 | alpha startup | 0.00196940 |
| `01a08e0a-2fa0-7a31-a173-f660a54ee800` / 2 | alpha startup | 0.00058900 |
| `01a08e0a-316e-7453-9ab9-b27c3c7c2766` | notify-only input | **Unknown** |

The unknown-cost input counts as one input, not free usage. The ledger's
conservative estimate for the metered generations is **$0.03729**; it does not
bound the unknown input. Claude remained login-only, with no paid turn submitted.
Implementer/reviewer spend is unavailable here and belongs to orchestrator
metering. Full billing is unverified. Token counts and generation identities
remain in [cost-ledger.json](l5-restarts/run6/runtime/cost-ledger.json).

### Run6 teardown, gates, and deviations

The controller and step worker exited **1**. Teardown found **zero survivors**,
closed the private port, removed the auth copy before deleting the root, and
verified root removal. The entire sanitized daemon JSONL is retained directly:
**424 rows**, SHA-256
`76db90f8bdc2d6602b547d3943aef34935caedefc8e3372e501b60d683db72c1`.
There were **zero transient busy refusals**. The corrected owner filter recorded
387 samples, at most one owner, maximum gap **0.565 s**; with no restart-self,
these samples do **not** prove exclusion across an owner transition.

| Exact command, from checkout root | Exit | Result |
|---|---:|---|
| `just build-daemon` | 0 | Checkout-local release daemon built; no install. |
| `just check-quick` | 0 | Rust test compilation, frontend typecheck, 150 files / 2,519 frontend tests passed. |
| `just lint` | 0 | Clippy, frontend and workflow/recipe checks passed. |
| `just test-contracts` | 0 | Renderer, harness and module-boundary contracts passed. |
| `just test-rust-unit` | Not run | No `src-tauri/` diff. |

Gates ran **after teardown**, in a credential-free home and private PID namespace
with real harness CLIs blocked. Cargo admission used the exact required census;
no admission sample had three running Cargo jobs, so no waiting was required.
Own Cargo builds used one job and this checkout's target directory. Gate cleanup
passed. [verification.json](l5-restarts/run6/verification.json) checks packet hashes,
sanitation, pane captures <=60 lines, no scratch survivors, no product diff,
and a clean Mesh worktree. Snapshot interning retains 37 files as 32 payloads.

Deviations: the integration attempt9 checkout was absent (`git show` exit 128),
so the retained run5/run3 adaptation was reused and messaging run2 was inspected
read-only. The extra startup-card completion predicate caused this attempt's
harness failure; steps 2–6 stopped as the spec requires. The independent Opus
lens is unavailable in this agent's tool surface and remains with the invoking
orchestrator. No product fix, Mesh change, descriptor edit, plan-ledger edit,
installation or release was made. There is no green numbered runtime step to
commit; the red-first controller preparation and final failure packet are committed.


## Run6 continuation — ordered restart evidence

The user authorized continuation from the committed tree. The original run6
packet remains unchanged. This fresh runtime used the same Taurhaus product base
`a7e6db7e`, protocol 27, and Mesh `310144d` with its enabled descriptor unchanged.
Both native Codex 0.153.4 siblings, canonical builder policy, alpha tmux, beta
app-server, private roots/PID namespace/tmux/port, and login-only lead were retained.
No product change, Mesh commit, installation, release, or plan-ledger edit occurred.

The observer correction accepts a host user-card `item/started` as well as
`item/completed`, while still requiring completion of the matching startup turn
and host-sourced idle. Two regression checks failed first against `d8146e39`,
then all **33 offline checks passed**. The corrected helper also admits the real
retained run6 startup witness. See [red/green evidence](l5-restarts/run6/continuation/green.json)
and [retained startup check](l5-restarts/run6/continuation/retained-startup-check.json).

| Step | Outcome | Classification and retained evidence |
|---|---|---|
| 1. Initialize, baseline delivery/read, identities | **PASS** | Runtime; committed `71daf570`. Alpha initially exited during native Codex SQLite migration 38 (`external_agent_config_imports already exists`). One supported `resume_member` recovered it before the startup guard expired. Both transport guards and both baselines then passed. |
| 2. Bounded work and pending mail | **PASS, qualified** | Runtime pending predicates passed on both seats; committed `510f6d7a`. Alpha's paced turn measured 44.747 s. Beta's original turn was interrupted before executing Python; its continuous >=30 s window at this boundary is **UNPROVED, harness timing**. The preserved controller PASS applies to pending state, not that duration subclaim. |
| 3. Normal Taurhaus restart | **PASS** | Runtime; committed `414a4c90`. SIGINT shutdown, identical startup arguments, PID/start ticks `3828891/31082992` → `3862764/31100113`, protocol 27. Beta host stopped and recovered through `resume_member`; alpha stayed attached. |
| 4. First backlog delivered, identities/no replay | **PASS** | Runtime; committed `e32a48cf`. Both original pending IDs have one transport receipt and one explicit read. Logical identities and incarnation stayed stable; alpha generation stayed 2, beta advanced 1→3. Baselines did not replay. |
| 5. Mesh owner restart and fresh backlog | **FAIL** | **Harness/model pagination.** Both messages were pending at restart-self. Alpha notification was submitted once, but its native `mesh read` returned a continuation cursor; the model stopped with “unavailable done” without fetching the marker page. Reply assertion expired after 180.1 s. Beta has one native-enqueued receipt, one host card item and its marker reply. Owner exclusion subclaim **PASS** from passive evidence. |
| 6. Final read/mark and reconciliation | **NOT RUN** | Blocked by the required stop on step-5 failure. Failure-path export and owned teardown passed. |

The one-off startup recovery is retained in
[startup_recovery.py](l5-restarts/run6/continuation/startup_recovery.py), with
its exact RPC requests/results in `startup-alpha-recovery.json` inside the packed
snapshots. It did not repair or modify the database. The failed startup had no
model turn. No corrective prompt was sent after alpha's step-5 pagination failure.
The controller waited for the marker reply before invoking its own explicit-read
paging, so that bookkeeping did not repair the seat's incomplete read.

### Continuation restart and exposure accounting

The Taurhaus pending samples were at **1789090462.274217 / .274282**;
SIGINT was initiated at **1789090462.279091**. The Mesh samples were at
**1789090530.679019 / .679070**; restart-self began at **1789090530.680064**.
Both samples satisfy accepted-for-recipient, no exposure receipt, and current
attributed working activity. Neither boundary waited for a delivery receipt.

| Marker | Seat | Message ID | Final witness |
|---|---|---|---|
| baseline | alpha | `a6047c43-39e5-4ff9-b6f7-588e9345e402` | One submitted receipt, one seat read |
| baseline | beta | `7337971c-58be-40eb-8e7b-b0ad6e60bffe` | One native-enqueued receipt, one host card, one explicit read |
| Taurhaus backlog | alpha | `d2c30179-5f2c-409d-aecd-e06c923fec53` | One submitted receipt, one seat read |
| Taurhaus backlog | beta | `920fa64b-417b-44f0-a900-a15305669a70` | One native-enqueued receipt, one host card, one explicit read |
| Mesh backlog | alpha | `6e45caf8-5e75-477b-95da-b43abfa6605c` | One submitted notification; **no marker read/exposure/reply** |
| Mesh backlog | beta | `8bbb0d39-83b4-405f-813a-b7db9000cf85` | One native-enqueued receipt, one host card and marker reply; final explicit read unrun |

The pending alpha message remains traceable in the journal; complete consumption
and model action are not established. No blanket exactly-once or no-lost-obligation
claim is made. `obligation-accounting.json` contains all accepted targets and receipts;
`step5-alpha-pagination.json` and `step5-alpha-delivery-probe.txt` preserve the
native read/cursor result and bounded pane capture.

The Mesh owner changed epoch **2→3**, namespace PID **1247→6410**, external PID
**3831100→3877835**. Across restart-self through both transport receipts,
**88 passive samples**, maximum gap **0.538419 s**, contained at most one owner.
Old-owner departure was observed at **1789090531.816083**, before the first delivery
at **1789090571.556566**. This post-teardown assessment uses the complete retained
census; the live step-5 worker failed on alpha's reply before reaching its own
census assertion. Second-boundary working turns measured **43.945 s alpha /
46.155 s beta**, both explicitly using `python3`. At the first boundary, beta's
original turn has no completion; the resumed turn lasted **47.353 s**, which does
not prove a continuous >=30-second window crossing that restart.

### Continuation spend

Runtime: **2026-09-11 01:31:32.446305 UTC → 01:38:32.525363 UTC**, **420.08 s**.
There were **14 model-input IDs**, plus one failed native startup without a model
turn (conservative cap count **15**, below 20). The metered total is
**$0.01652596**, below the fresh $0.30 metered cap. Two inputs are cost-unknown:
notify-only `01a08e19-80c7-7370-ba52-2d97f5d5f202`, and interrupted beta turn
`01a08e19-eef1-7720-a00c-4c32a84b21bd`. They count as inputs, not free usage.
The ledger's conservative estimate for measured generations is **$0.39617160**;
that estimate is not billed spend or a bound on the unknown inputs. Claude had
no paid turn. Implementer/reviewer spend is unavailable here and remains with
the orchestrator. Every metered generation follows; full token counts and
thread IDs are in [cost-ledger.json](l5-restarts/run6/continuation/runtime/cost-ledger.json).

| Turn ID | Generation within turn | Metered USD |
|---|---:|---:|
| `01a08e17-74ca-7783-96a6-eee669e34071` | 1 | 0.00109384 |
| `01a08e19-c8d0-7ea2-9d93-5310576ff555` | 1 | 0.00060768 |
| `01a08e1a-0c24-7223-b17e-49bf7af1d8fd` | 1 | 0.00081836 |
| `01a08e1a-0c24-7223-b17e-49bf7af1d8fd` | 2 | 0.00059544 |
| `01a08e1a-0c24-7223-b17e-49bf7af1d8fd` | 3 | 0.00042652 |
| `01a08e1a-cf24-7312-80ed-5449c131b8e3` | 1 | 0.00046332 |
| `01a08e1a-f861-71f3-b74d-62f857ab3311` | 1 | 0.00067572 |
| `01a08e1a-f861-71f3-b74d-62f857ab3311` | 2 | 0.00066392 |
| `01a08e1a-f861-71f3-b74d-62f857ab3311` | 3 | 0.00050600 |
| `01a08e1b-ae12-7e13-a431-2a35cd995672` | 1 | 0.00054300 |
| `01a08e19-7f0c-7810-bb1a-7100cdc936e2` | 1 | 0.00072744 |
| `01a08e19-7f0c-7810-bb1a-7100cdc936e2` | 2 | 0.00182892 |
| `01a08e19-a585-7c03-a3d9-d1c8f29519f3` | 1 | 0.00043056 |
| `01a08e19-a585-7c03-a3d9-d1c8f29519f3` | 2 | 0.00057316 |
| `01a08e19-e9d2-7410-9b22-3d6a265ec1a1` | 1 | 0.00055504 |
| `01a08e19-e9d2-7410-9b22-3d6a265ec1a1` | 2 | 0.00114160 |
| `01a08e19-e9d2-7410-9b22-3d6a265ec1a1` | 3 | 0.00040272 |
| `01a08e1a-9e2b-72c1-bcbe-73a2f471959d` | 1 | 0.00051192 |
| `01a08e1a-9e2b-72c1-bcbe-73a2f471959d` | 2 | 0.00079724 |
| `01a08e1a-f350-7842-9dff-52148e9d81fc` | 1 | 0.00056920 |
| `01a08e1a-f350-7842-9dff-52148e9d81fc` | 2 | 0.00082332 |
| `01a08e1a-f350-7842-9dff-52148e9d81fc` | 3 | 0.00046688 |
| `01a08e1b-a4ef-7fd2-87ce-c23744d9fa2f` | 1 | 0.00057108 |
| `01a08e1b-a4ef-7fd2-87ce-c23744d9fa2f` | 2 | 0.00073308 |

### Continuation gates, retention, and limits

All gates ran after verified runtime teardown in credential-free isolated roots.

| Exact command | Exit |
|---|---:|
| `just build-daemon` | 0 |
| `just check-quick` | 0 |
| `just lint` | 0 |
| `just test-contracts` | 0 |
| `just test-rust-unit` | Not required: no `src-tauri/` diff |

Cargo admission used the required census and 30-second polls; check-quick waited
through two samples with three foreign Cargo processes, then proceeded at two.
Own Cargo commands used one job and the checkout-local target. No foreign process
was stopped. The controller recorded zero transient-refusal retries; beta's Mesh
journal separately contains busy/pending attempts before its single native-enqueued
receipt. These are retained rather than counted as duplicate exposure.

Controller exit **1** reflects the step-5 assertion. Owned cleanup passed:
zero survivors, private port closed, auth removed before root deletion, root absent.
The complete sanitized daemon JSONL has **1,076 rows**, SHA-256
`7a55bb12f284efc85ad569cb5cbcc498211c01430c9dcc7ddba148badcd36d85`.
Snapshot interning retains **168 files / 119 unique payloads**, alongside the
direct daemon log, host events, owner observations, usage and cost records.
[Verification](l5-restarts/run6/continuation/verification.json) passed syntax,
payload hashes, sanitation, pane bounds, cleanup, no product diff and clean Mesh.
Gate stdout with a trailing blank line is retained losslessly as a JSON string.

Deviations and outstanding limits: the absent integration reference checkout
required the already-retained adaptation; this continuation added one supported
alpha startup recovery; first-boundary beta duration is unproved; step 5 stopped
on model pagination and step 6 stayed unrun. The independent Opus evidence lens
is unavailable in this tool surface and remains with the invoking orchestrator.
See [final-audit.json](l5-restarts/run6/continuation/final-audit.json) for all
classifications and original evidence. The four green numbered steps were each
committed with the requested trailers; the final packet preserves their scope
without converting the overall lane verdict to PASS.

## Run7 — seventh-attempt evidence

Taurhaus `a7e6db7e`, protocol **27**; Mesh `310144d`, unchanged enabled Codex **0.153.4** descriptor. Both native siblings copied to the scratch bin. Alpha used tmux, beta app_server; both `gpt-5.6-luna`, effort `low`. Production initialize used the builder canonical policy and a login-only Claude lead.

**FAIL step 5 (harness); later steps NOT RUN**. Runtime **336.60 s**, **14/20 counted inputs**, **$0.018016400/$0.30 metered estimate**, **2 unknown-cost inputs**. Unknown costs are counted, never treated as free; full billed spend is unverified. No total-lane PASS or release approval is claimed without the independent evidence review.

| Step | Outcome / classification | Evidence |
|---|---|---|
| 1 | **PASS — runtime** | [Outcome](l5-restarts/run7/runtime/step1-outcome.json).  |
| 2 | **PASS — runtime** | [Outcome](l5-restarts/run7/runtime/step2-outcome.json).  |
| 3 | **PASS — runtime** | [Outcome](l5-restarts/run7/runtime/step3-outcome.json).  |
| 4 | **PASS — runtime** | [Outcome](l5-restarts/run7/runtime/step4-outcome.json).  |
| 5 | **FAIL — harness** | [Outcome](l5-restarts/run7/runtime/step5-outcome.json). Observer predicate failed despite one accepted target and one transport exposure per seat; pending owner retries are not duplicate exposure. |
| 6 | **NOT RUN — blocked by earlier failure** | [Outcome](l5-restarts/run7/runtime/step6-outcome.json).  |

### Identities and accepted targets

Team incarnation: `9cf87ad7e054dc65dfa8a5245964bfc77d4f0de0348cfcecf64af96752265e2e`. Taurhaus PID/start ticks **62158 / 31291682 → 85236 / 31296604**. Alpha session `01a08e37-4697-7993-926b-4a9cc9b91380`, pane `%2`, generation **1 → 1**. Beta thread `01a08e37-4943-7e72-9ad2-fd1def65ef93`, pane **%3 → %15**, generation **1 → 3** through supported stop/recovery.

| Boundary / seat | Message ID | Delivery ID | Attempts / exposures / read |
|---|---|---|---|
| baseline / alpha | `8a9e78f5-5e5b-4c89-8206-cbffa97f1d8e` | `b2b49290-7868-4751-8f87-e06ab9eb86c7` | 1 / 1 / True |
| baseline / beta | `4bf62ce7-bdc5-47d0-9f47-e0be370bca73` | `4931df6f-9260-40a3-8a15-a8790340cb76` | 1 / 1 / True |
| taurhaus-backlog / alpha | `5fbc9d08-5af1-4bb7-b79d-c50ecd7aaaa7` | `8fa6edf3-a096-4823-b268-1f64d387d433` | 1 / 1 / True |
| taurhaus-backlog / beta | `f225eae3-daa6-4107-8d17-6e5cb966c516` | `021c603b-1b02-4f2e-948d-36a478bddad9` | 19 / 1 / True |
| mesh-backlog / alpha | `a966024b-4b5d-42fd-8b85-99e11eeb6a38` | `fa963dba-4197-4f5c-88da-54d9ce6fc278` | 1 / 1 / True |
| mesh-backlog / beta | `7dc3dce9-4ba1-4df2-8f3f-21060b399e53` | `866255fc-ed56-4177-bc5d-e3acc942b349` | 17 / 1 / False |

### What this attempt establishes

Steps 1–4 passed their runtime predicates. Both baseline IDs were read; both first-boundary markers were accepted and unexposed in the same attributed-working sample, immediately followed by normal daemon SIGINT/restart with identical arguments and new PID/start ticks. Beta required supported `resume_member`; logical identities and team incarnation stayed stable, generations did not regress, and both markers were delivered/read once without baseline replay.

Step 5 **FAIL — harness, observer attempt-count predicate**: alpha has one durable attempt and one submitted notification; beta has **17 distinct attempts**, with **16 pending `pre_input_failure: IO error: delivery: thread_active` receipts** followed by **one native_enqueued** receipt and its card in host events. These are Mesh-owned retries, not controller resends. The raw observer timeout says receipt/witness missing, but both witnesses exist: the failed conjunct is exactly-one-attempt accounting. No duplicate exposure or lost obligation is established. Beta also had 19 attempts / one exposure at the first boundary, accepted by step 4. The corrected observer counts one accepted target and one exposing receipt, paired only with the exposing attempt ID; pending retries are deferral evidence. The observed retry cadence (16 refusals over about 43 seconds) is a separate observation, not the lane verdict.

Step 6 **NOT RUN** under stop-on-failure. Read-only post-teardown accounting finds one transport exposure for each of the six marker IDs; alpha’s final marker has a read receipt, beta’s final marker does not. This offline accounting does not substitute for the required explicit final reads. The cursor-following step-6 implementation is offline-tested, not runtime-certified here.

Working-window measurements: alpha **46.254 s** at the Taurhaus boundary and **44.376 s** at the Mesh boundary; beta **46.251 s** at the Mesh boundary. Beta’s original Taurhaus-boundary turn has no completed >=30-second window: the daemon restart interrupted it before python3 began, and the paced task ran after supported resume. That subclaim is **UNPROVED — harness timing**.

Owner epoch **2 → 3**, PID **71538 → 103340**; the old owner was gone before first delivery. Across **435** restart-window samples, maximum observed owners was **1**. However, maximum gap **1.713 s** violates the ≤1-second cadence. Required owner exclusion is **UNPROVED — harness cadence**, despite no observed overlap.

### Runtime evidence

[Controller with offline review corrections](l5-restarts/run7/controller.py), [ordered assertions](l5-restarts/run7/steps.py), [commands/RPCs and exits](l5-restarts/run7/runtime/events.jsonl), [complete daemon JSONL](l5-restarts/run7/runtime/taurhaus.log.jsonl), [host events](l5-restarts/run7/runtime/host-events.jsonl), [owner census](l5-restarts/run7/runtime/owner-observations.jsonl), [lossless snapshots](l5-restarts/run7/runtime/snapshots.json), [review-corrected audit](l5-restarts/run7/final-audit.json). Only the interpreted step-5 classification is corrected; raw journal, daemon JSONL and snapshot observations remain unchanged. Commit `775085af` retains the original verdict and executed controller sources; the linked sources include offline fixes. Snapshot filenames map to SHA-256-keyed exact payloads; `pack.unpack` restores them. Pane captures contain at most 60 lines. Acceptance, transport delivery, explicit read and model action remain separate.

Daemon JSONL: **898 complete rows**, SHA-256 `e84a0850f6004a5316288fb4ac6496320cff4dcda0c3f55d982ff91105eb3aa3`. Controller exit **1**. Controller transient busy refusal episodes: **0**; the controller retries only named `host member busy` / `lock busy` refusals within a 65-second deadline.

The step-5 implementation checks transport receipts and native witnesses plus owner exclusion; the seat’s own read is observational. The step-6 implementation (not run this attempt) executes `mesh read --name <seat> --unread --mark-read --json` with explicit root/team and unchanged filters, following `--since <cursor>` until `done: true`, then pages the journal and reconciles both boundaries. Scratch AGENTS.md also requires seats to follow empty or nonempty `done: false` pages.

### Every measured spend

Rates: inherited packet API-equivalent estimates, $0.20/$0.02/$1.20 per million uncached input/cached input/output tokens. Reasoning is included in output. These are not invoice amounts. All generations are listed; a turn without metering is explicitly unknown.

| Thread / turn | Generation | Input / cached / output (reasoning) | USD estimate |
|---|---:|---|---:|
| `01a08e37-4943-7e72-9ad2-fd1def65ef93` / `01a08e37-4f22-75f3-be42-b4989b30c6c8` | 1 | 11025 / 6912 / 68 (57) | 0.001042440 |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e37-6048-7662-97e0-52c775ea4776` | 1 | 9307 / 3840 / 98 (26) | 0.001287800 |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e37-6048-7662-97e0-52c775ea4776` | 2 | 10818 / 8960 / 63 (54) | 0.000626400 |
| `notify-only session` / `01a08e37-6269-7e20-be53-6071fd290912` | — | Unreported | **Unknown** |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e37-850d-7e83-b1bd-91e5c7d84c11` | 1 | 10939 / 9984 / 81 (9) | 0.000487880 |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e37-850d-7e83-b1bd-91e5c7d84c11` | 2 | 11816 / 9984 / 50 (34) | 0.000626080 |
| `01a08e37-4943-7e72-9ad2-fd1def65ef93` / `01a08e37-ab06-7051-8381-f41cdd58b046` | 1 | 11921 / 9984 / 13 (0) | 0.000602680 |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e37-e590-7a11-9b4f-656371ea6a98` | 1 | 11958 / 11008 / 180 (74) | 0.000626160 |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e37-e590-7a11-9b4f-656371ea6a98` | 2 | 12796 / 11008 / 66 (19) | 0.000656960 |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e37-e590-7a11-9b4f-656371ea6a98` | 3 | 13118 / 12032 / 5 (0) | 0.000463840 |
| `01a08e37-4943-7e72-9ad2-fd1def65ef93` / `01a08e37-ec78-7a33-af00-efcc983ba1ef` | — | Unreported | **Unknown** |
| `01a08e37-4943-7e72-9ad2-fd1def65ef93` / `01a08e38-0827-71c1-9039-b01b61975ca6` | 1 | 12732 / 11008 / 153 (13) | 0.000748560 |
| `01a08e37-4943-7e72-9ad2-fd1def65ef93` / `01a08e38-0827-71c1-9039-b01b61975ca6` | 2 | 13517 / 12032 / 71 (8) | 0.000622840 |
| `01a08e37-4943-7e72-9ad2-fd1def65ef93` / `01a08e38-0827-71c1-9039-b01b61975ca6` | 3 | 13811 / 6912 / 5 (0) | 0.001524040 |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e38-9dce-7ed1-842a-798910a383a2` | 1 | 13184 / 12032 / 88 (16) | 0.000576640 |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e38-9dce-7ed1-842a-798910a383a2` | 2 | 14072 / 13056 / 17 (0) | 0.000484720 |
| `01a08e37-4943-7e72-9ad2-fd1def65ef93` / `01a08e38-bf2a-7881-81bf-8fcb13d68065` | 1 | 13934 / 13056 / 28 (7) | 0.000470320 |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e38-e550-7b72-b1d7-54e8e0ec4482` | 1 | 14181 / 13056 / 124 (26) | 0.000634920 |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e38-e550-7b72-b1d7-54e8e0ec4482` | 2 | 14963 / 13056 / 45 (0) | 0.000696520 |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e38-e550-7b72-b1d7-54e8e0ec4482` | 3 | 15267 / 14080 / 5 (0) | 0.000525000 |
| `01a08e37-4943-7e72-9ad2-fd1def65ef93` / `01a08e38-e9c2-7b83-ae4d-475af9db3796` | 1 | 14054 / 13056 / 145 (7) | 0.000634720 |
| `01a08e37-4943-7e72-9ad2-fd1def65ef93` / `01a08e38-e9c2-7b83-ae4d-475af9db3796` | 2 | 14831 / 13056 / 70 (7) | 0.000700120 |
| `01a08e37-4943-7e72-9ad2-fd1def65ef93` / `01a08e38-e9c2-7b83-ae4d-475af9db3796` | 3 | 15124 / 13056 / 5 (0) | 0.000680720 |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e39-9812-7141-a3da-be74a7c16174` | 1 | 15332 / 15104 / 78 (6) | 0.000441280 |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e39-9812-7141-a3da-be74a7c16174` | 2 | 16238 / 15104 / 833 (18) | 0.001528480 |
| `01a08e37-4697-7993-926b-4a9cc9b91380` / `01a08e39-9812-7141-a3da-be74a7c16174` | 3 | 17105 / 15104 / 80 (63) | 0.000798280 |
| `01a08e37-4943-7e72-9ad2-fd1def65ef93` / `01a08e39-a5c6-7621-83e9-e171970bb1d2` | 1 | 15239 / 14080 / 13 (0) | 0.000529000 |

[Cost ledger](l5-restarts/run7/runtime/cost-ledger.json) and [native usage rows](l5-restarts/run7/runtime/usage-events.json). Build, read-only observers, export and gates add zero trial inputs. Implementer/reviewer spend is separately owned by the invoking orchestrator.

### Red → green, gates and teardown

Four new offline acceptance checks: initial run exited **1** (two assertion failures and one missing-helper error); cursor-following behavior passed already. After the step-5 transport correction and scratch pagination instructions, all **37** offline checks passed, exit **0**. Regression comments name `e6fa7d0e`. [Red](l5-restarts/run7/red.txt), [green](l5-restarts/run7/green.txt).

| Exact command, checkout root | Exit |
|---|---:|
| `just check-quick` | **0** |
| `just lint` | **0** |
| `just test-contracts` | **0** |

All gates ran **after teardown**, with credential-free scratch roots and real harness executables blocked. Cargo used one build job and this checkout’s own target directory; preflight waited in 30-second polls only when at least three Cargo processes already existed. No `src-tauri/` diff, so `just test-rust-unit` was not required.

[Cleanup](l5-restarts/run7/runtime/cleanup.json): survivors `[]`, private port closed `True`, auth copy explicitly removed `True`, root removed `True`. [Gate cleanup](l5-restarts/run7/gates/gate-cleanup.json) records reaped commands and removed scratch roots. No foreign process was signaled.

### Offline review correction (no new paid attempt)

Verified both major findings against the retained journal and native witnesses: the corrected transport predicate proves alpha (1 attempt / 1 exposure) and beta (17 attempts / 1 exposure). Step 5 remains FAIL — harness, because the original observer stopped there; step 6 remains NOT RUN. No runtime PASS is inferred. Historical run1 scope/outcome headings are now explicit.

Test-first evidence: `python3 -m unittest discover -s docs/design/evidence/e2e/l5-restarts/run7 -p '*test.py'` initially ran 42 tests and exited 1 with six assertion failures (transport retry acceptance, step-6 retry accounting, diagnostic classification, census deadline, and direct test entrypoint). The report regression then failed separately (43 tests, exit 1), reproducing the product headline. After correction all 43 tests passed, exit 0; `python3 docs/design/evidence/e2e/l5-restarts/run7/support_test.py` ran all 27 checks, exit 0. Six new tests retain Regression comments naming `540f23ea` or `775085af`.

Re-run after teardown: `just check-quick` exit 0 (19.11 s), `just lint` exit 0 (6.58 s), `just test-contracts` exit 0 (8.03 s). The existing isolated gate wrapper used a credential-free home, blocked real harness executables, one Cargo job and this checkout’s own target. Each Cargo preflight found two existing processes, so no wait was required. Gate cleanup confirmed children reaped and scratch roots removed. No Rust diff; Rust unit execution was not required.

This review adds **0 Codex inputs / $0 seat spend**; all original measured spends and unknown costs above are retained. No paid rerun was performed in this local correction round. Deadline-based sampling reduces scan-plus-sleep drift but cannot erase or excuse the retained 1.713-second gap. The optional run3 credential-literal redaction is deferred: those historical files are untouched, and their old allowlist tests intentionally encode those literals; no token material is present.

### Deviations and limits

- Spec-referenced integration checkout absent (git show exit 128); inspected its retained attempt9 sources here and messaging run2 sources read-only.
- Unknown-cost inputs counted separately; metered estimate is not an invoice or complete billed spend.
- Independent Opus evidence lens and implementer/reviewer metering belong to invoking orchestrator; Opus unavailable in this tool surface.
- Step 5 stopped on a harness observer predicate that counted pending retries as exposures; step 6 was not run.
- The original beta turn at the Taurhaus boundary was interrupted before python3 began; its >=30-second continuous window is unproved. The resumed task is not substituted as original-turn evidence.
- The passive owner census saw at most one owner and old-owner departure before delivery, but its 1.713-second maximum gap exceeds the <=1-second requirement; sampling cadence coverage is unproved (harness).
- No product, Mesh descriptor, installation, release, plan-ledger, or other Taurhaus checkout change. Only the specified Mesh worktree was built; no Mesh commit.

## Run8 — eighth-attempt evidence

Taurhaus `26c06132`, protocol 27; Mesh `310144d`, unchanged enabled Codex 0.153.4 descriptor. Both native siblings copied; alpha tmux, beta app_server, gpt-5.6-luna at low; canonical initialize and login-only Claude lead.

**UNPROVED — run 8: required working-window coverage (harness timing)**. Runtime 199.87 seconds; 15/20 counted inputs (including one unprompted warm-up); $0.016596320/$0.30 metered; 2 unknown-cost turns. Unknown is never treated as free. Independent Opus review remains with the invoking orchestrator.

| Step | Outcome / classification | Evidence |
|---|---|---|
| 1 | PASS — runtime | [Outcome](l5-restarts/run8/runtime/step1-outcome.json). Both baselines delivered and explicitly read. |
| 2 | PASS — runtime | [Outcome](l5-restarts/run8/runtime/step2-outcome.json). Both markers pending at the Taurhaus boundary; beta original >=30-second window UNPROVED (harness timing). |
| 3 | PASS — runtime | [Outcome](l5-restarts/run8/runtime/step3-outcome.json). Normal Taurhaus restart; new PID/start ticks and protocol 27; supported beta recovery. |
| 4 | PASS — runtime | [Outcome](l5-restarts/run8/runtime/step4-outcome.json). Both backlog IDs delivered/read once; stable identities; no baseline replay. |
| 5 | PASS — runtime | [Outcome](l5-restarts/run8/runtime/step5-outcome.json). Mesh owner epoch/PID changed; both fresh IDs exposed once; owner census PASS. |
| 6 | PASS — runtime | [Outcome](l5-restarts/run8/runtime/step6-outcome.json). Explicit reads followed cursors to done; six accepted targets, six exposures, six read obligations reconciled. |

### Runtime evidence

[Exact controller](l5-restarts/run8/controller.py), [ordered steps](l5-restarts/run8/steps.py), [commands, RPCs and exits](l5-restarts/run8/runtime/events.jsonl), [complete daemon JSONL](l5-restarts/run8/runtime/taurhaus.log.jsonl), [lossless snapshots](l5-restarts/run8/runtime/snapshots.json), [final audit](l5-restarts/run8/final-audit.json). Snapshot filenames map to SHA-256-keyed payloads; `pack.unpack` restores them. Panes contain at most 60 lines.

Daemon JSONL: 624 complete rows, SHA-256 `a819de4765d1f40e8b5bb132aa69bf1236195c71b7576d089b82d3ab35d1645e`. Controller exit 0; 0 transient controller refusals observed; the retry policy allows named busy refusals within 65 seconds.

Count one accepted target and at most one submitted/native_enqueued receipt per delivery ID; pair only the exposing attempt and require its native witness. Pending owner retries are deferral evidence. Step 6 follows every explicit read cursor to done. Transport and read receipts remain separate.

### Every measured spend

Inherited packet rates: $0.20/$0.02/$1.20 per million uncached input/cached input/output tokens; reasoning included in output. API-equivalent estimates, not invoice amounts. Warm-up: one counted TUI start, zero model prompts, $0 metered. Claude lead: zero turns. Builds, observers and gates: zero trial inputs.

| Thread / turn | Generation | Input / cached / output (reasoning) | USD estimate |
|---|---:|---|---:|
| `01a08e61-32d2-7540-8d4a-448d710dd682` / `01a08e61-3b0a-71f2-a2f5-5f84e06c72a5` | 1 | 11162 / 6912 / 109 (82) | 0.001119040 |
| `01a08e61-32be-7e31-9775-e58aaa81cad3` / `01a08e61-4d96-7922-870e-75b37ea8bfdc` | 1 | 9302 / 3840 / 99 (27) | 0.001288000 |
| `01a08e61-32be-7e31-9775-e58aaa81cad3` / `01a08e61-4d96-7922-870e-75b37ea8bfdc` | 2 | 10808 / 1792 / 88 (79) | 0.001944640 |
| `notify-only session` / `01a08e61-4f79-7e00-8f1a-5179ef296646` | — | Unreported | **Unknown** |
| `01a08e61-32be-7e31-9775-e58aaa81cad3` / `01a08e61-776c-76b0-896d-0d286c112f24` | 1 | 10954 / 9984 / 79 (8) | 0.000488480 |
| `01a08e61-32be-7e31-9775-e58aaa81cad3` / `01a08e61-776c-76b0-896d-0d286c112f24` | 2 | 11835 / 8960 / 124 (108) | 0.000903000 |
| `01a08e61-32d2-7540-8d4a-448d710dd682` / `01a08e61-9a2e-7b02-b4ed-db168e7019ad` | 1 | 11381 / 11008 / 13 (0) | 0.000310360 |
| `01a08e61-32be-7e31-9775-e58aaa81cad3` / `01a08e61-b94f-7b23-9b68-8c097cd80f4f` | 1 | 12051 / 11008 / 189 (72) | 0.000655560 |
| `01a08e61-32be-7e31-9775-e58aaa81cad3` / `01a08e61-b94f-7b23-9b68-8c097cd80f4f` | 2 | 12899 / 11008 / 63 (15) | 0.000673960 |
| `01a08e61-32be-7e31-9775-e58aaa81cad3` / `01a08e61-b94f-7b23-9b68-8c097cd80f4f` | 3 | 13217 / 12032 / 5 (0) | 0.000483640 |
| `01a08e61-32d2-7540-8d4a-448d710dd682` / `01a08e61-bd88-7e00-a38a-38a369fca305` | — | Unreported | **Unknown** |
| `01a08e61-32d2-7540-8d4a-448d710dd682` / `01a08e61-dcd3-7a42-bf96-03dfde194b5a` | 1 | 12193 / 11008 / 117 (21) | 0.000597560 |
| `01a08e61-32d2-7540-8d4a-448d710dd682` / `01a08e61-dcd3-7a42-bf96-03dfde194b5a` | 2 | 12969 / 12032 / 55 (7) | 0.000494040 |
| `01a08e61-32d2-7540-8d4a-448d710dd682` / `01a08e61-dcd3-7a42-bf96-03dfde194b5a` | 3 | 13281 / 12032 / 5 (0) | 0.000496440 |
| `01a08e61-32be-7e31-9775-e58aaa81cad3` / `01a08e62-702d-72d1-8065-d69c368b1689` | 1 | 13283 / 13056 / 79 (8) | 0.000401320 |
| `01a08e61-32be-7e31-9775-e58aaa81cad3` / `01a08e62-702d-72d1-8065-d69c368b1689` | 2 | 14168 / 13056 / 16 (0) | 0.000502720 |
| `01a08e61-32d2-7540-8d4a-448d710dd682` / `01a08e62-a00b-74f2-869d-44c4386ba1fb` | 1 | 13400 / 13056 / 16 (0) | 0.000349120 |
| `01a08e61-32be-7e31-9775-e58aaa81cad3` / `01a08e62-c483-7b30-9b74-0c919540ded5` | 1 | 14276 / 9984 / 124 (7) | 0.001206880 |
| `01a08e61-32be-7e31-9775-e58aaa81cad3` / `01a08e62-c483-7b30-9b74-0c919540ded5` | 2 | 15060 / 13056 / 54 (6) | 0.000726720 |
| `01a08e61-32be-7e31-9775-e58aaa81cad3` / `01a08e62-c483-7b30-9b74-0c919540ded5` | 3 | 15369 / 14080 / 5 (0) | 0.000545400 |
| `01a08e61-32d2-7540-8d4a-448d710dd682` / `01a08e62-c919-71b1-8102-a4f6030d1fc6` | 1 | 13508 / 6912 / 104 (8) | 0.001582240 |
| `01a08e61-32d2-7540-8d4a-448d710dd682` / `01a08e62-c919-71b1-8102-a4f6030d1fc6` | 2 | 14271 / 13056 / 46 (0) | 0.000559320 |
| `01a08e61-32d2-7540-8d4a-448d710dd682` / `01a08e62-c919-71b1-8102-a4f6030d1fc6` | 3 | 14574 / 14080 / 5 (0) | 0.000386400 |
| `01a08e61-32be-7e31-9775-e58aaa81cad3` / `01a08e63-76c1-7512-8509-644b8279ab19` | 1 | 15434 / 15104 / 78 (7) | 0.000461680 |
| `01a08e61-32d2-7540-8d4a-448d710dd682` / `01a08e63-7d70-7961-9116-6504fcbd45d9` | 1 | 14693 / 14080 / 13 (0) | 0.000419800 |

### Restart and working-window measurements

| Boundary / seat | Original working seconds | Outcome |
|---|---:|---|
| taurhaus-backlog / alpha | 45.367 | PASS — runtime |
| taurhaus-backlog / beta | None | UNPROVED — harness timing |
| mesh-backlog / alpha | 44.97 | PASS — runtime |
| mesh-backlog / beta | 44.947 | PASS — runtime |

Beta at the first boundary: the pending sample occurred 3.459 seconds after its original turn started, before a python3 command was retained. That original turn has no completion record; execution after supported recovery cannot prove its original >=30-second window. This required subclaim is UNPROVED — harness timing, despite the six delivery/restart/read predicates passing.

Measured intervals use original turns, never a resumed substitute. Pending samples, daemon PID/start ticks, seat identities, owner epoch and process census are retained in the final audit and lossless snapshots.

| Boundary / seat | Message ID | Attempts / exposures / explicitly read |
|---|---|---|
| baseline / alpha | `a845d9b8-e555-4111-92bc-7e445159b138` | 1 / 1 / True |
| baseline / beta | `92f424be-caa9-498e-95ab-f41441383202` | 1 / 1 / True |
| taurhaus-backlog / alpha | `e9bf44ce-be2c-4776-85b7-59063b73c24b` | 1 / 1 / True |
| taurhaus-backlog / beta | `1f0faa16-41a7-4007-b7bc-02909af9f149` | 19 / 1 / True |
| mesh-backlog / alpha | `d5d56d42-772b-4fe1-9f37-49d7284e8c4c` | 1 / 1 / True |
| mesh-backlog / beta | `742fab9f-e101-4dc2-b591-1a75a0e0f460` | 16 / 1 / True |

Owner census: PASS; PID 755964 → 779904; 94 samples; maximum owners 1; maximum gap 0.561s. one owner per passive sample across both epochs; bounded evidence only.

[Cost ledger](l5-restarts/run8/runtime/cost-ledger.json). Implementer/reviewer spend is separately owned by the invoking orchestrator.

### Verification and teardown

Four new offline regressions failed first, exit 1; all 48 inherited and new checks then passed, exit 0. A fifth regression then failed on the absent verdict coverage guard; the corrected audit refuses full PASS when a required window is unproved. Checks cover warm-up before initialize, composer/clean exit/SQLite barrier, counted warm-up input, and removal of stale audit claims. Regression comments name the original commits. [Red](l5-restarts/run8/red.txt), [green](l5-restarts/run8/green.txt), [integrity checks](l5-restarts/run8/verification.json).

| Exact gate from checkout root | Exit | Seconds |
|---|---:|---:|
| `just check-quick` | 0 | 42.15 |
| `just lint` | 0 | 45.33 |
| `just test-contracts` | 0 | 44.37 |

Gate timing and exact outputs are retained under [gate sidecars](l5-restarts/run8/gates/). All gates ran after teardown in credential-free scratch roots with real harness executables blocked. Cargo preflight polls only when at least three Cargo processes exist, at 30-second intervals; one build job and checkout-local target. No Rust diff; `just test-rust-unit` does not apply.

[Cleanup](l5-restarts/run8/runtime/cleanup.json): survivors `[]`; private port closed `True`; auth copy explicitly removed `True`; root removed `True`. No foreign process signaled.

### Deviations and limits

- Spec-referenced integration checkout absent (git show exit 128); inspected its retained attempt9 sources here and messaging run2 sources read-only.
- Beta original Taurhaus-boundary turn has no completed >=30-second interval; paced work after recovery is not substituted. Required timing coverage remains unproved (harness).
- Warm-up quit needed one recorded Enter confirmation after loading; no model prompt or restart added.
- Unknown-cost inputs counted separately; metered estimate is not an invoice or complete billed spend.
- Independent Opus evidence lens and implementer/reviewer metering belong to invoking orchestrator; Opus unavailable in this tool surface.
- No product change, Mesh descriptor edit/commit, install, release, plan ledger edit, or mutation in another Taurhaus checkout.
