# Lane 7 — FAIL / unavailable at startup: harness read-receipt predicate

**Step 1 failed before task creation; steps 2–6 were not run.** The seat did
receive and explicitly read onboarding, and the daemon reported fresh idle.
The controller discarded the read receipt because it required `recipient`,
where Mesh's read receipt identifies the actor with `reader_name: alpha`.
This is a **harness defect**, not evidence of a Taurhaus or Mesh defect.
The original controller timeout classified it as Taurhaus; the
[offline adjudication](l7-ledger/adjudication.json) preserves and corrects that
classification. No product change or second runtime trial was made.

| Ordered step | Outcome | Classification / evidence |
| --- | --- | --- |
| 1. Create/assign one task; initialize ledger with its frozen assignment | **FAIL before these operations** | Harness startup predicate; initialization of the team succeeded, but the readiness wait timed out after 120 seconds. [Raw outcome](l7-ledger/run/step1-outcome.json), [adjudication](l7-ledger/adjudication.json). |
| 2. Seat writes standalone note; ledger entry; live render | **NOT RUN** | Blocked by step 1; no artifact or declaration. |
| 3. Seat submits RESULT; source completion and ledger receipts | **NOT RUN** | No lifecycle submission, no source commitment and no intake rejection. |
| 4. Ledger-only retry; identical receipt; no second completion | **NOT RUN** | No retry of any kind. |
| 5. Freeze writer; snapshot; offline current/narrative render | **NOT RUN** | No snapshot or offline read-back claim. |
| 6. Source/ledger reconciliation and export | **NOT RUN** | Required receipt comparison unavailable; [receipt availability table](l7-ledger/run/receipt-table.json). Mandatory failure teardown separately **PASS**. |

The live controller exited **1**, after **124.362 seconds** of runtime, below
the 12-minute cap. It made only the initialization/onboarding input reservation;
no assignment or subsequent send occurred. This run does not establish the lane's
ledger PASS criterion. [Controller exit](l7-ledger/run/controller-exit.json).

## Runtime and receipt evidence

Taurhaus product source was `106f06c7`; the exact executed controller commit was
`64f3d81c`. The designated Mesh worktree remained at `1f7447f`, without descriptor
or source edits. The private daemon answered protocol **27**. Copied native Codex
**0.153.4** ran **gpt-5.6-luna / low**, confirmed by the scratch rollout context.
Both native siblings were copied. The Claude lead was login-only with zero paid
inputs. [Candidate and binary digests](l7-ledger/run/startup-preflight-summary.json),
[private ping](l7-ledger/run/ping.json), [turn evidence](l7-ledger/run/native-turn-meter.json).

Canonical initialization used the builder's actual `DEFAULT_CANONICAL_POLICY`,
lead=`claude/tmux`, alpha=`codex/tmux`, and production
`coordination.initialize_team`. Runtime identity, root, attachment generation,
private socket/pane/PID/start ticks and attributed activity were captured in
[initial runtime](l7-ledger/run/step1-runtime.json),
[pane identity](l7-ledger/run/step1-pane-identity.json),
[operation result](l7-ledger/run/step1-operation.json), and
[final activity](l7-ledger/run/final-activity.json).

For onboarding message `a0ac01e9-82a6-4cb7-b925-444a63f53480`:

- Transport: journal sequence **6**, `stage: submitted`, recipient `alpha`.
- Explicit read: sequence **7**, event `09bb75a9-825b-4a2c-ae5b-0cf48dc4122a`,
  `kind: consumed_by_read`, `reader_name: alpha`, `reader: alpha@l7-ledger`.
- The executed predicate returns **false** over these captured rows and idle;
  the corrected offline predicate returns **true**. This is an offline replay,
  not a rerun or a green runtime step.

The full sanitized [journal segment](l7-ledger/run/team/state/messaging-v2/segments/000001.jsonl)
and [commands/RPCs](l7-ledger/run/events.jsonl) distinguish acceptance, transport
and explicit read. Message bodies are redacted. Pane captures contain at most
60 lines and redact card text. Complete native message/tool-result rows were not
exported; the retained native excerpts cover metering and model identity only.
No claim relies on a prompted reply or on a `stage: pending` row.

## Every recorded spend

Fresh run caps were **12 Codex inputs / USD 0.20 / 12 minutes**. The inherited
meter conservatively records **two turn identities**, one attachment generation,
and one controller input reservation. One turn has full token counters; another
notify-only identity has no retained token counters. **Total dollar spend and
the USD 0.20 cap are therefore unverified**, not zero or assumed within budget.
Metering did not block initialization, the receipt wait, or teardown.

| Turn identity | Evidence | API-equivalent USD | Conservative USD |
| --- | --- | --- | --- |
| `01a08e08-9b9d-7d61-a236-b9ab623301cd` | Completed rollout turn: 20,287 input, 10,752 cached input, 252 output tokens | **0.00242444** | **0.02464680** |
| `01a08e08-9df0-70f3-86be-c40886758378` | Notify-only identity; no corresponding retained rollout counter | **Unavailable** | **Unavailable** |
| Claude lead | Login-only; no model input | **0** | **0** |

Rates inherited from the specified integration/messaging controller are
USD 0.20 / 0.02 / 1.20 per million input / cached input / output tokens.
The conservative calculation prices all recorded input/output at USD 1.20/M.
These are API-equivalent estimates, not invoices. The known subtotal is
USD **0.00242444**, with an additional unmetered identity. Implementer/reviewer
spend is outside seat accounting and belongs to the orchestrator.
[Cost ledger](l7-ledger/run/cost-ledger.json), [counter excerpts](l7-ledger/run/native-turn-meter.json).

## Isolation, teardown, checks and deviations

All runtime homes, project/data/temp roots and sockets were scratch-only;
`TMUX` was absent. Bubblewrap hid operator homes and provided a private PID
namespace; the private daemon listened on probed port **46863**, not 17233.
Exactly one authorized authentication file was copied at mode 0600. No credential
source path, auth contents, member control token, installation ID or account
usage row is in the evidence. Complete daemon row order was retained with private
fields sanitized: **162 source rows / 162 exported rows**, including shutdown.
[Daemon JSONL](l7-ledger/run/taurhaus.log.jsonl), [row-count manifest](l7-ledger/run/daemon-log-manifest.json).

Teardown stopped only owned processes using PID/start-tick checks and namespace
ownership. **No survivors; port closed; scratch root removed; credential copy
removed.** [Teardown audit](l7-ledger/run/cleanup.json).

Offline tests failed first, then passed. The live-schema regression specifically
failed `test_live_read_receipt_schema_uses_reader_name` with `False is not true`;
all **7 tests pass** after the one controller fix. Its `// Regression:` comment
names `030980a7`. [Red](l7-ledger/reader-schema-red.txt), [green](l7-ledger/reader-schema-green.txt).
The original helper is preserved as [support-at-execution.py](l7-ledger/run/support-at-execution.py);
[current controller](l7-ledger/controller.py), [runtime machinery](l7-ledger/runtime.py),
and [tests](l7-ledger/controller_test.py) remain reviewable. No paid rerun followed.

Builds: initial `just build-daemon` exited **101** because the fresh worktree
lacked the ignored Tauri resource placeholder. `just ensure-tauri-resources`
exited **0**, followed by `just build-daemon` **0** and the designated Mesh
`cargo build --bin mesh` **0**. This prerequisite retry preceded all paid input.
[Build ledger](l7-ledger/builds.json), [initial failure](l7-ledger/daemon-build-initial-red.txt).
Cargo used checkout-local targets and one build worker; machine-wide admission
waits only at three or more Cargo processes, with 30-second polls and a 30-minute
queue deadline.

All required gates ran **after teardown** and now pass:

| Command | Initial exit | Final exit | Result |
| --- | --- | --- | --- |
| `just check-quick` | 127 | **0** | Rust test compilation, Svelte check, 150 frontend files / 2,521 tests passed. |
| `just lint` | 127 | **0** | Frontend and repository structure/workflow guards passed. |
| `just test-contracts` | **0** | **0** | 68 tests passed: 15 renderers, 20 harness conformance, 33 module boundaries. |

The two initial 127 exits were missing `svelte-check` / `knip` in this fresh
checkout. `bun install --frozen-lockfile` exited **0**; only those two failed
gates were retried. The final lint retry waited in 30-second polls while three
other Cargo processes were present. No product-file edit was needed.
[Initial gate results](l7-ledger/checks-result.json),
[successful retries](l7-ledger/gate-retries.json),
[Cargo admission observations](l7-ledger/gate-cargo-polls.jsonl),
[final audit](l7-ledger/final-audit.json).
No tracked `src-tauri/` diff exists, so `just test-rust-unit` is not required.

Other deviations: the historical lane-2 worktree is absent, so its versioned
run-3 controller/evidence in this checkout were used. The Mesh RC lacks the
referenced `docs/design/ledger-*.md`; USAGE.md and its implementation/tests were
read instead. Preparatory test/controller commits precede the numbered audit;
no numbered step was green, so no green numbered-step commit exists. No plan
ledger rows were edited. An independent **Opus evidence review is not available
in this executor** and remains for the orchestrator; the workflow is incomplete
without it. No product fix, Mesh commit, descriptor edit, release, install,
fault injection, load test or lifecycle replay occurred.
