# Lane 5 run3 — UNAVAILABLE: step 3 harness failure after the Taurhaus restart

Latest attempt: **run3; step 1 PASS; step 2 pending-state PASS with a duration
shortfall; step 3 FAIL (harness); steps 4–6 NOT RUN.** Both markers were pending
when the Taurhaus restart began. The Mesh restart and complete cross-boundary
reconciliation remain unproved. All required gates and cleanup pass.
See [run3 evidence](#run3-third-attempt-evidence) below. Prior attempts remain history.

## Historical first attempt and offline continuation

**Step 1 PASS; step 2 FAIL (harness); steps 3–6 NOT RUN.** No product defect or
restart-survival result is established. The controller stopped at the first failed
step, retained evidence, and removed all owned runtime processes and credentials.
Independent Opus review belongs to the invoking small-change orchestrator and is
not claimed by this implementer.

## Candidate and scope

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

## Ordered outcomes

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
its two identified step-3 faults. The raw assertion/exit remain in
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
