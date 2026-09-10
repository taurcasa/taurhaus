# L6 rollback — run2 IN PROGRESS (step 1 PASS)

Run2 follows the **2026-09-11 amendment** in `docs/design/e2e-coverage-audit.md`,
section 6, read from the orchestrator’s main checkout: format first, ownership
second. The exact run2 controller and new sidecars live under
[l6-rollback/run2/](l6-rollback/run2/). Run1 below remains historical evidence.

## Historical run1 — ownership-first contract observation

The mandated ownership-first rollback is refused by Mesh `310144d` while the
team remains canonical: **`delivery: canonical_downgrade_required`**, exit **1**.
Step 1's busy-deferral corner is **UNPROVED**; steps 3–6 were not run. This is a failed operational rollback
under the audit, despite safe preservation of B. No product change, reordered
rollback, forced flag edit, descriptor edit, Mesh commit, install, or release.

| Ordered audit step | Outcome | Classification and observed evidence |
| --- | --- | --- |
| 1. Initialize; deliver/read A; observe B pending during ordinary work | **UNPROVED** | **Harness evidence gap.** Initialization and A's delivery/read are S-runtime verified. B was accepted while working, but the recorded heartbeat predates acceptance and no retained pending obligation names B. Scheduler opportunity and busy deferral are unproved; the PASS recorded in `82e2655b` is superseded. |
| 2. Lead requests `team delivery --owner members` | **FAIL** | **Mesh product contract.** Exit 1, `canonical_downgrade_required`; permanent prerequisite refusal, not a named temporary quiescence refusal. No handoff request, rollback compatibility file, epoch change, or ownership boundary. B intact. |
| 3. Settle/retry once; verify one committed ownership boundary | **NOT RUN** | Stopped at permanent step-2 failure as required; no retry or ownership-transfer claim. |
| 4. `team format --legacy --quiescent`; reconcile B | **NOT RUN** | No downgrade command or reconciliation report. Resulting format remains **2**, owner **team**. |
| 5. Guarded RC member executor; account for B; fresh C reply | **NOT RUN** | No member executor started, no C created, no legacy-delivery claim. |
| 6. Explicit read/ack; reconcile A/B/C; export and teardown | **NOT RUN** | No rollback read/ack or reconciliation. Mandatory failure export/teardown independently **PASS**. |

[Controller outcome](l6-rollback/run/controller-exit.json),
[step-2 command and exit](l6-rollback/run/step2-command.json),
[exact commands/RPCs/results](l6-rollback/run/events.jsonl).
The live controller exited **1** after **24.921 seconds**. All positive waits
provided 90–120 second deadlines; they could finish early on observed success.
Busy/lock refusals have bounded retries; this permanent prerequisite refusal is
not transient and was not retried.

## What the rollback refusal preserved

The exact command, executed as the registered lead inside the private namespace:

```sh
/tmp/th-l6-qgmptfx3/home/.local/bin/mesh team delivery --owner members \
  --claude-dir /tmp/th-l6-qgmptfx3/claude --team l6-rollback --name lead
# exit 1: error: IO error: delivery: canonical_downgrade_required
```

`/home/mstie/projects/mesh-l6/src/delivery/ownership.rs:278–281` rejects
`Members` when `format::supported(..., 2) == 2`, before lead authorization,
quiescence checks, handoff creation, or epoch fencing. This explains the
observed refusal. Changing the command order would violate this lane's ordered
steps, so no alternate rollback was attempted.

| Identity / disposition | Observed value |
| --- | --- |
| Team incarnation | `19889ba6d58a872128b635b97967711ec220ee32b0bb7a2d7938a597ea8f7042` |
| Epoch / owner / format before and after refusal | **2 / team / 2** |
| Handoff request / rollback compatibility / ownership-change events | **Absent / absent / 0** |
| Alpha native session | `01a08d8f-8beb-7731-96ef-a3f92eb457f4` |
| Attachment / context generation | **1 / "0"** |
| Socket / pane / pane PID / start ticks | `/tmp/th-l6-qgmptfx3/tmux/tmux-1000/default` / `%2` / **135** / **30192684** (private namespace) |
| A marker / logical ID | `A-c79df580` / `d94184ac-d1e6-48e7-a17a-65ac2ed1fb9c` |
| A delivery ID / disposition | `d8a7d12f-6211-4407-8dc6-25015f435672` / **one submitted receipt + consumed_by_read** |
| B marker / original logical ID | `B-cd1489b7` / `cbb72528-371a-4bc0-90b6-1a150c0b318d` |
| B delivery ID | `244196ad-289f-46be-940e-0b984a60e1bd` |
| B legacy mapping allocated at acceptance | `62ded8e8-480d-4880-823c-332c9f514cfe`; this is not evidence of legacy projection or delivery |
| B final disposition | **Accepted, unread; no attempt_started, submitted, or read receipt.** Busy deferral is unproved because scheduler opportunity was not demonstrated. |
| C | Not created |

B's purported pending observation at `2026-09-10T23:03:31.537Z` used production activity
`likely_working`, observed at `23:03:31.085Z`. It was **acceptance without a
receipt while working**, only 204 ms after acceptance at `23:03:31.3336Z`.
The owner's heartbeat was `23:03:30.8987Z`, before acceptance, with no
`pending_since` or `last_defer_reason`; neither retained pending obligation
names B. This does not prove that the owner had an opportunity to defer B.
The busy-deferral corner is therefore unproved, and step 1 cannot be called
PASS. B's logical and delivery identities remain in the journal after refusal.
The corrected controller requires a heartbeat at or after this acceptance and
keeps polling within the existing 90-second deadline. The original runtime
sidecars, including the unsupported step-1 PASS, remain historical evidence;
this report supersedes that classification without rewriting observations.
The step-2 boundary snapshot was byte-identical to the step-1 snapshot and was
deduplicated with an explicit alias.
[Pending observation](l6-rollback/run/pending/cbb72528-371a-4bc0-90b6-1a150c0b318d.json),
[boundary snapshot](l6-rollback/run/step1-pending-boundary.json),
[A transport/read history](l6-rollback/run/A-history.json),
[complete canonical journal](l6-rollback/run/team/state/messaging-v2/segments/000001.jsonl),
[final runtime](l6-rollback/run/final-runtime.json).

## Candidate and isolation

Taurhaus product **`a7e6db7e484290542ac35ead468f9809968bb4f0`**, branch
`feat/e2e-l6-rollback`; private daemon ping confirmed **protocol 27**, version
0.9.7. Mesh built only in `/home/mstie/projects/mesh-l6` at
**`310144de1f7939e7fbb2d80ab42ebd00580ce15d`**. This guarded RC still reports
numeric version 0.2.29; it is not the excluded stock 0.2.29 executor.
There was no hosted seat and no descriptor edit. Native Codex reported
**0.153.4**, with actual launch **gpt-5.6-luna / low**.

| Copied binary | SHA-256 |
| --- | --- |
| Taurhaus daemon | `5ca8150fe8c15d3bf770a51b7eaf7b94be7b18b0bd56f82c0521d500563aa21e` |
| Mesh RC | `e3ddbd2184433aa08889f3fcc3bdc33443057f2d7fa7e819e97facd494e22991` |
| Codex | `56ef98ab4032d317ab26e9b5e5a175650717351edb16ed9cde0cb6d1734d62da` |
| codex-code-mode-host | `3e85d67471825f73d02ff5f7e047ca1f6ca8caa3f59e4c6e8d9ca6ca7302cb45` |

Runtime HOME, all harness/config/data/temp roots, project, and tmux socket were
scratch-only. Bubblewrap hid operator homes and supplied a private PID namespace;
TMUX was absent. The private daemon used probed port **47513**, never 17233.
Exactly the operator-authorized auth file was copied into empty CODEX_HOME with
mode **0600**, never logged/exported, then removed. The Claude lead stayed in its
unauthenticated startup/theme screen and took **zero model turns**.
Production `coordination.initialize_team` used the builder's actual canonical
messaging policy and creation-time `delivery: tmux` for both seats.
[Candidate](l6-rollback/run/candidate.json), [ping](l6-rollback/run/ping.json),
[initialization](l6-rollback/run/step1-operation.json),
[command-capable instructions](l6-rollback/run/scratch-AGENTS.md).

## Every observed spend and the metering limit

| Input / turn ID | Input / cached / output tokens | API-equivalent USD |
| --- | --- | --- |
| Onboarding — `01a08d8f-9547-7c20-ad72-6dd2e5545982` | 20,448 / 6,912 / 297 | **0.00320184** |
| A delivery — `01a08d8f-bda8-7692-a25d-ea84be176fa9` | 42,048 / 36,096 / 262 | **0.00222672** |
| Ordinary B-work — `01a08d8f-dcf1-70e0-90f9-4428484aa469` | **Unknown**; still active at mandatory failure teardown | **Unknown, not zero** |
| B pending reservation | No terminal submission or model turn | **0 observed** |
| C / recovery / retries / compaction | Not run | **0** |
| Claude model inputs | 0 | **0** |
| Implementer / independent reviewer | Enclosing orchestrator accounting | Not exposed to this lane |

**Known spend: $0.00542856 plus the unmetered ordinary turn.** The conservative
priced-subset estimate is $0.075666; it is not a total bound. Therefore the
**$0.20 total cap is unverified**, not claimed satisfied. The observed input cap
is respected: **3 submitted inputs, 3 rollout turn IDs, 4 reservations including
unsent B, one attachment generation**, against 10 allowed. A separate native
notify-only ID is retained and is not counted as another model input.
No metering predicate blocked the ownership command or any lifecycle operation.
No additional paid attempt was made.

The [cost ledger](l6-rollback/run/cost-ledger.json) retains all five usage rows,
individual deltas and turn identities. Rates are inherited packet estimates:
$0.20/M uncached input, $0.02/M cached input, $1.20/M output; not invoices.
The native transcript export precedes namespace shutdown, so it cannot establish
an eventual final billing row for the interrupted turn. This is an evidence
limitation, separate from the proven Mesh refusal.

## Export, teardown, tests and gates

The **complete emitted daemon JSONL (101 rows)** is retained, including rows
emitted during shutdown; no event-selection or tail filter. Stderr records
`Received shutdown signal` and `taurhaus-daemon shut down cleanly`. The journal,
workflow rows, native transcript, receipt/activity snapshots and passive lock
samples are retained. All pane excerpts are at most 60 lines. The deterministic
packaging pass trims trailing blank pane lines and aliases byte-identical
snapshots; complete streams are retained separately. Standalone command logs
remove trailing whitespace only, with hashes in
[the normalization record](l6-rollback/command-log-normalization.json); command
and exit JSON records retain their original output excerpts.
[Daemon JSONL](l6-rollback/run/taurhaus.log.jsonl),
[daemon stderr](l6-rollback/run/daemon-stderr.txt),
[manifest](l6-rollback/export-manifest.json), [packaging script](l6-rollback/pack.py).

Teardown used only owned PID/start-tick identities and the daemon's normal
SIGINT shutdown. The namespace reaped its descendants. **No scratch daemon,
team/member executor, tmux server, Codex, code-mode host or Claude survives;
port closed, auth copy removed, scratch root removed.** A separate read-only
audit verified all 69 retained files, 13 aliases, complete JSONL rows, bounded
panes, absence of credential-shaped data/operator-home paths in runtime exports,
and no surviving recorded process identity.
[Cleanup](l6-rollback/run/cleanup.json), [audit](l6-rollback/final-audit.json).

**22 offline tests pass**: six lane tests plus 16 inherited shared-harness and
credential checks. Initial red: transient retry, duplicate/unverified boundary,
and changed independent message state. Additional reds: the inherited
object-only parser raised `JSONDecodeError` on a generated legacy array; the
meter omitted a legacy executor input (`2 != 3`). All tests use synthetic values,
tempdirs and mocks; none launches a CLI or reads real harness credentials.
The parser regression comment names introducing controller commit `57c8ff36`.
No product regression was fixed.
[Original red](l6-rollback/red.txt), [array red](l6-rollback/legacy-array-red.txt),
[meter red](l6-rollback/legacy-meter-red.txt), [green](l6-rollback/green.txt).

| Exact root command | Exit | Outcome |
| --- | --- | --- |
| `just check-quick` | **1 initial; 0 retry** | Initial SessionHistory failures; unchanged exact-command retry: **2,519 tests pass**, Rust compile and typecheck pass |
| `just lint` | **0** | Rust/frontend/workflow/recipe lint pass |
| `just test-contracts` | **0** | **68 tests pass** (15 renderer, 20 harness, 33 boundary) |

[Exact gate results](l6-rollback/gates-result.json). The initial quick gate had three
SessionHistory assertion failures and one `transformCallback is not a function`
unhandled rejection. A focused run passed all 28 tests (exit 0), then the exact
full quick gate passed without product changes. Both runs are retained; no
flaky test was hidden or relabeled as a product fix.

No `src-tauri/` diff, so conditional `just test-rust-unit` does not apply.
The initial `just build-daemon` exited **101** because ignored `resources/mesh`
was absent. `just ensure-tauri-resources` exited **0**; the retry and Mesh build
both exited **0**. `bun install --frozen-lockfile` exited **0**. Cargo admission
was polled before every build/gate, waiting only if at least three Cargo
processes existed, with 30-second polls and a 30-minute bound. Targets remained
checkout-local; no gate overlapped the paid window.

## Reproduction and deviations

From this checkout root, with a new output directory and separately tracked
remaining run budget:

```sh
python3 -B -m unittest discover -s docs/design/evidence/e2e/l6-rollback -p '*_test.py'
just ensure-tauri-resources
python3 -B docs/design/evidence/e2e/l6-rollback/checks.py build
python3 -B docs/design/evidence/e2e/l6-rollback/controller.py --auth-source "$AUTHORIZED_SOURCE" \
  --out docs/design/evidence/e2e/l6-rollback/run-review
# Only after teardown:
python3 -B docs/design/evidence/e2e/l6-rollback/checks.py gates
```

`run-review` must not exist; choose another fresh directory for a later trial.
The default remains `run`, with the same must-not-exist guard. The historical
`pack.py` and `audit.py` target the original `run` packet, not the new output.
`AUTHORIZED_SOURCE` is the exact standing-authorized single file pinned in
[preflight.py](l6-rollback/preflight.py), shared by the standalone preflight and
[controller](l6-rollback/controller.py); there is no home fallback. Reproduction
starts at step 1 and stops at any failed step; it cannot skip ahead to steps 3–6.

- The prescribed L2 worktree was absent (Git exit 128). Its run-3 controller was
  recovered read-only from local commit `2690352e`; the same commit's run-5
  corrections supplied the mandated shared startup/submission/delivery rules.
  No other Taurhaus checkout was changed.
- Mesh's `docs/design/delivery-stage2b-brief.md` identifies itself as a membership
  addendum and says the full brief is absent. The separate
  `docs/analysis/delivery-stage2-s2b.md` exists and was read during this fix round;
  it is an implementation/isolated acceptance packet, not the full brief. The
  original lane read the addendum, `docs/analysis/journal-stage3-storage.md`,
  `USAGE.md`, and rollback source contracts. No alternate rollback order was
  inferred as permission.
- Build-resource preparation/retry was necessary; no tracked product change.
- The first quick gate failed in existing SessionHistory tests; focused diagnosis
  and one unchanged full retry passed. Lint and contracts each passed once.
- Step 2's permanent Mesh refusal stopped steps 3–6. The dollar cap remains
  unverified because the ordinary input was interrupted before final metering.
- Independent Opus evidence review belongs to the enclosing small-change
  workflow and did not execute inside this implementer lane. No review approval
  or complete workflow PASS is claimed.

## Review fix round

All five supplied findings were confirmed and addressed in the named harness
and report files; no product or original runtime sidecar changed. Four new
offline regression tests name introducing commit `82e2655b`. Before the fixes,
the 26-test suite exited **1** with four failing subtests and two errors:
working activity admitted missing/stale owner heartbeats, standalone preflight
rejected the authorized pin (78), and output selection was unavailable.
After the fixes the same suite exited **0**, with **26 tests passing**, including
equal/newer heartbeat acceptance, submission exclusion, shared pin enforcement,
CLI output selection/default, and existing-evidence preservation. Credentials
were mocked; no real harness or credential access occurred in these tests.

No paid rerun occurred: **0 new Codex/Claude inputs, $0 new seat spend**, and no
daemon, member executor, tmux server or model seat was started. Historical spend
remains $0.00320184 + $0.00222672 + one unknown ordinary turn; its total cap
remains unverified. Step 1 is unproved (harness); step 2 remains FAIL (Mesh);
steps 3–6 remain NOT RUN. The dead boundary bindings and unused controller state
were removed. Replaying the retained B observation through the corrected
predicate returned no pending evidence, as required.

| Fix-round exact root command | Exit | Observed result |
| --- | --- | --- |
| `just check-quick` | **0** | Rust compile/typecheck and all **2,519** frontend tests pass |
| `just lint` | **0** | Rust/frontend/workflow/recipe lint pass |
| `just test-contracts` | **101 initial; 0 retry** | Retry passes all **68** contracts (15 renderer, 20 harness, 33 boundary) |

The initial contracts run scanned its own output under `.check-logs/l6-review`
and flagged its printed retired-tool test name. Moving only these generated
logs to excluded `src-tauri/target/l6-review/` resolved the harness artifact
collision; the exact command passed unchanged. Both attempts are retained in
that local ignored directory; the original committed gate sidecars are unchanged.
Cargo admission found zero existing Cargo processes before each gate. Every
gate used this checkout's `src-tauri/target`, one build job, and ran after the
historical teardown, with no paid window. No tracked `src-tauri/` change occurred,
so `just test-rust-unit` was not required. This fix round adds no live rollback
coverage and leaves the original unknown spend explicitly unresolved.

Run2 step 1: PASS (S-runtime); see run2/run/step1-outcome.json.
