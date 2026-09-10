# INCOMPLETE — latest run 4, L4 step 1 PASS; continuation in progress

Run 4 initialized and exchanged/read both markers. This checkpoint is not a whole-lane PASS.
See [Run 4](#run-4) for the current evidence; runs 2 and 3 below are historical.

## Historical run 2 — hosted process exited before transport readiness

The authorized continuation resolved both earlier controller/setup mistakes: the
read-only `CODEX_HOME/auth.json` copy used the reference controller's fallback,
and the lane-built Mesh binary was placed at the uncommitted Tauri resource path.
**Both builds passed.** The real private protocol-27 daemon then accepted production
`coordination.initialize_team` for the canonical lead/alpha/beta team and returned:

```text
failed_step: launch_host
Conflict: app-server exited before transport readiness
```

**Classification: Taurhaus hosted-launch boundary.** This names the observed
failure boundary, not a proven underlying native-process defect. The launcher
sets child stderr to `Stdio::null()` and reports its exit without the native
reason (`src-tauri/src/coordination/hosted_process.rs:42–70`). The deeper cause is
unverified; this packet does not attribute it to Mesh, credentials, or a migration
race. No product change or runtime retry followed the failure.

| Audit step | Outcome | Evidence / classification |
|---|---|---|
| 1. Initialize; exchange/read markers; retain identities and receipts | **FAIL** | Production initialize failed at `launch_host` after validation, team creation and lead addition. Taurhaus hosted-launch boundary. |
| 2. Stop every seat through supported daemon stop | **NOT RUN** | Stopped after step 1; behavior not evaluated. |
| 3. Accept one pending obligation per stopped seat | **NOT RUN** | Stopped after step 1; behavior not evaluated. |
| 4. Call `resume_team` once and poll its own status | **NOT RUN** | No resume call made; no historical skip substituted. |
| 5. Verify incarnation, adapters, generations, recovery and pending deliveries | **NOT RUN** | No resumed generation or pending marker existed. |
| 6. Explicit read/mark, no old replay/member executor, export and teardown | **NOT RUN** | Workflow assertions not reached. Failure cleanup/export completed separately. |

Individual outcomes are [step 1](l4-resume-team/run2/step1-outcome.json),
[step 2](l4-resume-team/run2/step2-outcome.json),
[step 3](l4-resume-team/run2/step3-outcome.json),
[step 4](l4-resume-team/run2/step4-outcome.json),
[step 5](l4-resume-team/run2/step5-outcome.json), and
[step 6](l4-resume-team/run2/step6-outcome.json).

## Runtime evidence

- Checkout: `/home/mstie/projects/taurhaus-l4-resume-team`, branch
  `feat/e2e-l4-resume-team`; product source `6f61f6117a75625ce4ec6d325879730f1800c473`.
- Mesh: only `/home/mstie/projects/mesh-l4`, source
  `a6ee29681a44a472c0ce7b33b9630d4865c93012`. Its 0.153.4 descriptor was compiled
  `trial`/enabled with `taurhaus-daemon-owned-thread/1`, `strict-config/1`,
  `daemon-owned/1`; wildcard stayed disabled. Source restored, no flip committed.
- Runtime `ping` returned **protocol 27**, daemon version **0.9.7**, private port
  **46147**. Copied native CLI returned **codex-cli 0.153.4**.
- Initialize run: **`init_2b3cf70baf3b4185a9228f9c4ffecff9`**; logging run:
  **`run_d470c58eb8db431b864fc1b7fd23d74f`**.
- Team **`l4-resume`**, lead `lead`/Claude, `alpha`/Codex/tmux,
  `beta`/Codex/app_server; both Codex seats requested **gpt-5.6-luna, low**.
  Request included the builder's exact canonical retention policy.
- No hosted socket observer was created. The app-server exited before transport
  readiness; no hosted transcript/input RPC was issued. There was no marker send,
  onboarding stage, model response, explicit read, stop-session or resume RPC.
- Initialization's own failure cleanup removed the team/runtime/journal state
  before the final snapshot. No published thread/session ID, attachment generation
  or team incarnation was retained. These identities are **unavailable evidence**,
  not invented values; the operation's create-team/add-lead successes do not prove
  a usable team.

[Commands and RPCs](l4-resume-team/run2/events.jsonl),
[initialize status samples and every reported step](l4-resume-team/run2/step1-operation.json),
[canonical policy](l4-resume-team/run2/policy.json),
[version probe](l4-resume-team/run2/codex-version.json),
[bootstrap command/script](l4-resume-team/run2/bootstrap.json),
[daemon lifecycle events](l4-resume-team/run2/daemon-events.json),
[daemon stderr tail](l4-resume-team/run2/daemon-stderr.json),
[retained state/process identities](l4-resume-team/run2/final-state.json),
[remaining infrastructure pane, 48 lines](l4-resume-team/run2/final-pane0.json).

The operation was polled with a 150-second deadline and stopped on an explicit
terminal failure, not an early timeout. Other positive assertions have deadlines
of at least 60 seconds. Evidence collection used complete JSONL rows, deduplicated
host events, bounded daemon diagnostics and pane captures limited to 60 lines;
there was no evidence-size abort or fault injection.

## Every turn and cost

| Seat | Launch-attempt accounting | Paid inputs / turn IDs | Spend |
|---|---|---|---:|
| alpha | Conservatively reserve one launch attempt | 0 / none | $0 |
| beta | One observed hosted-launch failure | 0 / none | $0 |
| lead | Login-only configuration; no model input | 0 / none | $0 |
| Earlier attempt | No runtime launch | 0 / none | $0 |
| **Lane total** | **At most 2 Codex seat-start attempts** | **0 / none** | **$0** |

[Observed usage ledger](l4-resume-team/run2/cost-ledger.json) and
[cumulative attempt/cap reconciliation](l4-resume-team/run2/attempt-budget.json).
There were no `turn/start` submissions, rollout turn/usage records or completed
onboarding before failure. Zero is not inferred from reset compaction counters.
Including conservatively counted launches, the lane is within **16 inputs/starts**
and **$0.25**. Runtime ended within seconds, below 15 minutes. Implementer/reviewer
spend is separate orchestrator accounting and is not exposed here.

## Exact controllers, red/green and gates

Executed from the assigned checkout:

```sh
python3 -B docs/design/evidence/e2e/l4-resume-team/build2.py
python3 -B docs/design/evidence/e2e/l4-resume-team/controller.py
python3 -B docs/design/evidence/e2e/l4-resume-team/gates2.py
```

The runtime controller exited **1**. Its exact executed source and support are
frozen as [executed-controller.py](l4-resume-team/run2/executed-controller.py) and
[executed-preflight.py](l4-resume-team/run2/executed-preflight.py), matching commit
`4f946ee5`. [build2.py](l4-resume-team/build2.py) and
[gates2.py](l4-resume-team/gates2.py) retain the exact build/gate commands.
The current controller includes an offline review fix; it was **not rerun**.

Red-first auth and accounting tests observed import failure (exit **1**) before
implementation, then five passing tests (exit **0**). Controller review added a
resume-report refusal regression guard: red import failure **1**, then six tests
passed **0**. It prevents a terminal RPC response with failed members or a refused
team-daemon startup from passing step 4. Regression comments identify `0267819e`
and `4f946ee5`. Tests use generated tempdirs or in-memory records, never real auth,
real CLIs, network, or subprocesses. [Test evidence](l4-resume-team/continuation-tests.json).

| Command | Exit | Evidence |
|---|---:|---|
| `cargo build --bin mesh` in the designated Mesh worktree | **0** | [Build](l4-resume-team/build2/mesh.json) |
| `just build-daemon` | **0** | [Build](l4-resume-team/build2/daemon.json) |
| `just check-quick` | **0** | [Gate](l4-resume-team/gates2/check-quick.json) |
| `just lint` | **0** | [Gate](l4-resume-team/gates2/lint.json) |
| `just test-contracts` | **0** | [Gate](l4-resume-team/gates2/test-contracts.json) |

Builds used checkout-local targets. Cargo was probed before this continuation
build and was idle; [wait evidence](l4-resume-team/build2/cargo-wait.json).
[Binary digests](l4-resume-team/build2/binaries.json) bind the runtime copies:

```text
mesh    334257dcd3bd34d9c60a772b6b5c5b774f4d1914c5a233cd849a713e44ab1717
daemon  3f2330b7b6c39070270f9277c6f3f0b6e6f1a280a39529cc39a8c6d94343c6eb
```

No `src-tauri/` diff exists; `just test-rust-unit` was not required. No numbered
runtime step passed, so there is no fabricated per-step green commit. Green
controller/build preparation and subsequent evidence are committed separately,
with the requested Co-Authored-By trailer.

## Isolation, cleanup and deviations

Only the user-authorized auth source was read, to copy `auth.json` into the private
Codex home with mode 0600. Auth contents were never logged; operator config,
instructions, histories and hooks were not copied. Runtime HOME, harness roots,
project, temp/data paths, tmux socket and daemon port were private. `$TMUX` was
absent. Bubblewrap hid operator homes and gave descendants a private PID namespace;
the host filesystem was read-only apart from the scratch bind, with private
tmpfs mounts for `/home`, `/tmp` and `/run`. The scratch project had command-capable
AGENTS.md; unrelated harnesses were blocked.

The controller terminated/waited its own child process groups and private PID
namespace. The final audit recorded **no survivors**, a closed private port, removed
scratch root and auth, restored descriptor, and removed uncommitted Mesh resource.
The gate runner removed the placeholder resource again after its recipes completed.
No installed binary, other lane, operator pane or unrelated process was changed.
[Cleanup](l4-resume-team/run2/cleanup.json),
[owned identities before cleanup](l4-resume-team/run2/owned-before-cleanup.json),
[final independent census](l4-resume-team/run2/final-audit.json).

Limitations/deviations:

- The initial auth/build blockers were controller/setup mistakes, corrected under
  the user's explicit continuation. Earlier top-level attempt artifacts remain
  historical; they do not describe this runtime result.
- Step 1 failed, so steps 2–6 were intentionally not executed under the binding
  stop-on-failure rule, despite the report's `retryable: true` flag.
- Some executed collector strings over-redacted embedded scratch `home/.local/bin`
  paths. The preserved controller, scratch-root identity and binary digests retain
  the command construction. The current collector fixes that redaction boundary;
  the original observations were not rewritten to look like raw evidence.
- Exact native child-exit reason, destroyed team incarnation and pre-teardown seat
  identities were not available. No root-cause certainty or later-step coverage is
  claimed from source inspection.
- Current-controller review tightened unexecuted resume/identity/recovery-card
  assertions. These later paths remain runtime-unverified.
- The independent Opus evidence lens remains for the orchestrator; no Opus tool is
  callable in this implementer session. No product changes or plan ledger edits.


## Run 3 — FAIL at step 1: Mesh ownership admission

**Verdict: FAIL; workflow incomplete.** The required private runtime ran once
from `f62bb158` (product base `6398bfa3` is an ancestor), protocol **27**, with
Mesh **`fcb9647`**. No product code changed, no paid retry occurred, and no
`resume_team` call was substituted for the failed initialization.

The exact sanitized production report is:

```text
failed_step: opt_in_delivery
Backend error: mesh team activation failed: error: IO error: delivery: quiescent required before opt-in: IO error: delivery: team owner already holds lifetime lock
```

**Classification: mesh (product ownership-admission refusal during Taurhaus
initialization).** The raw controller event classified the caller boundary as
`taurhaus`; source/evidence review refined this to the Mesh refusal boundary.
Taurhaus’s initialize report records successful validation, creation, lead
addition, pane/session launch, Mesh join and daemon startup before opt-in fails.
The retained config already has `delivery_owner: team`; passive FLOCK evidence
shows the private team owner holding `state/delivery/owner.lock`. Mesh’s
`delivery/ownership.rs` requests the owner lifetime lock before opt-in and
`delivery/store.rs` emits the refusal. This identifies the observed conflict;
it does not claim a fully diagnosed root cause or a fix.

| Ordered step | Outcome | Classification / evidence |
|---|---|---|
| 1. Initialize, exchange/read each marker, retain identities/receipts | **FAIL** | **mesh**: `opt_in_delivery` refusal; no marker exchange or read occurred. |
| 2. Supported stop of every seat; retain team/task/journal | **NOT RUN** | Not evaluated; stop-on-failure. Cleanup is not this lifecycle test. |
| 3. Accept one obligation per stopped seat | **NOT RUN** | Not evaluated; no backlog generated. |
| 4. One `resume_team` plus its own status polling | **NOT RUN** | Not evaluated; zero calls, no historical skip used as evidence. |
| 5. Preserved identities, new generations, recovery and pending delivery | **NOT RUN** | Not evaluated; no resumed generation. |
| 6. Read/mark, no replay/member executor, export/teardown | **NOT RUN** | Workflow assertions not evaluated; failure export/cleanup completed separately. |

All six machine-readable outcomes are under [run3/](l4-resume-team/run3/).
[Operation status samples](l4-resume-team/run3/step1-operation.json) retain every
reported transition and refusal, with a 150-second polling deadline. The explicit
terminal refusal ended the operation, not a shortened timeout. Runtime lasted
**8.266 seconds**, including failure cleanup.

### S-runtime identities and evidence

- Initialization run: `init_c75303346ae648bf8aa8ec78ea89c58e`.
- Daemon logging run: `run_6d0cc98434c64a8ab126990d70395a49`.
- Team: `l4-resume`; incarnation
  `51284c8270caa852e39e0291520af71c8f9650e0051d327d1ed97478d240b772`;
  messaging format **2**, owner **team**, exact builder retention policy.
- alpha: Codex/tmux, session `01a08b53-f140-71f3-ab0a-a56e83366f41`,
  pane `%2`, attachment generation **1**, context generation **"0"**.
- beta: Codex/app_server, thread/session
  `01a08b53-f825-7fa3-b8fe-5c200f8a2d38`, pane `%3`, attachment generation
  **1**, host generation `f9760754-ca78-491d-a43c-748816b84c37`.
- lead: scratch unauthenticated Claude login screen, pane `%1`, generation **1**;
  native session ID absent, **no model turn**.

[Final state](l4-resume-team/run3/final-state.json),
[activity snapshot](l4-resume-team/run3/final-activity.json),
[passive locks](l4-resume-team/run3/final-locks.json),
[owned processes with PID/start ticks](l4-resume-team/run3/owned-before-cleanup.json),
[commands/RPCs](l4-resume-team/run3/events.jsonl),
[complete sanitized daemon JSONL, 52 rows](l4-resume-team/run3/taurhaus.log.jsonl),
[bounded stderr](l4-resume-team/run3/daemon-stderr.json), and the four
`final-pane*.json` captures (each ≤60 lines) retain the observed boundary.
The journal is empty and task inventory is empty; no message/delivery/assignment
IDs for the requested markers exist.

The previous `launch_host` failure did **not** recur. beta’s real host loaded the
scratch `AGENTS.md`, created its thread and accepted baseline onboarding. Its
attachment reports the exact verified descriptor identities
`taurhaus-daemon-owned-thread/1`, `strict-config/1`, `daemon-owned/1`, build
**0.153.4**, transport `unix-websocket`. The Mesh descriptor was explicitly
**trial/enabled**, not claimed production-eligible. Both native siblings were
copied from the installation resolved by `shutil.which('codex')`, with individual
digests in `events.jsonl`; daemon/Mesh digests are in
[build/binaries.json](l4-resume-team/run3/build/binaries.json).

**alpha idle subclaim is NOT ESTABLISHED.** The final snapshot attributes alpha
correctly, at high confidence, to its own session/pane, but reports `active`.
Initialization failed before the positive attributed-idle assertion was reached.
No idle state was forged or inferred from beta’s separate hosted idle event.

### Every input and spend

| Seat | Starts | Observed paid inputs / turn | Spend |
|---|---:|---|---|
| alpha | 1 | 0 observed; no marker/onboarding turn record retained | $0 observed; no total-cost certification |
| beta | 1 | 1 baseline onboarding: `01a08b54-01ed-7271-a64a-5a85bb7f2b2f` | **UNKNOWN**; no token/completion receipt retained |
| lead | 1 login-only launch | 0 model inputs | $0 |
| Controller-submitted marker/recovery turns | — | 0 | $0 |

Conservative start-plus-input count is **3/16**. **Total spend and the $0.25 cap
are UNVERIFIED.** The raw meter’s `$0` is an empty metered subtotal, never evidence
of free onboarding. Cleanup attempted a read-only hosted transcript collection,
which returned exactly `HOST_OPERATION_FAILED: host member busy`; the executed
collector then exited observation and terminated its owned namespace. No later
usage row can be recovered from the deleted scratch home, and no account usage
endpoint/rows were consulted to invent a per-turn cost. See
[raw ledger](l4-resume-team/run3/cost-ledger.json),
[retained native turn event](l4-resume-team/run3/rollouts.json), and
[explicit reconciliation](l4-resume-team/run3/spend-reconciliation.json).
Implementer/reviewer spend belongs to the orchestrator’s separate accounting.

### Validation, cleanup and limitations

Exact commands (checkout root):

```sh
python3 -B docs/design/evidence/e2e/l4-resume-team/run3/build.py
python3 -B docs/design/evidence/e2e/l4-resume-team/run3/controller.py
python3 -B docs/design/evidence/e2e/l4-resume-team/run3/gates.py
python3 -B -m unittest discover -s docs/design/evidence/e2e/l4-resume-team/run3 -p test_support.py
python3 -B docs/design/evidence/e2e/l4-resume-team/run3/audit.py
```

Builds used checkout-local targets and the run-2 resource preparation. Cargo was
polled with `pgrep -af '(^|/)cargo( |$)'` until idle, within the 30-minute deadline.
Mesh source was restored after building and again at runtime teardown. The main
trial executable and its matching Cargo executable hardlink were removed.

| Command | Exit |
|---|---:|
| `cargo build --bin mesh` (designated Mesh worktree) | 0 |
| `just build-daemon` | 0 |
| Runtime controller | **1** |
| Initial offline tests: red → green | **1 → 0**, four tests |
| Evidence review guards: red → green | **1 → 0**, six tests |
| `just check-quick` | 0 |
| `just lint` | 0 |
| `just test-contracts` | 0 |
| Evidence/privacy/process audit | 0 |

[Gate commands/exits/tails](l4-resume-team/run3/gates/) retain validation evidence.
There is no `src-tauri/` diff, so `just test-rust-unit` is not required. The tests
use generated tempdirs/in-memory rows, never real credentials, CLIs or network.
Regression comments name `4f946ee5` (partial runtime/log filtering) and `f62bb158`
(refusal classification and unmetered subtotal ambiguity). Executed controller
and support are frozen by commit **`f62bb158`**; the later offline support guard
changes were **not rerun against a live seat**.

[Cleanup](l4-resume-team/run3/cleanup.json) and
[independent census/privacy audit](l4-resume-team/run3/final-audit.json) verify
**zero survivors**, closed private listener, removed scratch root/auth,
restored descriptor and removed trial binary. Fourteen owned PID/start-tick
identities were rechecked. The attempt-9 bubblewrap layout hid operator homes,
used a private PID namespace, tmux server and probed daemon port, removed `$TMUX`,
and exposed only scratch writable roots to runtime children. Only the explicitly
authorized auth file was copied at mode 0600; no operator config/history was
copied. No observer connected to an app-server socket. No fault injection,
product edits, install, release or plan-ledger edits occurred.

Deviations/limits: step 1’s product refusal requires steps 2–6 to remain NOT RUN;
alpha idle and marker receipts were not reached; failure cleanup’s busy-read
handling left one started turn unmetered, so the dollar cap cannot be certified;
the independent Opus evidence lens is unavailable in this session and remains
for the orchestrator. No numbered step passed, so no numbered green-step commit
is claimed. Preparation and the checked failure packet are separate commits.


## Run 4

Step 1 **PASS**: production initialize, both initial replies, completed transport
and explicit read receipts. alpha is attributed and idle. Four model inputs plus
two conservative start reservations (6/16); all four turns metered, API-equivalent
$0.005385760, conservative all-token upper estimate $0.079182000.

[Step 1 outcome](l4-resume-team/run4/step1-outcome.json),
[identities](l4-resume-team/run4/original-identities.json),
[activity](l4-resume-team/run4/alpha-activity.json),
[journal/state](l4-resume-team/run4/step1-state.json),
[ledger](l4-resume-team/run4/cost-ledger.json).

Steps 2–6 pending at this checkpoint. No resume call yet.
