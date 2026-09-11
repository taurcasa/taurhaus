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
