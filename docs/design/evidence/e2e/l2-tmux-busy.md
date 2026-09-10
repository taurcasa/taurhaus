# UNAVAILABLE — Lane 2 stopped at the credential preflight

No runtime claim is established. The executed [preflight](l2-tmux-busy/preflight.py)
exited **78** before step 1: no explicit disposable `auth.json` source was
provided. This is a **harness prerequisite failure**, not a Taurhaus or Mesh
product failure. The shared execution contract explicitly forbids an operator
home credential fallback. The source path was requested during execution.

| Ordered audit step | Outcome | Classification / evidence |
| --- | --- | --- |
| 1. Initialize and capture terminal/runtime identity | NOT RUN — setup unavailable | Harness: [preflight result](l2-tmux-busy/preflight-result.json); no team initialized |
| 2. Ordinary response, observe working, send Q | NOT RUN | Blocked by step 1 prerequisite |
| 3. Busy pending → fresh idle → one submission/reply | NOT RUN | No activity, receipt or exposure evidence |
| 4. Second response, Q2 pending, managed stop, passive lock sample | NOT RUN | No process or terminal lock acquired |
| 5. Resume once, new generation, Q2 once, no Q replay | NOT RUN | No generation or resumed delivery evidence |
| 6. Explicit read/mark, reconcile and teardown | NOT RUN | No journal; preflight created no runtime resources |

The 120-second value is the **idle snapshot freshness bound**, never a required
waiting period. This packet does not assert busy deferral, terminal exclusion,
generation safety, transport submission, explicit reading, model action or PASS.

## Inputs and scope

- Worktree: `/home/mstie/projects/taurhaus-l2-tmux-busy`, branch
  `feat/e2e-l2-tmux-busy`, starting commit
  `6f61f6117a75625ce4ec6d325879730f1800c473`, source protocol 27.
- Mesh: `/home/mstie/projects/mesh-l2`, detached commit
  `a6ee29681a44a472c0ce7b33b9630d4865c93012`. No source or descriptor edit.
- Read the audit's shared execution contract and Lane 2 using the requested
  absolute path in the main worktree, followed by the messaging brief and runtime
  exclusion contract. Read attempt-8 continuation controller, actions, steps,
  support/tests, passive collector and accounting dependencies via `git show`
  in the trial worktree; none was executed.
- Intended runtime: login-only Claude lead and Codex `alpha`, tmux delivery,
  `gpt-5.6-luna`, low. Actual Codex build/model and runtime identities are
  **unavailable** because no real CLI was launched.
- No product edits, alternate checkout edits, installation, hosted seat,
  descriptor changes, process fault injection, load or stress run.

## Exact executed preflight and TDD

From the checkout root:

```sh
python3 -B -m unittest discover -s docs/design/evidence/e2e/l2-tmux-busy -p '*_test.py'
python3 -B docs/design/evidence/e2e/l2-tmux-busy/preflight.py
```

The [test](l2-tmux-busy/preflight_test.py) was written first. Observed red:
exit 1, `ModuleNotFoundError: No module named 'preflight'`
([log](l2-tmux-busy/preflight-red.txt)). After implementation, five tests passed,
exit 0 ([log](l2-tmux-busy/preflight-green.txt)). Tests use generated temporary
files and mocked filesystem boundaries; they neither read real credentials nor
invoke a CLI. This is new preflight logic, not a product regression fix.

The exact executed controller is the credential preflight linked above. It has
no runtime-launch branch. A live six-step controller was not executed or claimed
complete after the binding setup prerequisite failed.

## Cost and cleanup

| Spend category | Inputs / turns | USD | Basis |
| --- | --- | --- | --- |
| Codex seat, including onboarding/recovery/retries | 0 | 0 | No seat process started |
| Claude lead | 0 | 0 | No lead process started |
| Lane runtime total | 0 of 10 Codex inputs | 0 of 0.20 | No paid input submitted |
| Implementer / independent reviewer | Not seat spend | Unavailable here | Separately budgeted by orchestrator |

Turn IDs, session IDs and generations: empty; no token estimates or account usage
rows were collected. No scratch root, credential copy, daemon, tmux server or
Codex child was created by the preflight. No runtime process needed termination.
Build and gate children are waited to completion separately.

## Builds, gates and review

The exact [build/gate driver](l2-tmux-busy/checks.py) ran as:

```sh
python3 -B docs/design/evidence/e2e/l2-tmux-busy/checks.py
```

It sets `CARGO_TARGET_DIR` to this checkout's `src-tauri/target`, or the
designated Mesh worktree's `target`. Before each build it polls
`pgrep -af '(^|/)cargo( |$)'`, with a 30-minute deadline and no interference with
other lanes. Initial wait: 90 seconds; retry wait: 180 seconds; Mesh wait:
290 seconds. These are build queue waits, not seat runtime or idle waits.

| Command | Exit | Observed result |
| --- | --- | --- |
| `bun install --frozen-lockfile` | 0 | Installed missing checkout-local frontend dependencies |
| `just build-daemon` — initial attempt | 101 | Fresh checkout lacked ignored `resources/mesh` placeholder |
| `just ensure-tauri-resources` | 0 | Repository-supported placeholder preparation; no source change |
| `just build-daemon` — retry | 0 | Release binary built from recorded checkout |
| `cargo build --bin mesh` — only in `mesh-l2` | 0 | Requested `target/debug/mesh` built; never executed or installed |
| `just check-quick` | 0 | Rust test compilation, typecheck, 150 frontend test files / 2,495 tests passed |
| `just lint` | 0 | Rust/frontend/workflow/recipe lint passed |
| `just test-contracts` | 0 | 15 renderer + 20 harness + 33 module-boundary tests passed |

[Preparation evidence](l2-tmux-busy/preparation.json) retains the initial build
failure and deduplicated wait observations. [Final check results](l2-tmux-busy/checks-result.json)
retain exact commands, exits, bounded log excerpts and both SHA-256 digests:

- Daemon: `0fdde7d6b7b88126c83c4d9adac950e47b2c195a65dcca4ab60a64b4cdd19811`.
- Mesh: `e2eeb78c86b275d8678bba2131e466fa809d9bb6067269db0aeb677211de96fd`.

No `src-tauri/` diff exists, so the conditional `just test-rust-unit` gate does
not apply. [Cleanup audit](l2-tmux-busy/cleanup-audit.json) records the completed
driver and absence of surviving lane-built executables. No scratch credentials
were copied; none needed removal. An independent Opus lens has not run in this
implementer session and remains with the surrounding workflow; this packet
grants no review approval.

## Requested continuation

The continuation started from a clean tree at `924f0e01`; all completed
preflight work was already committed. No disposable authentication source was
supplied with the continuation, so the six runtime steps remain NOT RUN under
the same harness prerequisite failure. The required source path was requested
again; no credential fallback or paid runtime retry was attempted.

All three requested gates were rerun from the checkout root using the existing
`checks.py` command runner. [Continuation results](l2-tmux-busy/continuation-gates.json)
retain their exact commands, local target paths, durations and bounded excerpts:
`just check-quick` **0**, `just lint` **0**, `just test-contracts` **0**.
No source implementation changed, so there is no new red/green logic cycle or
conditional Rust-unit gate. Cumulative seat spend remains **$0**, with **zero
Codex inputs and zero Claude turns**. This commit records gate evidence, not a
green numbered runtime step.

## Deviations

- Required scratch authentication was not supplied. Execution stopped before
  initialization; all six runtime steps remain unverified.
- Consequently, no green numbered runtime step exists to commit. Only the
  tested preflight and truthful unavailable evidence are committed.
- One build-setup correction: ran the repository resource-placeholder recipe
  after initial `just build-daemon` exit 101, then observed the retry pass.
- No independent Opus evidence result is present in this packet.
