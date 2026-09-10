# UNAVAILABLE — L4 stopped before step 1: missing disposable authentication

Classification: **harness prerequisite**, not a Taurhaus or Mesh defect. The
runtime preflight exited **78** with:

```text
missing explicit disposable auth.json source; no fallback permitted
```

The shared execution contract in `e2e-coverage-audit.md` requires an explicit
disposable credential source outside real harness homes and says missing scratch
authentication is unavailable. No source was supplied in the task or environment;
the request for its path received no answer before this attempt stopped. No real
harness home was inspected, no credential was read or copied, and no runtime
launch or model input occurred. This packet does not claim successful recovery.

| Step | Required outcome | Observed outcome | Classification |
|---|---|---|---|
| 1 | Initialize; exchange/read markers; retain identities and receipts | NOT RUN — auth preflight blocked before initialize | Harness |
| 2 | Stop all seats via daemon; retain team/task/journal state | NOT RUN — step 1 unavailable | Harness |
| 3 | Accept one pending obligation per stopped seat | NOT RUN — step 1 unavailable | Harness |
| 4 | Call `coordination.resume_team` once; poll its status | NOT RUN — no resume call made | Harness |
| 5 | Verify identities/generations/recovery cards and both pending deliveries | NOT RUN — no resumed generations | Harness |
| 6 | Explicit read/mark; no completed-message replay or member executors; export | NOT RUN — no runtime team; preflight cleanup recorded separately | Harness |

## Exact attempt and evidence

Executed from `/home/mstie/projects/taurhaus-l4-resume-team`, branch
`feat/e2e-l4-resume-team`, initial commit
`6f61f6117a75625ce4ec6d325879730f1800c473`, protocol 27. The separate Mesh worktree
is `/home/mstie/projects/mesh-l4`, commit
`a6ee29681a44a472c0ce7b33b9630d4865c93012`.

```sh
python3 -B docs/design/evidence/e2e/l4-resume-team/controller.py
```

This is a **preflight-only controller**, retained exactly as executed. It contains
no unexecuted implementation of steps 1–6. Supplying auth later cannot launch this
recorded attempt. A continuation needs the six-step runtime controller, with the
same cumulative lane ledger, before making any paid input.

- [Controller](l4-resume-team/controller.py), [auth guard](l4-resume-team/preflight.py),
  [preflight result](l4-resume-team/preflight-result.json).
- [Build/gate controller](l4-resume-team/prepare.py) and
  [provenance](l4-resume-team/provenance.json).
- [Cost ledger](l4-resume-team/cost-ledger.json) and
  [preflight cleanup](l4-resume-team/cleanup.json).

The audit's shared contract and section 4 were read via the requested absolute
source path, followed by the messaging brief and trial `HEAD` versions of the
attempt-8 continuation controller, actions, steps, support and tests. The latter
were read with `git show`, never executed or edited. The original controller's
operator-auth fallback and shared `mesh-push` restoration were not used.

There are **no** team incarnations, session/thread IDs, attachment generations,
message IDs, journal rows, receipts, pane captures, lock samples or resume status
results: none were created. Historical cold-resume skips supply no evidence here.
Codex 0.153.4 was requested but not verified by launching a CLI after preflight.

## Cost ledger

| Item | Inputs / turns | Spend |
|---|---:|---:|
| Alpha tmux Codex | 0 | $0 |
| Beta hosted Codex | 0 | $0 |
| Login-only Claude lead | 0 | $0 |
| Onboarding / recovery / retries | 0 | $0 |
| Total runtime seat spend | **0** | **$0** |

Caps remain ≤16 Codex inputs, ≤$0.25, ≤15 minutes runtime, cumulative across the
lane. Zero is established from no launch/input, not from zero/reset token counters.
Implementer and reviewer spend belongs to the separate orchestrator budget and is
not exposed to this controller.

## Red/green, builds and gates

The offline auth guard was written test-first. The first run exited **1** with
`ModuleNotFoundError: No module named 'preflight'`; after implementation, all three
tests passed, exit **0**. They cover missing explicit auth, forbidden real harness
roots before access, and regular disposable files versus symlinks. All files used
by tests are generated in tempdirs; tests invoke no real CLI, subprocess or network.
This is new harness guard logic, not a product regression fix, so there is no
invented regression-introducing commit.

```sh
python3 -B -m unittest discover -s docs/design/evidence/e2e/l4-resume-team -p 'test_*.py'
```

[Red](l4-resume-team/red.txt), [green](l4-resume-team/green.txt),
[exit records](l4-resume-team/test-results.json),
[tests](l4-resume-team/test_preflight.py).

Build preparation and the three exact requested gates are in progress. This
paragraph will be replaced with observed exit codes before final handoff.

## Cleanup and limitations

No scratch runtime daemon, app-server child, TUI, tmux server, Codex process,
listener or auth copy exists from this attempt because the preflight controller
has no launch/socket/copy operation. Build and gate children are independently
owned by `prepare.py` and terminated/waited in its `finally` block if interrupted.
Only this lane's Mesh descriptor is eligible for restoration.

No product files or plan ledger rows are changed. No numbered runtime step is
green, so there are no per-numbered-step success commits. The evidence/guard work
is committed separately without asserting runtime coverage.

The required independent Opus evidence lens is **not run in this implementer
session**: no Opus model or workflow tool is callable here. It remains the
orchestrator's review route. Missing auth and missing independent review both
preclude PASS. No fault injection, real-home fallback, alternate initialize,
remove/re-add, account/root move, old-binary rollback, load or stress run occurred.
