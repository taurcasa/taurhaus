# L6 rollback — IN PROGRESS (step 1 PASS)

Evidence-only lane on `feat/e2e-l6-rollback`, Taurhaus product `a7e6db7e`,
protocol 27, Mesh `310144d` in `/home/mstie/projects/mesh-l6`. No product,
descriptor, installation, release, or plan-ledger change.

The binding lane spec and audit were read from their explicitly named absolute
paths. The prescribed L2 worktree no longer exists (Git command exit 128).
Its run-3 controller was recovered read-only from local commit `2690352e`;
startup preflight, confirmed submissions, receipt + attributed fresh idle + read,
reply evidence, and non-gating metering incorporate that same commit's run-5
shared-harness corrections. No other Taurhaus checkout was changed.
The full delivery-stage2b brief is absent in Mesh; its membership addendum,
`docs/analysis/journal-stage3-storage.md`, `USAGE.md`, and the rollback source
contracts were read. The two sanctioned rollback commands are used as lead.

| Ordered audit step | Outcome | Classification |
| --- | --- | --- |
| 1. Initialize, deliver/read A, observe B pending while busy | PASS | S-runtime verified |
| 2. Request member ownership; preserve refusal/handoff details | NOT RUN | Ordered after 1 |
| 3. Settle/retry once; verify one ownership boundary | NOT RUN | Ordered after 2 |
| 4. Quiescent legacy downgrade; retain B identity/read/transport | NOT RUN | Ordered after 3 |
| 5. Guarded RC executor; B accounted for and fresh C reply | NOT RUN | Ordered after 4 |
| 6. Explicit read/ack, reconcile, export, teardown | NOT RUN | Ordered after 5 |

## Reproduction and validation

From this checkout root:

```sh
python3 -B -m unittest discover -s docs/design/evidence/e2e/l6-rollback -p '*_test.py'
python3 -B docs/design/evidence/e2e/l6-rollback/checks.py build
python3 -B docs/design/evidence/e2e/l6-rollback/controller.py --auth-source "$AUTHORIZED_SOURCE"
# Only after teardown:
python3 -B docs/design/evidence/e2e/l6-rollback/checks.py gates
```

`AUTHORIZED_SOURCE` is exactly the operator-authorized single Codex auth file,
pinned independently in the controller. It is copied alone into empty scratch
CODEX_HOME (0600), never exported, then removed. Each run needs a fresh output
directory; reruns require their remaining budget to be carried forward.
The controller pins both native Codex siblings, version 0.153.4, Luna/low,
a private PID namespace, a private tmux server and a probed non-default daemon
port. The Claude lead is login-only and takes no model turn.

Five new offline rollback tests observed three assertion-level reds before
implementation: transient busy retry, one verified ownership boundary, and
independent message-state preservation. All 21 offline tests now pass, including
16 reused isolation/shared-harness tests. The inherited object-only parser also failed on a generated legacy read array; the parser now accepts both shapes. No product regression was fixed.
See [red](l6-rollback/red.txt), [green](l6-rollback/green.txt), and
[exact controller](l6-rollback/controller.py).

Run caps: at most 10 Codex starts/inputs and USD 0.20, 12 minutes live runtime.
Model costs are token-derived API-equivalent estimates using the inherited
packet's rates; they are not invoices. Implementer/reviewer spend is controlled
by the orchestrator and is not exposed to this lane. Independent Opus evidence
review belongs to the enclosing small-change workflow; no approval is claimed here.
