# PASS — run 10: all six steps

**Latest verdict:** run10 completed all six ordered steps: native AMBER/BIRCH exchange, explicit lead read, genuine compact hook recovery (context 0→1), then FERN received and acted on once followed by explicit read/mark. Every numbered step was committed when green. No product changes.

Known seats **$0.06517434**, separate approved Opus review **$0.18747000**; combined known **$0.25264434**. Compact summarizer and foreign-notify costs remain unknown as permitted by the ruling. Inputs **Claude 5/8, Codex 5/12**. Gates **0/0/0**, controller/driver **0/0**, zero survivors, complete **432-row daemon JSONL** and sanitized `sessions/` directory retained. [Run10 report](l1-claude-lead/run10/report.md), [disposition](l1-claude-lead/run10/final-disposition.json), [runtime reconciliation](l1-claude-lead/run10/runtime-reconciliation.json), [spend](l1-claude-lead/run10/spend-summary.json).

## Run9 — superseded by run10

**Run9 verdict (superseded):** run9 observed real native compaction recovery, but the inherited collector/predicate omitted Claude attachment rows and timed out at step5. The original failure remains retained; its evidence-backed classification is harness. FERN step6 was not run. This is not an end-to-end PASS.

Known metered seats **$0.04418519**, separate Opus review **$0.11605500** (approved); compact summarizer and foreign-notify spend unknown. Inputs **Claude 4/8, Codex 4/12**. Gates **0/0/0**, controller/driver **2/2**, zero survivors, complete **503-row daemon JSONL** and sanitized `sessions/` directory retained. [Run9 report](l1-claude-lead/run9/report.md), [disposition](l1-claude-lead/run9/final-disposition.json), [native attachments](l1-claude-lead/run9/step5-native-hook-attachments.json), [spend](l1-claude-lead/run9/spend-summary.json).

## Run8 — superseded by run9

**Run8 verdict (superseded):** run8 stopped at the required lead session attribution check after 65 seconds. Claude authenticated and consumed native onboarding, but daemon `session_id` / transcript path stayed null. Underlying product ownership is unresolved; this is not an authentication refusal. No controller input, assignment publication, compact or FERN occurred.

Known seats **$0.01570408**, **1/8 Claude and 1/12 Codex onboarding reservations**, foreign startup-notify usage unknown. Gates **0/0/0**, driver/controller **2/2**, zero survivors, all **197 daemon rows** retained. [Run8 report](l1-claude-lead/run8/report.md), [disposition](l1-claude-lead/run8/final-disposition.json), [spend](l1-claude-lead/run8/spend-summary.json), [teardown](l1-claude-lead/run8/teardown-audit.json).

## Run7 — superseded by run8


**Run7 verdict (superseded):** run7 stopped at step 2; steps 3–6 NOT RUN. Mesh reported an unknown onboarding delivery outcome before the inherited controller appended its no-read setup input to the pending notification. Alpha replied READY without reading the card. The raw `FAIL/taurhaus` classification is retained; the reviewed report limits attribution to a compromised harness prerequisite and leaves the preceding transport behavior unresolved. No live publication or compaction claim.

Known seats **$0.01308100**, **1/8 Claude and 2/12 Codex reservations**, zero compactions; foreign startup-notify usage unknown. Required gates **0/0/0**, driver/controller **2/2**, zero runtime/gate survivors, all **211 daemon rows** retained. [Run7 report](l1-claude-lead/run7/report.md), [disposition](l1-claude-lead/run7/final-disposition.json), [spend](l1-claude-lead/run7/spend-summary.json), [teardown](l1-claude-lead/run7/teardown-audit.json). Independent review recorded in the run7 section below.

## Run6 continuation — superseded by run7

**Run6 verdict (superseded):** steps 1–4 PASS; step 5 UNAVAILABLE — harness; step 6 NOT RUN. The parser correction worked, but the daemon-only fixture did not exercise the app task persistence/snapshot-publication path. The real Mesh assignment reached Claude; operational task context remained empty through 65 seconds. No `/compact` or recovery PASS is claimed.

Known continuation seats **$0.04618644**, **3/8 Claude and 4/12 Codex reservations**, zero compactions. All required gates **0/0/0**, driver/controller **2/2**, zero survivors, all 407 daemon rows retained. [Report](l1-claude-lead/run6/continuation/report.md), [disposition](l1-claude-lead/run6/continuation/final-disposition.json), [snapshot diagnosis](l1-claude-lead/run6/continuation/snapshot-diagnosis.json), [spend](l1-claude-lead/run6/continuation/spend-summary.json).

## Initial run6 attempt — superseded by requested continuation

**Initial run6 verdict: steps 1–4 PASS; step 5 UNAVAILABLE — harness; step 6 NOT RUN.** Mesh assigned the real in-progress task, but a warning preceding its JSON broke the controller parser before `/compact`. No product failure or recovery PASS is claimed. The parser was corrected offline after teardown; no paid retry.

Known seats **$0.04608355** plus unknown unfinished assignment-response spend; **3/8 Claude, 4/12 Codex reservations**, zero compactions. Required gates **0/0/0**; driver/controller **2/2**; zero survivors. [Run6 report](l1-claude-lead/run6/report.md), [disposition](l1-claude-lead/run6/final-disposition.json), [spend](l1-claude-lead/run6/spend-summary.json), [complete daemon JSONL](l1-claude-lead/run6/taurhaus.log.jsonl).

## Historical run5 summary — superseded by run6

**Historical run5 verdict: steps 1–4 PASS; step 5 UNAVAILABLE — harness; step 6 NOT RUN.** Alpha retained its own session with notify-sourced idle; both marker sends landed, both replies reached Claude natively, and explicit read receipts followed. The reused controller falsely counted compact-generated rows as extra inputs and stopped; the unassigned fixture also caused the hook to skip recovery. No product change.

Known metered seats **$0.04779965**, compact usage **unknown**; complete attempt spend is unverified. Four Claude reservations/submissions, four reserved Codex inputs, one compact. All required gates **0/0/0**, runtime/controller exit **1**, zero survivors. See the [run5 report](l1-claude-lead/run5/report.md), [disposition](l1-claude-lead/run5/final-disposition.json), [spend](l1-claude-lead/run5/spend-summary.json), and [complete daemon JSONL](l1-claude-lead/run5/taurhaus.log.jsonl).

## Historical run4d summary — superseded by run5

**Historical run4d verdict: step 1 PASS; step 2 FAIL — taurhaus (activity/idle signal ownership inference); steps 3–6 NOT RUN.** The corrected controller passed native readiness, and Claude executed both marker sends. Both were accepted, but alpha received neither within the full 65-second delivery window: Taurhaus exported uncertain activity and Mesh deferred with “activity not freshly idle.” This differs from run4c's retracted harness readiness refusal.

| Ordered step | Outcome / classification |
|---|---|
| 1. Initialize; signed-in, team-bound Claude; roots, owner, hooks | PASS |
| 2. Two Claude sends and recipient exposure | FAIL — taurhaus; accepted, no exposure |
| 3. Seat read/replies and native Claude uptake | NOT RUN — dependency on 2 |
| 4. Claude explicit read/mark with cursors | NOT RUN — dependency on 2 |
| 5. Ordinary compact and genuine recovery card | NOT RUN — dependency on 2 |
| 6. Fresh native mail after recovery | NOT RUN — dependency on 2 |

[Run4d report](l1-claude-lead/run4d/report.md), [final disposition](l1-claude-lead/run4d/final-disposition.json), [runtime excerpts](l1-claude-lead/run4d/review-excerpts.json), [complete daemon JSONL](l1-claude-lead/run4d/taurhaus.log.jsonl), [cleanup audit](l1-claude-lead/run4d/teardown-audit.json). Fresh attempt spend: Claude **$0.02040220**, Codex **$0.00470888**, seats **$0.02511108**; upper-rate amount **$0.28845840**; counts **2/8 Claude, 3/12 reserved Codex**, zero compact. Runtime **113.686 s**, driver/controller exit **2**, zero survivors. Required gates **check-quick 0, lint 0, test-contracts 0**. Independent Opus review **approved**; reviews cost **$0.343795** including the failed first invocation; seats plus reviews **$0.36890608**. No product changes. Detailed historical spend and deviations are in the report.

## Run 4c latest-summary record — superseded by run4d


**Latest verdict (corrected by review): step 1 PASS; step 2 UNAVAILABLE — harness; steps 3–6 NOT RUN.** The real run on 2026-09-10 stopped on a derived activity prerequisite after alpha had consumed its onboarding card and completed its native turn. The spec's product-FAIL trigger (card staying pending) did not occur. Neither marker send was attempted, so this is not a passing step 2 or a demonstrated Taurhaus product failure.

| Ordered audit step | Outcome / classification | Evidence |
|---|---|---|
| 1. Initialize; signed-in, team-bound Claude; roots, owner and hooks | **PASS** | [Step 1 outcome](l1-claude-lead/run4c/step1-outcome.json); committed `4cb26f5f`. |
| 2. Claude sends two markers; recipient exposure | **UNAVAILABLE — harness** | [Native transcript](l1-claude-lead/run4c/codex-transcript.json), [review excerpts](l1-claude-lead/run4c/review-excerpts.json): native turn completed, composer empty, card consumed; derived `uncertain` activity stopped the controller before sends. |
| 3. Alpha explicit read/replies and Claude native uptake | **NOT RUN — dependency on 2** | [Step driver exits](l1-claude-lead/run4c/driver-steps.json). |
| 4. Claude explicit read/mark with cursors | **NOT RUN — dependency on 2** | No lead read or read receipt claimed. |
| 5. Ordinary `/compact` and genuine recovery card | **NOT RUN — dependency on 2** | No compact input or post-compact generation. |
| 6. Fresh native mail after recovery | **NOT RUN — dependency on 2** | Failure export and [cleanup](l1-claude-lead/run4c/cleanup.json) completed separately. |

The retained [run4c report](l1-claude-lead/run4c/report.md) and [run4c final disposition](l1-claude-lead/run4c/final-disposition.json) contain the **superseded `FAIL — taurhaus` classification**. This review correction supersedes their interpretation, without rewriting the raw run packet or implying a new execution. Journal sequence 7 records `consumed_by_read` by alpha, context `explicit-mesh-cli`; the transcript contains `[taurhaus] recovery_card` and a completed native turn. The activity snapshot remains a required recorded observation, with the missing derived-idle condition disclosed as a deviation.

Run4c spend: Claude **$0.01179540**, Codex **$0.00511548**, seats **$0.01691088** (upper-rate estimate **$0.14121140**), inputs **1/8 Claude and 1/12 Codex**, zero typed inputs or compactions. Its independent review cost **$0.164920**, seats plus review **$0.18183088**. Earlier seats **$0.10193487**, earlier completed reviews **$0.910785**, earlier interrupted startup and timed-out review **unknown**, implementer/orchestrator usage unavailable. [Spend summary](l1-claude-lead/run4c/spend-summary.json). This local correction starts no model seats and incurs **$0 new seat/reviewer spend**; no earlier spend is charged to a future attempt.

Run4c driver/controller exit **2**, step drivers **0 / 1**; runtime **81.524 seconds**, zero survivors, private listener closed and scratch auth removed. The complete 226-row daemon JSONL remains retained. This fix round preserves that completed run: the controller rejects overwriting its existing `events.jsonl`, and the requested local scope excludes replacing sidecars or starting another attempt. The generic driver rename is deferred because it requires changing callers outside the named files. No product, Rust, descriptor, plan-ledger or other-checkout edits.

Review-fix verification and gate exits are recorded in the run-4c review correction below. Original run4c gates were all **0**; they do not establish the unrun marker or compaction behavior.

## Continuation run (superseded)

Historical run-3 verdict: run 3 passed the repaired harness and alpha attribution checks, then stopped on canonical replies lacking delivery attempts and native uptake. Mesh roster exclusion is the proximate mechanism; the flag writer remains unresolved, including claude-code, and Taurhaus skipped its recovery ensure. See [run-3 evidence](l1-claude-lead/run3/report.md); historical runs remain below.

The user authorized continuation from `ff8dc10c`. Setup repair is committed in
`9e71cc15`; green numbered step 1 is committed in `5a7fb7ee`. The first run's
missing-hook failure remains historical evidence under `run/` and at
`ff8dc10c`. No product files, descriptors, plan ledgers or other worktrees were
edited. Historical continuation evidence is under `continued-run/`.

| Ordered audit step | Outcome / classification | Evidence |
|---|---|---|
| 1. Initialize; signed-in, team-bound Claude; roots/owner/hooks | **PASS** | Production hook installer ran before LaunchNew; real `SessionStart(compact)` registered. Sole initialize in this continuation completed with canonical format 2, owner `team`, lead `%1`, alpha `%2`. Claude generated authenticated native replies. |
| 2. Claude sends two markers; recipient exposure | **FAIL acceptance / UNAVAILABLE — harness** | Claude executed both sends, both were accepted. Alpha never exposed either marker. Its copied runtime lacked `codex-code-mode-host`, and the initial setup prompt stayed in the composer until a second Enter. Only 27.98 seconds remained after that Enter, below the required 60-second opportunity. This cannot establish a product defect. |
| 3. Seat explicit read/reply and Claude native uptake | **NOT RUN — dependency on 2** | No alpha reply or external lead read. |
| 4. Claude explicit read/mark with cursors | **NOT RUN — dependency on 2** | No `mesh read` or read receipt claimed. |
| 5. Ordinary `/compact` and genuine recovery card | **NOT RUN — dependency on 2** | Registration is proven; compaction execution and recovery are not. |
| 6. Fresh native mail after recovery | **NOT RUN — dependency on 2** | Teardown was performed independently and passed. |

[Final disposition](l1-claude-lead/continued-run/final-disposition.json) supersedes
the controller's provisional `product-owner-pending-evidence-review` label;
its raw failure reason is retained in
[result.json](l1-claude-lead/continued-run/result.json).
No paid step was retried after the failure.

## Production setup and runtime evidence

The harness adapter [install_hook.rs](l1-claude-lead/install_hook.rs) links the
already-built production library and calls
`coordination::compact_hook::ensure_compact_hook_installed`. It does not write
a substitute recovery card or reimplement hook installation. The helper runs
inside the private Bubblewrap namespace against the scratch teams root and
scratch daemon executable, before the seats start. No drain descriptor was
enabled. [Registered hook](l1-claude-lead/continued-run/production-hooks-before-launch.json).

- Taurhaus base `6f61f611`, protocol **27**, daemon release SHA256
  `28c1937eb99b8495814654d113b3642fd63b051c1550a0e6610662abac4184cd`.
- Mesh `a6ee29681a44a472c0ce7b33b9630d4865c93012`, designated `mesh-l1`
  worktree only, SHA256
  `88b42445f0b6d90ef8c029af0faa6e8f2411a06843625d0349a7d68a98ce4e68`.
- Initialize run `init_718fd858f16f4bc4897e36daa0d9f9c1`.
- Claude Code **2.1.267**, actual model `claude-haiku-4-5-20251001`,
  session `a2dddba1-e73b-4f58-b908-8dcfedabf8e4`, attachment 1,
  context generation `0`, managed LaunchNew `%1`.
- Codex **0.153.4**, `gpt-5.6-luna low`, tmux `%2`, session
  `01a08a86-bfdf-7273-9602-02880a7f3d72`, actual setup turn
  `01a08a8a-9d0d-74c1-b18a-f28b7ad3be21`, response `READY`.

Claude executed these commands in Bash (the real scratch selector was used):

```sh
mesh send alpha 'ACTION REQUIRED: Remember marker L1_AMBER_42. Do not run tools or send a reply yet; the controller will request step 3.' --summary 'L1 marker A' --claude-dir "$CLAUDE_CONFIG_DIR" --team l1-claude-lead --name lead
mesh send alpha 'ACTION REQUIRED: Remember marker L1_BIRCH_73. Do not run tools or send a reply yet; the controller will request step 3.' --summary 'L1 marker B' --claude-dir "$CLAUDE_CONFIG_DIR" --team l1-claude-lead --name lead
```

| Marker | Canonical message ID | Delivery ID |
|---|---|---|
| `L1_AMBER_42` | `2a0be2a5-3cba-4885-9124-8037c65635b4` | `977b42c1-f557-4dd5-bc05-5f5d598474a9` |
| `L1_BIRCH_73` | `d520abc2-467e-492b-be39-4790f7f3e43b` | `27ac7268-fb10-45b4-8a9a-fb6a304829fc` |

Both journal rows say `message_accepted`, `wake_eligible: true`, recipient alpha.
Neither acceptance proves terminal exposure. Final alpha health reports
`completed: 0`, `failures: 0`, and
`IO error: delivery: pending: activity not freshly idle`.
The final pane additionally reports `codex-code-mode-host: host executable was
not found`. Even after a corrected setup submission, this runtime does not
establish a command-capable Codex seat for step 3.

[Claude tool transcript](l1-claude-lead/continued-run/claude-transcript.json),
[Codex transcript](l1-claude-lead/continued-run/codex-transcript.json),
[commands and exits](l1-claude-lead/continued-run/events.jsonl),
[structured daemon events](l1-claude-lead/continued-run/daemon-events.json).
Complete segment rows, projections, runtime/activity/health and bounded panes
are archived together; no truncated JSONL tail is treated as a complete row.
The failure occurred before journal pagination/export; direct snapshots are
not represented as completed paginated-reader coverage.

Claude's generated startup card induced two unrequested self-messages. The
first send attempt omitted the recipient and failed; Claude corrected it, then
sent another wait-state note to itself. Both accepted self-messages appeared
as native teammate inputs without any `mesh read`. This is partial native-mail
evidence, not the required alpha-reply round trip. Their inputs and every API
generation are counted. Lead `inboxes/lead.json` existed and was empty after
native consumption; the required seat-reply projection remains untested.

## Cumulative spend and cleanup

[Final ledger](l1-claude-lead/continued-run/final-cost-ledger.json) retains the
previous run's **$0.02195125** estimate / **$0.06416** upper-rate amount.
The continuation adds **$0.04525085**, including **$0.00187940** for Codex.
Cumulative seat estimate **$0.06720210**; upper-rate amount **$0.65789640**.
These are token-based API-equivalent estimates, not invoices.

| Current Claude API message | Estimated USD |
|---|---:|
| `msg_011CeuUfXBWvBSQXB3koQ9b6` | 0.01532915 |
| `msg_011CeuUgVYKToc9a5NFWm5H5` | 0.00449185 |
| `msg_011CeuUgoreR1X3SinNoVaNJ` | 0.00254050 |
| `msg_011CeuUh19o7TCMwdgrADoFw` | 0.00678790 |
| `msg_011CeuUhycoMUBZjsJDpBL6W` | 0.00325060 |
| `msg_011CeuUiEGrQTDrHxeQz4gJG` | 0.00378575 |
| `msg_011CeuUtSJRmWBvdhjdoBySi` | 0.00414540 |
| `msg_011CeuUthtJ7gHAZhEw5ApSo` | 0.00304030 |

The ledger has full input/cache/output fields. Haiku rates are $1/$0.10/$1.25/$5
per million input/cache-read/cache-write/output tokens, with all tokens also
priced at $5/M for the upper-rate column.
[Anthropic pricing](https://platform.claude.com/docs/en/about-claude/pricing).
Luna uses the trial's $0.20/$0.02/$1.20 rates; its 9,367 input and 5 output
tokens are priced once. No reasoning tokens were double charged.

Conservative cumulative input counts: **5 Claude / 6 Codex**, including the
previous run, native self-messages, both reserved alpha exposures and the
additional Enter. Eight current Claude API messages belong to four current
inputs; API messages are not silently relabeled as eight user inputs.
No `/compact` occurred. Combined runtime **451.76 seconds**, below 15 minutes.
The raw controller counter omitted the two self-mail inputs; the final ledger
explicitly corrects this. The final fix round makes future controller admission use that corrected
observed-input count (offline tested; no paid replay). No input was sent near the ceiling in this attempt.

The private PID namespace was stopped, all owned children waited, no survivors
remained, the private port closed, and the scratch root/Codex auth copy were
removed. Claude's credential was never copied; its read-only mount placeholder
was zero bytes after shutdown. [Cleanup](l1-claude-lead/continued-run/cleanup.json).
Runtime controller exit **2**; step-2 driver exit **1** (failed assertion).

## Reproduction, gates and review

Exact controllers: [controller.py](l1-claude-lead/controller.py),
[step1.py](l1-claude-lead/step1.py), [steps.py](l1-claude-lead/steps.py),
[actions.py](l1-claude-lead/actions.py). Later step branches were not executed
or validated by this failed attempt; they are not a complete passing-lane claim.
Explicit credential/binary arguments use the same authorized single-file
sources as the prior run; no fallback or other account was used.

```sh
TRIAL_EVIDENCE_LABEL=renewed-verification python3 docs/design/evidence/e2e/l1-claude-lead/build_gates.py --build
just --justfile docs/design/evidence/e2e/l1-claude-lead/harness.just build-hook
python3 docs/design/evidence/e2e/l1-claude-lead/controller.py --evidence-label continued-run --claude-credential "$AUTHORIZED_CLAUDE_SINGLE_FILE" --codex-auth "$AUTHORIZED_CODEX_SINGLE_FILE" --claude-binary "$PINNED_CLAUDE_BINARY" --codex-binary "$PINNED_CODEX_BINARY"
python3 docs/design/evidence/e2e/l1-claude-lead/step1.py
python3 docs/design/evidence/e2e/l1-claude-lead/steps.py 2
TRIAL_EVIDENCE_LABEL=renewed-final-verification python3 docs/design/evidence/e2e/l1-claude-lead/build_gates.py
```

The events retain the extra alpha setup prompt, corrected Enter and intermediate
captures. Both builds and the production helper link exited **0**. Cargo
admission used the required `pgrep` poll and 30-minute maximum wait.
Six new offline tests went red then green, including control-token redaction;
the existing six remain green. Tests use generated data and no real CLI or
credential root. Scratch Mesh control-state files are excluded from the final
packet; tokens are not retained. No `src-tauri/` changes, so the Rust-unit gate
is not applicable.

Final gates and the new independent Opus evidence lens are recorded below.
Earlier Opus review cost was **$0.280255**, separately budgeted from seats.
The explicit two-member setup and credential rules remain the operator's
exceptions to the audit's older beta-hosted/disposable-source wording.

## Final gates and evidence archive

| Command | Exit |
|---|---:|
| `just check-quick` | **0** |
| `just lint` | **0** |
| `just test-contracts` | **0** |

The gate runner exited **0**, waited its children and removed its private root.
Frontend: 150 files / 2,495 tests; contracts: 68 tests.

After teardown, 380 snapshot paths were consolidated into 103 unique
objects in [snapshots.json](l1-claude-lead/continued-run/snapshots.json).
The `aliases` map preserves each original path, including `step1/`, `final/`
and `step2-failure/`; pane contents have at most 60 lines.
No evidence-size threshold interrupted a runtime step.

The runtime controller/support were loaded from `9e71cc15`; the step-2
driver is at `5a7fb7ee`. Final token redaction and packing occurred after
shutdown; [source provenance](l1-claude-lead/continued-run/executed-source.json)
identifies the exact executed versions.

The first continuation Opus review timed out at 180 seconds, returned no
verdict or usage JSON, and was cleaned up. Its spend is **unknown**, with a
configured `$0.65` ceiling; it is not recorded as free or approved. One shorter
evidence-only request uses a separate `$0.20` ceiling and low effort.

## Continuation review disposition

The shorter independent `claude-opus-4-6` evidence lens returned **approve**,
two minors, zero majors, exit **0**, one turn costing **$0.130925**.
[Review](l1-claude-lead/continued-review-brief/review-result.json).
One fix round made `cap_inputs` the authoritative final-ledger field
(**5 Claude / 6 Codex**), keeping controller reservations separately, and
added an offline-tested native-input correction to future admission. No
historical model inputs or usage rows were changed.

The `-15` child exit belongs to the owned Bubblewrap bootstrap, stopped with
SIGTERM by the controller during ordinary teardown. It is not a seat failure.
The private namespace then removed its daemon, tmux server and seats; the
independent PID/start-tick audit found no survivors. Both reviewer namespaces
were also waited and removed, with zero-byte credential placeholders.

Known cumulative seat plus completed-review estimates total **$0.47838210**
($0.06720210 seats + $0.280255 prior review + $0.130925 new review), **plus
unreported spend for the timed-out review** (configured ceiling $0.65).
No zero-cost or complete-total-cost claim is made for that missing review
usage. Trial seat cost remains fully metered and below its $2 cap.
Implementer/orchestrator usage is separate and not visible in these ledgers.
The approval is for the truthful stopped-run report, not a passing E2E lane.

## Restart preparation after `3a1a8fc1`

The current tree had no uncommitted green numbered step. Step 1 remains committed
in `5a7fb7ee`; step 2 remains **UNAVAILABLE (harness)** and steps 3–6 remain
**NOT RUN**. This continuation performs offline harness repair only.

Four new regression tests first exited **1** (four errors), then **0** after
the repair; all **21** offline tests pass. Comments name the introducing
commits. The controller now copies both native Codex executables from the same
explicit bundle, validates the companion before accessing credentials, allows
the pasted input to settle before one Enter, and requires a completed alpha
startup turn before step 2 sends its markers. These changes have **not** been
validated with another paid runtime. No input or mutation is automatically retried.

Restart accounting accepts the authoritative `cap_inputs` field. The caller
must explicitly provide the prior ledger, cumulative runtime and fresh evidence
label; the old first-run ledger and elapsed-time constant are no longer implicit.
Stopped controllers reject later actions, and control-auth files are excluded
from snapshots. Historical runtime evidence and costs remain unchanged.

The committed counts leave **3 Claude inputs**. A fresh startup followed by
steps 2–6 needs **at least 6**: onboarding, sends, native reply uptake (assuming
both replies batch together), explicit read, compact and fresh native uptake.
The controller therefore stops before credential provisioning under the unchanged
8-input cap. The user has been asked to raise the cumulative Claude cap to 12;
that change is **not yet authorized or implemented**. The $2 seat cap, 12 Codex
input cap and 900-second runtime cap remain unchanged. Prior runtime is
451.759904 seconds; [admission record](l1-claude-lead/restart-verification/restart-admission.json).
This is a shortage of remaining inputs, not a rejection of the authorized
transcript budget method or read-only Claude credential mount.

The current controller invocation must additionally include:

```sh
--prior-ledger docs/design/evidence/e2e/l1-claude-lead/continued-run/final-cost-ledger.json --prior-runtime-seconds 451.759904 --evidence-label restart-run
```

Step drivers must use the matching `TRIAL_EVIDENCE_LABEL=restart-run`. The earlier
reproduction block records the historical commands at their cited commits.
No new daemon, Mesh, Claude, Codex or tmux runtime was started, and no new
credential was copied or mounted. Additional seat/reviewer spend is **$0**;
cumulative seat estimate remains **$0.06720210**, upper-rate **$0.65789640**.
The earlier unknown timed-out reviewer spend remains unknown. The earlier Opus
approval covers the stopped-run evidence; no new runtime verdict is claimed.

This continuation's required gates all exited **0**: `just check-quick`,
`just lint`, `just test-contracts`. Invocation:
`TRIAL_EVIDENCE_LABEL=restart-verification python3 docs/design/evidence/e2e/l1-claude-lead/build_gates.py`.
The runner exited **0**, waited its children and removed its credential-free
scratch root. [Gate results](l1-claude-lead/restart-verification/gate-result.json).
Cargo admission waited for other lanes without signaling their processes.
The lane-authored diff from merge base `6f61f611` contains no Rust changes;
the independently advancing `main` branch does not change that scope, so
`just test-rust-unit` is not required. `git diff --check` also passed.


## Run 3 — FAIL at step 3 (product owner unresolved: taurhaus | mesh | claude-code); steps 1–2 PASS

This run reached the native-mail boundary with a complete, command-capable
Codex runtime. It stopped on the first failed assertion; no paid retry,
compaction, product change, descriptor edit, or hosted seat followed.

| Ordered step | Outcome | Observed evidence |
|---|---|---|
| 1. Initialize, authentication, team binding, owner and hooks | **PASS**, commit `d34ef9eb` | One production initialize, format 2 / team owner; managed lead `%1`, alpha `%2`; authenticated Haiku `READY`; production `SessionStart(compact)` hook registered before launch. |
| 2. Claude sends two markers; alpha exposure | **PASS**, commit `21cc42f1` | Claude Bash sends accepted at sequences 11/12; alpha terminal submissions at 15/16, explicit alpha reads at 17/18. Alpha session is attributed and idle; onboarding submitted at 6 and read at 7. |
| 3. Alpha explicit read/replies; Claude native uptake | **FAIL — product owner unresolved** | Both replies accepted at 19/20, but neither has a delivery attempt or `native_enqueued` receipt. Neither reply appears in a Claude native teammate input. Full 65-second native opportunity, with 831.91 seconds remaining on the outer deadline when it began. |
| 4. Claude explicit read/mark | **NOT RUN**, dependency on 3 | No Claude or external `mesh read` of the lead inbox. |
| 5. Ordinary `/compact` and recovery card | **NOT RUN**, dependency on 3 | Startup registration is proven; post-compaction recovery is unproved. |
| 6. Fresh native reply after recovery | **NOT RUN**, dependency on 3 | Cleanup ran independently and passed. |

The live lead's `isActive` flip is bounded to **12:43:20–12:43:24 UTC**:
`snapshots.json` alias `step1-observation-end/team/config.json` has `true`,
and `step1/team/config.json` has `false`. This precedes every step-2 action;
`events.jsonl` records the first `controller_input` at **12:43:26.057 UTC**.
Initialization's automatic Claude startup had already run; this is not evidence
that Claude Code was inactive during the interval.

**Proximate mechanism — Mesh:** the pinned scheduler excludes inactive members
in `src/delivery/scheduler.rs:138–148` and exits an excluded worker at line 261.
The lead worker's last retained heartbeat is 12:44:03 UTC; its health file is
absent at final capture, while alpha's worker continues. The **Mesh team-daemon
process itself stayed alive to teardown**, PID **1212168**, as recorded in
`cleanup-before.json:170–186`. Roster exclusion explains the missing delivery
attempts at sequences 19/20; it does not identify the trigger or establish that
excluding an inactive member is a Mesh defect.

**Recovery gate — taurhaus:** `taurhaus.log.jsonl:132` records
`coordination.team_daemon.skipped` at **12:43:49.512 UTC**, operator `lead`,
reason `inactive_lead_control_identity`. `daemon-events.json:803` records
`self_heal.pass.completed` with `team_daemons_ensured: 1`; lines **2412, 4033,
5199, 5965** record **0** from that pass onward (also JSONL line 133 for the
first zero). The guard is `src-tauri/src/coordination/orchestrator/teardown.rs:723–726`;
the skipped ensure operation is `mesh team-daemon start` in
`src-tauri/src/coordination/runtime/system.rs:388–408`. Thus Taurhaus's
self-heal cannot recover this inactive-lead state through that path, even
though the existing team-daemon process remains alive. These rows establish
Taurhaus's recovery behavior, not that Taurhaus wrote the flag.

**Trigger unresolved — explicitly including claude-code:** the tripwire at
`src-tauri/src/coordination/stores/config.rs:944–951` documents an external
inactive-flag writer and says Taurhaus/Mesh only write true for live non-leads.
Its non-lead qualification and repair-on-save context do not prove the writer
for this lead. However, `snapshots.json` alias
`latest/team/inboxes/team-lead.json` retains Claude Code's native `from: lead`
idle notification, `msgV`/`msg_id` shape, at **12:43:22.869 UTC**, inside the
flip interval. `runtime-session-snapshot.json` records that session launched
with `--team-name l1-claude-lead --agent-name lead`. Together these narrow a
**claude-code native team-state writer candidate** in the same tree and time
window. No retained write trace attributes the config mutation to a process,
so the product owner remains **unresolved among taurhaus | mesh | claude-code**.
Route investigation to the flag writer and Taurhaus recovery gate as well as
Mesh delivery; do not treat the intended roster exclusion as the proven root
cause. No product repair or paid rerun was attempted.

The empty `inboxes/lead.json` snapshots are **non-discriminating**: they are
compatible with successful native consumption. In particular, journal sequences
9/10 show `delivery_attempt`/receipt via `native-mailbox/1`, followed by the
native teammate input at `claude-transcript.json` timestamp 12:43:58.753 UTC.
The failure rests on the **absent delivery attempts for sequences 19/20** and
absent required reply markers in Claude's native inputs after the full window.

An earlier unsolicited alpha onboarding note *did* arrive natively at
12:43:58.753 UTC and Claude responded “I'm awaiting step 3 from the controller.”
That is a partial positive control, not either required reply marker. Alpha
also read the two markers while processing its step-2 terminal notification;
its explicit step-3 unread/mark-read command consequently returned an empty,
`done: true` page. The corresponding marker read receipts are sequences 17/18.
The generated onboarding context induced that extra note despite the role's
no-unsolicited-messages instruction; the note and its native Claude input are
included in the ledger. One malformed `mesh send --to/--message` attempt was
corrected by the model inside the same metered turn, not retried by the controller.

### Runtime identities and harness repairs

- Worktree branch stayed `feat/e2e-l1-claude-lead`; source base `6398bfa3`
  is an ancestor of executed controller commit `68cd3999`. Daemon protocol **27**,
  version **0.9.7**, SHA256 `260f7e15468984b718d3e1b4ea40258dd8552d73c4c1df3e5e91fc2b6123e4fd`.
- Designated Mesh worktree is detached at **`fcb9647`**, actual version string
  `mesh 0.2.29`; SHA256 `8f847bc0a993f811d368e5f71d0ac55594393f57010847e4e083f27e97583e40`.
  No installed Mesh replacement or descriptor change.
- Claude Code **2.1.267**, model **`claude-haiku-4-5-20251001`**, session
  `f83f5821-a979-4172-8ae4-50d4ac761c28`, attachment 1 / context generation 0.
- Codex **0.153.4**, **`gpt-5.6-luna low`**, session
  `01a08b58-0628-7fc0-9ba4-db3ae587c5e0`, attachment 1. Both native binaries
  were resolved from the installed launcher and copied together; the code-mode
  host actually executed Mesh commands in step 3.
- Initialize ID `init_12b834d5e78b46a18f306ce9933d6a1e`. Builder's canonical
  `messaging.retentionPolicy`, creation-time `delivery: tmux` for both members,
  private daemon on a probed non-default port, private tmux/PID namespace,
  scratch project with command-capable AGENTS.md, inherited `TMUX` removed.
- Both `codex_submission_confirmed` rows in `events.jsonl` record an empty
  composer **and a new turn ID**. Fixed filenames overwrote the first pane and
  confirmation sidecars; only the second survives at 12:44:08.995 UTC. The
  first confirmation is supported by the event, not an independently retained
  submission pane. The offline controller fix now names both files per step/input.
  The first setup submission confirmed at 12:43:29.267 UTC; its attributed-idle
  window began at 12:43:30.356 UTC. No second Enter or paid-turn retry was needed.
  Alpha's saved snapshot says `idle`, `high`, source `notify`; the daemon session
  snapshot says `activity_attribution: attributed`, `member_name: alpha`.

| Marker | Accepted message ID | Delivery ID |
|---|---|---|
| Claude → alpha `L1_AMBER_42` | `f77e936f-1813-4e10-a6c4-5c653525abac` | `bf94b6e1-3cb8-49be-9d92-0db86e613308` |
| Claude → alpha `L1_BIRCH_73` | `54f4905b-64a8-4f8f-a543-6ddabaab1cb9` | `e21bdcc0-476a-4f69-a2c0-7f87f3da91c1` |
| Alpha → lead reply A | `a7d4d849-ebd1-4110-8fc1-fb92b1441a50` | `05ee895f-1b6b-4915-a4ba-eaed095a53fb` |
| Alpha → lead reply B | `e25b9530-9aa8-4b2b-ae34-2b1c392e81d1` | `0e8d1d19-d097-44a3-a209-dc6a0b36d114` |

### Spend, evidence and teardown

Run-3 Claude spend **$0.02692005**; Codex **$0.00781272**;
run-3 seat estimate **$0.03473277**. Historical seat estimate **$0.06720210**
is retained, for **$0.10193487 cumulative**, or **$1.07581780** at the
conservative upper token rates. These are transcript-based estimates, not bills.
No compact was attempted or charged as zero. Complete input/cache/output fields
and all generation/turn identities are in the cost ledgers.

| Claude API message / Codex turn suffix | Estimated USD |
|---|---:|
| `msg_011Ceun6U8dRzxqFwYcs3Tm5` | 0.01683875 |
| `msg_011Ceun99XmgEs7oKndsPMEk` | 0.00395785 |
| `msg_011Ceun9RtmfNndPH9YwBb5J` | 0.00282385 |
| `msg_011Ceun9ZGXaqJU7kNbqTaWi` | 0.00329960 |
| Codex `1a113e000cfd` — setup | 0.00188800 |
| Codex `ae7149616265` — onboarding | 0.00230892 |
| Codex `842968ae` — marker exposure/read | 0.00121552 |
| Codex `da9806cb7993` — explicit read/replies | 0.00240028 |

Run-3 conservative input accounting is **3 Claude / 5 Codex**, including
onboarding, the unsolicited native note and reserved exposures. Actual Codex
turn IDs: **4** (the two marker exposures batched). The ledger’s
`current_inputs.claude: 2` counts controller onboarding/typed reservations;
`controller_inputs.claude: 3` additionally counts the observed native alpha note.
Neither field adds historical input counts in this run; history is listed separately. Across the historical runs
and run 3: **8 Claude / 11 Codex**, still within 8/12. Runtime **136.496059 s**;
historical plus current runtime **588.255963 s**. The executed run incorrectly admitted fresh
input/time counters while retaining prior spend: **cumulative input-cap enforcement
did not satisfy the shared contract**. Realized cumulative counts/time stayed
within the caps, but no Claude input headroom remains for steps 4–6. The offline
correction restores `cap_inputs` in `prior-spend.json`; its disclosure and original
zero `controller_inputs` preserve what actually ran. Any future continuation must
carry run 3 too, not reuse only the earlier 5/6 counts. This correction does not
retroactively validate admission or change the retained runtime ledgers.
Every next input checked observed spend plus a $0.50-or-larger reserve, and the
Claude bound also records 8 × the highest metered per-input usage (**$0.13471**).

The complete **339-row daemon JSONL** is retained. No daemon account-usage rows
occurred at the configured log level. Exact sanitized commands/RPC exits,
Claude tool transcripts, Codex session metadata and tool calls, all 20 journal
rows, read receipts, runtime/activity/config snapshots, native inbox observations,
passive locks and pane captures of at most 60 lines are included. Repeated snapshots
are deduplicated only after shutdown; no evidence-size threshold interrupted a step.
Step-2 journal pagination completed; the final failure uses the complete captured
journal segment, not a claimed second paginated export.

Controller exit **2**, step-3 driver exit **1**; step-1/2 drivers exit **0**.
The owned Bubblewrap bootstrap received SIGTERM during normal teardown (exit -15).
The PID/start-tick inventory and independent trial-ID audit found **no survivors**;
the private listener closed and scratch root was removed. Codex's sole 0600 auth
copy was removed. Claude's single-file read-only credential mount was never copied;
its writable-layer placeholder remained zero bytes. No operator process was killed.

### Validation and deviations

27 offline tests pass. New checks observed red first: four missing-helper errors,
one explicit-lead-read helper error, and one opaque-internals retention assertion
failure; each then passed. Fixtures are generated and invoke no real CLI or
credential path. Required gates `just check-quick`, `just lint`, and
`just test-contracts` each exited **0** in credential-free namespaces from the
checkout root (2,509 frontend tests; contract results retained). No `src-tauri/` file changed, so the Rust-unit conditional gate is inapplicable.
Both native builds and the production hook adapter link exited **0** after Cargo
admission polling. The first independent Opus request exited **1**, `budget_exhausted`, returning
no verdict. Its metered cost is **$0.236270**, exceeding its configured $0.20
limit at the completed-turn boundary; the CLI flag was not a hard billing ceiling.
The shorter evidence-only Opus request exited **0**, returned **approve** with
two minors and no majors, and cost **$0.151525**. One report fix round clarified
controller versus native input counts and retained the unknown-writer caveat.
That historical approval applied to the earlier stopped-run report, not an E2E
PASS or this offline correction. Run-3
review spend totals **$0.387795**; seats plus reviews for this run total
**$0.42252777**. Reviewer spend is separate from the seat cap.

Binding operator exceptions to the older audit setup: no hosted beta; two distinct
markers/replies use alpha. Claude credentials use the authorized read-only mount;
Codex copies only the authorized auth file. Historical review spend remains
**$0.280255 + $0.130925**, plus **unknown** usage from the previous timed-out review
(configured ceiling $0.65). Implementer/orchestrator usage is not visible here.
No plan ledger rows were edited. Later steps remain unvalidated by this stopped run.

Known historical-plus-current seat and completed-review estimates total
**$0.90090987**, plus the earlier timed-out review’s unknown spend (configured
ceiling $0.65). Both new reviewer namespaces were waited and removed; their
credential placeholders remained zero bytes.

The run-3 directory contains `evidence-index.json`, per-generation/turn cost
ledgers, the complete `taurhaus.log.jsonl`, and `snapshots.json`. The latter
interns **497 snapshot paths into 162 unique objects** and maps each original filename to its object,
including `step1/`, `step2/`, `step3-failure/`, `final/`, gates and review outputs.
`provenance.json` identifies executed source; `postprocessing.json` identifies
the subsequent offline evidence-sanitizer repair. No paid run used that repair.

### Review round 1 correction (offline)

All supplied findings were verified; none was skipped. The classification above
supersedes the earlier Mesh-only routing. Four offline regression tests in the
named `controller.py` first failed (exit 1): missing writer/recovery citations,
prior counts yielding 3/5 instead of 8/11, overwritten submission evidence, and
a late paid confirmation lost on deadline validation. Tests use retained public
packet metadata and generated in-memory observations; no CLI or credential is
accessed. The confirmation deadline is now computed once and saved before an
insufficient-window stop. Tests stay in this named file to respect the requested
file scope. Historical raw runtime evidence and paid step outcomes are unchanged.
No new seat or reviewer spend occurred; implementer usage remains unavailable.

Validation of this correction: **31 offline tests pass** (27 existing plus the
four `controller.ReviewRegressionTests`, exit **0**). Run the four with
`PYTHONPATH=docs/design/evidence/e2e/l1-claude-lead python3 -m unittest controller.ReviewRegressionTests`;
the existing suite uses `python3 -m unittest discover -s docs/design/evidence/e2e/l1-claude-lead -p 'test_*.py'`.
Required commands ran from this checkout root in credential-free namespaces:

| Gate | Observed exit / result |
|---|---|
| `just check-quick` | **0**; 2,509 frontend tests; typecheck: 0 errors, 0 warnings |
| `just lint` | **0** |
| `just test-contracts` | First **101**, harness log-location contamination; retry **0**, 15 CLI renderer + 20 harness conformance + 33 module-boundary tests |

The first contract run's `retired_gemini_tool_literal_does_not_return` scanned
the wrapper's generated `.check-logs/l1-review-round1/gates/gate-isolation.json`
as source and rejected its scratch Antigravity directory spelling. Those
untracked gate records were preserved with `.json.log` suffixes (logs are not
source), and only the failed gate was rerun; no test or product code was changed
or bypassed. Logs remain locally under `.check-logs/l1-review-round1/` and
`.check-logs/l1-review-round1-contracts-retry/`; results are recorded here to keep
tracked changes within the five named files.

The original waiting gate wrapper was stopped before it launched any gate
(exit **1**, handled SIGTERM 15). Its cleanup passed. A two-second admission poll
replaced the coarse 30-second poll, retaining the original first-admission
30-minute cutoff. Both subsequent wrappers exited **0**, waited all children,
and removed their scratch roots; independent trial-ID process audits found
**zero survivors**. No daemon, tmux, Claude or Codex seat was started, no
credential was copied or mounted, and no new paid run or review occurred.
No `src-tauri/` file changed in this correction, so `just test-rust-unit` was
not applicable. The historical lane FAIL and steps 4–6 NOT RUN remain unchanged.


## Run 4 — superseded pre-ruling admission record (no paid startup)

The requested Taurhaus base `1db4f9bf` is an ancestor of this branch; protocol
remains 27. The existing run-3 controller now pins that base and the designated
Mesh worktree at detached `ed59187`. No product or descriptor was changed.
The latest run does **not** test whether the activity/membership fixes resolve
the earlier native-mail failure.

The binding audit's shared contract says all prior seat starts/inputs count
across the lane and a controller restart cannot reset them. The corrected
run-3 ledger records **8 Claude / 11 Codex** cumulative inputs. Admission of
one more Claude input failed with **exit 2**, `claude input cap`, before any
credential access or paid startup. The fourth-run instruction does not expressly
amend that cumulative rule. A budget-scope clarification is pending; no fresh
budget is inferred. This is a **harness admission blocker**, not a rejection of
the authorized read-only credential mount or transcript-based budget method.

| Ordered step | Outcome / classification |
|---|---|
| 1. Initialize, signed-in team-bound Claude, owner and hooks | **NOT RUN** — harness budget prerequisite |
| 2. Claude sends both distinct markers to alpha | **NOT RUN** — dependency on 1 |
| 3. Alpha reads/replies; Claude native uptake | **NOT RUN** — dependency on 1 |
| 4. Claude explicitly reads/marks with cursors | **NOT RUN** — dependency on 1 |
| 5. Ordinary `/compact`, real hook and recovery card | **NOT RUN** — dependency on 1 |
| 6. Fresh native reply after recovery; export | **NOT RUN** — dependency on 1 |

[Admission](l1-claude-lead/run4/admission.json),
[corrected prior ledger](l1-claude-lead/run4/prior-spend.json), and
[preparation provenance](l1-claude-lead/run4/preparation.json) retain the
observations. New seat spend is **$0**; new reviewer spend is **$0**.
Historical seat estimates remain **$0.10193487**, conservative upper-rate
**$1.07581780**, with **588.255963 seconds** cumulative runtime. Historical
completed-review estimates remain **$0.798975**, plus the earlier timed-out
review's unknown spend; implementer/orchestrator usage is unavailable. The
per-generation and per-turn spend tables above remain authoritative. Nothing
was charged as a free compaction: no compact ran.

The new offline revision-pin test first failed with `AttributeError` for the
missing base constant (exit 1), then passed after updating the reused
controller's pins. All **32 offline checks** pass (28 discovered plus four
controller regression tests; exit 0). Fixtures invoke no real CLI or credential.
See [red](l1-claude-lead/run4/offline-red.txt) and
[green](l1-claude-lead/run4/offline-green.txt).

All required gates ran from the checkout root inside a credential-free
private PID namespace: `just check-quick` **0** (2,518 frontend tests, zero
typecheck errors/warnings), `just lint` **0**, `just test-contracts` **0**
(15 CLI-renderer, 20 harness-conformance and 33 module-boundary tests). The
gate wrapper exited **0**, waited its children and removed its scratch root;
an independent trial-ID process scan found **zero survivors**.
[Gate audit](l1-claude-lead/run4/gate-audit.json). No `src-tauri/` diff exists,
so `just test-rust-unit` is not applicable. The checkout-local `just build-daemon` and designated Mesh worktree
`cargo build --bin mesh` both exited **0** after bounded Cargo admission
polling. Neither binary was installed or launched. The build wrapper exited
**0**, waited its children and removed its credential-free scratch root; the
independent trial-ID audit found **zero survivors**.
[Build audit and SHA256 digests](l1-claude-lead/run4/build-audit.json). The independent Opus runtime-evidence lens has not run;
there is no runtime evidence to approve and no PASS claim. No run-4 daemon
JSONL, mailbox, pane, receipt or transcript exists because no runtime process
was started. The Claude credential has not been accessed or copied, and no
Codex auth copy was created. No plan ledger rows were edited.


Run-4 deviations/limits: paid execution remains blocked by the unresolved
fresh-budget scope; all six runtime steps and the independent Opus lens are
unavailable/not run. Actual CLI versions, authenticated seats, native mailbox
uptake, alpha attribution, and compaction recovery are therefore unverified
on the new binaries. The requested no-hosted-beta setup would use two alpha
markers, as in run 3, but no team was created. No credential fallback,
product fix, descriptor edit, paid retry or budget reset occurred. Both unpaid
wrappers exited 0 and left no owned child. The preparatory revision-pin commit
and evidence checkpoints do not claim any numbered runtime step as green.


## Run 4 — fresh-budget execution

The earlier pre-ruling admission refusal above is superseded. Budget clarification is resolved by the following binding authorization; runs 1–3 are closed, and their ledgers are retained unchanged. `run4/admission-ledger.json` starts at 0 Claude / 0 Codex / $0 / 0 seconds and embeds the closed-run history.

> ORCHESTRATOR BUDGET RULING FOR RUN 4 (binding; do not refuse on it): runs 1–3 are CLOSED. Run 4 starts with a FRESH budget of <= 8 Claude inputs (including the `/compact`), <= 12 Codex inputs and <= USD 2. The controller's cumulative admission ledger, which records 8/8 Claude inputs consumed by the earlier runs, is RESET for run 4: record the reset in the run-4 evidence with this ruling quoted as its authorization, keep the earlier runs' ledgers intact as history, and enforce the fresh caps within run 4 exactly as before (stop before the next input if spend plus one more turn could exceed a cap). Budget clarification is not pending — this paragraph is it; continue.

The run-3 controller and confirmed-submission logic are reused with explicit run-4 output/admission paths. No product or descriptor changes. Execution results will be appended below.

## Run 4 execution — UNAVAILABLE: step-1 harness race, no paid retry

This is the latest run's verdict. The earlier run-4 pre-ruling admission refusal is superseded: the orchestrator explicitly closed runs 1–3 and authorized fresh caps of 8 Claude inputs (including compact), 12 Codex inputs, $2, and 15 minutes. The ruling is quoted verbatim in [run-4 admission ledger](l1-claude-lead/run4/admission-ledger.json); the prior 8/11 input ledger remains unchanged in `prior-spend.json` and embedded history. No budget clarification is pending.

| Ordered step | Outcome and classification |
|---|---|
| 1. Initialize, signed-in/team-bound lead, owner and hooks | **FAIL — harness**: immediate session-ID assertion before scanner attribution; overall lane **UNAVAILABLE**. |
| 2. Claude sends both distinct markers to alpha | **NOT RUN** — dependency on step 1. |
| 3. Alpha reads/replies; native Claude uptake and distinguishing response | **NOT RUN** — dependency on step 1. |
| 4. Claude explicitly reads/marks its inbox with cursors | **NOT RUN** — dependency on step 1. |
| 5. Ordinary `/compact`, genuine hook and recovery card | **NOT RUN** — dependency on step 1. |
| 6. Fresh native reply after recovery; export | **NOT RUN** — dependency on step 1; failure evidence exported and teardown completed. |

### Exact stop and partial observations

`step1-driver.log` points to the executed `step1.py:23`: `assert lead['session_id'] and lead['paneId']=='%1'`. The retained `step1-observation-end/team/runtime/lead.json` has pane `%1`, attachment 1, context generation `0`, but null session identity. The driver asserted immediately instead of allowing the required 60-second scanner opportunity. Runtime was **6.333125 seconds**. This is insufficient observation to diagnose a Taurhaus attribution product defect.

The raw `result.json` says `claude_auth_unavailable` because `run3_driver.py` mapped every step-1 error to that fallback. That is **unsupported**; `final-disposition.json` supersedes the classification without rewriting raw runtime evidence. The Claude pane shows **Haiku 4.5 · Claude Max**, and `claude-transcript.json` contains a native startup recovery-card input at **2026-09-10 16:37:42.868 UTC**, team `l1-claude-lead`, agent `lead`, session `e0d73e8d-b9b5-4998-98a8-f9828b526724`. No assistant generation completed, so authenticated model completion remains unverified. This was not an observed login refusal.

`initialize-result.json` records one canonical initialize, ID `init_234aab592ca34c9dae7d2541ecc37dcd`, all nine pipeline steps succeeded, format 2, delivery owner `team`, two managed launch-new tmux seats. Production `SessionStart(compact)` registration is retained in `production-hooks-before-launch.json`. No descriptor was edited and no hosted seat was created; the authorized variant uses two alpha markers in steps 2–3.

The complete captured journal has six rows. Lead startup message `c0ae4bd6-b0e9-4c8e-89db-5945b1fd06cd`, delivery `060936ad-5886-4e48-875d-a0f2b3eefec7`, has `native-mailbox/1` attempt and `native_enqueued` receipt (sequences 3–4). The startup card then appears in Claude's native input without any `mesh read`. `inboxes/lead.json` observations are preserved in snapshots; an empty post-poll projection is not proof of failed uptake. Alpha startup message `d3a75483-29f4-4333-b8e4-288c83ebb762` has tmux `submitted` receipt (sequence 6). These are startup observations, **not** the two required reply markers, explicit reads, model actions, or a post-compaction recovery card. Alpha attribution/idle/card completion, native reply routing after the membership fix, and steps 2–6 remain unverified. No external reader marked the lead inbox; no paginated journal export was reached.

### Provenance, costs and cleanup

Executed controller commit **bbe8fdc4**; Taurhaus merged base **1db4f9bf**, protocol **27**, version **0.9.7**. Checkout-local daemon SHA256 `9dfc9daacbe21380405cec063c37a433becfdb32b2fffffd20813879f679f590`. Designated Mesh worktree detached at **ed59187**, version string **mesh 0.2.29**, runtime SHA256 `35b1caf1e6c2379550a79c7c6e024de128af7f6b5b73f59d14ad32d23f63ca17`. Both `just build-daemon` and `cargo build --bin mesh` exited **0**, following bounded Cargo admission polling; production hook adapter link exited **0**. No binary was installed.

Actual Claude **2.1.267**, model **claude-haiku-4-5-20251001**; Codex **0.153.4**, model **gpt-5.6-luna**, effort **low**. Both native Codex siblings were resolved from the installed launcher and copied together. The confirmed-submission logic was retained, but no controller-typed model submission was reached. Full binary digests and sanitized exact commands/RPC results are in `events.jsonl` and `isolation.json`.

| Spend item | Observed amount |
|---|---|
| Claude startup, 1 reserved input / native card input | **Unknown**: interrupted before any assistant usage generation. |
| Codex startup, 1 reserved input; turn `01a08c2e-ab5b-7873-bb59-518ebb12df15` | **Unknown**: one started turn, no retained token-usage row. |
| Controller-typed inputs / compactions | **0 / 0**; no compact was attempted. |
| Run-4 total seat spend | **Unknown**, not $0; the $2 dollar bound is unverified. |
| Historical closed-run seat estimate | **$0.10193487**; not charged to run-4 admission. |
| Historical completed reviewer estimate | **$0.798975**, plus an earlier timed-out review's unknown usage. |
| Run-4 independent Opus review | **$0.111810**, one completed request; separate from the seat cap. |
| Implementer/orchestrator | Unavailable to this controller. |

`cost-ledger.json` preserves zero arithmetic counters **with `metering_complete: false`**. These zero counters do not mean free turns. The initial two onboarding starts were reserved; no subsequent paid input was admitted, and there was no retry or reset after this execution. Teardown was immediate on the harness failure and did not retain completed startup usage. The transcript-based cap cannot be certified retrospectively.

Runtime HOME, all harness roots, project/data/temp roots, private tmux socket, and probed non-default daemon port were scratch-only. Bubblewrap hid operator homes and mounted the single authorized Claude credential read-only; it was never copied. Only the authorized Codex auth file was copied, mode 0600. The complete **42-row daemon JSONL** is retained in `taurhaus.log.jsonl`; no account-usage row occurred. Bounded panes, native transcript, Codex session/turn metadata, runtime/config/activity snapshots, receipts and passive locks are retained and deduplicated after shutdown.

Controller and driver exited **2**; step-1 driver **1**. The owned Bubblewrap bootstrap exited **-15** during normal teardown. `cleanup.json` and independent `teardown-audit.json` show **zero surviving trial processes**, closed private listener, removed scratch root/Codex auth, and zero-byte Claude mount placeholder. No operator process was killed.

### Offline correction and deviations

Three new offline regressions observed red before implementation: fresh-budget reset helper missing (1 error), then missing attribution-wait and failure-classification helpers (2 errors). **35 offline tests pass** after fixes (31 discovered and four existing controller regressions). Generated fixtures invoke no real CLI or credential path. The immediate identity assertion originated in **a94047bc**; the blanket authentication fallback in **b1ca7979**. The offline correction **ccc62453** waits 65 seconds for attribution and no longer diagnoses auth from a generic driver error. It was **not used in this paid run** and does not change its outcome. No paid rerun is authorized by this report.

The exact gates ran in credential-free namespaces from the checkout root: **`just check-quick` 0**, **`just lint` 0**, **`just test-contracts` 0**. Check-quick passed 2,518 frontend tests with zero typecheck errors/warnings; contracts passed 15 renderer, 20 harness-conformance and 33 module-boundary tests. The gate wrapper exited 0. See `execution-gate-audit.json` and the retained gate logs. No `src-tauri/` file changed, so conditional `just test-rust-unit` is inapplicable. The independent **claude-opus-4-6** evidence lens exited **0**, verdict **approve**, one minor explicitly accepting the transparent unknown-spend disclosure with no fix required. One request cost **$0.111810** (2 input, 10,965 cache-creation, 86 output tokens). It approves the stopped-run report, not missing runtime steps. Its scratch namespace was waited and removed; the credential placeholder remained zero bytes. Independent process audits found no runtime, build, gate or reviewer survivor. Known closed-run seats plus completed historical/current reviews total **$1.01271987**, excluding this run’s unknown seat spend, the earlier timed-out review’s unknown spend and unavailable implementer/orchestrator usage.

Deviations: step 1 lacked its required attribution observation window; the raw driver failure label was inaccurate; interrupted startup metering cannot certify the dollar bound. Those defects make this lane unavailable. Steps 2–6, completed signed-in generation, alpha fresh-idle attribution, native marker round trip and compaction recovery remain untested. The authorized no-beta and credential-source exceptions are followed. No product change, descriptor edit, plan-ledger edit, unrelated checkout mutation, or paid retry occurred. No numbered runtime step was green; commits record preparation, offline tests and stopped-run evidence only.

Full run-4 sidecar report: [report](l1-claude-lead/run4/report.md).


Evidence packaging: `snapshots.json` maps **206 original paths to 88 unique objects**, including `initialize/`, `step1-observation-end/`, `latest/`, `final/`, build/gate logs and the Opus review. References to files in those directories are snapshot aliases. Top-level runtime files, including the full daemon JSONL and step-1 driver log, remain direct files. The earlier pre-ruling gate records and disposition are retained as `pre-ruling-gates/` aliases and `pre-ruling-final-disposition.json`; closed-run ledgers were not changed. `execution-gate-audit.json` supersedes the historical gate audit for this paid execution.


### Continuation verification after 2ed11ccf

The tree was clean and all previous green work was committed. An exact-match check found a missing space in the quoted ruling heading (`RULINGFOR`); the report and helper now match the binding text. The corrected quote is retained in `run4/continuation-check/authorization-quote.json`. The historical admission ledger is unchanged; this typography correction does not reset the budget again. Red and green checks are retained alongside it.

All **35 offline tests passed**. The controller's admission path was checked with a mocked command boundary against the retained run-4 ledger: it rejects the next input with `unverified metering before next input`, makes zero CLI calls, and counts only run 4's 1 Claude / 1 Codex inputs. This is not the superseded 8/8 cumulative-cap refusal. Since the interrupted startup has no usage records and its scratch root was removed, the remaining run-4 dollar headroom cannot be established from retained evidence. The continuation request permits further work but supplies no replacement dollar-budget accounting for that unknown amount. No new paid input or credential access occurred.

The explicitly requested gates ran again from this checkout root in a credential-free private namespace: **`just check-quick` 0**, **`just lint` 0**, **`just test-contracts` 0**; wrapper **0**, all children waited, scratch root removed, independent process audit **zero survivors**. No Rust diff, so `just test-rust-unit` remains inapplicable. New seat/reviewer spend **$0/$0**; implementer spend unavailable. The existing step-1 FAIL and steps 2–6 NOT RUN remain unchanged. See `run4/continuation-check/gate-audit.json`, `admission-check.json` and the gate files retained by alias in its `snapshots.json`.


## Run 4 — third attempt (run 4c): original report, classification superseded

**Original verdict (superseded by the review correction above): step 1 PASS; step 2 FAIL (taurhaus); steps 3–6 NOT RUN.** The repaired controller ran for real on 2026-09-10, 16:59–17:00 UTC. Previous run-4 refusal and interrupted-startup verdicts above are historical. No product change or retry was made. Full evidence and detailed per-step classification: [run4c report](l1-claude-lead/run4c/report.md).

| Ordered step | Outcome / classification |
|---|---|
| 1. Initialize, authenticated/team-bound Claude, owner and hook | **PASS**, committed `4cb26f5f`. |
| 2. Claude sends two distinct markers to alpha | **FAIL — taurhaus**, before sends: alpha's exported activity never became freshly idle during the full 65-second prerequisite window. |
| 3. Alpha read/replies and native Claude uptake/action | **NOT RUN**, step-2 dependency. |
| 4. Explicit Claude inbox read/mark with cursors | **NOT RUN**, step-2 dependency. |
| 5. Ordinary compact and genuine native recovery card | **NOT RUN**, step-2 dependency. |
| 6. Fresh post-recovery native reply/action | **NOT RUN**, step-2 dependency; failure export and teardown completed. |

Alpha **was session-attributed and consumed its onboarding card**. Its startup turn completed at 16:59:31.246 UTC, and its final pane shows an empty composer. The runtime snapshot reports idle/low-confidence/unattributed activity, while the exported member activity moves from `likely_working` to `uncertain`, never the required `idle`. This was originally classified as an activity-export failure. Review establishes that the derived activity gate was a harness prerequisite; onboarding exposure succeeded, so it does not establish the specified product failure. Retained evidence: [raw outcome](l1-claude-lead/run4c/step2-outcome.json), [runtime snapshot](l1-claude-lead/run4c/runtime-session-snapshot.json), [observation window](l1-claude-lead/run4c/step2-window-0.json), [review excerpts](l1-claude-lead/run4c/review-excerpts.json), and full [226-row daemon JSONL](l1-claude-lead/run4c/taurhaus.log.jsonl).

Lead's startup native mailbox projection and authenticated Haiku completion passed. Alpha's card has tmux `submitted` and explicit `consumed_by_read` receipts. The lead inbox file exists and ends empty after native polling; no lead read or terminal-delivery receipt occurred. These are startup observations, not the unrun marker round trip or compaction. All seven journal rows are retained; journal CLI pagination was not reached. Alpha attempted four unsolicited invalid send commands during its one startup turn, contrary to trial instructions; every attempt returned an error and no reply was accepted.

Both binding budget rulings are quoted in the [fresh run4c admission ledger](l1-claude-lead/run4c/admission-ledger.json). The standing rule's authorization is: “EVERY attempt of this lane starts with the FRESH budget (<= 8 Claude inputs incl. `/compact`, <= 12 Codex inputs, <= USD 2 of metered spend)”. Earlier unknown startup spend is retained as history and does not consume run4c headroom. No budget question was raised.

Current spend: **Claude $0.01179540**, **Codex $0.00511548**, seats total **$0.01691088**, conservative upper-rate estimate **$0.14121140**; inputs **1/8 Claude, 1/12 Codex**, zero controller-typed inputs and zero compactions. Metering is complete. Independent **claude-opus-4-6** review cost **$0.164920**, separately capped at $0.20; current seats plus review **$0.18183088**. Historical runs 1–3 seats **$0.10193487**, completed earlier reviews **$0.910785**, earlier run-4 seats and timed-out reviewer **unknown**; offline continuation new spend zero. Implementer/orchestrator spend unavailable. See [spend summary](l1-claude-lead/run4c/spend-summary.json).

Built locally from Taurhaus base **1db4f9bf**, actual protocol **27**; Mesh **ed59187** built only in designated `mesh-l1`, descriptor unchanged. `just build-daemon`, Mesh build and production hook adapter each exited **0**. Actual Claude **2.1.267 / claude-haiku-4-5-20251001**, Codex **0.153.4 / gpt-5.6-luna / low**; both native Codex siblings provisioned. Runtime controller code from the fixed attempt was reused, with only run4c evidence/admission/review routing changes. The new routing regression observed red, then **33 offline tests passed**; generated tests never used real credentials or CLIs.

Exact gates, run from this checkout root inside the credential-free namespace: **`just check-quick` 0**, **`just lint` 0**, **`just test-contracts` 0**, wrapper **0**. No Rust diff; conditional `just test-rust-unit` inapplicable. [Gate audit](l1-claude-lead/run4c/gate-audit.json). Opus review exited **0**, **approve**, two accepted minor findings, no fix required; this approves a stopped-run report, not E2E PASS. [Review summary](l1-claude-lead/run4c/review-summary.json).

Runtime **81.524 seconds**, driver/controller **2**, step drivers **0 / 1**. [Cleanup](l1-claude-lead/run4c/cleanup.json) and [independent teardown audit](l1-claude-lead/run4c/teardown-audit.json) verify zero surviving trial processes, private port closed, scratch root and Codex auth copy removed, Claude credential never copied and mount placeholder zero bytes. Build/gate/review cleanup also passed. No operator process was killed. Sidecar snapshots are deduplicated after shutdown; aliases preserve original snapshot, gate and review paths.

Deviations/limits: first failure stopped step 2 before sends; steps 3–6 remain unverified. Alpha's invalid startup sends and the corrected interim activity description are disclosed. The authorized no-beta variant and credential exceptions were followed. No product, descriptor, plan-ledger or other Taurhaus checkout was edited; this branch was not switched.

### Run 4c review correction — controller fixed offline

Review of the retained packet confirms `codex_ready == true`, `codex_submitted == true` with the empty final composer, recovery card exposed, and derived `alpha_attributed_idle == false`. The first failure was therefore the controller's derived-view prerequisite, introduced in `17a07099`. Step 2 is **UNAVAILABLE — harness**, rather than an established product failure; steps 3–6 remain **NOT RUN**. The original sidecars and complete daemon JSONL remain unchanged. The corrected six-row summary is directly below the headline; the continuation section is explicitly historical.

The controller now waits for native turn completion and the confirmed-empty composer, records runtime/activity snapshots and the missing derived-idle observation as a deviation, then checks onboarding exposure and proceeds to the two marker sends. A typed onboarding timeout can receive `taurhaus` ownership only with current daemon response, readable runtime, fresh activity, attributed session and a pending card without exposure. Native transcript/read receipts override an old pending projection. Missing observations yield harness unavailability; a message-string prefix no longer establishes ownership. The log-retention counter is now `excluded_usage_rows`, which survives sanitization. Run-3 tests inspect their retained report/disposition, without constraining the mutable latest headline.

Seven new offline regression tests cover marker submission despite uncertain activity, readiness/composer checks, failure ownership, fresh/consumed-card observations, the exclusion counter and headline independence. The exact step-2 regression failed red with **0 marker-send inputs versus expected 1**; the mutable-headline test failed on `unresolved`, and the missing retention/facts helpers failed before implementation. Journal fixtures were checked against the native payload shape; the consumed-card assertion also failed red before correction. After implementation, **33 discovered tests plus 11 controller tests passed (44 total)**. Generated tests start no CLI and access no real harness home. A separate document check failed before the historical heading/current table correction, then passed with six current rows and both run4c links.

Required commands ran from the checkout root in the existing credential-free gate wrapper: **`just check-quick` 0**, **`just lint` 0**, **`just test-contracts` 0**. Contracts first exited **101 twice** because the repository-wide retired-identifier scan included generated isolation JSON, then its `.txt` archive, under `.check-logs`; these were harness-log placement failures. Retaining those logs with a `.log` suffix and writing the retry output outside the scanned checkout resolved the contamination. The final exact contract command passed all **68 tests** without changing product code, assertions or exclusions. Each wrapper exited **0** (its per-gate exit records, not wrapper exit alone, determine the result). All children were waited, all three scratch roots removed, and independent process audit found **zero survivors**. Detailed local red/green logs, failed gate records and final audit are under `.check-logs/l1-review-fix/`; no additional tracked sidecars were introduced. No `src-tauri/` diff, so `just test-rust-unit` is inapplicable.

Scope/deviations: this is the supplied review's local fix round, not a replacement paid run. The completed run4c packet is preserved; no credential access, model-seat restart, new compact, paid retry or additional reviewer invocation occurred. **New seat spend $0; new reviewer spend $0; implementer usage unavailable.** All original spend remains as itemized above, with unknown history still unknown. The driver rename is deferred to the suggested follow-up because its callers (`test_run4.py` and execution routing) lie outside the named-file correction. The supplied independent review is the review lens for this fix round; the prior sidecar's approval of product ownership is superseded, and no new independent approval is claimed. No product, descriptor, ledger row, branch or other-checkout changes.

## Run 4 — fourth attempt (run4d): real execution


The corrected controller ran for real on 2026-09-10, for 113.686 seconds. Step 1 PASS; step 2 FAIL; steps 3–6 NOT RUN under the binding first-failure rule. Unlike run4c, the native-readiness gate passed and Claude actually executed both marker sends. Both were accepted, but neither reached alpha during the full 65-second exposure window. This is a delivery failure, not a readiness-gate refusal or an E2E PASS.

| Step | Outcome / classification | Evidence |
|---|---|---|
| 1. Initialize; signed-in, team-bound Claude; owner/roots/hooks | PASS | `step1-outcome.json`, committed `1ad06730`; canonical format 2, owner team, production SessionStart(compact) hook, authenticated Haiku generation. |
| 2. Claude sends two markers; recipient exposure | FAIL — taurhaus (ownership inference) | Claude Bash tools accepted `aeef42ef-9ba8-4a64-94ed-c1a2810407be` (AMBER) and `a8950c6a-3203-4da3-bb09-c7ff7f421774` (BIRCH), journal sequences 8–9. Neither has a delivery attempt/receipt or native Codex exposure. `step2-window-3.json`, `review-excerpts.json`. |
| 3. Explicit seat reads/replies; Claude native uptake | NOT RUN — dependency on 2 | No alpha reply/native round trip claimed. |
| 4. Explicit Claude read/mark, cursors | NOT RUN — dependency on 2 | No lead mesh read or consumed_by_read receipt. |
| 5. Ordinary /compact, new generation and genuine recovery card | NOT RUN — dependency on 2 | Zero compact inputs. Startup hook/card is not compact evidence. |
| 6. Fresh post-recovery native mail | NOT RUN — dependency on 2 | Failure export and cleanup performed separately. |

## Delivery evidence and ownership

Alpha's attributed session is `01a08c76-8916-7d71-96e5-2331f4ce7dcb`, native completed onboarding turn `01a08c76-8dec-7d41-b519-1acdeb763c55`; the final composer is empty. Onboarding message `1bf8147d-1c37-4309-b269-d4252ce3fe89` was submitted through tmux and explicitly consumed by alpha (journal 5–7). Its card was exposed, so the pending-onboarding-card trigger does not apply.

At failure, the daemon responded, runtime was readable, the activity snapshot was fresh, and the session path/identity was attributed. Nevertheless, Taurhaus exported `activity_confidence: uncertain`; its runtime session view said idle/low/none. Mesh health reported `pending: activity not freshly idle`, completed 1 (onboarding only), failures 0. The accepted marker rows had no attempts. Taurhaus's classifier requires positive attribution/non-low confidence for idle (`src-tauri/src/coordination/activity_export.rs:603`); Mesh correctly refuses a non-idle export (`mesh-l1/src/delivery/runtime.rs:180`). Route this observed starvation to Taurhaus's activity/idle signal owner. The deeper loss-of-confidence cause is not proven; no product fix is made here. The raw controller's `product-owner-pending-evidence-review` label remains in `result.json`.

Claude session `caffb557-d8e6-46b8-baeb-f2b7c35b977c` ran both exact commands retained in `claude-transcript.json` and `review-excerpts.json`. Its statement “delivered” is model text only; journal acceptance is the strongest marker receipt. The transcript proves the typed input started a real turn and both sends completed before the recipient window. The final empty Claude composer is retained, but the reused controller has no explicit pre-window Claude composer-confirmation event (Codex typed-input confirmation code was unchanged; no Codex input was typed this attempt).

Lead `inboxes/lead.json` existed; the startup card used native-mailbox/1, with no Claude terminal receipt. There are no seat replies to test native uptake here. The complete nine-row journal segment is archived directly as a snapshot, not described as a paginated reader export. All 299 complete daemon JSONL rows are retained in `taurhaus.log.jsonl`; excluded usage rows 0. Repeated snapshots are losslessly interned: `snapshots.json` uses `objects[aliases[path]].content` (401 paths, 112 objects). Each pane capture is at most 60 lines. Quota banners/rate_limits were redacted with a generated-data regression; token metering is preserved. Local intermediate commits retain the earlier quota metadata; the final packet is scrubbed. No credential contents were retained.

## Budget, provenance, gates and cleanup

Fresh run4d caps are 8 Claude inputs including compact, 12 Codex inputs and $2 metered. `admission-ledger.json` quotes the STANDING BUDGET RULE and fourth-attempt ruling verbatim, starts at zero, and preserves earlier spend as history. No paid rerun occurred.

| Spend | USD |
|---|---:|
| Claude generation msg_011CevBxaqXHBXgkpnp3FAyZ | 0.01163915 |
| Claude generation msg_011CevC1FmPkgZuPVmWkXUBo | 0.00547050 |
| Claude generation msg_011CevC1feL3Mx3rivR9mXMj | 0.00329255 |
| Claude total | 0.02040220 |
| Codex sole onboarding turn | 0.00470888 |
| Run4d seats | 0.02511108 |
| All-token upper-rate amount | 0.28845840 |

Counts: Claude 2/8 (startup + one typed input), Codex 3/12 conservatively reserved (startup + two marker exposures that never occurred); actual Codex turns 1, typed Codex inputs 0, compact 0. Metering is complete. Claude cap × largest observed input is $0.09311320; next-turn admission reserved $0.50. These are token-based API-equivalent estimates, not invoices. Rates and complete usage fields are in `cost-ledger.json`; reasoning output is not double charged.

Historical, excluded from run4d: runs 1–3 seats $0.10193487; run4c seats $0.01691088; earlier interrupted run4 startup unknown; earlier completed reviews $1.075705 (including run4c $0.164920); earlier timed-out review unknown. Implementer/orchestrator spend unavailable. Independent review is approved; its invocation accounting and corrected brief are recorded below, separately from seat spend.

Taurhaus base `1db4f9bf`, protocol 27; Mesh `ed59187` in the designated mesh-l1 worktree, no descriptor edits. Actual Claude 2.1.267 / claude-haiku-4-5-20251001; Codex 0.153.4 / gpt-5.6-luna / low. Both native Codex siblings were copied together. The spec's sole alpha replaces the audit's alpha/beta; no hosted seat. Binary digests and initialization identity are in `provenance.json` and `initialize-accepted.json`.

Gate exits: `just build-daemon` 0; lane Mesh `cargo build --bin mesh` 0; production hook adapter 0; **`just check-quick` 0; `just lint` 0; `just test-contracts` 0**. Required gates ran from this checkout root under the existing credential-free Bubblewrap wrapper, with real CLIs blocked and real harness homes hidden. Cargo admission waited for unrelated work without touching it. No `src-tauri/` diff, so `just test-rust-unit` is inapplicable. Runtime controller and driver exit 2; step drivers 0 / 1. Offline admission red: run4d label AssertionError, exit 1; green 18 checks, exit 0. Quota-redaction red: retained rate_limits, exit 1; green 25 checks, exit 0. Earlier discovered suite 34 passed before the added quota test.

`cleanup.json` and independently repeated `teardown-audit.json` show zero survivors, private port closed, scratch root and Codex auth copy removed, and the Claude credential placeholder still zero bytes. Only a single read-only credential bind exposed Claude auth; no Claude credential copy. Gate cleanup also waited all children and removed its scratch root. No product, descriptor, plan-ledger, branch or other Taurhaus-checkout changes. The only non-test harness change before launch was admitting/routing the run4d label. Steps 3–6 remain unverified.


### Final review and evidence audit

Independent tool-free **Opus approved** this failed-run report; no required fix. The first review returned `error_max_budget_usd`, exit **1**, after **$0.245870** (above its nominal $0.20 CLI setting), with no verdict. The brief-routing bug was reproduced red (missing bounded-packet selector), fixed offline, and its eight tests passed. A local brief-generation assertion also caused one review-reader exit before any model process ($0); that cleanup is retained. The corrected smaller review exited **0**, cost **$0.097925**, and approved. Total current reviews **$0.343795**; seats plus reviews **$0.36890608**. Both paid invocations are retained; neither restarted a lane seat. Historical unknown spend remains unknown, so no total-program spend is claimed.

The review's cleanup note is resolved here: child exit **-15** is SIGTERM sent to the owned long-running Bubblewrap namespace during ordinary finally cleanup after the step-2 failure. It was not fault injection. An independent final scan found zero runtime/reviewer survivors; all scratch roots are removed (`final-process-audit.json`). The unused preliminary reviewer root was also removed. The second minor finding requires no action: prior timed-out review spend remains unknown and excluded from run4d.

Final unpaid Python verification: **36 discovered tests + 11 controller regressions = 47 passed**, exits **0/0**. Required gates remain **check-quick 0, lint 0, test-contracts 0**; the later review/redaction changes are confined to Python harness/evidence, covered by those offline tests. Archive scan found no forbidden operator paths, credential patterns, or live quota disclosures; each pane has at most 60 lines. All 299 daemon rows remain. Raw earlier checkpoint quota metadata is historical and disclosed; final retained packet is scrubbed. `review-disposition.json` retains both findings and the full invocation accounting. Latest verdict remains **FAIL step 2 — taurhaus**, with **steps 3–6 NOT RUN**.


## Run 5 — fifth attempt: attribution and native replies pass; compact unavailable

**S-runtime: steps 1–4 PASS; step 5 UNAVAILABLE — harness; step 6 NOT RUN.** Run5 proves the requested post-onboarding Codex attribution and native Claude mailbox round trip. It does not prove compaction recovery or a complete attempt dollar cap. No product change or paid retry occurred.

| Step | Outcome and retained proof |
|---|---|
| 1 | PASS. One production `coordination.initialize_team`, format 2, team delivery owner, managed signed-in Claude lead, alpha tmux; production SessionStart(compact) hook registered. `initialize-result.json`, `production-hooks-before-launch.json`, `step1-outcome.json`. |
| 2 | PASS. Claude Bash sends AMBER and BIRCH; alpha remains attributed to its own session after its first turn, with notify-sourced idle. Both sends have submitted receipts and explicit-read tool-result exposure. `review-excerpts.json`, `alpha-activity-observations.jsonl`, `codex-transcript.json`, `step2-outcome.json`. |
| 3 | PASS. Alpha explicitly reads and sends both replies. Claude receives both as native teammate messages and distinguishes them before any explicit lead read. `step3-before-read/`, native transcript row 13 and response row 15, journal 16–21. |
| 4 | PASS. Claude executes the explicit read/mark; journal 23–24 separately proves `consumed_by_read`, context `explicit-mesh-cli`. The read returned `done=true`; no further cursor page was required. `step4/`, `claude-transcript.json`. |
| 5 | UNAVAILABLE — harness. Real `/compact` boundary and hook received/resolved, but hook skips `no_resumable_task_context`; runtime generation remains 0. Controller then aborts on a false input count before its full observation window. `result.json`, `budget-stop-reproduction.json`, `review-excerpts.json`, `final/`. |
| 6 | NOT RUN — dependency on step 5. No fresh FERN input, recovery uptake, or post-recovery exactly-once claim. `driver-steps.json`. |

Directory references resolve through `snapshots.json` after deduplication. The complete 338-row `taurhaus.log.jsonl` stays a direct file (zero excluded usage rows). Runtime duration: **109.274 seconds**, driver/controller exits **1/1**, ordered driver exits **0, 0, 0, 0, 1**. Each passing step has its own commit.

### Identity and mailbox evidence

Alpha session **01a08d4d-6eb8-7700-b6b5-8c7ae4f11ce6** stays bound to its rollout and writer-lock name. A foreign ephemeral notify for **01a08d4d-758f-7580-b505-67a7bc87379b** occurs at 21:51:02.426 UTC. Alpha's own first completion arrives at 21:51:30.831 UTC; the scanner records **active → idle, source notify** at **21:51:31.268 UTC**, retaining alpha's ID. Further notify-idle transitions follow both subsequent turns. Passive samples and `codex-session-files.jsonl` retain runtime, activity and lock/rollout names; no post-first-turn unattribution was observed.

The two outbound message IDs are **a11b2e08-4965-4125-80ff-e5cdde74a81e** and **787abdaa-b145-49f2-afa8-76bcdf11833f** (journal 8–9). Receipts 12–13 are `submitted`, 14–15 `consumed_by_read` by alpha. Marker text appears in actual Codex tool results, beyond mere prompt/acceptance presence.

Replies **f45b809d-2717-4f38-b132-7f514b0518c3** and **ba2147c9-0ec7-47dd-b4e7-d31913b53502** are accepted at 16–17, then `native_enqueued` via `native-mailbox/1` at 20–21. Claude session **c42a4d9d-0c53-4298-b6f5-6b3cf6774f4d** receives both in one native teammate batch and responds: “Markers confirmed: L1_AMBER_42 and L1_BIRCH_73 received by alpha.” `inboxes/lead.json` exists and is empty by the pre-explicit-read snapshot; native transcript plus projection receipts prove uptake, without inferring a canonical read receipt from it. Later explicit-read receipts are separate. No terminal-delivery receipt for Claude was observed, no drain descriptor was enabled, and no external reader marked the lead inbox before step 3.

### Step 5 boundary and classification

The controller reserved **4 Claude inputs**: onboarding, step-2 send request, step-4 read request and the one compact. Its retained ledger incorrectly reports **9**, because `cumulative()` takes the maximum of controller inputs and every string-valued transcript user row. The compact adds a summary, local-command caveat, command echo and local-command output; the native reply batch is counted too. The false cap stop happened about 28 seconds after compact input, before the required 120-second observation ended. This is a harness stop, not proof of a product timeout or actual input-cap exceedance. The unchanged runtime controller is retained, with an offline evidence reproduction; this lane makes no repair or rerun.

At 21:52:43.118–.127 UTC, the real hook received `source=compact`, resolved lead, and skipped **no_resumable_task_context**. The operational snapshot has empty task ID/status and role ID. `compact_hook.rs:698` explicitly skips snapshots without resumable task context. Therefore this fixture cannot establish a recovery failure for a properly assigned seat. The summary's old recovery-card wording is not a new hook card. Context generation and card revision remain unchanged.

### Spend, isolation and verification

Known Claude usage **$0.03495185**, Codex **$0.01284780**, known seats **$0.04779965**; retained upper-rate amount **$0.60329760**. Six ordinary Claude API generations, three Codex native turns, four reserved Codex inputs, one compact. The compaction summarizer has no separate usage-bearing generation in the retained transcript: **compact spend and complete attempt spend are unknown**, not zero. The controller's `metering_complete=true` only establishes its Codex rollout accounting and does not prove complete compact metering. See `spend-summary.json` for current review spend and all historical amounts.

Historical seats, excluded from this fresh attempt: runs 1–3 **$0.10193487**, run4c **$0.01691088**, run4d **$0.02511108**, interrupted run4 startup unknown. Earlier completed reviews **$1.419500**, earlier timed-out review unknown. Implementer/orchestrator usage unavailable. Independent Opus review is recorded separately; no total-program spend claim.

Built from required Taurhaus base **a7e6db7e**, protocol **27**, checkout-local target; Mesh **310144d**, designated mesh-l1 only. `build-identities.json` retains binary SHA-256 digests. Actual Claude **2.1.267 / claude-haiku-4-5-20251001**; Codex **0.153.4 / gpt-5.6-luna / low**, both native siblings. No hosted seat, descriptor edit, installation or release. Startup composer/model footer observed. Typed Codex step-3 input confirmed by empty composer plus new turn before its 65-second opportunity.

Build exits: daemon **0**, Mesh **0**, production hook adapter **0**. Exact checkout-root gates in credential-free namespaces: **`just check-quick` 0; `just lint` 0; `just test-contracts` 0**. No `src-tauri/` diff; conditional `just test-rust-unit` inapplicable. Offline setup checks observed red (four setup failures, then hook-admission failure), then green; final Python test totals are recorded in `offline-verification.json`. No tests invoked real CLIs or real harness homes.

Runtime `cleanup.json` and independent `teardown-audit.json`: **zero survivors**, private port closed, scratch root and Codex auth copy removed; Claude credential only a read-only single-file bind, zero-byte placeholder after unmount, never copied. SIGTERM/-15 is ordinary owned-namespace cleanup, not fault injection. Gate/build/review children are waited and their scratch roots removed. Full scope deviations are listed in `final-disposition.json`.


Independent **claude-opus-4-6** evidence review exited **0**, **approve**, no required fix. One request cost **$0.156075**, separately from seats. Known current seats plus review **$0.20387465**, **plus unknown compact spend**; no complete total or $2-cap proof is claimed. Review credential placeholder remained zero bytes and its scratch root was removed. Final offline verification: **41 discovered tests + 11 controller regressions = 52 passed**, both exits 0. No runtime logic fix or paid rerun followed the step-5 stop.

Final archive audit: **714 snapshot paths → 202 unique objects**, all **338 daemon JSONL rows retained**. No forbidden operator paths, credential patterns or oversized pane captures were found. Independent final process audit found **zero survivors** across runtime, build, gates and reviewer; all four scratch roots are removed. See `final-audit.json`.


### Run5 continuation verification — 2026-09-11

The tree at `8794545f` was clean; passing runtime steps 1–4 were already committed individually. The requested gates ran again from this checkout root inside the credential-free namespace: **`just check-quick` 0**, **`just lint` 0**, **`just test-contracts` 0**; wrapper **0**. Offline tests: **41 discovered + 11 controller regressions**, exits **0/0**. No Rust diff, so `just test-rust-unit` remains inapplicable. All owned gate children were waited; an independent audit found zero survivors and the scratch root removed. Gate logs are retained under `run5/continuation-check/snapshots.json`, with results in `continuation.json`.

No green runtime step remained uncommitted. **Step 5 remains UNAVAILABLE — harness; step 6 NOT RUN.** Mandatory teardown removed the original sessions, and the binding spec freezes the runtime controller and requires stopping at the first failure. This continuation does not change the controller/fixture or launch a replacement attempt. New seat/reviewer spend **$0/$0**; implementer usage unavailable. Prior compact spend remains unknown. The existing Opus approval covers the stopped-run report; no new reviewer invocation or runtime success is claimed.

## Run 6 — controller preparation

The three offline regression checks failed before implementation (exit 1): input count 9 versus 4, missing real-assignment fixture, missing run6 route. After the corrections, 44 discovered tests and 11 controller regressions passed (exits 0/0). The assignment is created and assigned through Mesh as lead, then the production operational snapshot is polled before compact; no recovery state is synthesized. Prior runs retain their original ledgers as history.

### Run 6 outcome — stopped before compact

On 2026-09-10 at 22:21–22:22 UTC, steps 1–4 passed and were committed individually. Alpha kept its own session with notify-sourced idle despite a foreign startup notification. AMBER and BIRCH were explicitly read and replied to; Claude received and distinguished both natively before its explicit read. Step 5 created and assigned task #1 successfully, then the controller misparsed a Mesh warning plus JSON (exit 1). The operational task poll and `/compact` were never reached. Step 5 is **UNAVAILABLE — harness**; step 6 **NOT RUN**.

Runtime 84.707 seconds; driver/controller 2/2; teardown confirmed zero survivors, closed private listener and removed scratch credentials. Known Claude $0.03456175 + Codex $0.01152180 = **$0.04608355**, plus unknown unfinished assignment-response spend. Zero compactions. All three exact gates passed. Initial 55 offline checks passed; the one permitted offline fix round reproduced warning parsing and misclassification, then passed **56 checks**. Full results, deviations and review accounting are in the run6 report. Run5 is superseded as the headline, with its history retained.

Run6 independent **claude-opus-4-6** review **approved**, exit 0, one request **$0.192415**. Known seats plus review **$0.23849855**, plus unknown unfinished assignment-response spend. Two minor evidence-clarity notes, no required fix; zero reviewer survivors and scratch root removed. Latest outcome remains steps 1–4 PASS, step 5 UNAVAILABLE (harness), step 6 NOT RUN.

## Run6 requested continuation

Continuation routing was added without changing the committed runtime parser/fixture behavior. The new route test failed red (exit 1), then 46 discovered tests and 11 controller regressions passed (exits 0/0). The original run6 files and historical spend remain unchanged.

### Requested continuation result

The renewed execution request was run from clean `027bc5e5`; all previous green work was already committed. Fresh isolated sessions re-established and committed steps 1–4 (`999449d8`, `1bdf9005`, `bcc64a1c`, `bf7ee51a`). Step 5 then timed out on missing operational task context, despite successful canonical assignment and native delivery. Source-backed classification: **harness prerequisite omitted**, not an established product defect. Step 6 was not run. Original run6 packet: 77 files verified unchanged.

Known continuation Claude **$0.03747000**, Codex **$0.00871644**, total **$0.04618644**; foreign startup-notify cost separately unknown. Runtime 148.533 seconds, zero compactions, zero survivors. All three gates passed; 57 offline checks passed after routing red. Full evidence, limitations and independent review are in the continuation report.

Continuation independent **claude-opus-4-6** review approved (exit 0), no required fixes. Failed review **$0.213400** plus successful shorter review **$0.092825** = review **$0.306225**; known continuation seats plus review **$0.35241144**, plus unknown foreign startup-notify usage. Both review attempts, original run6 spend and unknowns are retained. Reviewer cleanup passed. Latest outcome remains steps 1–4 PASS, step 5 UNAVAILABLE (harness prerequisite), step 6 NOT RUN.

### Run6 continuation verification — 2026-09-11

The tree at `0b219c2b` was clean; steps 1–4 were already committed individually. The required first-failure stop remains: **step 5 UNAVAILABLE — harness; step 6 NOT RUN**. The assignment exists, but the daemon-only fixture did not run app task persistence and operational-snapshot publication. Source inspection confirmed that the production-linked helper cannot call the test-only `sync_member_snapshot`; no alternate publisher, synthetic snapshot, product edit or replacement runtime was introduced.

Exact checkout-root gates ran again in the credential-free namespace: **`just check-quick` 0, `just lint` 0, `just test-contracts` 0**; wrapper 0. Offline verification: **46 discovered + 11 controller regressions passed**, exits 0/0; an additional seven-test subset also passed. No tests were added and no new red was observed; previous red-first evidence remains retained. No `src-tauri/` diff, so `just test-rust-unit` is inapplicable. All gate children were waited, scratch root removed, and an independent process scan found zero survivors.

All 155 previously tracked run6 sidecars were verified unchanged. New seat/reviewer spend **$0/$0**; implementer spend unavailable. Previous known continuation seats plus reviews **$0.35241144**, initial run6 **$0.23849855**, and all historical unknowns remain in the [spend ledger](l1-claude-lead/run6/continuation/spend-summary.json). The existing Opus approval covers the stopped-run report. [Verification and source findings](l1-claude-lead/run6/continuation-check/continuation.json); [deduplicated gate logs](l1-claude-lead/run6/continuation-check/snapshots.json).

## Run7 — 2026-09-11, fresh isolated attempt

The run6 continuation controller was reused with the app's production operational-snapshot publication added after real Mesh assignment. Two generated-data tests failed red (missing publication and run7 route), then **48 discovered tests + 11 controller regressions passed**. The assignment gains explicit footer fields in its persisted description; the controller copies those values and task identity/time into one `coordination.publish_operational_snapshots` request. No recovery snapshot/card/hook payload is generated by the harness. This addition was **not reached live** because the first-failure stop occurred earlier.

| Step | Outcome / classification |
|---|---|
| 1. Initialize, signed-in team-bound Claude, production compact hook | PASS — committed `5df747da` |
| 2. Claude sends markers; alpha exposure | UNAVAILABLE — harness onboarding prerequisite; no sends ran |
| 3. Explicit seat reads/replies; native lead uptake | NOT RUN — dependency on 2 |
| 4. Claude explicit read/mark | NOT RUN — dependency on 2 |
| 5. Assignment publication and ordinary compact | NOT RUN — dependency on 2 |
| 6. FERN after recovery | NOT RUN — dependency on 2 |

The exact sequence matters: journal sequence 4 recorded `outcome_unknown` / “paste may be present; post-wait validation failed; submit withheld” before the controller input. The controller then appended a READY-only/no-read prompt to the pending notification and waited 65 seconds for a card read. Its input contaminates that read assertion; it does not explain or exonerate the earlier product event. The raw result remains intact and the final source/evidence-based classification is harness unavailable. No product fix or paid rerun. Alpha ended attributed to its own session with notify-sourced idle, but no logged notify-to-idle edge; the full PR #172 claim is not established here.

Runtime **90.540 s**, step-driver exits **0/1**, controller/driver **2/2**. Claude generation **$0.01114540**, Codex turn **$0.00193560**, known seats **$0.01308100**, upper-rate amount **$0.07127460**. Foreign ephemeral notify usage unknown; all historical spend is retained and excluded from fresh admission. Build daemon/Mesh/hook exits **0/0/0**. Post-teardown **`just check-quick` 0, `just lint` 0, `just test-contracts` 0**. No Rust diff, so `just test-rust-unit` is inapplicable. Zero owned survivors, private listener closed, scratch roots and Codex auth removed; Claude credential read-only bind only, zero-byte placeholder, never copied. No product, descriptor, plan-ledger, branch or other Taurhaus-checkout edits.

Independent **claude-opus-4-6** evidence review **approved**, exit **0**, one request **$0.14575000**. Two minor clarity notes, no required fix: raw/final classification distinction and explicitly incomplete total spend. Known current seats plus review **$0.15883100**, plus unknown foreign startup-notify usage; implementer/orchestrator spend unavailable. Reviewer child exited, scratch root removed, credential placeholder zero bytes. No new runtime or controller fix followed the stop.

Final archive audit: **390 snapshot paths → 104 unique objects**, all **211 complete daemon JSONL rows** retained, zero excluded usage rows; 21 pane captures checked at ≤60 lines. No forbidden operator path or secret pattern found. Independent process audit found **zero survivors** across all four scratch namespaces; all roots removed. Run6 sidecars unchanged. Packed paths resolve through `snapshots.json` → `objects[aliases[path]].content`; full daemon JSONL stays directly readable.

## Run 8 — controller preparation

Five generated-data tests failed red before the run8 route and passive input guard. The additional in-flight-after-submitted case also failed red. Green: 53 discovered tests + 11 controller regressions. Run7 controller reused with the pending-delivery rule: first alpha access waits for onboarding submitted + read, fresh idle, no pending delivery and an unheld terminal lock sampled through owned-process `/proc` fdinfo. Every subsequent Codex input repeats this check. The app snapshot publication sequence remains unchanged; the evidence route and task subject are run8. No product change.

## Run 8 — stopped execution

One authorized attempt on 2026-09-10, 23:34–23:35 UTC. The unchanged step-1 prerequisite timed out after 65 seconds: the lead had a real Haiku generation and native team-bound onboarding, but the daemon's lead runtime retained `session_id: null` and `jsonl_path: null`. No paid retry, controller input or product change followed. This is an incomplete required runtime identity check, not `claude_auth_unavailable`. The underlying Taurhaus versus Claude Code attribution cause is unresolved because the scratch Claude session registry was not retained. Raw controller and final dispositions both say UNAVAILABLE/harness.

| Step | Outcome | Classification / evidence |
|---|---|---|
| 1. Initialize / signed-in team lead / hook | UNAVAILABLE | Harness prerequisite: lead session attribution missing through 65 s. `step1-driver.txt`, `runtime-session-snapshot.json`, `final/team/runtime/lead.json`; authenticated generation and native onboarding in `claude-transcript.json`; production hook in `production-hooks-before-launch.json`. |
| 2. Claude marker sends / recipient exposure | NOT RUN | First-failure stop; no controller inputs, no AMBER/BIRCH sends. |
| 3. Explicit seat reads / native lead replies | NOT RUN | First-failure stop. Alpha's onboarding read is not this step. |
| 4. Explicit lead read / acknowledgment comparison | NOT RUN | First-failure stop; no lead Mesh tools executed. |
| 5. Assignment / app publication / compact / recovery | NOT RUN | First-failure stop; no task, publication request, compact, new generation or recovery claim. |
| 6. FERN after recovery / export | NOT RUN | First-failure stop; final failure evidence exported and teardown completed. |

## Runtime evidence

Taurhaus base **a7e6db7e**, controller **d4481a7e**, protocol **27**; designated Mesh **310144d**, production descriptor unchanged, no hosted seat. Daemon built with `just build-daemon`, Mesh with `cargo build --bin mesh` only in mesh-l1, production hook adapter linked against this checkout's release library: exits **0/0/0**. Digests and commands: `build/gates/gate-daemon-build.json`, `build/gates/gate-mesh-build.json`, `hook-build.json`, binary events in `events.jsonl`. Actual Claude **2.1.267 / claude-haiku-4-5-20251001**, Codex **0.153.4 / gpt-5.6-luna / low**; both native Codex siblings copied to scratch bin.

Canonical initialize ran once, completed, format 2, team delivery owner. Lead native onboarding message `6c510e48-b6cb-43e4-95d1-f499502bb475` has `native_enqueued` receipt, inbox projection and a native `<teammate-message>` in Claude session `41dceb61-eccb-4c9e-8a4b-33e5aff023bf`. This proves signed-in native uptake, not daemon session attribution or explicit `consumed_by_read` by the lead. The final daemon snapshot exports the lead as none/low with no session/path. Its tmux pane, launch root, attachment generation and process identity are retained.

Alpha's onboarding message `a23bbb7e-b59f-45ba-bc51-d6d44e6e7360`, delivery `e7b33d6d-ebc8-43f6-a8c5-bfdfe848fbca`, has `submitted` (sequence 6) and `consumed_by_read` by alpha (sequence 7); the card is in its tool-result transcript. Final alpha activity is fresh idle/high/source notify, attributed to its own session `01a08dab-dea5-7c43-a4f2-6a43a744aa3c`. Both its own notify and foreign ephemeral notify `01a08dab-e549-7c42-bfbb-871897c8cd15` are retained. See `alpha-activity-observations.jsonl`, `alpha-runtime-observations.jsonl`, `codex-session-files.jsonl`, `codex-notify.jsonl`, `codex-transcript.json`. The required step-2 marker-delivery claim remains untested.

Alpha departed from the inherited READY-only/no-unsolicited-send instruction: it read onboarding, then tried five unsolicited Mesh sends using invalid syntax/handles. All were rejected, and the seven-row journal contains no alpha-authored accepted message. These are model tool attempts within one reserved onboarding turn, not five controller inputs or controller retries. The transcript retains their errors; they are not successful replies.

The run8 change adds only the pending-delivery input rule and evidence routing to the run7 controller. Before any Codex controller input, it waits up to 125 seconds for onboarding submitted plus read, no pending/in-flight delivery, attributed fresh idle, and an unheld terminal lock sampled passively through owned-process `/proc` fdinfo. The first prepare-alpha action has the same guard. No guard action/input was reached live because step 1 stopped first; the terminal input-lock rule is covered offline only. Existing app task-sync publication remains exactly one production request constructed from the persisted real task and footer, never a fabricated recovery card; not reached live.

## Cost and cleanup

One reserved onboarding input each: **Claude 1/8, Codex 1/12; zero controller-typed inputs; zero compactions**. Claude one metered generation **$0.01099540**, Codex one retained metered turn **$0.00470868**, known seats **$0.01570408**, upper-rate retained estimate **$0.15560520**. Claude cap times largest observed input: **$0.08796320**. Foreign startup-notify usage is **unknown**; controller `metering_complete=true` describes retained rollout accounting only, so neither complete spend nor a complete dollar-cap proof is asserted. No compact ran: compact spend $0. Offline tests/builds/gates have $0 paid seat spend. Implementer/orchestrator spend unavailable. Historical runs 1–7 and their unknowns remain in `admission-ledger.json` / `spend-summary.json` and are excluded from fresh admission. Independent review is separately accounted below.

Runtime **72.240 s**, step-driver **1**, controller/driver **2/2**. Cleanup and an independent post-gate scan found zero owned survivors, private listener closed, scratch roots removed, Codex auth copy deleted. Claude credential was only a single read-only Bubblewrap bind; zero-byte placeholder proves it was never copied. Complete daemon JSONL: **197/197 rows retained**, **0 excluded usage rows**. No token/secret/installation/usage rows disclosed. No owned daemon, private tmux server, Claude or Codex survives. No operator process killed.

## Validation and deviations

Exact checkout-root post-teardown gates: **`just check-quick` 0; `just lint` 0; `just test-contracts` 0**. Credential-free gate namespace hides real harness homes and blocks real runtime CLIs. Cargo admission used the prescribed `<3` running-process threshold, 30-second polls, local targets. No `src-tauri/` diff, so `just test-rust-unit` is inapplicable.

Five generated-data tests observed red (two assertion failures, three missing-helper errors), plus an additional in-flight-after-submitted assertion failed red. Green: **53 discovered tests + 11 controller regressions**, exits **0/0**. Tests use mocks/tempdirs, no real CLI or credentials. One preparation commit; no numbered runtime step passed, so there are no numbered PASS commits.

Full deviations in `final-disposition.json`: first-failure stop before steps 2–6; underlying lead attribution owner unresolved; unsolicited rejected onboarding sends; foreign notify spend unknown; journal pagination not reached (all seven rows retained); pending-input lock gate not reached; publication not reached. No descriptor, product, plan-ledger, branch or other Taurhaus-checkout edits. No Mesh commit.

Snapshot paths resolve via `snapshots.json` after deduplication; the complete daemon JSONL remains a direct file. This report is a stopped-run evidence result, not E2E PASS or release approval.


Independent **claude-opus-4-6** evidence review **approved**, retry exit **0**, one minor metering note and no required fix. First invocation exited **1** without a verdict (`budget_exhausted`), reporting **$0.23303000** despite a configured $0.20 maximum; the smaller-packet retry cost **$0.11230000**. Both invocations and cleanup are retained. Total review **$0.34533000**, known current seats plus review **$0.36103408**, plus unknown foreign-notify spend. No defensible foreign-session upper bound exists without its usage-bearing rollout, so the minor suggestion to bound it is documented as unsubstantiated rather than invented. Both reviewer children exited, scratch roots were removed, and credential placeholders stayed zero bytes. No runtime rerun or post-runtime controller fix.

## Run 9 — stopped execution

Run9 initialized once, exchanged AMBER/BIRCH through Claude's native mailbox, explicitly acknowledged them, published the real lead assignment, and performed one ordinary `/compact`. The production hook **delivered the real recovery card into Claude native additional-context attachment rows**. The inherited step5 collector/predicate ignores attachments and timed out after 120 seconds. This is a harness false-negative, not a Taurhaus missing-card defect. No FERN input or runtime retry followed. This stopped run is not an end-to-end PASS.

| Step | Outcome / classification | Evidence |
|---|---|---|
| 1. Initialize, signed-in team lead, production hook | PASS, with permitted harness/environment exception | Canonical format 2, team owner, native lead onboarding; `initialize-result.json`, `step1/`, `production-hooks-before-launch.json`. `sessions/` has a host-PID peer key but no PID JSON registry; `claude_registry_absent` permits continuation under run9. |
| 2. Claude sends both markers; alpha exposure | PASS / runtime | Claude Bash tools accepted `fcda7ea1-3300-47ed-b7d1-51e09b54041a` (AMBER), `47e4eb2a-79bc-4f38-a2ae-b66e8fae2d36` (BIRCH). Journal submissions 12/13 and reads 14/15; `step2/`, `step2-journal-page-0.json`. |
| 3. Alpha reads/replies; Claude native uptake | PASS / runtime | Replies `6fa95929-a5e0-4c2b-ae7d-93efd7345f56` / `4bc6f2aa-c649-4c85-813b-13a9bf59bbb0`; native messages at 00:00:51.364 / 00:00:55.491 UTC, each distinguished in lead text before explicit lead read; `step3-before-read/`, transcripts, journal. |
| 4. Claude explicit inbox read/mark | PASS / runtime | Lead `consumed_by_read` journal rows 22–24; distinct from earlier `native_enqueued` 18/21; `step4/`, journal and Claude Bash transcript. |
| 5. Real assignment, app publication, compact, recovery | UNAVAILABLE / harness assertion | Task #1 assigned in progress; production publication accepted 1, skipped 0; real hook received/resolved/delivered, context 0→1, new card identity and unchanged role. Native attachment evidence was excluded by inherited collector and user/system-only predicate; `step5-native-hook-attachments.json`, `step5-evidence-reconciliation.json`, `step5-driver.txt`. |
| 6. Post-recovery FERN, exactly-once action, export | NOT RUN / first-failure stop | No FERN input or post-recovery action claim. Failure export and teardown completed. |

Taurhaus base **a7e6db7e**, Mesh **310144d**, protocol **27**, Claude **2.1.267 / claude-haiku-4-5-20251001**, Codex **0.153.4 / gpt-5.6-luna / low**. The daemon was built in this checkout via `just build-daemon`; Mesh was built only in mesh-l1. Builds and production hook adapter exited **0/0/0**; commands/digests in `build/gates/`, `hook-build.json` and `events.jsonl`. No hosted seat or descriptor edit. Both native Codex siblings were copied into scratch bin. Startup composer/footer preflight passed.

The runtime Bubblewrap mount namespace retained tmpfs homes and the single-file read-only Claude credential bind, with **no `--unshare-pid` and no `--proc` remount**. Host lead PID **2888614** wrote only a `.key` containing peer-channel metadata, not a `<pid>.json` session registry. All registry directory files are retained under `sessions/`, with `peerToken` redacted; `index.json` distinguishes registry files from all files. An initial observer mistake classified the key as a mismatched registry; the live index was corrected from real filenames during the original 65-second window, before any step2 input, then its helper fix was tested red→green. No native file, PID, activity or hook state was fabricated. The live index correction preceded the offline reproducer; the reusable helper change followed red→green.

Alpha remains attributed to **01a08dc2-d697-75a3-b917-ca24413ee259** through notify-sourced idle transitions at daemon JSONL rows **120, 233, 279**, despite a foreign ephemeral startup notify. Runtime/activity observations, rollout and lock filenames, complete Codex transcript and notify records are retained. The run8 pending-delivery guard was reused: first input requires submitted/read onboarding, all inputs require no pending/in-flight alpha delivery, fresh attributed idle and a passively unheld terminal lock. `step2-input-guard.json` / `step3-input-guard.json` record admission; no controller text was typed during pending delivery. No terminal receipt for Claude was observed; lead receipts are native-enqueued/read. The production descriptor stayed unchanged.

The controller played **the app's task sync**, not a recovery writer: task #1, subject **Run9 native mailbox recovery**, assignment **80d8b711-039c-49ef-a21f-7a96298c1b67**. `step5-task-record.json` is the real Mesh task; `step5-publication-request.json` contains one `coordination.publish_operational_snapshots` call built from that record and its assignment footer; response is **published 1 / skipped 0**. Request/response, task creation/assignment commands and the readable operational snapshot are retained. Nothing else was synthesized.

`/compact` generated a real compact boundary. `taurhaus.log.jsonl` rows **373/374/376** record the hook received/resolved/delivered at 00:01:45 UTC, returning **3484 bytes** of context. Context generation **0→1**, content revision **2cd47b20d7da3d753eabd9d0ead96072ff5f4b8302456732645e03df4a2ac053 → 2cce49cfd07e3b3e9c73936e772de2f67758123baea328baaa8ba70ebb493dd3**, effective role revision unchanged. Native `hook_success` (`SessionStart:compact`) and `hook_additional_context` (`SessionStart`) attachments at **00:01:45.568 UTC** contain the actual lead recovery card and current assignment. They were read directly from the scratch Claude JSONL before teardown and retained separately. The original `claude-transcript.json` excludes those rows because `Trial.snapshot` retains only assistant/user/system; `steps.py` additionally requires user/system for its card predicate. Both facts explain the false negative. Raw controller FAIL/product-owner-pending-evidence-review and exit **2**, step5 driver exit **1**, remain untouched; the evidence-backed final classification is **harness**. No product failure is asserted.

Known metered seats: **Claude $0.03567015 + Codex $0.00851504 = $0.04418519**; upper-rate retained estimate **$0.76864000**. **Claude 4/8**, **Codex 4/12** input reservations (onboarding included); three Claude typed inputs include one compact, one Codex typed input, 3 actual Codex turns. Claude cap × largest observed input is **$0.08905320**. Per-generation tokens/costs and every observed Claude input group are in `cost-ledger.json` and `spend-summary.json`. Compact summarizer and foreign startup-notify usage are **unknown**, not zero; retained-rollout `metering_complete=true` does not prove total billing. Fresh admission excludes runs1–8; historical costs remain linked. Offline tests/builds/gates cost $0 paid seat spend; implementer/orchestrator spend unavailable. Independent review is recorded separately below.

Runtime **250.629 s**. Cleanup verified zero owned survivors, closed private port, removed scratch roots/Codex auth copy, and a zero-byte Claude mount placeholder: Claude credential never copied. Removing PID isolation required explicit PID/start-time-validated owned-descendant cleanup, with no operator process killed. Complete daemon JSONL **503/503 rows**, zero excluded usage rows. `cleanup.json`, `teardown-audit.json` and `post-runtime-audit.json` retain independent checks. No secrets or forbidden operator-home paths remain in evidence; pane captures are at most 60 lines.

Offline: original five new tests observed **one assertion failure plus four missing-helper errors**; key-only registry fixture then failed before the helper fix. Final **59 generated-data tests + 11 controller regressions** passed. No offline test accesses real harness homes or invokes a real CLI. Four numbered green steps were committed; two small fixture commits precede/follow those as recorded. No `src-tauri/` diff, so `just test-rust-unit` is inapplicable. Exact post-teardown gates are recorded below after completion.

Deviations are listed in `final-disposition.json`: permitted missing registry, live index correction and its TDD ordering, owned-descendant cleanup, native attachment oracle false-negative, step6 first-failure stop, and unknown spend. No product fix, plan-ledger edit, branch switch, other Taurhaus-checkout work or Mesh commit. Session registry sidecars remain directly accessible; other snapshots may resolve through `snapshots.json` after deduplication.

Exact checkout-root gates after teardown: **`just check-quick` exit 0**, **`just lint` exit 0**, **`just test-contracts` exit 0**. The credential-free gate namespace hides real harness homes and blocks real runtime CLIs; Cargo admission uses 30-second polls only when at least three Cargo processes already run. The frontend lane executed **2519 passing tests**. Gate commands/logs, isolation and cleanup are retained under `gates/`.

Independent **claude-opus-4-6** evidence review **approved**, exit **0**, cost **$0.11605500**, no required fixes. Its two minor notes require no code change; the arithmetic clarification is: the upper-rate retained estimate charges every retained Claude input/output/cache token at $5/million, and every retained Codex input/output token at $1.20/million, without cache discounts. This totals $0.76864000, excludes unknown unreported work, and is not a complete billing upper bound. Known seats plus separate review **$0.16024019**, plus unknown compact/foreign-notify spend. Reviewer child exited, scratch root removed, credential placeholder remained zero bytes. Native card's literal role line is `Role: unavailable` (no named role template); persisted effective role revision is unchanged and task #1 identity is explicit. No paid runtime retry, observer assertion rewrite, or product fix followed the stop.

## Run 10 — complete execution

Run10 completed the native Claude mailbox round trip, real compaction recovery and fresh FERN action. All six numbered step drivers and the controller exited **0**. Each numbered step was committed when green. Gates and independent review are recorded below when complete.

| Step | Outcome / classification | S-runtime evidence |
|---|---|---|
| 1. Initialize and signed-in lead | PASS / runtime, permitted registry exception | One production canonical initialize, format 2/team owner; authenticated Claude onboarding and production compact hook. `initialize-result.json`, `production-hooks-before-launch.json`, `sessions/`, `step1-registry-poll.json`. |
| 2. Claude sends AMBER and BIRCH | PASS / runtime | Lead Bash sends accepted as `0e726250-bb3f-4535-9c88-6155c8717b1a` / `47136280-fb39-499e-abde-6c3bc1b74c67`; alpha submitted/read exposures retained in Codex tool-result rows and journal. |
| 3. Alpha reads/replies; lead receives natively | PASS / runtime | Alpha replies `636c93f9-06ea-4552-9f2e-62ae26d2a254` / `19dbbe18-98b5-453e-aa64-1902242ba4a8` surfaced as native teammate rows and were distinguished by lead text before explicit lead read; `step3-before-read/`, `marker-reconciliation.json`. |
| 4. Lead explicit read/mark | PASS / runtime | Claude Bash `mesh read --unread --json --mark-read`; canonical `consumed_by_read` receipts distinct from native-enqueued receipts. `step4/`, `step4-journal-page-0-parsed.json`. |
| 5. Real compact and native recovery | PASS / runtime | One real task #1 assignment, app-wire publication accepted 1/skipped 0, one `/compact`; daemon received/resolved/delivered rows **361/362/364** at **00:28:36 UTC**, **3486 bytes**. Context **0→1**, new card revision, unchanged effective role contract, native hook attachments name lead/team and current assignment. |
| 6. Fresh FERN after recovery | PASS / runtime | `e889e3bf-a446-4756-81d5-06210b5e55f6` accepted once; native enqueue journal **30**, one teammate row, one lead text response, final explicit read receipt **32**. No terminal transport receipt for Claude. `step6-before-read/`, `step6/`, `runtime-reconciliation.json`. |

Lead response after native FERN: **“L1_FERN_91 recovered; assignment state preserved through compact.”**. This is one observed action in the bounded run, not a general exactly-once guarantee. Final read was requested only after capturing that native response; it requested no sends or further message action. `marker-reconciliation.json` preserves source rows, message IDs, native UUIDs, assistant message IDs, and all relevant receipts.

Taurhaus base **a7e6db7e**, Mesh **310144d**, daemon protocol **27**, Claude **2.1.267 / claude-haiku-4-5-20251001**, Codex **0.153.4 / gpt-5.6-luna / low**. Checkout-local `just build-daemon`, separate mesh-l1 `cargo build --bin mesh`, and production hook adapter all exited **0**; exact commands and binary digests in `build/gates/`, `hook-build.json`, and `events.jsonl`. No hosted seat, descriptor edit, drain activation, Mesh commit, product change, or plan-ledger edit.

The runtime reuses run9's Bubblewrap mount isolation **without `--unshare-pid` or `--proc` remount**. All writable harness roots, project, private daemon port and private tmux server are scratch-only; inherited TMUX was removed. Claude's credential was a single read-only file mount and never copied; only the explicitly authorized Codex auth file was copied at mode 0600. Both native Codex siblings were copied. Startup composer/footer preflight succeeded. Host Claude PID **3131197** wrote a peer key but no `<pid>.json` registry during 65 seconds; the explicit run9 `claude_registry_absent` exception permits continuation. `sessions/` retains every filename and sanitized content, excluding the peer token. No registry/index mutation occurred during run10.

Alpha stayed attributed to its own session with notify-sourced idle transitions, including beside the foreign ephemeral startup notify. See `notify-idle-evidence.json`, complete daemon JSONL, `alpha-activity-observations.jsonl`, `alpha-runtime-observations.jsonl`, Codex rollout/lock names and `codex-notify.jsonl`. The first and later alpha input guards require submitted/read onboarding, no pending delivery, fresh idle and a passively unheld terminal lock; `step2-input-guard.json`, `step3-input-guard.json`, `step6-input-guard.json` retain the admissions. No input was typed through a pending delivery.

The controller plays **the app's production task sync**: task **#1**, subject **Run10 native mailbox recovery**, assignment **368b413d-a1b1-47ad-84ce-d87dd120cd00**, status **in_progress**. The one `coordination.publish_operational_snapshots` request was constructed from the real Mesh task and assignment footer; source record, exact request, response and operational snapshot are retained. It synthesized no recovery state or hook payload. Card content revision changed **01e079b50f220b53a008b3fa9163749a4622e861ef3fd219b2423d829ea19422 → eb41a6f55cec10a20f5ea3ead1150388f9a47722c0d873535c40f35d2ba8df02**. `step5-native-hook-attachments.json` contains native `hook_success` / `SessionStart:compact` and `hook_additional_context` attachments. The role has no named template (`Role: unavailable`), but its effective role contract is unchanged and its lead/team/task identity is explicit.

Known metered seats: **Claude $0.05376690 + Codex $0.01140744 = $0.06517434**. Retained upper-rate estimate **$1.03419760** charges all Claude retained input/output/cache tokens at $5/million and Codex retained input/output tokens at $1.20/million; it excludes unknown work and is not total billing. Reservations **Claude 5/8, Codex 5/12**, including onboarding and one compact; four Claude typed inputs, two Codex typed inputs, **4** actual Codex turns. Claude cap × largest observed input group **$0.10463320**. Every retained generation/input-group cost and Codex rollout usage is in `cost-ledger.json` and `spend-summary.json`. Compact summarizer and foreign ephemeral-notify spend remain **unknown**, not zero; implementer/orchestrator spend unavailable. Tests/builds/gates launch no paid seat. Runs1–9 are historical, excluded from fresh admission. Separate review spend is appended below.

Runtime **172.251 seconds**. Teardown independently verified **zero owned survivors**, closed private port, removed runtime root and Codex auth copy; Claude mount placeholder remained zero bytes. Complete daemon JSONL **432/432 rows**, no omitted usage rows, retained through teardown. All pane captures are ≤60 lines. `cleanup.json`, `teardown-audit.json`, `post-runtime-audit.json`, and `log-retention.json` record the checks.

TDD: the run10 reproducer observed **three failures** (label rejection, collector dropping attachment, predicate rejecting native recovery); the final-read regression separately failed before implementation. **64 generated-data tests plus 11 controller regressions passed**. Offline tests use temporary/generated data and mocks, never real harness homes or CLIs. No `src-tauri/` diff, so `just test-rust-unit` is not required. The only behavior added beyond the step5 predicate correction is the ruling's final step6 read/mark, previously absent from that unexecuted branch. The offline post-runtime transport audit initially mistook native-mailbox attempt claims for terminal transport; its query was corrected without changing runtime evidence or outcome. Full deviations are in `final-disposition.json`.

Exact checkout-root gates **after teardown**: `just check-quick` **exit 0**, `just lint` **exit 0**, `just test-contracts` **exit 0**. Gates ran in the inherited credential-free namespace with runtime CLIs blocked; Cargo admission checked machine-wide processes and waits in 30-second polls only at three or more. Every gate child was waited and the gate root removed. Logs, commands, exits, and isolation are retained in `gates/`.

Independent **claude-opus-4-6** evidence review **approved**, exit **0**, one turn, **$0.18747000**; no required fixes. Known seats plus separate review **$0.25264434**, with unknown work still explicitly excluded. Review child exited and scratch root was removed; credential placeholder remained zero bytes. Review command, prompt, result and cleanup are retained. The reviewer’s minor notes confirm the native transport distinction and the disclosed unknown spend under the standing ruling.

Session directory files and complete daemon JSONL remain directly accessible. Repeated snapshot, build, gate and review directory files resolve through the content-addressed `snapshots.json` aliases; no runtime evidence was discarded for size.
