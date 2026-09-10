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

Build/gate preparation ran separately after the runtime preflight stopped:

```sh
python3 -B docs/design/evidence/e2e/l4-resume-team/prepare.py
```

It polled `pgrep -af '(^|/)cargo( |$)'` every 10 seconds and observed idle after
880.9 seconds, within the 30-minute cap. It did not stop any competing build.
[Deduplicated wait observations](l4-resume-team/cargo-wait.json).

| Command | Exit | Observed result |
|---|---:|---|
| `cargo build --bin mesh` in the separate Mesh worktree | 0 | Built `target/debug/mesh` with the scratch-only 0.153.4 trial descriptor |
| `just build-daemon` from the checkout root | **101** | Harness/setup failure: `resource path resources/mesh doesn't exist`; no daemon executable/digest claimed |
| `bun install --frozen-lockfile` | 0 | Installed checkout dependencies |
| `just check-quick` | **0** | Rust test compilation, typecheck and 2,495 frontend tests passed |
| `just lint` | **0** | All lint recipes passed |
| `just test-contracts` | **0** | All three requested contract test binaries passed |
| `git -C /home/mstie/projects/mesh-l4 checkout -- src/delivery/app_server/capabilities.rs` | 0 | Restored the descriptor; Mesh worktree verified clean |

The daemon build failure is an additional **harness/setup** result, not a product
runtime failure. The gate recipes subsequently provisioned their normal local
resource placeholders. No runtime retry or daemon startup followed. Builds used
checkout-local targets; gates ran from the assigned checkout with scratch harness
homes. No `src-tauri/` diff exists, so `just test-rust-unit` was not required.

The trial descriptor changed only the 0.153.4 entry to `trial`/enabled, with
`host=taurhaus-daemon-owned-thread/1`, `configuration=strict-config/1`, and
`trust=daemon-owned/1`, verified against `hosted.rs`. The wildcard stayed disabled.
The binary was never installed or executed; runtime scratch installation was not
reached. Its SHA-256 is
`52ea8dc17f4aecfef7795b4f3999965570a9a60f9e0c6cb98185ade53ff85613`.

Exact command/exit records and bounded diagnostic tails:
[Mesh build](l4-resume-team/mesh-build.json),
[trial diff](l4-resume-team/mesh-trial-descriptor.diff),
[daemon build](l4-resume-team/daemon-build.json),
[daemon error](l4-resume-team/daemon-build.txt),
[binary digest](l4-resume-team/binaries.json),
[check-quick](l4-resume-team/gate-check-quick.json),
[lint](l4-resume-team/gate-lint.json),
[contracts](l4-resume-team/gate-test-contracts.json),
[restoration](l4-resume-team/descriptor-restore.json).
Each build/gate `.json` has a same-basename `.txt` tail limited to 60 lines.

## Cleanup and limitations

No scratch runtime daemon, app-server child, TUI, tmux server, Codex process,
listener or auth copy exists from this attempt because the preflight controller
has no launch/socket/copy operation. Build and gate children are independently
owned by `prepare.py` and terminated/waited in its `finally` block if interrupted.
Only this lane's Mesh descriptor was restored. The final single `/proc` census
found **no process retaining the private build-root environment**; its scratch
root was removed. The preparation controller exited 0 (individual command exits
above remain authoritative). No auth copy was ever created. See the
[build owner identity](l4-resume-team/build-owner.json) and
[final audit](l4-resume-team/final-audit.json). The audit found no disallowed
operator-home paths in retained evidence. Unrelated Cargo processes were left
alone.

No product files or plan ledger rows are changed. No numbered runtime step is
green, so there are no per-numbered-step success commits. The evidence/guard work
is committed separately without asserting runtime coverage.

The required independent Opus evidence lens is **not run in this implementer
session**: no Opus model or workflow tool is callable here. It remains the
orchestrator's review route. Missing auth and missing independent review both
preclude PASS. No fault injection, real-home fallback, alternate initialize,
remove/re-add, account/root move, old-binary rollback, load or stress run occurred.
