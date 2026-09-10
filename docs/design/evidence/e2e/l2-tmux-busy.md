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

Build and gate evidence will be appended when the required commands finish.
No `src-tauri/` source diff is intended; `just test-rust-unit` is conditional on
that diff. An independent Opus lens has not run in this implementer session and
remains with the surrounding workflow; this packet grants no review approval.

## Deviations

- Required scratch authentication was not supplied. Execution stopped before
  initialization; all six runtime steps remain unverified.
- Consequently, no green numbered runtime step exists to commit. Only the
  tested preflight and truthful unavailable evidence are committed.
- No independent Opus evidence result is present in this packet.
