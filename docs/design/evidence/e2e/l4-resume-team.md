# FAIL — L4 step 1: hosted process exited before transport readiness

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
