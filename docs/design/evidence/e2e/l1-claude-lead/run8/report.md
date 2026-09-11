# L1 run8 — step 1 UNAVAILABLE (harness); steps 2–6 NOT RUN

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
