# UNAVAILABLE — Run 4d: steps 1–4 PASS; step 5 harness metering stop before resume

## Historical run 2 — FAIL at step 1: idle Codex prompt, onboarding permanently pending

**Classification: Taurhaus product defect.** Canonical initialization succeeds,
but alpha never receives its onboarding card during a **90.2-second** positive
poll. Its real Codex 0.153.4 pane shows a ready **gpt-5.6-luna low** prompt;
production activity remains `uncertain`, and Mesh reports
`IO error: delivery: pending: activity not freshly idle`. There is **no hosted
seat** and no crossed hosted identity: alpha's `session_id` and `jsonl_path`
remain null. The deeper cause is unproved. No product change or paid retry was
made. [Outcome](l2-tmux-busy/run/step1-outcome.json),
[final pane](l2-tmux-busy/run/final-pane-2.txt),
[runtime](l2-tmux-busy/run/final-runtime.json),
[activity](l2-tmux-busy/run/final-activity.json),
[Mesh pending reason](l2-tmux-busy/run/team/state/delivery/health-alpha.json).

| Ordered audit step | Outcome | Classification and evidence |
| --- | --- | --- |
| 1. Initialize; capture terminal/runtime identity | **FAIL** | Taurhaus. Initialize completed, required attachment facts and lock inode were recorded, but onboarding stayed accepted/undelivered and the real seat never became freshly idle. |
| 2. Bounded response; observe working; send Q | **NOT RUN** | Blocked by step 1. No ordinary input or Q was submitted. |
| 3. Q pending → fresh idle → one submission/reply | **NOT RUN** | No busy deferral, idle-delivery age or reply claim. |
| 4. Second response; Q2 pending; managed stop; passive lock evidence | **NOT RUN** | No Q2 or stop_session call. Startup lock samples do not establish stop/delivery exclusion. |
| 5. Resume once; new generation; Q2 once; no Q replay | **NOT RUN** | No resume_member call or generation-safety claim. |
| 6. Explicit read/mark; reconcile; export and teardown | **NOT RUN** | Message/read reconciliation not reached. Mandatory failure teardown independently **passed**. |

The controller exited **1** at the first failure. Runtime lasted approximately
**93.4 seconds**, below the 12-minute cap. **120 seconds is snapshot freshness,
not a wait period**; the 90-second poll is a finite scheduler-opportunity deadline.
[Controller exit](l2-tmux-busy/run/controller-exit.json).

## Candidate and isolation

- Checkout `/home/mstie/projects/taurhaus-l2-tmux-busy`, branch
  `feat/e2e-l2-tmux-busy`; product source `6f61f6117a75625ce4ec6d325879730f1800c473`.
  Exact controller at execution: `5a010958` (built on the previous three preflight
  commits). Actual private daemon ping reports **protocol 27**, version 0.9.7.
- Mesh `/home/mstie/projects/mesh-l2`, detached
  `a6ee29681a44a472c0ce7b33b9630d4865c93012`; checkout-local debug binary.
  No descriptor or source edit; hosted descriptor remains disabled.
- Disposable root `/tmp/th-l2-enwdjc07`; private daemon port **45371**, probed
  before launch. Scratch HOME and every harness/config/data/temp root; inherited
  TMUX absent. Bubblewrap hides `/home`, `/tmp`, `/run` and gives children a private
  PID namespace, private tmux server, scratch project and pinned binary copies.
  Mesh is used from the scratch HOME's `.local/bin` only.
- Standing operator authorization was honored: **exactly auth.json** copied into
  an otherwise empty CODEX_HOME at **0600**. No source fallback, credential contents
  or installation IDs exported. The copy was deleted at teardown.
- Production `coordination.initialize_team` used the builder's actual canonical
  retention policy, lead=`claude`/tmux and alpha=`codex`/tmux. Team is format **2**,
  `delivery_owner: team`; no member delivery daemon and no app-server seat.
  Lead remained at Claude's first-run theme/login setup, with **zero model turns**.
- Scratch [AGENTS.md](l2-tmux-busy/run/scratch-AGENTS.md) permits the specified Mesh
  lifecycle/read commands and forbids file changes. The managed Codex policy is
  `codex --yolo` inside the isolated namespace. No fault injection, process freeze,
  forged activity, forced contention, installation, release or plan-ledger edit.

[Builds and Cargo queue observations](l2-tmux-busy/live-build.json),
[ping](l2-tmux-busy/run/ping.json),
[canonical config](l2-tmux-busy/run/team/config.json),
[initialize result](l2-tmux-busy/run/step1-operation.json),
[commands/RPCs/digests](l2-tmux-busy/run/events.jsonl).

| Actual executable | SHA-256 |
| --- | --- |
| Taurhaus release daemon | `0fdde7d6b7b88126c83c4d9adac950e47b2c195a65dcca4ab60a64b4cdd19811` |
| Mesh RC debug | `81ed8accaa2197827ce9b7c77a93449b2f18ea0467feed848343b933712921d9` |
| Codex 0.153.4 | `56ef98ab4032d317ab26e9b5e5a175650717351edb16ed9cde0cb6d1734d62da` |

## Runtime and transport evidence

| Fact | Observed value |
| --- | --- |
| Team incarnation | `22cb580d00c7c335b10eb8d224da01c9d79c2a1708c8c67bec751348df72bd74` |
| Root authority revision | `0` |
| Terminal contract / attachment / context generation | `1` / `1` / `"0"` |
| Socket | `/tmp/th-l2-enwdjc07/tmux/tmux-1000/default` |
| Session / pane / namespace PID / start ticks | `$0` / `%2` / `136` / `24956166` |
| Terminal lock inode | `1798976` |
| Alpha onboarding message | `8ed9b206-5ebd-4310-aa88-bfe1ce709f5c` |
| Alpha journal delivery ID | `024e6ca4-9cde-4860-90e2-cbfb24e705fc` |
| Recovery delivery identity | `1b1648ab42eb4970ba7fa556571a3b128d26ea86322ee1c533235ba88b4ccce2` |

The alpha recovery claim has `stage: accepted`, `offered_bytes: 0`,
`returned_by_read_bytes: 0`, and journal projection `pending`. Acceptance is
**not** terminal submission, explicit read or model uptake. The bounded journal
contains the lead and alpha startup acceptances; no alpha submission/reply was
observed. The final activity row at `2026-09-10T08:31:47.271945624+00:00` says
`pane_alive: true`, `pane_foreign: false`, `active_non_shell_process: true`,
`recent_io: false`, `activity_confidence: uncertain`.
[Journal rows](l2-tmux-busy/run/team/state/messaging-v2/segments/000001.jsonl).

No rollout session file or notify event was created. Generated Codex filenames
include thread-writer lock and shell snapshot identity
`01a08a70-8595-7ea0-a004-4ae0385d4e2f`; Taurhaus never bound it to alpha.
Only relevant filenames/sizes are retained; caches/database contents were not
exported. [Session inventory](l2-tmux-busy/run/codex-session-inventory.json),
[empty notify export](l2-tmux-busy/run/codex-notify.json).

The passive sampler caught **two startup samples with FLOCK holders**, then no
holder, on inode `1798976`. Host PID `3275043`, start ticks `24956104`, owned the
WRITE flock; a managed terminal child inherited it. This establishes startup
lock use only. It does **not** establish the unrun managed-stop contender.
[Kernel fdinfo samples](l2-tmux-busy/run/terminal-locks.jsonl),
[filtered daemon events](l2-tmux-busy/run/daemon-events.json).
No lock was acquired by the observer and no product process was paused/signalled
until normal shutdown during teardown.

## Cost ledger and cleanup

| Spend category | Count | Observed model spend USD |
| --- | --- | --- |
| Codex alpha seat starts, including onboarding reservation | **1 of 10 cap units** | **0**: no submitted onboarding or model turn observed |
| Codex model turns / ordinary inputs / recovery / retries | **0 / 0 / 0 / 0** | **0** |
| Claude lead model turns | **0** | **0** |
| Total observed API-equivalent model spend | No token usage rows | **0 of 0.20** |
| Implementer / reviewer | Separately budgeted by orchestrator | Unavailable to lane |

Turn ID ledger is empty. This is an observed no-model-input result, not an
account invoice lookup. No account usage rows were read/exported. Startup counts
against the input cap even though its queued card never caused a model turn.
[Cost ledger](l2-tmux-busy/run/cost-ledger.json).

Finally invoked the owned daemon's SIGINT shutdown handler, waited for its private
PID namespace to exit and reaped controller-owned children. **No surviving
scratch daemon, tmux server or Codex process; port closed; auth removed; scratch
root removed.** No foreign process was killed. [Cleanup](l2-tmux-busy/run/cleanup.json),
[independent post-run audit](l2-tmux-busy/final-audit.json).

## Exact controller, tests, gates and limitations

Run only from this checkout:

```sh
# SOURCE denotes exactly the file explicitly authorized by the operator.
python3 -B docs/design/evidence/e2e/l2-tmux-busy/controller.py --auth-source "$SOURCE"
python3 -B -m unittest discover -s docs/design/evidence/e2e/l2-tmux-busy -p '*_test.py'
just check-quick
just lint
just test-contracts
```

[Controller](l2-tmux-busy/controller.py) is unchanged from the executed commit
`5a010958`. Its **steps 2–6 branches are unexecuted** and grant no coverage.
[Executed support helper](l2-tmux-busy/support-executed.py) preserves the precise
original implementation. [Offline export script](l2-tmux-busy/pack.py) reframes
parsed journal arrays as complete JSONL rows, narrows the generated-file inventory,
and deduplicates identical files. [Aliases](l2-tmux-busy/export-manifest.json)
retain each original snapshot name. All pane files are at most 60 lines; no
collector size limit aborted execution. No journal CLI read was made, so no
cursor-page traversal is claimed.

Red-first evidence:

- Explicit-source authorization test: **exit 1**, unsupported keyword
  `authorized_source`; after the narrow override, six preflight tests passed.
  The `// Regression:` comment names `70d3a95d`.
- New offline controller tests: **exit 1**, missing `support` module; five passed
  after implementation (partial JSONL, multi-session cost sum, unknown notify-only
  turns, message-scoped receipts, sensitive-field/path filtering).
- One post-run collector correction: **exit 1**, nested scratch `/home` path
  incorrectly scrubbed. After narrowing the matcher, **12 tests passed, exit 0**.
  The regression comment names `5a010958`; no runtime retry or product edit.

**Collector limitation:** original exports also scrubbed the `/home` component
inside scratch executable paths, leaving some argv entries as
`/tmp/th-l2-enwdjc07<operator-path-redacted>`. Those bytes were not reconstructed.
Exact intended commands and binary roles remain in the executed controller and
binary digests; preserved runtime/activity fields, pending reasons and pane
captures independently establish the failure. Current support fixes future
exports only. [Red](l2-tmux-busy/sanitizer-red.txt),
[green](l2-tmux-busy/sanitizer-green.txt).

| Required command | Exit | Result |
| --- | --- | --- |
| `just check-quick` | **0** | Rust test compilation, typecheck; 150 frontend files / 2,495 tests pass |
| `just lint` | **0** | Rust, frontend, workflow and recipe lint pass |
| `just test-contracts` | **0** | Renderer, harness and module-boundary contracts pass |

[Exact gate exits/tails](l2-tmux-busy/live-gates.json). Gates ran during the live
poll; the subsequent offline sanitizer correction passed its targeted test suite.
No `src-tauri/` diff, so the conditional Rust-unit execution gate does not apply.
Both requested builds exited **0** after bounded Cargo-queue waits.

Deviations: the operator's explicit credential authorization supersedes the old
preflight restriction; no unauthorized fallback was used. Failure at step 1
mandated stopping before steps 2–6, so there is no green numbered runtime commit.
The controller's scratch-path redaction limits some command evidence as described
above. The required independent **Opus lens is unavailable** in this session
(no callable Opus reviewer; the login-only Claude lead must not take model turns),
so no review approval or lane PASS is claimed. Earlier preflight/gate packets
remain historical; this real runtime result supersedes their unavailable verdict.


## Historical run 3 — FAIL at step 1

**Classification: Taurhaus readiness failure; product cause provisional.** The #163 base resolves alpha's
session identity to `01a08b57-d623-7511-b516-86d018112f06`, but it does **not**
establish attributed fresh idle readiness in this real trial. At a ready
**Codex 0.153.4 / gpt-5.6-luna low** prompt, the production member activity
snapshot reports `active`, `likely_working` and `uncertain`; it never reports `idle`. The final
runtime-session row has `state: idle`, `activity_attribution: none`,
`activity_confidence: low`, `jsonl_path: null`. Mesh keeps onboarding
`pending: activity not freshly idle`. No `launch_ready` event was observed.
Session identity and activity attribution are separate facts; this is not the
run-2 null-session-identity failure relabeled as unchanged.

The one initialization completed. Its subsequent **90-second** positive poll
failed; total runtime was **96.16 seconds**, exit **1**. **120 seconds is the
idle-snapshot freshness bound, not a waiting period.** No retry, direct input,
product patch, hosted seat or descriptor edit was made.
[Outcome](l2-tmux-busy/run3/run/step1-outcome.json),
[ready native pane](l2-tmux-busy/run3/run/final-pane-2.txt),
[member activity](l2-tmux-busy/run3/run/final-activity.json),
[runtime-session attribution](l2-tmux-busy/run3/run/final-runtime-sessions.json),
[pending reason](l2-tmux-busy/run3/run/team/state/delivery/health-alpha.json).

| Ordered step | Run 3 outcome | Classification / evidence |
| --- | --- | --- |
| 1. Initialize; terminal identity; attributed idle and card delivery | **FAIL** | **Taurhaus readiness failure (provisional product attribution)**. Terminal facts and session ID exist; attributed idle and submitted onboarding do not. |
| 2. Bounded response; working snapshot; Q | **NOT RUN** | Blocked by step 1; no ordinary input or Q. |
| 3. Q pending → fresh idle → one submission/reply | **NOT RUN** | No busy-window or delivery claim. |
| 4. Bounded response; Q2 pending; managed stop and lock | **NOT RUN** | No Q2, managed stop, or lifecycle-exclusion claim. |
| 5. Resume once; new attachment; Q2 once; no Q replay | **NOT RUN** | No resume or generation-safety claim. |
| 6. Explicit read/mark; reconcile; export and teardown | **NOT RUN** | No journal read or reconciliation. Mandatory failure export and teardown independently **PASS**. |

### Run 3 candidate, isolation and S-runtime packet

The controller ran at `551dbc8d495fd73706c598276a2f8a6f7aa8e10a`, containing
merged main `6398bfa3` and #163, on the unchanged branch
`feat/e2e-l2-tmux-busy`. The daemon was built with **`just build-daemon`** into
this checkout's `src-tauri/target`; ping confirms **protocol 27**, version 0.9.7.
Mesh was verified and built only in `/home/mstie/projects/mesh-l2`, detached at
`fcb9647f46deef7c298df472f5550bf1c59e182f`, with **`cargo build --bin mesh`** and
its checkout-local target. Both builds exited **0**, after recorded, bounded
Cargo queue polls. No install, release or external-worktree mutation occurred.
[Builds](l2-tmux-busy/run3/builds.json),
[candidate](l2-tmux-busy/run3/run/candidate.json),
[ping](l2-tmux-busy/run3/run/ping.json).

| Copied executable | SHA-256 |
| --- | --- |
| Taurhaus daemon | `7093375e20e07ea1f2a752b116365ee2b39de13f03896f94357fff9a49a2e9cf` |
| Mesh RC | `c761ae4e6742567f0e787bbc2e0cf281946c125a422b78307fb5186dfb90e9dd` |
| Codex 0.153.4 | `56ef98ab4032d317ab26e9b5e5a175650717351edb16ed9cde0cb6d1734d62da` |
| codex-code-mode-host | `3e85d67471825f73d02ff5f7e047ca1f6ca8caa3f59e4c6e8d9ca6ca7302cb45` |

Both native Codex siblings were resolved from the real installation and copied
before launch. Runtime HOME, all harness/config/data/temp roots, the project,
and tmux socket lived under `/tmp/th-l2-6_217q4o`. Bubblewrap supplied a private
PID namespace and hid `/home`, `/tmp`, and `/run`; inherited TMUX was absent.
The private daemon used probed port **45321**, never 17233. Mesh ran solely from
the scratch HOME's `.local/bin`. Exactly the authorized `auth.json` was copied
into otherwise-empty CODEX_HOME, mode **0600**, with no fallback or export.
The copy was removed. The real login-only Claude lead took **zero model turns**.

Production `coordination.initialize_team` used the builder's actual canonical
`messaging` retention block and creation-time `delivery: tmux` for both seats.
The resulting team is canonical format 2, `delivery_owner: team`; alpha has no
member delivery daemon or hosted attachment. The scratch command-capable
[AGENTS.md](l2-tmux-busy/run3/run/scratch-AGENTS.md) permits the required Mesh
commands; no model executed them in this failed run.
[Exact commands/RPCs](l2-tmux-busy/run3/run/events.jsonl),
[initialize outcome](l2-tmux-busy/run3/run/step1-operation.json),
[team config](l2-tmux-busy/run3/run/team/config.json).

| Attachment / delivery fact | Run 3 value |
| --- | --- |
| Team incarnation | `af70d14d6fb971a827d7b40ada064954bc3f4edfce2d9e1d0b7523c6ad0d0f2f` |
| Root / authority revision | scratch `claude` / `0` |
| Terminal contract / attachment / context generation | `1` / `1` / `"0"` |
| Socket / session / pane | `/tmp/th-l2-6_217q4o/tmux/tmux-1000/default` / `$0` / `%2` |
| Pane PID / start ticks | `138` / `26472058` (private namespace) |
| Codex PID / session ID | `141` / `01a08b57-d623-7511-b516-86d018112f06` |
| Alpha onboarding message ID | `f89b7346-8500-4c44-a66e-b8d3e4e1b897` |
| Journal delivery ID | `825a7504-7366-401a-b612-85fdd4086afd` |

[Runtime record](l2-tmux-busy/run3/run/final-runtime.json) includes the full
launch root, recovery claim and generations. The onboarding claim remains
`accepted`, journal projection `pending`, with **zero offered bytes** and
**zero returned-by-read bytes**. Neither acceptance nor the recovery record's
`last_delivered` field establishes terminal submission or model uptake.
[Full journal](l2-tmux-busy/run3/run/team/state/messaging-v2/segments/000001.jsonl).
No rollout or notify event was created. The generated session evidence consists
of matching thread-writer-lock and shell-snapshot filenames; contents and caches
were not exported. [Inventory](l2-tmux-busy/run3/run/codex-session-inventory.json).

The **complete daemon JSONL, all 245 rows**, is retained through shutdown,
without event selection or tail clipping. It records 23 activity state changes,
with `process_io` transitions to active and `none` transitions to idle, and no
`launch_ready` observation. The pane also shows
`Update available! 0.153.4 -> 0.154.0`: Codex performed non-turn background work. `launch_ready` requires a quiet `rchar` window in
`src-tauri/src/session_scanner/idle/codex_readiness.rs`; background Codex I/O is
therefore an **unexcluded candidate cause** of the repeated active/idle changes
and failure to accrue that window. The log does not prove that update checking
caused those reads. The observed readiness failure is firm; attribution to a
Taurhaus product defect remains provisional, with this harness confounder
unresolved. No product fix is proposed here.
[Complete daemon log](l2-tmux-busy/run3/run/taurhaus.log.jsonl),
[stderr](l2-tmux-busy/run3/run/daemon-stderr.txt).

Passive `/proc/*/fdinfo` sampling caught **one startup FLOCK-holder sample**;
[lock inode](l2-tmux-busy/run3/run/step1-terminal-lock.json) and
[holder samples](l2-tmux-busy/run3/run/terminal-locks.jsonl) are retained.
That proves startup lock use only. Step 4 was not reached, so it supplies no
managed-stop exclusion evidence. No lock was acquired by an observer; no process
was paused, frozen, signalled as an experiment, or subjected to load/stress.
All pane captures are at most 60 lines. Identical files were deduplicated with
[explicit aliases](l2-tmux-busy/run3/export-manifest.json); distinct daemon and
journal rows remain intact. Offline pane exports remove trailing blank lines
only, resolving the initial whitespace-check failure; the exact captures remain
in the command log. The review added [pack.py](l2-tmux-busy/run3/pack.py), which
replays the manifest against those captures, checks byte identity before pane
deduplication, and verifies all five retained exports and three aliases.
Its default mode is read-only; `--write` reproduces the declared exports.
No evidence-size cap aborted the step.

### Run 3 spend, cleanup, tests and gates

| Spend category | Run 3 count | Observed spend USD |
| --- | --- | --- |
| Codex seat start / onboarding reservation | **1 of 10 cap units** | **0**; queued card never submitted |
| Codex turn IDs / ordinary inputs / recovery / retries | **0 / 0 / 0 / 0** | **0** |
| Claude model turns | **0** | **0** |
| Total observed API-equivalent / conservative model spend | No token-usage events | **0 / 0**, within **$0.20** |
| Implementer / independent reviewer | Orchestrator budget, outside seat cap | Not exposed to this lane |

[Cost ledger](l2-tmux-busy/run3/run/cost-ledger.json) has an empty turn ledger.
The zero is supported by no submitted alpha card, no ordinary input, no rollout
and no notify event; it is not an inference from reset usage counters or an
account-usage lookup. No real account usage row or credential was exported.

Finally, the controller used the owned daemon's normal SIGINT shutdown handler,
waited for namespace exit and reaped its children. **No surviving scratch daemon,
tmux server, Mesh or Codex process; port closed; auth removed; scratch root
removed.** It killed no foreign process. Teardown is independently verified by
[audit exit 0](l2-tmux-busy/run3/final-audit.json) and the
[cleanup record](l2-tmux-busy/run3/run/cleanup.json).

The original six synthetic offline controller checks passed. Their initial red
records showed missing imports, not failed behavioral assertions. The review fix
round now runs **17 tests**, including run3-local credential preflight tests.
A deliberate offline `ready_session` stub returning the first row unconditionally
also produced an assertion-level red: the mismatched `%3` pane was not rejected.
This is a retrospective mutation check, not an original pre-implementation run.
The original readiness implementation is unchanged and passes the same test.
Regression comments
name the earlier controller commit `8e8f1287` for the partial runtime, incomplete
JSONL retention and pre-claim deferral assumptions. New readiness checks reject
missing session IDs, stale idle, mismatched panes, un-attributed rows and degraded
scanner snapshots. Tests use synthetic values/tempdirs, launch no CLI and never
inspect real harness homes. The unchanged product regression tests from #163 were
not relabeled as lane tests.
[Tests](l2-tmux-busy/run3/controller_test.py),
[red](l2-tmux-busy/run3/red.txt),
[pending red](l2-tmux-busy/run3/pending-red.txt),
[attribution red](l2-tmux-busy/run3/attribution-red.txt),
[green](l2-tmux-busy/run3/green.txt).

| Required exact gate (checkout root) | Exit | Result |
| --- | --- | --- |
| `just check-quick` | **0** | Rust tests compile; frontend typecheck and **2,509 tests** pass |
| `just lint` | **0** | Rust, frontend, workflow and recipe lint pass |
| `just test-contracts` | **0** | Renderer, harness conformance and module-boundary tests pass |

[Exact gate exits, timings and final log lines](l2-tmux-busy/run3/checks-result.json).
The offline 17-test suite, packaging replay and cleanup/export audit also exit **0**.
No `src-tauri/` diff exists, so the conditional `just test-rust-unit` gate does
not apply. No full `just check` was run.

Deviations / remaining review: the operator's explicit one-file authentication
authorization overrides the shared contract's generic source restriction.
Step 1 failure mandates stopping before steps 2–6, leaving **no green numbered
runtime commit**. The run-3 controller's remaining-step branches are unexecuted
and make no coverage claim. The original execution had no callable Opus reviewer.
The operator subsequently
supplied the independent Opus round-1 findings addressed below; no additional
reviewer was launched, and the scratch Claude lead was not used as one. The
historical audit's unavailable-review field describes the original execution.
No workflow PASS or post-fix review approval is claimed.


### Run 3 reproduction and review fix round

Run these offline checks from this checkout root. The subshell ensures discovery
loads **run3's** support and preflight modules, rather than the frozen run-2 copy:

```sh
(cd docs/design/evidence/e2e/l2-tmux-busy/run3 && python3 -B -m unittest discover -s . -p '*_test.py')
python3 -B docs/design/evidence/e2e/l2-tmux-busy/run3/pack.py
python3 -B docs/design/evidence/e2e/l2-tmux-busy/run3/audit.py
python3 -B docs/design/evidence/e2e/l2-tmux-busy/run3/gates.py
```

For reference, the live run's build/controller sequence is below. `SOURCE` must
be supplied privately as the operator's explicitly authorized one-file source;
never echo it or credential contents. These are live trial commands, **not part
of the offline review rerun**. The controller deliberately refuses an existing
`run/`; preserve this packet and use a separately allocated evidence directory
and budget for any subsequent trial. Build queue polling belongs to `build.py`.

```sh
python3 -B docs/design/evidence/e2e/l2-tmux-busy/run3/build.py
python3 -B docs/design/evidence/e2e/l2-tmux-busy/run3/controller.py --auth-source "$SOURCE"
```

The review confirmed all supplied findings. The live credential call now passes
a pinned authorization constant independently of the candidate. A red test
against `57c8ff36` reached the mocked copy for each of four forbidden harness-home
paths; the corrected call refuses them and still accepts the exact authorized
file with all credential metadata/copy operations mocked. The local preflight
suite covers the frozen run3 copy; historical run-2 files remain unchanged.

Step-1 waits now assign Taurhaus ownership only when their specific assertion
times out; RPC/probe exceptions keep the default harness classification.
The original `step1-outcome.json` is preserved as recorded, with its product
classification qualified in this report. The dead gate queue wait and unused
freshness-age calculation were removed. Send and step-6 read now share a JSON
parser that accepts a banner followed by compact or multiline JSON. Its regression
first failed with `JSONDecodeError` against the old send implementation.
The packaging regression first failed because `pack` was absent, then passed
with the replay implementation; the real packet replay also exits 0.

All three exact gates were rerun for this fix round and exited **0**;
`checks-result.json` now records these latest timings and log excerpts.
This fix round ran only synthetic tests, packet checks and the exact three gates;
no paid trial was repeated, no real harness was invoked, and no credential was
read or copied. **Additional seat starts: 0; Codex/Claude inputs: 0/0; additional
seat spend: $0.00.** Historical run 3 remains step 1 FAIL and steps 2–6 NOT RUN.
Implementer/reviewer spend is not exposed here and remains outside the seat cap.

## Run 4 — third attempt UNAVAILABLE at step 3 (earlier attempts retained below)

Latest: [run 4c](#run-4c--confirmed-busy-input-stopped-at-step-3-latest-attempt)
passed steps 1–2 and stopped at step 3 on a harness timeout. The first two
attempts below are historical evidence.

### First attempt — historical result

**Classification: harness evidence failure; no Taurhaus or Mesh product failure
established in this run.** The frozen run-3 controller exited **1** after
**94.000 seconds**, with the raw message `alpha onboarding not delivered/completed
with fresh idle activity` and classification `taurhaus`. That composite predicate
misidentifies the evidence: alpha is attributed and freshly idle, and the card
was submitted, explicitly read by Codex, and answered. The predicate demands
card text in a **user** row, whereas Mesh submits an inbox notification and Codex
retrieves the card in a **tool result**. Separately, a notify-only turn from a
different session has no retained usage accounting. Its cost is **unknown**, so
no further input was permitted. The raw outcome is preserved; the replayable
[diagnosis](l2-tmux-busy/run4/diagnosis.json) qualifies it without a runtime retry.

| Ordered audit step | Outcome | Classification / evidence |
| --- | --- | --- |
| 1. Initialize and record runtime/terminal identity | **FAIL / incomplete** | **Harness.** Required runtime facts, attribution, fresh idle, one onboarding submission and one explicit read are observed. Controller's user-card predicate is false and metering is incomplete. [Raw outcome](l2-tmux-busy/run4/run/step1-outcome.json), [runtime snapshot](l2-tmux-busy/run4/run/final-runtime-sessions.json), [journal](l2-tmux-busy/run4/run/team/state/messaging-v2/segments/000001.jsonl). |
| 2. Ordinary bounded response; working; Q | **NOT RUN** | Blocked by step 1; no ordinary response or Q input. |
| 3. Q pending → fresh idle → one submission/reply | **NOT RUN** | No measured Q busy-deferral coverage. |
| 4. Second response; Q2 pending; managed stop and lock sample | **NOT RUN** | No Q2 or `stop_session`; startup locks do not prove this contender. |
| 5. Resume once; new generation; Q2 once; no Q replay | **NOT RUN** | No `coordination.resume_member`; no generation-safety claim. |
| 6. Explicit Q2 read; reconcile; export/teardown | **NOT RUN** | Q2 reconciliation not reached. Mandatory failure export and cleanup independently passed. |

### Candidate and observed delivery

The checkout stayed on `feat/e2e-l2-tmux-busy`; product source matches
**`1db4f9bf`**, with no `src/` or `src-tauri/` diff. The executed controller/support
are frozen in **`f60ae885`**. Run 4 copies run 3's controller, adding candidate-pin
preflight only; the later support edit removes account quota fields during export.
Mesh was checked out detached at **`ed591876fdbe9832d18ab7992ce7583c80bc8c64`**
in `/home/mstie/projects/mesh-l2`, built locally, and used only from scratch
`HOME/.local/bin`; source and descriptors were unchanged. Both native Codex
siblings were copied. Actual Codex is **0.153.4**, **gpt-5.6-luna low**; actual
private daemon ping is **protocol 27**, version 0.9.7.
[Builds/queue probes](l2-tmux-busy/run4/builds.json),
[binary digests and exact sanitized commands](l2-tmux-busy/run4/run/events.jsonl),
[ping](l2-tmux-busy/run4/run/ping.json).

| Binary | SHA-256 |
| --- | --- |
| Taurhaus daemon | `3b4c6e3937466064687eac9f3685b33ac6acf33f511d16fdf8dd2eb01fc4d8ab` |
| Mesh | `9599b3b9a3e22070dfc0765130953f0e2e735bb87f1c5869aa237469197d766c` |
| Codex | `56ef98ab4032d317ab26e9b5e5a175650717351edb16ed9cde0cb6d1734d62da` |
| codex-code-mode-host | `3e85d67471825f73d02ff5f7e047ca1f6ca8caa3f59e4c6e8d9ca6ca7302cb45` |

Production `coordination.initialize_team` used the builder's actual canonical
messaging policy, a login-only Claude lead and alpha with creation-time
`delivery: tmux`. Config is format **2**, `delivery_owner: team`. The scratch
root was `/tmp/th-l2-i_py2myk`, probed private port **46483**. All harness, home,
project, data, temp and socket roots were scratch-only; inherited TMUX was absent.
Bubblewrap hid operator homes and supplied a private PID namespace. Exactly the
authorized `auth.json` was copied at **0600**, with no fallback; no other operator
configuration was copied. Claude stayed at its first-run theme screen and took
**zero model turns**. [Initialize result](l2-tmux-busy/run4/run/step1-operation.json),
[Claude pane](l2-tmux-busy/run4/run/final-pane-1.txt).

Alpha's contract/attachment/context are **1 / 1 / "0"**; root revision **0**;
incarnation `8117c5d23d2222115365595f024839c48e7151664e77f95e13ea96c9e3d25c84`.
Socket `/tmp/th-l2-i_py2myk/tmux/tmux-1000/default`, session **$0**, pane **%2**,
pane PID **135**, start ticks **27776475**; session identity
`01a08c1e-dcdc-7bf3-8069-acb3a802a38a`. The final activity and runtime snapshot
both show **idle**, with high-confidence attribution. The initial prompt was
classified idle using `launch_ready`. **120 s is snapshot freshness**, not an
artificial waiting period. [Runtime](l2-tmux-busy/run4/run/final-runtime.json),
[activity](l2-tmux-busy/run4/run/final-activity.json),
[final pane](l2-tmux-busy/run4/run/final-pane-2.txt).

Onboarding message **`f6238d79-b8a7-47f6-a6c1-23e0fd6c1e81`** has one
`submitted` receipt at **16:20:29.304907 UTC**, generation 1 and matching pane
identity; one `consumed_by_read` receipt follows at **16:20:36.041055 UTC**.
Codex's successful tool result contains the recovery card; it replies `— done`.
Acceptance, terminal submission, explicit read and model reply remain separate
observations. The lead has native enqueue only, with no terminal submission.
No Q/Q2 journal pagination or step-6 completion is claimed.

The passive observer recorded four FLOCK-bearing samples on inode **1638350**:
two Taurhaus `launch` samples and two Mesh `paste+submit` samples. The inode stayed
constant. No process was paused or signalled as an experiment, and no lock was
acquired by the observer. [FLOCK evidence](l2-tmux-busy/run4/run/terminal-locks.jsonl).

### Every observed spend and evidence limit

One seat-start/onboarding reservation was made; there were **zero** direct
ordinary inputs, Q/Q2 inputs, resumes, retries or compactions. The ledger counts
**two distinct turn IDs**, one with complete rollout usage and one notify-only.

| Spend | Identity / input, cached input, output tokens | USD |
| --- | --- | --- |
| Completed onboarding turn, first model-call increment | `01a08c1e-e1e5-73b0-b4a4-4bd12703805d`; 8,818 / 3,840 / 114 | **0.00120920** |
| Same turn, second increment | 10,637 / 7,936 / 98 | **0.00081652** |
| Known onboarding total | 19,455 / 11,776 / 212 | **0.00202572** |
| Notify-only turn | `01a08c1e-e4bf-72b2-a558-dbbebeed86b3`, session `01a08c1e-e4a5-7bd2-ab7f-855148e68f4b`; no rollout usage | **UNKNOWN** |
| Claude lead | Zero model turns | **0** |
| Implementer / reviewer | Orchestrator's separate budget; no Opus reviewer ran | Unavailable to lane |

Known usage priced conservatively at $1.20/M for every input/output token totals
**$0.02360040**. The normal API-equivalent rates are inherited estimates
($0.20/M input, $0.02/M cached input, $1.20/M output), not billing evidence.
**Total spend and the $0.20 ceiling remain unverified**, because the unmatched
notify ID is neither free nor safely attributable to the measured turn. The
controller stopped with two recorded IDs, below the 10-input threshold, and sent
no next input. [Raw ledger](l2-tmux-busy/run4/run/cost-ledger.json),
[notify identities](l2-tmux-busy/run4/run/codex-notify.json),
[increments and classification](l2-tmux-busy/run4/diagnosis.json).

### Cleanup, gates and deviations

Cleanup used the owned daemon's SIGINT handler and waited for its private PID
namespace to exit. The independent audit confirms **no surviving scratch daemon,
tmux server or Codex process**, closed port, credential copy removed and scratch
root deleted. All started build/gate/controller processes exited. No foreign
process was killed. [Cleanup](l2-tmux-busy/run4/run/cleanup.json),
[independent audit](l2-tmux-busy/run4/final-audit.json).

The **complete 139-row daemon JSONL**, including shutdown, is retained in
[taurhaus.log.jsonl](l2-tmux-busy/run4/run/taurhaus.log.jsonl). Account quota fields
are removed, while turn token counts remain. Three byte-identical files were
deduplicated through [aliases](l2-tmux-busy/run4/export-manifest.json); pane files
are at most 60 lines. No collector size limit aborted the trial.

| Command | Exit / result |
| --- | --- |
| `just build-daemon` | **0**, checkout-local target after bounded Cargo polling |
| Mesh `cargo build --bin mesh` | **0**, lane-owned worktree/target |
| `just check-quick` | **0** |
| `just lint` | **0** |
| `just test-contracts` | **0** |
| Offline controller/preflight tests | **0**, 19 tests |
| Offline export and independent cleanup/evidence audit | **0** |

[Gate exits/tails](l2-tmux-busy/run4/checks-result.json).
No Rust diff exists, so the conditional `just test-rust-unit` gate does not apply.
Pin-preflight red was **exit 1**, missing `validate_candidate`, then 18 tests passed.
The export regression was **exit 1**, retained `token_count.rate_limits`, then 19
passed; its `// Regression:` comment names **`b320480f`**.
[Pin red](l2-tmux-busy/run4/red.txt), [pin green](l2-tmux-busy/run4/green.txt),
[export red](l2-tmux-busy/run4/sanitizer-red.txt),
[export green](l2-tmux-busy/run4/sanitizer-green.txt).

Deviations/limits: the frozen controller's raw Taurhaus label is qualified as a
harness failure using observed submission/read/idle evidence; its recovery-card
user-row assumption originated in **`7b9cbb3a`**. The metering ambiguity is retained,
not normalized away. The model-issued onboarding read omitted `--claude-dir`,
relied on scratch `CLAUDE_DIR`, and did not follow its returned cursor; this is not
a compliant step-6 read. Initial transcript collection/inspection included account
quota fields; the committed export removes them with a regression guard. There
was no product change and no runtime retry. No numbered runtime step received a
green commit because the first composite step failed. The required independent
**Opus evidence lens and Workflow runner are unavailable** through this session's
tools; no cross-family approval or full-workflow PASS is claimed.

### Run 4 continuation verification

The requested continuation began at `49201aae` with a clean tree; every completed
green preparation/export step was already committed. Fresh verification passed:
`just check-quick` **0**, `just lint` **0**, `just test-contracts` **0**, all **19**
offline tests **0**, and the independent cleanup/evidence audit **0**.
[Continuation gate results](l2-tmux-busy/run4/continuation-checks.json) preserve
these exits separately from the original trial's gates. No product diff exists.

No paid restart or additional input occurred. The original stop-on-first-failure
rule and incomplete spend accounting still prevent proceeding to steps 2–6;
the deleted scratch runtime cannot be resumed. This verification does not change
the run-4 unavailable/incomplete verdict or supply the missing Opus review.

### Run 4, second attempt (run4b) — historical verdict: UNAVAILABLE / INCOMPLETE

**Step 1 PASS; step 2 FAIL (harness); steps 3–6 NOT RUN.** The relaunch
applied both orchestrator rulings, then stopped on a new harness observation:
a literal `tmux send-keys` followed immediately by `Enter` left the ordinary
response in Codex's composer. No second rollout turn or production busy snapshot
appeared during the 90-second poll. Q was never sent. The final pane proves the
unsubmitted text; the exact input-handling cause is unproved. This is **not a
Taurhaus or Mesh product failure**, and no product code was changed or runtime
input retried. [run4b diagnosis](l2-tmux-busy/run4b/diagnosis.json),
[final pane](l2-tmux-busy/run4b/run/final-pane-2.txt),
[complete command/results](l2-tmux-busy/run4b/run/events.jsonl).

| Ordered step | Outcome / classification | Evidence |
|---|---|---|
| 1. Canonical initialize, runtime contract, delivered onboarding | **PASS**, product path observed | [ready snapshot and receipts](l2-tmux-busy/run4b/run/step1-ready.json), [pane identity](l2-tmux-busy/run4b/run/step1-pane-identity.json), [terminal lock](l2-tmux-busy/run4b/run/step1-terminal-lock.json) |
| 2. Ordinary response, observed working, send Q while busy | **FAIL — harness**; response remained in composer, no observed busy window, Q not sent | [outcome](l2-tmux-busy/run4b/run/step2-outcome.json), [final pane](l2-tmux-busy/run4b/run/final-pane-2.txt) |
| 3. Q pending, then fresh idle and one matching submission/reply | **NOT RUN — blocked by step 2**, no product classification | [outcome](l2-tmux-busy/run4b/run/step3-outcome.json) |
| 4. Second busy response, Q2 pending, normal managed stop and lock sample | **NOT RUN — blocked by step 2**, no product classification | [outcome](l2-tmux-busy/run4b/run/step4-outcome.json) |
| 5. One managed low-effort resume, new generation, Q2 once and no Q replay | **NOT RUN — blocked by step 2**, no product classification | [outcome](l2-tmux-busy/run4b/run/step5-outcome.json) |
| 6. Explicit Q2 read, reconciliation, export and teardown | **NOT RUN as a workflow step**; failure export and teardown completed separately | [outcome](l2-tmux-busy/run4b/run/step6-outcome.json), [cleanup audit](l2-tmux-busy/run4b/final-audit.json) |

The unchanged product is **1db4f9bf**, protocol **27**; the separate `mesh-l2`
worktree was detached at **ed59187** and its descriptor stayed unchanged.
Checkout-local `just build-daemon` and `cargo build --bin mesh` in `mesh-l2`
both exited **0**, after the prescribed Cargo-process probe.
[build commands](l2-tmux-busy/run4b/builds.json),
[binary SHA-256 digests](l2-tmux-busy/run4b/checks-result.json).
Actual native Codex **0.153.4**, **gpt-5.6-luna / low** ran with both native
siblings in the scratch bin. The Claude lead remained login-only, with no
Claude model turn. Production initialization used the builder's canonical
messaging policy and creation-time tmux delivery. The sole authorized auth file
was copied alone into the empty scratch Codex home as mode 0600. Child processes
were inside the private PID namespace with operator homes hidden, a private
tmux server and private daemon port **46929**.

Alpha's runtime had `terminalContract=1`, attachment generation **1**, context
generation **"0"**, pane **%2**, pane PID **140**, start ticks **27903816**, session
**$0**, and terminal-lock inode **1740006**. The scratch root, socket, launch-root
incarnation and authority revision are retained in the ready record. The real
Codex session **01a08c32-4c14-7ba0-ba32-53edd53fc37c** was attributed and idle at
**high** confidence. Its snapshot at **16:42:01.363777467Z** was less than one
second old at the step-1 acceptance. Onboarding message
**1914645c-9df1-4c95-847a-6027144cb808** is retained in the diagnosis; the
receipt/read predicate verified **one tmux submission and one explicit read**.
The recovery card arrived through a tool result, not a user row. Startup passive
observation retained **7 lock samples, 5 with FLOCK holders**; these do not prove
managed-stop exclusion, which was not reached.

#### Run4b budget — every observed model-call spend

The fresh budget counted **2 / 10 inputs**: one Mesh onboarding terminal delivery
and one controller-issued ordinary response submission attempt. The latter
created no rollout turn. There was **one completed rollout turn**,
`01a08c32-50c8-75d2-99f7-4e1fd0f6b3bf`, containing these usage increments:

| UTC usage row | Input | Cached input | Output | USD |
|---|---:|---:|---:|---:|
| 16:41:50.813 | 8,874 | 6,912 | 205 | 0.00077664 |
| 16:41:56.598 | 11,290 | 6,912 | 201 | 0.00125504 |
| 16:41:59.926 | 11,828 | 11,008 | 110 | 0.00051616 |
| 16:42:01.285 | 12,164 | 11,008 | 32 | 0.00048976 |
| **Total** | **44,156** | **35,840** | **548** | **0.00303760** |

The inherited packet rates are $0.20 / $0.02 / $1.20 per million input / cached
input / output tokens; this is API-equivalent metered usage, not an invoice.
The upper estimate pricing every input/output token at $1.20/M is **$0.05364480**.
Both are below **$0.20**. Notify-only identity
`01a08c32-54a5-7b42-943d-f39f62694235` has no usage row: its cost is explicitly
**unknown-but-not-a-model-input**, and it did not block the run, per the ruling.
No Q/Q2 or resume spend occurred; Claude spend was zero (login-only).
[full cost ledger](l2-tmux-busy/run4b/run/cost-ledger.json).

#### Run4b verification, cleanup and limitations

- **Red → green:** the offline ruling tests first produced one failure
  (`metering_complete` false for a notify-only identity) and two errors (missing
  per-call usage ledger and receipt-based onboarding predicate). All **22**
  controller/preflight/ruling tests then passed. Regression comments identify
  `fb6200e0`, which inherited the faulty predicates. No test launches a CLI or
  reads real harness credentials. [red](l2-tmux-busy/run4b/rulings-red.txt),
  [green](l2-tmux-busy/run4b/green.txt),
  [tests](l2-tmux-busy/run4b/rulings_test.py).
- Gates from this checkout root: **`just check-quick` = 0**,
  **`just lint` = 0**, **`just test-contracts` = 0**.
  Check-quick ran **2,518 frontend tests**. No `src-tauri/` diff, so the conditional
  Rust-unit gate does not apply. [gate exits](l2-tmux-busy/run4b/checks-result.json).
- The controller exited **1** after **113.246 seconds**; it did not retry paid
  input or inject faults. Its `finally` stopped owned processes, removed the
  scratch credential and root, and verified no surviving daemon/tmux/Codex
  identities or private listener. The independent read-only cleanup/export audit
  exited **0**. [controller exit](l2-tmux-busy/run4b/run/controller-exit.json),
  [cleanup](l2-tmux-busy/run4b/run/cleanup.json).
- All **186 complete daemon JSONL rows** were retained, including shutdown,
  with only privacy sanitization. Pane captures are at most **60 lines**.
  Five byte-identical files were removed with explicit aliases; no daemon rows
  were deduplicated away. [daemon stream](l2-tmux-busy/run4b/run/taurhaus.log.jsonl),
  [alias manifest](l2-tmux-busy/run4b/export-manifest.json).
- The model also sent a Mesh clarification to the login-only lead after reading
  the unassigned onboarding card. This was within the same metered turn, but
  departed from the scratch instruction to send no other messages. It did not
  produce a Claude model turn. The rollout and journal retain the action.
- The required independent **Opus evidence lens is unavailable**: this session
  exposes no Workflow/Opus tool or Opus model override. No substitute review or
  review approval is claimed. Implementer/reviewer billing is outside the seat
  meter; no reviewer was started. Thus this is an incomplete evidence lane even
  apart from the step-2 stop.
- Changes are confined to this evidence report and `run4b/` controller/evidence
  sidecars. No product patch, descriptor edit, install, release, stress run,
  hosted seat, artificial idle stall or plan-ledger mutation was performed.
  Step 1 was committed immediately as **9f4ac4c5** after the preparation commit
  **31aac7d7**. No green step is claimed for steps 2–6.


### Run 4c — confirmed busy input; stopped at step 3 (latest attempt)

**UNAVAILABLE / incomplete evidence — harness timeout, no Taurhaus or Mesh
product failure established.** This single fresh attempt passed steps 1–2.
Step 3 observed Q deferred while busy, one terminal submission after fresh idle,
one explicit read, and an exact Mesh reply. The controller nevertheless timed
out waiting for a final assistant marker plus settled fresh idle and complete
metering. Two automatic context compactions occurred during Q's turn, which
remained active at teardown. The run stopped there without retrying any paid
input; steps 4–6 remain unproved. [reconciliation](l2-tmux-busy/run4c/diagnosis.json),
[final pane](l2-tmux-busy/run4c/run/final-pane-2.txt).

| Ordered step | Outcome and classification | Evidence |
|---|---|---|
| 1. Initialize, runtime contract, attributed idle and delivered/read onboarding | **PASS — product path observed** | [ready record and receipts](l2-tmux-busy/run4c/run/step1-ready.json), [pane identity](l2-tmux-busy/run4c/run/step1-pane-identity.json), [lock inode](l2-tmux-busy/run4c/run/step1-terminal-lock.json) |
| 2. Confirm ordinary input, observe production working, send Q | **PASS — busy window observed** | [confirmation](l2-tmux-busy/run4c/run/Q-work-confirmation.json), [working snapshot](l2-tmux-busy/run4c/run/Q-work-working.json), [Q acceptance](l2-tmux-busy/run4c/run/Q-accepted.json) |
| 3. Q pending, fresh-idle delivery and reply | **FAIL — harness predicate timeout; transport/read/Mesh-reply subclaims observed, settled turn unproved** | [outcome](l2-tmux-busy/run4c/run/step3-outcome.json), [pending evidence](l2-tmux-busy/run4c/run/pending/8c3ab763-9686-4825-9d29-8bab552581d3.json), [reconciliation](l2-tmux-busy/run4c/diagnosis.json) |
| 4. Q2 while busy, managed stop and passive exclusion | **NOT RUN — blocked by step 3** | [outcome](l2-tmux-busy/run4c/run/step4-outcome.json) |
| 5. Managed low-effort resume, new generation, Q2 once and no Q replay | **NOT RUN — blocked by step 3** | [outcome](l2-tmux-busy/run4c/run/step5-outcome.json) |
| 6. Q2 explicit read and reconciliation | **NOT RUN — blocked by step 3**; failure export/cleanup completed separately | [outcome](l2-tmux-busy/run4c/run/step6-outcome.json), [cleanup audit](l2-tmux-busy/run4c/final-audit.json) |

The Taurhaus product remains **1db4f9bf**, protocol **27**, with no `src/` or
`src-tauri/` diff. Mesh was detached at **ed59187** in the designated separate
`mesh-l2` worktree and its descriptor was unchanged. Cargo was probed before
both builds; checkout-local `just build-daemon` and Mesh `cargo build --bin mesh`
exited **0**. Native Codex **0.153.4**, **gpt-5.6-luna / low**, and both native
siblings were copied to the scratch bin. Production initialization used the
builder's canonical messaging policy, one tmux alpha and a login-only Claude
lead. Only the explicitly authorized `auth.json` source was copied into the
empty scratch Codex home, mode 0600; no real harness home was exposed to children.
Private daemon port **46345**, tmux and PID namespace belonged to this attempt.
[build records](l2-tmux-busy/run4c/builds.json),
[binary digests](l2-tmux-busy/run4c/checks-result.json),
[commands and runtime events](l2-tmux-busy/run4c/run/events.jsonl).

Alpha's session was **01a08c3e-aa6c-72d0-a63d-bff45b5cb3af**, attributed and idle
at high confidence at step 1. Its terminal contract was **1**, attachment
**1**, context **"0"**, pane **%2**, shell PID **137**, start ticks **27984903**,
tmux session **$0**, root-authority revision **0**, terminal-lock inode
**1782685**. The full root/incarnation/socket fields are retained in the ready
record. Onboarding **6f56878d-3d7e-462c-a91c-9675827989d4** had one tmux submission
and explicit read; its card was exposed through a tool result.

The controller pasted a request for **60 numbered lines, each with 15 words
about rivers**. It observed the composer text, sent Enter once, then observed
an empty composer and new rollout turn **01a08c3f-00f0-7243-9991-befef9cb6171**
at **16:55:34.605Z** before starting its 90-second busy observation window.
No second Enter was needed. Production `likely_working` was observed at
**16:55:34.680Z**. Q **8c3ab763-9686-4825-9d29-8bab552581d3** / marker
**Q-c36d8abc** was accepted at **16:55:34.836Z**. Scheduler health then recorded
`pending: activity not freshly idle`, with no terminal submission or native
marker exposure in the retained pre-idle observation. This is scheduler-health
evidence, not a pending journal receipt.

The preceding idle snapshot was observed at **16:56:01.682Z**; Q's sole terminal
submission followed at **16:56:02.131Z**, an idle age of **0.449 seconds**,
within the **120-second freshness bound**. Receipt attachment fields match the
recorded pane and generation. The observer's first post-receipt sample had
already changed back to `likely_working`; it is not an atomic lock-time idle
sample, and the controller's later idle assertion was never reached.
Codex explicitly read Q once and sent the exact marker through Mesh as reply
**423a47bb-d241-43e6-9019-1a9c2c2203f8** at **16:56:50Z**. That accepted Mesh
reply does not imply a final assistant answer, lead model uptake, or a settled
turn. The deadline at approximately **16:57:05Z** expired before those controller
predicates were satisfied. [journal, reads, identities and timeline](l2-tmux-busy/run4c/diagnosis.json).

#### Run 4c budget — every metered spend and known gap

**3 / 10 Codex inputs:** two Mesh terminal deliveries (onboarding and Q), plus
one confirmed controller submission. There were three rollout turns and one
separate notify-only identity. No Q2, resume, extra controller input, or Claude
model turn occurred. The nine retained usage increments were:

| UTC usage row | Input | Cached input | Output | USD |
|---|---:|---:|---:|---:|
| 16:55:18.635 | 8,866 | 6,912 | 180 | 0.00074504 |
| 16:55:25.105 | 11,250 | 7,936 | 215 | 0.00107952 |
| 16:55:28.037 | 11,868 | 11,008 | 70 | 0.00047616 |
| 16:55:31.743 | 12,273 | 11,008 | 124 | 0.00062196 |
| 16:55:33.781 | 12,625 | 12,032 | 23 | 0.00038684 |
| 16:56:01.311 | 13,133 | 12,032 | 1,407 | 0.00214924 |
| 16:56:41.809 | 11,107 | 6,912 | 127 | 0.00112964 |
| 16:56:47.359 | 12,460 | 9,984 | 211 | 0.00094808 |
| 16:56:50.920 | 14,895 | 12,032 | 90 | 0.00092124 |
| **Metered total** | **108,477** | **89,856** | **2,447** | **0.00845772** |

By turn: onboarding **01a08c3e-af99-7831-bc0d-748db5bf8661** cost
**$0.00330952**; ordinary response **01a08c3f-00f0-7243-9991-befef9cb6171**
cost **$0.00214924**; unfinished Q turn **01a08c3f-6d51-7cd3-b6fc-7c86f4a90e83**
reported **$0.00299896**. Rates inherited from the trial packet are
$0.20 / $0.02 / $1.20 per million input / cached-input / output tokens;
these are API-equivalent estimates, not invoices. Pricing all reported tokens
at $1.20/M gives **$0.13310880**. Both reported totals are below $0.20.
Notify-only **01a08c3e-b2ab-7492-a378-5d94a7194a4c** remains
**unknown-but-not-a-model-input**, as ruled, and did not block input.

**The complete dollar bound is unverified.** Automatic compactions completed
at **16:56:38.047Z** and **16:57:05.302Z**. Their usage rows had zero input/output
counters and nonzero `last_token_usage.total_tokens` (**7,119** and **7,269**);
these are unpriced, not free spends. Q was still active at teardown. The
inherited scratch `model_context_window=16384` was retained; the compactions
were automatic, not controller requests or fault injection. No subsequent
paid action was admitted. [all usage increments and turns](l2-tmux-busy/run4c/run/cost-ledger.json),
[unpriced boundaries](l2-tmux-busy/run4c/diagnosis.json).

#### Run 4c verification, cleanup and deviations

- **Red → green:** six new offline submission tests first errored because
  confirmed submission was absent; all **28** offline controller, preflight,
  ruling and submission tests then passed. The regression comment names
  **31aac7d7**, which inherited the immediate-Enter behavior. The implementation
  adapts Lane 1 run 3's last-composer/new-turn predicate with the binding
  5-second paste check, 10-second confirmation check and at most one extra Enter.
  No offline test invokes a real CLI or reads real harness credentials.
  [red](l2-tmux-busy/run4c/submission-red.txt), [green](l2-tmux-busy/run4c/green.txt),
  [tests](l2-tmux-busy/run4c/submission_test.py).
- Exact root gates: **`just check-quick` = 0**, **`just lint` = 0**,
  **`just test-contracts` = 0**. No Rust diff, so the conditional
  `just test-rust-unit` gate did not apply. [gate results](l2-tmux-busy/run4c/checks-result.json).
- Controller exit **1**, runtime **117.387 seconds**. Failure cleanup and the
  independent read-only export/process audit exited **0**: no surviving owned
  daemon, tmux or Codex process, private listener closed, auth copy and scratch
  root removed. Only owned processes were stopped. [cleanup](l2-tmux-busy/run4c/run/cleanup.json),
  [audit](l2-tmux-busy/run4c/final-audit.json).
- Retained **all 289 complete daemon JSONL rows**, including shutdown, with
  privacy sanitization only. Eight byte-identical exported files have explicit
  aliases; no daemon rows were deduplicated away. Captures are at most **60
  lines**. There were **8 passive lock samples, 5 with FLOCK holders** for
  launch/delivery. Managed-stop exclusion remains **NOT RUN**.
  [daemon stream](l2-tmux-busy/run4c/run/taurhaus.log.jsonl),
  [alias manifest](l2-tmux-busy/run4c/export-manifest.json),
  [lock evidence](l2-tmux-busy/run4c/run/terminal-locks.jsonl).
- The model sent one onboarding clarification to the login-only lead and
  replied to Q through Mesh despite the scratch instruction to send no other
  messages. Both sends are retained and included in their existing turn costs;
  neither started a Claude model turn. The inherited settled-assistant-reply
  predicate did not accept the Mesh reply as completion. No paid rerun followed.
- The required independent **Opus evidence lens remains unavailable**: no
  callable Workflow/Opus reviewer exists in this session. No substitute approval
  or paid reviewer was started; implementer billing is outside the seat meter.
- No product change, descriptor edit, install/release, hosted seat, artificial
  idle delay, pause/freeze, load/stress run, or plan-ledger edit. Evidence is
  confined to this report and `run4c/`. Preparation was committed as
  **451ec092**, step 1 as **5c7c6d38**, and step 2 as **1661296e** immediately
  after their green outcomes. This attempt makes no PASS claim for the complete
  lane or managed stop/resume.

#### Run 4c continuation verification

The continuation began at **c3b98cf9** with a clean tree. Steps 1 and 2 were
already committed as **5c7c6d38** and **1661296e**; no completed green runtime
step remained uncommitted. The frozen evidence reconciliation and all **28**
offline tests passed again, as did the read-only cleanup/export audit.

Fresh exact gates from this checkout root all exited **0**:
`just check-quick`, `just lint`, and `just test-contracts`.
[Continuation gate results](l2-tmux-busy/run4c/continuation-checks.json).
The product diff against **1db4f9bf** remains empty, so the conditional Rust-unit
gate does not apply. Original run gate records remain unchanged.

No paid runtime was restarted and no additional seat spend occurred. Step 3's
recorded failure and removed scratch runtime prevent an ordered step-4
continuation under the specification's stop-on-failure rule. Steps 4–6 remain
**NOT RUN**; the incomplete metering and missing Opus review remain unresolved.

#### Run 4d — controller preparation

Run 4d applies the fourth-attempt ruling to the inherited run-4c controller.
Four synthetic tests first failed for missing reply, separate-wait and
message-scoped deferral handling; a further schema test failed until the
journal author used canonical `payload.author.name`. All **32** offline tests
then passed. Tests use generated dictionaries/tempdirs, never real credentials
or CLIs. See [red](l2-tmux-busy/run4d/reply-red.txt),
[schema red](l2-tmux-busy/run4d/journal-schema-red.txt),
[green](l2-tmux-busy/run4d/green.txt).

The reply predicate accepts alpha's journal reply or any rollout row after
that message's submission. Reply, subsequent fresh idle and metering now have
three separately named waits and artifacts. Pending evidence is message-scoped
(acceptance, working snapshot, no receipt); scheduler health is corroboration.
The inherited scratch-only `model_context_window=16384` override is removed:
4c showed it caused automatic compactions with unpriced counters. The model's
normal context limit applies; no compaction is requested. All other isolation,
confirmed-submission, low-effort, budgets and ordered-step rules remain in force.
No product or Mesh descriptor changes. An independent Opus lens is left to the
orchestrator: this implementer session exposes no callable Opus reviewer.


#### Run 4d latest verdict — unavailable after four passing runtime steps

**Steps 1–4 PASS. Step 5 FAIL (harness), before the resume RPC; step 6 NOT RUN.**
The stop-on-failure rule ended this single fresh attempt; no product change or
paid retry followed. The interrupted Q2 work turn has a genuine `turn_aborted`
record but no new token-usage increment. Its last `token_count` repeats the
previous turn's cumulative counters. The separately named **90-second
`stopped-turn metering incomplete`** wait expired before the controller could
admit resume. This is an unverified-cost harness limitation, **not a demonstrated
Taurhaus or Mesh defect**. Managed resume and generation-safe Q2 delivery remain
unproved. [Step outcomes and reconciliation](l2-tmux-busy/run4d/diagnosis.json),
[controller exit](l2-tmux-busy/run4d/run/controller-exit.json).

| Ordered audit step | Outcome / classification | Runtime evidence |
|---|---|---|
| 1. Initialize, identity and onboarding | **PASS — S-runtime** | Attributed session `01a08c5c-6617-75b3-b64a-3d23e8eb94c9`, fresh idle, onboarding submitted and explicitly read. Terminal contract 1, generation 1, pane `%2`, session `$0`, PID 134, start ticks `28179762`; root revision 0. Lock inode 1826133. [Ready snapshot](l2-tmux-busy/run4d/run/step1-ready.json), [pane identity](l2-tmux-busy/run4d/run/step1-pane-identity.json), [lock](l2-tmux-busy/run4d/run/step1-terminal-lock.json). |
| 2. Real busy response, accept Q | **PASS — S-runtime** | Confirmed controller submission; production `likely_working` snapshot. Q `dcb01fc4-120e-41c2-8c86-956f7bc23fd2` accepted while working. [Confirmation](l2-tmux-busy/run4d/run/Q-work-confirmation.json), [working snapshot](l2-tmux-busy/run4d/run/Q-work-working.json), [acceptance](l2-tmux-busy/run4d/run/Q-accepted.json). |
| 3. Busy deferral, idle delivery and reply | **PASS — S-runtime** | Message-scoped acceptance with no receipt while working; Q first submitted **25.868479 s** later, idle age **0.938855 s**, same generation and complete pane identity. Exactly one submission, `consumed_by_read`, alpha's Mesh journal reply. [Pending facts and pre-idle transcript](l2-tmux-busy/run4d/run/pending/dcb01fc4-120e-41c2-8c86-956f7bc23fd2.json), [delivery](l2-tmux-busy/run4d/run/step3-delivery.json). |
| 4. Busy Q2 and managed stop | **PASS — S-runtime** | Second confirmed response; Q2 `0b86aa5f-9f0d-4456-96c1-ba147ef727ed` accepted and pending while working. Normal `stop_session` returned `{ok:true}`; retained health `session_dead`. Passive FLOCK holders on the same inode during `interrupt` and `stop-teardown`. Q2 retained with zero submissions. [Confirmation](l2-tmux-busy/run4d/run/Q2-work-confirmation.json), [pending](l2-tmux-busy/run4d/run/pending/0b86aa5f-9f0d-4456-96c1-ba147ef727ed.json), [stopped record](l2-tmux-busy/run4d/run/step4-stopped.json), [lock samples](l2-tmux-busy/run4d/run/terminal-locks.jsonl). |
| 5. Managed low-effort resume | **FAIL — harness prerequisite** | No `coordination.resume_member` call: interrupted-turn metering never gained a usage increment. No new generation, no resumed Q2 delivery, no claim for old-generation exclusion across resume. [Failure](l2-tmux-busy/run4d/run/step5-outcome.json). |
| 6. Explicit Q2 read and reconciliation | **NOT RUN — blocked by step 5** | Q2 stays traceable in the retained journal with zero submissions; no final Q2 read receipt. Cleanup/export completed separately. [Not-run outcome](l2-tmux-busy/run4d/run/step6-outcome.json), [journal](l2-tmux-busy/run4d/run/team/state/messaging-v2/segments/000001.jsonl). |

Step 3's three independent waits each passed and retain their own evidence:
[reply](l2-tmux-busy/run4d/run/step3-reply-wait.json),
[fresh idle](l2-tmux-busy/run4d/run/step3-idle-wait.json),
[metering](l2-tmux-busy/run4d/run/step3-metering-wait.json).
The pending diagnosis replays both messages against all journal rows committed
by their observed busy boundaries, with scheduler health excluded from the
predicate; member `last_defer_reason` is only corroboration. Q had exactly one
submission through teardown and did not replay in this observed interval.

#### Run 4d spend and runtime provenance

**4 Codex inputs of the 10-input cap:** two confirmed controller submissions
(one Enter each) plus two Mesh terminal deliveries (onboarding and Q). Q2 was
accepted but never terminal-delivered. **Metered spend $0.00704472 of $0.20**;
the interrupted fourth input's cost is **unknown, not zero**. A complete cost
bound is therefore unverified. No additional input was admitted. Earlier
attempts' spend is history and was not charged to this attempt. No Claude model
turn, hosted seat, compaction request or automatic compaction occurred.

Every positive usage increment is listed below; cumulative duplicates are not
charged twice. The final aborted-turn row repeats input 92,724 / cached 77,056 /
output 1,975, adding no attributable usage. [Ledger](l2-tmux-busy/run4d/run/cost-ledger.json),
[complete retained rollout](l2-tmux-busy/run4d/run/sessions/rollout-2026-09-10T19-27-40-01a08c5c-6617-75b3-b64a-3d23e8eb94c9.jsonl).

| UTC usage row | Input | Cached input | Output | USD |
|---|---:|---:|---:|---:|
| 17:27:47.447Z | 9,027 | 3,840 | 146 | 0.00128940 |
| 17:27:51.648Z | 11,386 | 7,936 | 113 | 0.00098432 |
| 17:28:17.522Z | 12,284 | 11,008 | 1,310 | 0.00204736 |
| 17:28:22.256Z | 13,652 | 12,032 | 129 | 0.00071944 |
| 17:28:26.859Z | 15,017 | 13,056 | 169 | 0.00085612 |
| 17:28:29.773Z | 15,523 | 14,080 | 96 | 0.00068540 |
| 17:28:31.044Z | 15,835 | 15,104 | 12 | 0.00046268 |
| **Metered total** | **92,724** | **77,056** | **1,975** | **0.00704472** |

- `01a08c5c-67a2-76d1-b779-f3eab40b93d8`: **$0.00227372**.
- `01a08c5c-9379-7d03-ade3-b3a3dc24f61b`: **$0.00204736**.
- `01a08c5c-fa85-7370-8336-fdc137d7cf9a`: **$0.00272364**.
- `01a08c5d-2c60-7341-ab83-2dd93cd00de9`: **unknown (interrupted model turn; completed abort, no new usage)**.
- `01a08c5c-6fb6-7ae1-a9b1-4a5afbc08d62`: **unknown-but-not-a-model-input (notify-only)**.

Rates are inherited from the trial packet: $0.20 input / $0.02 cached input /
$1.20 output per million tokens, API-equivalent estimates rather than invoices.
Pricing all reported tokens at $1.20/M gives $0.11363880; this does not assign
zero cost to the interrupted turn.

Builds used unchanged Taurhaus product **1db4f9bf**, protocol **27**, and the
separate Mesh worktree detached at **ed59187**. `just build-daemon` and
`cargo build --bin mesh` both exited **0** with checkout-local targets after
the required Cargo polling. Real scratch Codex reported **0.153.4**, model
**gpt-5.6-luna**, effort **low**; both native siblings were copied. The credential
copy was exactly the authorized `auth.json`, mode 0600, with no content exported.
The login-only Claude lead had its own empty scratch config. Canonical creation
used the builder's current policy and creation-time `delivery: tmux` for both
seats. Descriptor unchanged, no hosted bypass.
[Builds](l2-tmux-busy/run4d/builds.json),
[binary digests](l2-tmux-busy/run4d/checks-result.json),
[commands, initialization payload and identities](l2-tmux-busy/run4d/run/events.jsonl).

#### Run 4d final checks, teardown and deviations

- **Controller exit 1**, **157.712 seconds** total runtime. Read-only evidence
  reconciliation **0** and cleanup/privacy audit **0**. No surviving owned
  daemon, tmux server or Codex process; private listener closed; copied auth and
  scratch root removed. [Cleanup](l2-tmux-busy/run4d/run/cleanup.json),
  [audit](l2-tmux-busy/run4d/final-audit.json).
- **All 263 complete daemon JSONL rows retained**, including shutdown, with
  privacy sanitization only. **13** passive lock samples, **8** with FLOCK
  holders, including managed interrupt and teardown. **13** byte-identical
  exported files are mapped as aliases; no daemon rows removed. All pane
  captures are at most 60 lines. [Daemon](l2-tmux-busy/run4d/run/taurhaus.log.jsonl),
  [aliases](l2-tmux-busy/run4d/export-manifest.json).
- **32 offline tests passed.** Post-run schema reconciliation added a failing
  assertion for canonical `receipt` rows (introduced in `f6880113`); fixing that
  offline predicate made it green. Both actual busy boundaries replayed with
  zero receipts under the corrected predicate. No paid rerun or verdict change.
  [Schema red](l2-tmux-busy/run4d/receipt-schema-red.txt),
  [green](l2-tmux-busy/run4d/green.txt), [replay](l2-tmux-busy/run4d/diagnosis.json).
- Exact root gates all exited **0**, initially and after the offline schema
  correction: **`just check-quick`**, **`just lint`**, **`just test-contracts`**.
  No `src-tauri/` diff, so `just test-rust-unit` was not applicable.
  [Initial gates](l2-tmux-busy/run4d/initial-checks-result.json),
  [final gates](l2-tmux-busy/run4d/checks-result.json).
- Harness deviation: removed the inherited 16K context override; the real
  rollout reports a 258,400-token effective context window. Added the named
  pre-resume metering wait instead of immediately failing the inherited budget
  reservation. It exposed the interrupted-turn accounting gap; it did not
  trigger any additional model input or alter a product process.
- Required independent **Opus evidence review remains unavailable in this
  implementer session** and belongs to the orchestrator's review route. No
  substitute reviewer or model approval is claimed. Implementer billing is
  separate from the seat meter.
- No product edits, descriptor changes, installs/releases, fault injection,
  load/stress run, artificial 120-second idle wait, or plan-ledger edits.
  Preparation commit **f6880113**; immediate green-step commits **a0111dbd**
  (1), **fbaa2322** (2), **faea1b33** (3), **8646697b** (4). Full-lane PASS is
  withheld because steps 5–6, complete metering and Opus review are unresolved.
