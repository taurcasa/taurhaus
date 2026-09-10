# FAIL — Run 3 stops at step 1: session identity resolves, fresh idle readiness does not

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


## Run 3 — FAIL at step 1 (latest result)

**Classification: Taurhaus product defect.** The #163 base resolves alpha's
session identity to `01a08b57-d623-7511-b516-86d018112f06`, but it does **not**
establish attributed fresh idle readiness in this real trial. At a ready
**Codex 0.153.4 / gpt-5.6-luna low** prompt, the production member activity
snapshot alternates `active` / `uncertain`; it never reports `idle`. The final
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
| 1. Initialize; terminal identity; attributed idle and card delivery | **FAIL** | **Taurhaus**. Terminal facts and session ID exist; attributed idle and submitted onboarding do not. |
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
`launch_ready` observation. The precise cause of the missing readiness is not
proved by this lane; no product fix is proposed here.
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
journal rows remain intact. No evidence-size cap aborted the step.

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

Six synthetic offline controller checks pass. Red-first records show exit 1 for
missing support functions, then exit 0 after implementation. Regression comments
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
The offline six-test suite and cleanup/export audit also exit **0**.
No `src-tauri/` diff exists, so the conditional `just test-rust-unit` gate does
not apply. No full `just check` was run.

Deviations / remaining review: the operator's explicit one-file authentication
authorization overrides the shared contract's generic source restriction.
Step 1 failure mandates stopping before steps 2–6, leaving **no green numbered
runtime commit**. The run-3 controller's remaining-step branches are unexecuted
and make no coverage claim. The independent **Opus evidence lens remains
unavailable in this session** and must be supplied by the orchestrator; the
scratch Claude lead was not used as a reviewer. No workflow PASS or review
approval is claimed.
