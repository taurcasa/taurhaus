# INCOMPLETE — latest run 9, step 1 passed; remaining steps in progress

Run9 warmed the private Codex home, initialized both transports, and completed
one exchange/read per seat. See [Run 9](#run-9); this is a runtime checkpoint,
not a completed lane verdict.

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

**Verdict: FAIL at step 2; workflow incomplete. Classification: Taurhaus.**
Initialization and both marker exchanges passed, including the former run-3
`opt_in_delivery` boundary. All three supported `stop_session` RPCs then returned
`{"ok": true}` and their panes disappeared, but beta's daemon-owned app-server
remained alive throughout the **100-second** stop deadline. Exact assertion:

```text
supported stop_session left a recorded pane or Codex/app-server process alive
```

The survivor is the original hosted child: host PID **3161947**, namespace PID
**179**, start ticks **27753422**, matching beta's `appServer.processId/processStart`.
Only infrastructure pane `%0` remained; alpha's native process, beta's attached
TUI and the login-only lead had stopped. This is a supported-stop product boundary,
not a Mesh delivery refusal or a harness launch failure. Source inspection agrees:
`src-tauri/src/daemon/handlers.rs:1145` dispatches to
`src-tauri/src/session_scanner/control.rs:385`, which sends the CLI exit key and
tears down the pane, without stopping the separately owned host. No product fix
or alternative stop/remove/disband operation was attempted.

| Ordered audit step | Outcome | Classification and evidence |
|---|---|---|
| 1. Initialize, exchange/read one marker per seat, preserve identities/receipts | **PASS** | Runtime evidence: both replies once, alpha attributed idle, `submitted` / `native_enqueued` followed separately by `consumed_by_read`. |
| 2. Supported stop of every seat; inspect retained state | **FAIL** | **Taurhaus**: beta's original app-server survived 100 seconds. Team config, runtime and all 15 journal rows remained; task inventory empty. |
| 3. Accept a pending obligation per stopped seat | **NOT RUN** | Not evaluated: stop-on-failure. No pending markers created. |
| 4. One `resume_team`, poll its own status | **NOT RUN** | Not evaluated: zero resume calls; no historical skip substituted. |
| 5. Preserve identities/adapters, advance generations, recover pending work | **NOT RUN** | Not evaluated: no resumed generation. |
| 6. Explicit read, no replay/member executor, export/teardown | **NOT RUN** | Workflow assertions not reached; failure export/cleanup completed separately. |

Machine-readable [step 1](l4-resume-team/run4/step1-outcome.json),
[step 2](l4-resume-team/run4/step2-outcome.json),
[step 3](l4-resume-team/run4/step3-outcome.json),
[step 4](l4-resume-team/run4/step4-outcome.json),
[step 5](l4-resume-team/run4/step5-outcome.json),
[step 6](l4-resume-team/run4/step6-outcome.json).
The runtime controller exited **1**. Runtime including cleanup: **187.926 seconds**.

### Runtime identity and receipts

Taurhaus product base **`1db4f9bf`**, required fixes #161–#168, source protocol
**27**; runtime ping confirmed protocol **27**, daemon **0.9.7**. Mesh was built
only in `/home/mstie/projects/mesh-l4` at **`ed59187`**, with the scratch-only
0.153.4 descriptor `trial`/enabled and the verified daemon identities
`taurhaus-daemon-owned-thread/1`, `strict-config/1`, `daemon-owned/1`.
The wildcard remained disabled; no eligibility or release claim is made.
Both native Codex siblings were copied from the resolved installation; actual
version **0.153.4**, both seats **gpt-5.6-luna / low**.

Initialize run: `init_2b09dd05f2ae40ee877988612e40ad40`.
Team `l4-resume`, canonical format **2**, delivery owner **team**, incarnation
`be8866c5f8d3df1610162489660a6d564b77d4accbd5c43bebaa64a859254517`.
Exact builder retention policy retained in [policy.json](l4-resume-team/run4/policy.json).

| Seat | Adapter | Native session/thread | Pane | Attachment / context generation |
|---|---|---|---|---|
| alpha | tmux | `01a08c1b-57dc-7262-9bdf-781c0333c902` | `%2` | 1 / `"0"` |
| beta | app_server | `01a08c1b-5e7c-7aa2-a6fa-7c4ce4902e51` | `%3` | 1 / `"0"` |
| lead | Claude login-only | absent (no native model session) | `%1` | 1 / `"0"` |

All share retained private tmux session `$0`. beta's host generation was
`00dd369b-621d-4b0d-b993-337ed33c792b`. No lead model turn or credential was used.

| Seat | Marker | Message ID | Delivery ID | Transport / explicit read |
|---|---|---|---|---|
| alpha | `L4_old_alpha_974dd4` | `3f72b7a2-e879-4b70-b0a4-bd203202d84e` | `36685d19-b04d-43f6-b8ce-5a9e8400b035` | `submitted` / `consumed_by_read` |
| beta | `L4_old_beta_42e146` | `0104540f-79fb-4ede-9d1d-c8d624ba35c1` | `5df3ca04-e361-4f57-a050-e132244a6806` | `native_enqueued` / `consumed_by_read` |

Replies are independently present in the native transcripts; transport acceptance
alone was not treated as model action. Explicit reads ran as the named member
through the private Mesh CLI, each returned `done: true` on its first page.
No assignment IDs exist: this lane used message obligations, not tasks.

[Commands/RPCs and exits](l4-resume-team/run4/events.jsonl),
[every initialize status transition](l4-resume-team/run4/step1-operation.json),
[original identities](l4-resume-team/run4/original-identities.json),
[alpha attributed-idle snapshot](l4-resume-team/run4/alpha-activity.json),
[step-1 journal/state](l4-resume-team/run4/step1-state.json),
[stop-deadline process/pane observation](l4-resume-team/run4/step2-stop-poll.json),
[retained final state](l4-resume-team/run4/final-state.json),
[passive locks](l4-resume-team/run4/final-locks.json),
[complete sanitized daemon JSONL: 355 rows](l4-resume-team/run4/taurhaus.log.jsonl),
[daemon stderr](l4-resume-team/run4/daemon-stderr.json).
The `step1-pane*.json` and `final-pane0.json` captures are each at most 60 lines.
No observer connected to the hosted socket; host evidence came through the daemon.

### Every input and spend

| Seat / input | Turn ID | Input / cached input / output tokens | API-equivalent | Conservative |
|---|---|---:|---:|---:|
| alpha: onboarding | `01a08c1b-728c-7f53-b104-c73267729c74` | 20128 / 11776 / 202 | $0.002148320 | $0.024396000 |
| alpha: initial marker | `01a08c1b-9acc-7263-852d-b7af72589335` | 22901 / 19968 / 114 | $0.001122760 | $0.027618000 |
| beta: onboarding | `01a08c1b-66b5-7621-9309-69066ef18368` | 11203 / 6912 / 52 | $0.001058840 | $0.013506000 |
| beta: initial marker | `01a08c1b-b572-7c10-b7f1-c1c4dba776cd` | 11362 / 6912 / 23 | $0.001055840 | $0.013662000 |
| **Total: 4 completed inputs** | All metered | — | **$0.005385760** | **$0.079182000** |

Two Codex seat starts are additionally reserved: **6/16** conservative
starts-plus-inputs. There were no retries, recovery inputs, compaction, new backlog
turns or Claude model turns. Stops, read-only observations and cleanup added zero
inputs. The retained trial pricing basis is $0.20/$0.02/$1.20 per million
input/cached/output tokens; the conservative bound prices every token at $1.20/M.
These are token-based estimates, not an invoice or account-usage query. Both are
below **$0.25**. No missing/reset counter was called free usage.
[Ledger](l4-resume-team/run4/cost-ledger.json),
[spend reconciliation](l4-resume-team/run4/spend-reconciliation.json),
[native rows](l4-resume-team/run4/rollouts.json),
[deduplicated host events](l4-resume-team/run4/host-events.json).
Implementer/reviewer spend is separate orchestrator accounting.

### Controller, validation and cleanup

Executed controller/support commit **`2cb49f29`** reuses run 3 and attempt 9's
sandbox layout. Offline guards first failed with import errors (exit **1**) for
busy-read handling and cumulative turn metering, then **9 tests passed (0)**.
Regression comments name `e13eb5ff` (original run-3 commit `f62bb158`). These tests
use only generated tempdirs/in-memory records, never real credentials or CLIs.
[Offline red/green evidence](l4-resume-team/run4/offline-tests.json).

Exact commands from the assigned checkout root:

```sh
python3 -B docs/design/evidence/e2e/l4-resume-team/run4/build.py
python3 -B -m unittest discover -s docs/design/evidence/e2e/l4-resume-team/run4 -p test_support.py
python3 -B docs/design/evidence/e2e/l4-resume-team/run4/controller.py
python3 -B docs/design/evidence/e2e/l4-resume-team/run4/audit.py
python3 -B docs/design/evidence/e2e/l4-resume-team/run4/gates.py
```

Builds: `cargo build --bin mesh` in the designated Mesh worktree **0**;
`just build-daemon` **0**. The exact prebuild cargo probe observed no competing
Cargo process within the 30-minute bound. Checkout-local targets and temporary
Tauri resources were used. [Build commands/exits/digests](l4-resume-team/run4/build/),
[provenance](l4-resume-team/run4/provenance.json).

| Required gate (checkout root) | Exit | Evidence |
|---|---:|---|
| `just check-quick` | **0** | [Result](l4-resume-team/run4/gates/check-quick.json); 150 frontend files / 2518 tests passed |
| `just lint` | **0** | [Result](l4-resume-team/run4/gates/lint.json) |
| `just test-contracts` | **0** | [Result](l4-resume-team/run4/gates/test-contracts.json) |
| Final evidence/privacy/process audit | **0** | [Result](l4-resume-team/run4/final-audit.json) |

The gate runner exited **0**, waited all owned children, and removed the temporary
Tauri Mesh resource. `git diff --check` passed. Runtime assertion failure remains
**1**; passing repository gates do not change the step-2 verdict.
No `src-tauri/` diff exists, so `just test-rust-unit` is not required.

Cleanup stopped/waited only owned processes/private namespace, verified PID/start
ticks, closed the private listener and removed the scratch root/auth copy.
Independent census: **8 owned identities rechecked, zero survivors**, no privacy
violations. Mesh descriptor restoration exited **0**; the trial binary and its
matching Cargo executable hardlink were removed. All runtime roots were scratch,
`TMUX` absent, operator homes hidden by bubblewrap. Exactly the authorized auth
file was copied at mode 0600; no operator config/history/instructions were copied.
[Cleanup](l4-resume-team/run4/cleanup.json),
[independent audit](l4-resume-team/run4/final-audit.json).

Deviations/limits: step 2's product failure requires steps 3–6 to remain NOT RUN;
whole-team resume and post-resume replay/recovery remain unverified. The bounded
controller fixes preserve busy-read polling and cumulative spend; they do not
alter product behavior. All sanitized daemon rows are retained except the
explicitly prohibited account-usage rows. The Opus evidence lens remains for the
orchestrator (no Opus tool callable here). No product change, fault injection,
install/release, descriptor commit, plan-ledger edit or paid retry occurred.
Step 1 was committed before step 2; no passing step-2 commit is claimed.


## Run 5

**INCOMPLETE: steps 1–2 PASS; step 3 FAIL (harness); steps 4–6 NOT RUN.**
Runtime controller exited **1** after `alpha pending receipt missing while stopped`.
No product change or paid retry followed. This is not a successful whole-team
resume and does not reuse the suite's historical cold-resume skip as evidence.

| Step | Outcome / classification | Observed evidence |
|---|---|---|
| 1. Initialize, exchange/read | **PASS / runtime** | All nine initialize stages completed. Alpha attributed and idle; one unique reply per seat, alpha `submitted`, beta `native_enqueued`, then separate external `consumed_by_read` receipts. |
| 2. Stop all seats | **PASS / runtime** | Three supported `stop_session` calls returned `ok: true`; every seat pane and Codex/Claude process stopped, including beta's daemon-owned host. Config/runtime/journal retained. |
| 3. Backlog while stopped | **FAIL / harness** | Alpha message accepted; no presentation; health reported `pending: runtime session dead`. The inherited controller waited 100 seconds for a journal `stage: pending` receipt which this stopped path does not create. Beta send not reached. |
| 4. `resume_team` | **NOT RUN / not evaluated** | Stop after step 3; zero resume RPCs. |
| 5. Recovery/identity/delivery | **NOT RUN / not evaluated** | No resumed generation or recovery card claim. |
| 6. Read/no replay/executors | **NOT RUN / not evaluated** | No post-resume claim. |

[Step 1](l4-resume-team/run5/step1-outcome.json),
[step 2](l4-resume-team/run5/step2-outcome.json),
[step 3 raw outcome](l4-resume-team/run5/step3-outcome.json),
[steps 4](l4-resume-team/run5/step4-outcome.json),
[5](l4-resume-team/run5/step5-outcome.json),
[6](l4-resume-team/run5/step6-outcome.json),
[commands/RPCs/exits](l4-resume-team/run5/events.jsonl).

The raw generic controller classified step 3 as `taurhaus`. Evidence review
corrects that to **harness** without rewriting the raw result. At Mesh `310144d`,
`src/delivery/runtime.rs:171` returns the stopped-session pending refusal;
`runtime.rs:346` checks idle during resolution. The canonical scheduler resolves
at `src/delivery/scheduler.rs:443`, before constructing delivery receipts, and
`scheduler.rs:268` records pending refusals in health. The reused step-3 assertion
requires a journal receipt instead of that health evidence. This over-specific
assertion is not proof of a product defect. Recovery of the accepted obligation
is unverified because the binding stop-on-failure rule ended the run.
[Classification review](l4-resume-team/run5/classification.json),
[accepted alpha command](l4-resume-team/run5/pending-alpha-send.json),
[retained state/health/journal](l4-resume-team/run5/final-state.json).

S-runtime identities and observations:

- Taurhaus merged base **e4de06f9**, protocol **27**, build from this branch;
  executed controller/support commit **36b43bdd**. Mesh **310144d** detached in the
  designated separate worktree, with **no descriptor/source edit**.
- Both native Codex siblings were copied from the resolved installation; actual
  version **0.153.4**, model **gpt-5.6-luna**, effort **low**. The shipped
  `app_server` pin was observed `enabled: true` before initialize, with
  `taurhaus-daemon-owned-thread/1`, `strict-config/1`, `daemon-owned/1`.
  [Provenance](l4-resume-team/run5/provenance.json),
  [binary digests](l4-resume-team/run5/build/binaries.json),
  [capabilities](l4-resume-team/run5/delivery-capabilities.json).
- Team incarnation `b95fdaab78e4fcf91bef53f9ced75874b4edb6df185290eea3f48364ad4a64c5`;
  initialize run `init_5638ea736b614ef3b7bcb55824652fa0`;
  format **2**, delivery owner **team**, exact builder canonical policy.
- Alpha: tmux session **$0**, pane **%3**, native session
  `01a08d3a-f775-7763-992d-5da92c7bb7ca`, attachment **1**, context **0**.
  Beta: hosted, attached pane **%4**, session/thread
  `01a08d3a-fc2e-7130-935c-8c71c9a96224`, attachment **1**, context **0**.
  Lead: login-only pane **%2**, no native session ID or model turn.
  [Original identities](l4-resume-team/run5/original-identities.json),
  [step-1 activity](l4-resume-team/run5/step1-activity.json).
- Initial alpha message `b87b30de-a14b-4619-8ff1-8401cdd8fbf2`; initial beta
  `4157eba5-af58-4593-86b6-71f0c85ccc19`. Native replies and external explicit reads
  are separate claims; the controller did not ask the models to execute reads.
  [Markers](l4-resume-team/run5/markers.json),
  [step-1 journal/receipts](l4-resume-team/run5/step1-state.json).
- Step 2 retained only infrastructure pane **%0**, with no seat processes or
  runtime activity. Beta's stop advanced its retained attachment to **2**;
  `hosted.stop_session.host_stopped` names the original thread and
  `exit_status: signal: 9 (SIGKILL)` from the **supported product stop**.
  No crash/fault injection was performed. [Stop poll](l4-resume-team/run5/step2-stop-poll.json).
- Stopped alpha obligation `7fd086e9-98ed-4801-99e7-68e2344e9736`, delivery
  `6ec16d63-d15d-427c-904d-3056c6dc80b0`, journal sequence **16**.
  `accepted` / `projection: pending` do not prove a delivery receipt or presentation.
  No native reply, submission or native-enqueued record exists for it.
- Passive lock samples and pane captures (at most 60 lines each) accompany each
  snapshot. The **complete 344-row sanitized daemon JSONL** is retained, with only
  prohibited account-usage rows/fields removed; repeated diagnostic rows remain.
  [Daemon JSONL](l4-resume-team/run5/taurhaus.log.jsonl),
  [native rollouts](l4-resume-team/run5/rollouts.json),
  [host events](l4-resume-team/run5/host-events.json).

Every observed spend (USD; rates inherited from the trial packet, not an invoice):

| Native turn | Seat/input | API-equivalent | Conservative all tokens at $1.20/M |
|---|---|---:|---:|
| `01a08d3b-0271-7683-8d16-4e3776a9fbb1` | beta onboarding | 0.001029840 | 0.013110000 |
| `01a08d3b-0e58-7c10-a2dc-e0f4f4dbf18e` | alpha onboarding | 0.002276320 | 0.024534000 |
| `01a08d3b-369b-7740-9f51-d7804f5609ff` | alpha old marker | 0.001166160 | 0.027878400 |
| `01a08d3b-4d30-7ea2-b62c-cb3c75b09639` | beta old marker | 0.001166440 | 0.014379600 |
| **Total** | **4 turns** | **0.005638760** | **0.079902000** |

Zero unmetered turns. The controller additionally reserves both seat starts,
counting **6 ≤ 16**; the stopped alpha message caused **no paid input**. Step 2,
step 3 and teardown added zero observed spend. The fresh **USD 0.25** cap remained
verified by the retained conservative bound; earlier runs' spend is historical.
Runtime **201.199 seconds ≤ 900**. [Ledger](l4-resume-team/run5/cost-ledger.json),
[reconciliation](l4-resume-team/run5/spend-reconciliation.json).

Offline harness TDD: four added busy-refusal guards failed first (exit **1**;
missing retry helper / unhandled `lock busy`), then all **13** passed (exit **0**).
The reused six preflight accounting/selection tests also passed (exit **0**).
Only in-memory data and generated tempdirs are used by these tests, with no real
CLI/auth access. [Red](l4-resume-team/run5/red.txt),
[green](l4-resume-team/run5/green.txt).

Exact runtime/build/gate entry points from the assigned checkout root:

```sh
python3 -B docs/design/evidence/e2e/l4-resume-team/run5/build.py
python3 -B -m unittest discover -s docs/design/evidence/e2e/l4-resume-team/run5 -p test_support.py
python3 -B docs/design/evidence/e2e/l4-resume-team/run5/controller.py
python3 -B docs/design/evidence/e2e/l4-resume-team/run5/audit.py
python3 -B docs/design/evidence/e2e/l4-resume-team/run5/gates.py
```

Both `cargo build --bin mesh` (designated Mesh worktree) and `just build-daemon`
exited **0**. Builds/gates use checkout-local target directories and one Cargo
build job, with the exact machine-wide `pgrep` probe before each command. Waits
apply only at three or more existing Cargo processes, at 30-second intervals,
up to 30 minutes. Tauri resource placeholders were prepared using the recipe.
All required gates passed from the checkout root:

| Exact gate | Exit | Evidence |
|---|---:|---|
| `just check-quick` | **0** | [Result](l4-resume-team/run5/gates/check-quick.json); 150 frontend files / 2519 tests passed |
| `just lint` | **0** | [Result](l4-resume-team/run5/gates/lint.json) |
| `just test-contracts` | **0** | [Result](l4-resume-team/run5/gates/test-contracts.json) |
| Process/privacy audit | **0** | [Result](l4-resume-team/run5/final-audit.json) |
| `git diff --check` | **0** | No whitespace errors |

The gate runner exited **0** and waited for its owned children. No `src-tauri/`
diff exists, so `just test-rust-unit` is not required. Passing gates do not change
the failed runtime assertion or establish resume success. Steps 1 and 2 each had
their own green evidence commit before the next step; step 3 has a failure/cleanup
commit, with no passing commit claimed for steps 3–6.

Cleanup completed (exit **0**): seven owned PID/start-tick identities rechecked,
**zero survivors**, private port closed, scratch root/auth removed, Mesh source
unchanged. The shipped Mesh binary is retained in its build target; no trial
patch exists to restore. [Cleanup](l4-resume-team/run5/cleanup.json),
[independent process/privacy audit](l4-resume-team/run5/final-audit.json).

Deviations/limits: the referenced attempt-9 worktree did not exist (git exit 128),
so the existing run-4 sandbox/poll controller was reused. Its overly specific
step-3 assertion stopped this run; no corrective paid retry or product edit was
made. The raw classification is preserved and the evidence-based correction is
explicit above. The independent Opus evidence lens remains for the invoking
orchestrator; no Opus tool is callable here. Steps 4–6 and beta's stopped backlog
remain unverified. No installation/release, plan-ledger edit, descriptor mutation,
account/root move, unrelated CLI, stress test, or operator-process kill occurred.


## Run 6

**INCOMPLETE.** One live run on taurhaus `a7e6db7e` (#172 over #171), Mesh
`310144d`, protocol **27**, actual Codex **0.153.4**, **gpt-5.6-luna / low**.
The unchanged shipped `app_server` descriptor was verified enabled before
initialize. The lead remained login-only; no Claude model turn was requested.
[Provenance](l4-resume-team/run6/provenance.json),
[binary digests](l4-resume-team/run6/build/binaries.json),
[capabilities](l4-resume-team/run6/delivery-capabilities.json).

| Ordered step | Outcome | Evidence / classification |
|---|---|---|
| 1. Initialize, exchange/read one marker per seat | **PASS** | Runtime: one native reply per marker, tmux `submitted` / hosted `native_enqueued`, explicit `consumed_by_read`; alpha attributed and idle. [Outcome](l4-resume-team/run6/step1-outcome.json), [state](l4-resume-team/run6/step1-state.json), [activity](l4-resume-team/run6/step1-activity.json). |
| 2. Stop every seat without disband/remove | **PASS** | Runtime: supported `stop_session` ended all recorded seat panes and every `codex*`/Claude process, including the hosted child and native sibling. Team/journal state retained. [Outcome](l4-resume-team/run6/step2-outcome.json), [census](l4-resume-team/run6/step2-stop-poll.json). |
| 3. Accept one stopped obligation per seat | **PASS** | Runtime: accepted + projection `pending`, no presentation receipt; alpha health `pending: runtime session dead`, beta equivalent `pending: native_host_not_live`. Both polled with a 100-second deadline. [Alpha](l4-resume-team/run6/step3-alpha-backlog.json), [beta](l4-resume-team/run6/step3-beta-backlog.json). |
| 4. Call `resume_team` once; poll its own status | **PASS** | Runtime operation: all three members resumed; `failed_members: []`, `started_team_daemon: true`, no team-daemon warning. No initialize/remove/re-add substituted. [Outcome](l4-resume-team/run6/step4-outcome.json), [all status samples](l4-resume-team/run6/step4-operation.json). |
| 5. Identity/recovery/backlog assertions | **FAIL — harness** | `unmetered reset token counter` escaped from the inherited observer while waiting for the step-5 checkpoint release, before its assertion body. Separately, retained alpha identity contradicts the required same-session recovery (Taurhaus). [Outcome](l4-resume-team/run6/step5-outcome.json), [traceback](l4-resume-team/run6/controller-error.json), [analysis](l4-resume-team/run6/runtime-analysis.json). |
| 6. Explicit reads, no replay/executors, journal export | **NOT RUN** | Stopped after the step-5 observer failure. Final snapshots/cleanup are partial evidence, not the ordered step-6 checks. [Outcome](l4-resume-team/run6/step6-outcome.json). |

Initialize run `init_f62f5095a3d240f09fa8fd873e2b1bab`; resume run
`team-resume_f264479fe086459f862fe0e43c7f53a8`; daemon logging run
`run_b62d064cd91e409a99c718dec0fab89d`. Team incarnation
`5663698652764ea16f167254f8528fc7f9d4f56f5d190a1810b5a2fcac7d232b` and
private tmux session `$0` were retained. Alpha stayed tmux; beta stayed app_server.

| Seat | Original identity | Post-resume identity | Attachment / pane |
|---|---|---|---|
| alpha | `01a08d54-dc9c-79b0-bad6-5a6ea04f55aa` | **`01a08d56-c982-7c71-abd9-c9c7450aed07` — changed** | 1 → 2; %3 → %14 |
| beta | `01a08d54-e121-7d31-bd6a-d6e72d498dbf` | Same recorded thread | 1 → 2 at stop → 3 at resume; %4 → %15 |
| lead | Login-only, no session ID | Login-only, no session ID | 1 → 2; %2 → %13 |

Beta received a new owned host generation and an attach command resuming its
recorded thread. Alpha’s production launch command was fresh, without `resume`.
The ordinary tmux path in `prepare_resume` retains `resume_session_id` only for
an effort override; the separate hosted/rollback branch preserves it regardless
(`src-tauri/src/coordination/pipelines/members.rs:615–655`). This is an observed
Taurhaus identity deviation, independent of the meter failure; no product fix
was attempted. The completed step-4 RPC alone does not establish step-5 identity.
[Original identities](l4-resume-team/run6/original-identities.json),
[post-resume state](l4-resume-team/run6/step4-state.json),
[launch commands and receipt index](l4-resume-team/run6/runtime-analysis.json).

The final journal retains these distinctions:

- Initial alpha `e238a30b-5169-4f8b-829b-dbf3ff498ca9`: accepted seq 8,
  `submitted` seq 10, explicit read seq 11. Initial beta
  `59d1a1d5-a19c-4752-8bd2-6390358c68e4`: accepted seq 12,
  `native_enqueued` seq 14, explicit read seq 15.
- Backlog alpha `c2bc23e0-39ab-4881-a3d4-c92a2141571b`: accepted seq 16
  while stopped; generation-2 `submitted` seq 24, model’s explicit read seq 28,
  pending marker reply visible in the final pane.
- Backlog beta `00fc2149-8f8d-4f1b-b1f7-5eecb683de82`: accepted seq 17
  while stopped; resumed pending receipts seq 27/31 then generation-3
  `native_enqueued` seq 33. This proves transport acceptance, not a completed
  reply or explicit read. The receipt identifies an additional turn missed by
  the last controller meter sample.

The bounded final census contains no member delivery executor and the original
messages have no additional transport receipt in the retained journal. These
partial observations do **not** replace step 6’s explicit reads and cursor-paged
export. Recovery cards were recorded, but the one-native-card-per-generation
checks were not executed. [Final state/journal](l4-resume-team/run6/final-state.json),
[final alpha pane](l4-resume-team/run6/final-pane14.json),
[final beta pane](l4-resume-team/run6/final-pane15.json).

Every evidenced turn and observed spend (USD; inherited trial rates, not an invoice):

| Turn | Seat/input | API-equivalent observed | All tokens at $1.20/M | Completeness |
|---|---|---:|---:|---|
| `01a08d54-e765-7b60-bcd7-1414d2c54648` | beta onboarding | 0.001000640 | 0.013090800 | Complete |
| `01a08d54-f434-79a0-b5c9-5023bc854a36` | alpha onboarding | 0.001477440 | 0.024482400 | Complete |
| `01a08d55-1d1b-7c31-bd16-beed89397d08` | alpha initial marker | 0.001143960 | 0.027769200 | Complete |
| `01a08d55-33a4-7403-bbc0-598151dd6506` | beta initial marker | 0.000608480 | 0.014361600 | Complete |
| `01a08d56-d1dd-73b2-aa3e-52ced71f7e44` | resumed alpha backlog/read/recovery | 0.001989000 | 0.011292000 | Partial; last completion/counters not exported |
| `01a08d56-d5c7-7332-a3ef-765ba8611c34` | beta recovery | **Unknown** | **Unknown** | Started, no retained usage |
| `01a08d56-e9b7-7f02-8000-81bb3d5a40bc` | beta backlog | **Unknown** | **Unknown** | Native start proved by seq 33; absent from last meter sample |
| **Known subtotal only** | | **0.006219520** | **0.090996000** | **Final total and $0.25 cap unverified** |

Steps 1–3 completed at $0.00423052 metered / $0.079704 conservative, four turns;
steps 2–3 added no paid input. Step 4 initiated recovery/delivery, and its
checkpoint had two still-unmetered starts. Seven turn IDs are ultimately
evidenced, rather than the last meter’s six. Four conservative seat-start
reservations plus the pre-readiness hosted retry make **12 ≤ 16** evidenced
turns/starts/retry. The retry’s SQLite initialization exit preceded any model
turn; it was the product’s single supported launch retry, not a controller retry.
Runtime was **136.782 seconds ≤ 900**. No fresh input or second run followed failure.
[Last raw meter](l4-resume-team/run6/cost-ledger.json),
[checkpoint meter](l4-resume-team/run6/step4-cost-ledger.json),
[final spend audit](l4-resume-team/run6/final-spend-audit.json).

The raw reconciliation retains its inherited conservative-subtotal field naming;
the final spend audit explicitly separates API-equivalent and all-output-rate
subtotals. The reset assertion ran before exporting its offending token row and
also aborted cleanup’s usage drain. That row and final usage cannot be recovered
from the deleted scratch root. Missing usage is never zero.

Offline TDD first failed with **1 assertion failure + 4 missing-helper errors**
(exit **1**) for the stopped-backlog/census/headroom changes; after correction,
**19 tests passed** (exit **0**), including the controller’s both-seat health
polling path. The **six reused preflight tests passed** (exit **0**), explicitly
retained for the run6 ruling. Tests use generated tempdirs/in-memory stubs only;
no real CLI, network, auth or operator harness-home access.
[Red](l4-resume-team/run6/red.txt), [green](l4-resume-team/run6/green.txt),
[preflight output](l4-resume-team/run6/preflight.txt).

Commands run from the assigned checkout (the build helper runs Mesh Cargo only
in `/home/mstie/projects/mesh-l4`):

```sh
python3 -B docs/design/evidence/e2e/l4-resume-team/run6/build.py
python3 -B -m unittest discover -s docs/design/evidence/e2e/l4-resume-team/run6 -p test_support.py
python3 -B -m unittest discover -s docs/design/evidence/e2e/l4-resume-team -p test_preflight.py
python3 -B docs/design/evidence/e2e/l4-resume-team/run6/controller.py
python3 -B docs/design/evidence/e2e/l4-resume-team/run6/audit.py
python3 -B docs/design/evidence/e2e/l4-resume-team/run6/gates.py
```

Mesh `cargo build --bin mesh`, `just ensure-tauri-resources`, and
`just build-daemon` all exited **0**. Cargo used one build job per command and
checkout-local targets, with the exact `pgrep -af '(^|/)cargo( |$)'` probe before
builds/gates, waiting only at ≥3 existing Cargo processes (30-second polls,
30-minute limit). No shared target, descriptor edit or Mesh commit.

| Exact gate / check | Exit | Evidence |
|---|---:|---|
| `just check-quick` | **0** | [Result](l4-resume-team/run6/gates/check-quick.json) |
| `just lint` | **0** | [Result](l4-resume-team/run6/gates/lint.json) |
| `just test-contracts` | **0** | [Result](l4-resume-team/run6/gates/test-contracts.json) |
| Runtime controller | **1** | [Failure/last logs](l4-resume-team/run6/execution-result.json) |
| Process/privacy audit | **0** | [Result](l4-resume-team/run6/final-audit.json) |

No `src-tauri/` diff, so `just test-rust-unit` is not required. Passing gates do
not change the runtime verdict. Steps 1–3 were committed before releasing the
next step; step 4 has its own green commit, made after the observer’s automatic
failure/cleanup before the step-5 release could be sent.

Cleanup/export completed: **15 owned PID/start-tick identities rechecked, zero
survivors**, private port closed, scratch root and auth copy removed, Mesh source
unchanged. The **complete 366-row sanitized daemon JSONL** is retained, including
repeated diagnostics; prohibited usage records/fields are omitted. Pane captures
stay ≤60 lines; scratch signed-read receipt capabilities and usage notices were
redacted. [Cleanup](l4-resume-team/run6/cleanup.json),
[daemon JSONL](l4-resume-team/run6/taurhaus.log.jsonl),
[redactions](l4-resume-team/run6/redaction-audit.json).

Deviations/limits: the attempt-9 reference checkout is absent (git exit **128**),
so run5’s existing private namespace/tmux/root layout was reused as directed.
The preflight headroom check was corrected to use metered API-equivalent spend
plus $0.05 per prospective input, preserving the conservative comparison ledger;
otherwise the inherited all-output-rate reserve would refuse step 4 despite low
metered spend. The remaining inherited counter-reset assertion aborted the
observer and cleanup drain; steps 5–6 and complete metering therefore remain
incomplete. The raw failed row was not retained. Alpha’s changed identity is a
separate observed Taurhaus deviation. The independent Opus evidence lens remains
for the invoking orchestrator: no Opus/Workflow tool is callable here. No product
fix, paid rerun, installation/release, plan-ledger edit, descriptor mutation,
account/root move, stress/fault injection or operator-process kill occurred.


## Run 7

**INCOMPLETE, S-runtime attempted.** Fresh run from step 1 on unchanged Taurhaus
`a7e6db7e` + Mesh `310144d`; no prior run's runtime proof was reused. The branch
was already rebased onto `origin/main`. Both native Codex 0.153.4 siblings were
copied into the private bin; the shipped app-server descriptor remained enabled
and unmodified. Model `gpt-5.6-luna`, effort `low`; login-only Claude lead used no
paid input. The executed controller/support commit is `63841f81`, with digests,
binaries and setup in [provenance](l4-resume-team/run7/provenance.json),
[build results](l4-resume-team/run7/build/binaries.json) and
[capabilities](l4-resume-team/run7/delivery-capabilities.json).

| Step | Outcome / classification | Evidence |
|---|---|---|
| 1. Initialize, exchange and explicitly read both initial markers | **PASS / runtime** | [Outcome](l4-resume-team/run7/step1-outcome.json), [identities](l4-resume-team/run7/original-identities.json), [state/journal](l4-resume-team/run7/step1-state.json) |
| 2. Supported stop for all seats, including daemon-owned host | **PASS / runtime** | [Outcome](l4-resume-team/run7/step2-outcome.json), [empty seat census, including codex-code-mode-host](l4-resume-team/run7/step2-stop-poll.json) |
| 3. Both obligations accepted while stopped, with no transport receipt and stopped-member health refusal | **PASS / runtime** | [Alpha backlog](l4-resume-team/run7/step3-alpha-backlog.json), [beta backlog](l4-resume-team/run7/step3-beta-backlog.json) |
| 4. One whole-team resume and runtime export | **FAIL / harness**; the RPC itself completed successfully | [Status polling](l4-resume-team/run7/step4-operation.json), [corrected classification](l4-resume-team/run7/step4-outcome.json), [traceback](l4-resume-team/run7/controller-error.json) |
| 5. Verify recovery identity/cards and both backlog deliveries | **NOT RUN / not evaluated** | [Outcome](l4-resume-team/run7/step5-outcome.json) |
| 6. Explicit reads, no replay and executor reconciliation | **NOT RUN / not evaluated** | [Outcome](l4-resume-team/run7/step6-outcome.json) |

The resume report has `failed_members: []` and `started_team_daemon: true`.
The subsequent snapshot hit `'NoneType' object has no attribute 'startswith'`
in the controller's `clean()`: the new embedded-JSON redaction parsed a record
whose `method` was null, then applied a string operation. The raw event's generic
classifier incorrectly called this Taurhaus; the outcome sidecar explicitly
corrects it to **harness**, preserving the original event and classification.
Step4/final snapshots were not saved and cannot be recovered from the removed
scratch root. No replacement snapshot is inferred from logs.

A separate **Taurhaus observation** remains: alpha's new rollout is
`01a08d71-1657-7b43-bf91-6a1a2f19faa0`, versus original
`01a08d70-0b37-7012-a4cd-4cdedefb676a`. Its launch has no `resume`, while beta
uses `resume '01a08d70-11f0-7b92-894a-1b4ecd71c1c4'`. The ledger observed
attachment generations alpha=2 and beta=3. The ordinary tmux path in
`src-tauri/src/coordination/pipelines/members.rs:617–660` does not set the resume
session ID. This is an independent identity deviation, not a substitute for
executing step 5. [Machine-readable analysis](l4-resume-team/run7/runtime-analysis.json).

### Metering and budget

Cleanup **continued through the real resumed counter reset**. Beta's cumulative
input/output changed from 22,769/69 to 12,644/39 in the same rollout; the observer
recorded one `counter_epoch_reset` diagnostic and accumulated a new epoch instead
of throwing. Every observed turn has usage and completion, with no unmetered IDs.
[Final ledger](l4-resume-team/run7/cost-ledger.json),
[spend audit](l4-resume-team/run7/final-spend-audit.json),
[diagnostic](l4-resume-team/run7/runtime-analysis.json).

| Turn | Input | API-equivalent USD | All tokens at output rate USD |
|---|---|---:|---:|
| `01a08d70-1769-7303-a916-8380394ed148` | beta onboarding | 0.000991440 | 0.013071600 |
| `01a08d70-23b6-7602-ae85-e4745dc0541f` | alpha onboarding | 0.001837680 | 0.024474000 |
| `01a08d70-4c03-71f0-9e58-f990396eceb3` | alpha initial marker | 0.001352280 | 0.027763200 |
| `01a08d70-62cb-7fe3-883f-88f141c88e05` | beta initial marker | 0.000603880 | 0.014334000 |
| `01a08d71-1e33-7b21-b30e-ed3447969cb1` | resumed alpha recovery | 0.001331240 | 0.024385200 |
| `01a08d71-258f-7e72-b2d0-c23ad79a9b31` | resumed beta recovery | 0.000594160 | 0.015219600 |
| **Total** | **6 observed completed turns** | **0.006710680** | **0.119247600** |

Steps 1–3 each ended at four metered turns / $0.00478528; stopped backlogs
added no paid turn. Six turns plus four conservative seat-start reservations
are **10 ≤ 16**; no hosted launch retry was observed. Final observed metered spend
is **$0.00671068 ≤ $0.25**. Runtime was **79.181 seconds ≤ 900**.
The API-equivalent rates are the inherited trial rates ($0.20/$0.02/$1.20 per
million input/cached/output tokens), not a billing invoice. Automatic cleanup
observation submitted no input; no paid controller retry followed the failure.

### Review fixes and validation

All five supplied findings were verified and addressed. Counter epochs now retain
prior subtotals; repeated observation deduplicates diagnostics. One shared
`require_headroom(ledger, inputs, basis='api_equivalent_usd')` returns a refusal
before the next paid input; observation records cap facts without throwing.
Runtime completion and metering completeness are separate, and cleanup retries
observation errors until its deadline. Reconciliation names both cost bases.
Run6's step5 JSON now includes its separate Taurhaus identity observation.
Run6's privacy audit scans Python sources with an explicit authorized auth-source
exception, recorded in [its audit](l4-resume-team/run6/final-audit.json).
Run6's historical executed controller digests still identify its original code;
the files now contain the reviewed implementation used/adapted for run7.

Initial offline red: **5 failures + 3 errors**, including divergent headroom,
missing epoch support, observer cap abort and privacy coverage. A direct replay
against the old support also produced `AssertionError: unmetered reset token
counter`. Green: **25 tests**, plus **6 shared preflight tests**. The new redaction
failure was reproduced after the paid run with **1 failure + 1 error**; the null
method handling and classifier fix then passed **27 tests**. These final two
fixes are offline-only and were not followed by another paid run. All regression
tests use generated temporary data/in-memory stubs, without real harness CLI,
credentials or network access.
[Initial red](l4-resume-team/run7/red.txt), [green](l4-resume-team/run7/green.txt),
[preflight](l4-resume-team/run7/preflight.txt),
[redaction red](l4-resume-team/run7/red-redaction.txt),
[final green](l4-resume-team/run7/green-redaction.txt).

| Gate/check | Exit | Evidence |
|---|---:|---|
| Mesh `cargo build --bin mesh` | **0** | [Result](l4-resume-team/run7/build/mesh.json) |
| `just ensure-tauri-resources` / `just build-daemon` | **0 / 0** | [Resource preparation](l4-resume-team/run7/build/resources.json), [daemon](l4-resume-team/run7/build/daemon.json) |
| `just check-quick` | **0** | [Result](l4-resume-team/run7/gates/check-quick.json) |
| `just lint` | **0** | [Result](l4-resume-team/run7/gates/lint.json) |
| `just test-contracts` | **0** | [Result](l4-resume-team/run7/gates/test-contracts.json) |
| Runtime controller | **1** | [Result and last logs](l4-resume-team/run7/execution-result.json) |
| Process/privacy audit | **0** | [Result](l4-resume-team/run7/final-audit.json) |

Gates ran from this checkout root via `run7/gates.py`, using scratch harness
homes and this checkout's own `src-tauri/target`. Before each build/gate the exact
`pgrep -af '(^|/)cargo( |$)'` probe admitted work only below three Cargo processes;
30-second polls/30-minute deadline were configured, with no wait needed.
One build job per Cargo command. No `src-tauri/` diff; the conditional
`just test-rust-unit` requirement does not apply.

Teardown rechecked **15 owned PID/start-tick identities, zero survivors**, closed
the private port, removed the scratch root/auth copy, and verified unchanged
Mesh source. The **complete 310-row sanitized daemon JSONL** is retained;
prohibited account-usage rows/fields are excluded, and signed read cursors are
redacted. [Cleanup](l4-resume-team/run7/cleanup.json),
[daemon log](l4-resume-team/run7/taurhaus.log.jsonl).

Deviations: the added JSON redaction caused a new step-4 harness failure, fixed
red-first offline; steps 5–6 remain unexecuted under the binding stop-on-failure
rule. Alpha's separate identity deviation is retained. The unavailable attempt9
reference/layout deviation remains as recorded in provenance; run5's existing
private layout was reused. The supplied Opus review drove this fix round; a new
independent Opus review of run7 remains with the orchestrator (no callable Opus
runner here). Each passing runtime step was committed before the next release.
No product change, install/release, plan-ledger edit, descriptor mutation, Mesh
commit, account/root move, fault/stress injection or operator-process kill.


## Run 8

**INCOMPLETE; step 1 FAIL / harness (native Codex startup).** The single production
initialize operation reported completed, but alpha’s native 0.153.4 TUI exited
without a session identity. Its pane retained this error:

```text
failed to migrate queue DB ... queue_1.sqlite:
while executing migration 1: error returned from database:
(code: 1) table queued_items already exists
```

The controller then exhausted its **100-second** attributed-idle observation
window with `alpha attribution/idle missing`. The automatic observer initially
classified that boundary as Taurhaus; the final classification is **harness**
based on the native startup error. The separate product observation—initialize
reported completed despite the exited alpha TUI—is retained, not silently
corrected or used to claim a successful initialization.
Initialize launches the tmux seat and hosted app-server concurrently against
the fresh scratch CODEX_HOME; PR #164 retries this SQLite startup race on the
hosted launch path, while the tmux launch path has no equivalent child-startup
retry (its retry covers sending keys only), so this packet classifies the
observed native failure as a harness flake and flags the retry asymmetry for
product follow-up without claiming a proven product defect or a guaranteed
successful retry.
[Pane/error](l4-resume-team/run8/final-pane3.json),
[operation status](l4-resume-team/run8/step1-operation.json),
[observer outcome](l4-resume-team/run8/step1-outcome.json),
[classification adjudication](l4-resume-team/run8/step1-adjudication.json),
[analysis](l4-resume-team/run8/runtime-analysis.json).

| Step | Outcome / classification | Evidence |
|---|---|---|
| 1. Initialize, exchange/read one marker per seat, retain identities/receipts | **FAIL / harness** (adjudicated; observer: taurhaus): alpha native startup failed before markers; beta completed onboarding | [Outcome](l4-resume-team/run8/step1-outcome.json), [adjudication](l4-resume-team/run8/step1-adjudication.json), [final state](l4-resume-team/run8/final-state.json) |
| 2. Stop every seat through supported session-stop | **NOT RUN / not evaluated**: binding stop after step 1 | [Outcome](l4-resume-team/run8/step2-outcome.json) |
| 3. Accept both stopped-seat obligations; prove pending via acceptance, no transport receipt and member health refusal | **NOT RUN / not evaluated** | [Outcome](l4-resume-team/run8/step3-outcome.json) |
| 4. One whole-team resume and status polling | **NOT RUN / not evaluated**: no resume RPC | [Outcome](l4-resume-team/run8/step4-outcome.json) |
| 5. Resume/rebind identities, new generations, one recovery card and exactly one backlog delivery each | **NOT RUN / not evaluated** | [Outcome](l4-resume-team/run8/step5-outcome.json) |
| 6. Explicit reads, no replay, journal/transport/executor reconciliation | **NOT RUN / not evaluated**; failure teardown/export completed separately | [Outcome](l4-resume-team/run8/step6-outcome.json) |

### Candidate, isolation and retained evidence

Product base **106f06c7** (PRs #172, #174 and #175), verified as ancestor without
switching this branch. Mesh **f8ff76e**, detached and built only in the designated
Mesh worktree; shipped app-server descriptor **enabled**, unchanged. The private
runtime reported **protocol 27**, actual **Codex 0.153.4**; both native Codex
siblings were copied into the scratch bin. Model **gpt-5.6-luna**, effort **low**.
[Provenance and executed-source hashes](l4-resume-team/run8/provenance.json),
[binary digests](l4-resume-team/run8/build/binaries.json),
[capabilities](l4-resume-team/run8/delivery-capabilities.json),
[version](l4-resume-team/run8/codex-version.json),
[commands/RPCs](l4-resume-team/run8/events.jsonl).

Initialize ID `init_e1e510f472fe4665ac911b7057d203d1`; team incarnation
`dbdd271ec1ce606eb3b657698c47546d55690ec52b35af65bf4556e0a2c40dce`.
Alpha: tmux pane `%3`, generation 1, no session ID. Beta: hosted pane `%4`,
generation 1, thread `01a08dd2-4077-75d2-8e6f-5c3307b683d3`.
The lead remained login-only with zero model turns. No baseline marker,
backlog message, explicit read, `stop_session`, or `resume_team` was issued.

The controller reused run 7’s private PID namespace, private tmux infrastructure,
scratch roots, canonical policy, and command-capable instructions. Only the
explicit disposable credential source’s `auth.json` was copied, mode 0600, with
no credential fallback. No observer connected to the app-server socket. The
complete **268-row sanitized daemon JSONL** is retained; prohibited account usage
rows/fields and secrets are excluded. Pane excerpts contain at most 60 lines.
[Bootstrap](l4-resume-team/run8/bootstrap.json), [policy](l4-resume-team/run8/policy.json),
[daemon log](l4-resume-team/run8/taurhaus.log.jsonl),
[host events](l4-resume-team/run8/host-events.json),
[rollouts](l4-resume-team/run8/rollouts.json),
[activity](l4-resume-team/run8/final-activity.json),
[passive locks](l4-resume-team/run8/final-locks.json).

### Every input and spend

| Turn ID | Work | Input / cached / output tokens | Metered USD | All tokens at output rate USD |
|---|---|---:|---:|---:|
| `01a08dd2-46ba-7c01-8f1c-d4dc6cde0b2c` | beta onboarding, completed | 11219 / 6912 / 55 | 0.001065640 | 0.013528800 |
| **Total** | **1 observed input; none unmetered** | | **0.001065640** | **0.013528800** |

One observed input plus two conservative seat-start reservations = **3 ≤ 16**.
Alpha and Claude took zero observed model turns. Metered **$0.001065640 ≤ $0.25**;
runtime **113.427 seconds ≤ 900**. The rates are the inherited trial rates
($0.20/$0.02/$1.20 per million uncached input/cached input/output), an
API-equivalent meter, not an invoice. Runs 1–7 are excluded from this fresh budget.
No paid retry or additional authorization was requested.
[Ledger](l4-resume-team/run8/cost-ledger.json),
[reconciliation](l4-resume-team/run8/spend-reconciliation.json),
[spend audit](l4-resume-team/run8/final-spend-audit.json).

### Offline red → green, gates and teardown

Before the paid run, the two exact faulty historical observer functions were
restored in the copied controller solely for the offline red: reset accounting
from `95d31ae3` (original `c008cc2e`) and embedded JSON redaction from `5f102e6c`.
The new generated-record tests observed **1 failure + 1 error**: `unmetered reset
token counter` and `NoneType ... startswith`. Reusing run 7’s committed fixes
then passed: reset counters open a metering epoch and preserve prior spend;
null fields stay absent/null and nested JSON remains exportable. No earlier
runtime evidence/source was edited. The additional run-8 identity test first
failed on the missing checker, then passed the recorded resume/new-rollout/
notify-idle criterion, including negative controls. Final **30 tests PASS**
(three new tests, 27 reused guards), plus **6 preflight tests PASS**. All offline
tests use generated records/temp files and mocks, without live CLI or auth access.
[Observer red](l4-resume-team/run8/red.txt),
[identity red](l4-resume-team/run8/identity-red.txt),
[green](l4-resume-team/run8/green.txt),
[preflight](l4-resume-team/run8/preflight.txt).

The corrected step-3 predicate and step-5 assertions are present and offline
checked, but **not live-proven by this run**. In particular, run 8 supplies no
live confirmation of the #172/#174 resume fix or of backlog/recovery delivery.

| Gate/check | Exit | Evidence |
|---|---:|---|
| `Mesh build` | **0** | [Result](l4-resume-team/run8/build/mesh.json) |
| `Resource preparation` | **0** | [Result](l4-resume-team/run8/build/resources.json) |
| `Daemon build` | **0** | [Result](l4-resume-team/run8/build/daemon.json) |
| `just check-quick` | **0** | [Result](l4-resume-team/run8/gates/check-quick.json) |
| `just lint` | **0** | [Result](l4-resume-team/run8/gates/lint.json) |
| `just test-contracts` | **0** | [Result](l4-resume-team/run8/gates/test-contracts.json) |
| Runtime controller | **1** | [Exit and last logs](l4-resume-team/run8/execution-result.json) |
| Process/privacy audit | **0** | [Result](l4-resume-team/run8/final-audit.json) |

All required gates run from this checkout root **after teardown**, using scratch
harness homes and this checkout’s own `src-tauri/target`. Cargo admission uses
`pgrep -af '(^|/)cargo( |$)'`, 30-second polls, at most 30 minutes, admitting work
only below three existing Cargo processes; each Cargo command uses one build
job. No `src-tauri/` diff, so conditional `just test-rust-unit` is not applicable.
[Gate runner](l4-resume-team/run8/gates.py).

Teardown and the independent process/privacy census rechecked **13 owned
PID/start-tick identities**, found **zero survivors**, verified the private port
closed, removed the scratch root and credential copy, and confirmed unchanged
Mesh source. [Cleanup](l4-resume-team/run8/cleanup.json),
[audit](l4-resume-team/run8/final-audit.json).

Deviations: steps 2–6 were not run because step 1 failed, as the binding stop
rule requires. The specified attempt-9 reference worktree does not exist
(`git show` exit 128); run 7’s existing sandbox layout was reused as directed.
The historical observer functions were replayed offline because run 7’s final
commit already contains both fixes. The additional read-only private alpha pane
capture confirms the automatic `final-pane3.json` capture; its nonblank lines
are identical and add no evidence. The supplied independent Opus evidence lens
drives the offline fix round below; approval remains with the orchestrator.
No product edit, descriptor mutation,
Mesh commit, install/release, plan-ledger edit, account/root move, crash/stress
injection, or operator-process kill occurred.

### Run 8 review fix round

All six supplied findings were verified. The generated step-5 test invokes the
controller with numeric card contexts and string runtime `contextGeneration`,
matching the Rust serialization contract. Before the fix it failed with
`alpha recovery receipt generation mismatch`; four classifier subcases also
failed because `cap` matched `capabilities`/`capture` or a Mesh refusal lost to
the harness branch. Converting both runtime generations to integers and giving
explicit Mesh refusals precedence with specific cap phrases passed **32 offline
tests**. Negative controls still reject mismatched generations and duplicate
cards for both alpha and beta. Regression comments name introducing commit
`4a4c65d8`. Command: `python3 -B -m unittest discover -s
docs/design/evidence/e2e/l4-resume-team/run8 -v` (exit **0**; the preceding
`-p test_run8.py` red run exited **1**, five assertion failures).

The step-1 outcome's original four fields and exact `save()` formatting were
reconstructed from the unchanged controller failure event after the earlier
manual edit in `543236ef`; the later harness interpretation now lives only in
the separate `step1-adjudication.json` sidecar rather than extra outcome fields;
that sidecar references the outcome and the automatic pane
capture. Historical events, executed-source hashes and cleanup evidence remain
unchanged. The controller now omits the redundant post-deletion `auth_removed`
flag; `root_removed` already covers deletion of the scratch credential copy.

This fix round launches no paid seats and adds **0 inputs / $0 spend**. The
original run remains step 1 **FAIL / harness** (adjudicated), steps 2–6 **NOT RUN /
not evaluated**, with **$0.001065640** total metered spend. The controller's
one-run guard and binding stop rule preclude rerunning run8 or claiming live
resume coverage from these offline fixes.

Review-round gates ran from this checkout root after the original teardown:
`just check-quick` **0** (2,521 frontend tests), `just lint` **0**, and
`just test-contracts` **0**. The existing `run8/gates.py` runner was reused with
only its output destination redirected to `.check-logs/run8-review/`, preserving
the original run's gate artifacts. Each Cargo admission probe saw one existing
Cargo process, so no wait was required; each command used one build job and
this checkout's own `src-tauri/target`. Scratch harness homes were removed when
the runner exited **0** and its child commands had finished. No Rust diff:
`just test-rust-unit` remains inapplicable. Evidence consistency verification
also exited **0**, comparing the reconstructed outcome bytes against `save()`
and the unchanged failure event. `git diff --check` passed.

## Run 9

Step 1 **PASS / runtime**: initialize `init_5c000c2110b945aa8598a860ae5e2463`
completed; alpha is attributed idle, and both initial markers have one reply,
a transport receipt and explicit read. Four metered turns total **$0.004748080**;
seven inputs including conservative start reservations (one warm-up).

The [warm-up](l4-resume-team/run9/warmup.json) saw the composer, exited 0,
left valid SQLite databases and no Codex survivor. No startup model turn was
observed; any startup cost is unknown, not reported as free. The first batched
quit left `/quit` in the composer during MCP startup; one recorded Enter
completed that same quit without a new TUI or model input.

[Controller guards](l4-resume-team/run9/green.txt): 38 pass after observed red;
[preflight](l4-resume-team/run9/preflight.txt): six pass. Builds exit 0:
[Mesh](l4-resume-team/run9/build/mesh.json),
[daemon](l4-resume-team/run9/build/daemon.json).
[Provenance and hashes](l4-resume-team/run9/provenance.json) pin product
`106f06c7`, Mesh `1f7447f`, protocol 27, Codex 0.153.4, Luna/low.

Later step outcomes and post-teardown gates will be appended after execution.

Step 2 **PASS / runtime**: supported `stop_session` ended all three seat panes
and every Codex process (including the native sibling and hosted child). Only
the private infrastructure pane remains. Retained team/journal state exists;
beta attachment advanced to 2. No additional spend.

Step 3 **PASS / runtime**: one pending obligation accepted for each stopped
seat; both have no submitted/consumed/native-enqueued receipt and the matching
member health refusal. See `step3-alpha-backlog.json` and
`step3-beta-backlog.json`. No presentation while stopped; no additional spend.

Step 4 **PASS / runtime**: the single `resume_team` operation
`team-resume_4adb1fcec0784e92844f1f4d869b1bb1` completed, resumed all three
members, and started the team daemon without failed members or a daemon warning.
Recovery/pending turns are settling; final spend reconciliation follows teardown.
