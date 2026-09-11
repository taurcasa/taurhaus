"""Render corrected run7 interpretation; preserve runtime measurements and older attempts."""
import json
from diagnostics import classify_step5
from pathlib import Path
B=Path(__file__).resolve().parent
A=json.loads((B/'final-audit.json').read_text());S=A['spend'];D=B.parent.parent/'l5-restarts.md'
# Also correct old audit inputs so regeneration cannot republish the retry-count verdict.
A['step_outcomes']=[classify_step5(step,A['observed_obligation_accounting'])
                    if step['step']==5 else step for step in A['step_outcomes']]
failed=next((step for step in A['step_outcomes'] if step['outcome']=='FAIL'),None)
if failed:
 A['verdict']=f"FAIL step {failed['step']} ({failed['classification']}); later steps NOT RUN"
A['deviations']=[
 'Step 5 stopped on a harness observer predicate that counted pending retries as exposures; step 6 was not run.'
 if d.startswith('Step 5 failed the explicit one-attempt-per-id requirement:') or d.startswith('Step 5 failed the explicit one-attempt-per-id requirement.') else d
 for d in A['deviations']]
rows=['## Run7 — seventh-attempt evidence','',
 'Taurhaus `a7e6db7e`, protocol **27**; Mesh `310144d`, unchanged enabled Codex **0.153.4** descriptor. Both native siblings copied to the scratch bin. Alpha used tmux, beta app_server; both `gpt-5.6-luna`, effort `low`. Production initialize used the builder canonical policy and a login-only Claude lead.',
 '',f"**{A['verdict']}**. Runtime **{A['runtime_seconds']:.2f} s**, **{S['paid_inputs']}/20 counted inputs**, **${S['api_equivalent_usd']:.9f}/$0.30 metered estimate**, **{len(S['unmetered_turn_ids'])} unknown-cost inputs**. Unknown costs are counted, never treated as free; full billed spend is unverified. No total-lane PASS or release approval is claimed without the independent evidence review.",
 '', '| Step | Outcome / classification | Evidence |','|---|---|---|']
for s in A['step_outcomes']:
 rows.append(f"| {s['step']} | **{s['outcome']} — {s['classification']}** | [Outcome](l5-restarts/run7/runtime/step{s['step']}-outcome.json). {s.get('reason') or ''} |")
rows+=['','### Identities and accepted targets','',
 'Team incarnation: `9cf87ad7e054dc65dfa8a5245964bfc77d4f0de0348cfcecf64af96752265e2e`. Taurhaus PID/start ticks **62158 / 31291682 → 85236 / 31296604**. Alpha session `01a08e37-4697-7993-926b-4a9cc9b91380`, pane `%2`, generation **1 → 1**. Beta thread `01a08e37-4943-7e72-9ad2-fd1def65ef93`, pane **%3 → %15**, generation **1 → 3** through supported stop/recovery.',
 '', '| Boundary / seat | Message ID | Delivery ID | Attempts / exposures / read |','|---|---|---|---|']
for entry in A['observed_obligation_accounting']:
 target=next(t for t in entry['accepted'][0]['payload']['delivery_targets'] if t['recipient']==entry['seat'])
 rows.append(f"| {entry['label']} / {entry['seat']} | `{entry['message_id']}` | `{target['delivery_id']}` | {entry['attempt_count']} / {entry['transport_count']} / {entry['read_observed']} |")
rows+=['','### What this attempt establishes','',
 'Steps 1–4 passed their runtime predicates. Both baseline IDs were read; both first-boundary markers were accepted and unexposed in the same attributed-working sample, immediately followed by normal daemon SIGINT/restart with identical arguments and new PID/start ticks. Beta required supported `resume_member`; logical identities and team incarnation stayed stable, generations did not regress, and both markers were delivered/read once without baseline replay.',
 '', 'Step 5 **FAIL — harness, observer attempt-count predicate**: alpha has one durable attempt and one submitted notification; beta has **17 distinct attempts**, with **16 pending `pre_input_failure: IO error: delivery: thread_active` receipts** followed by **one native_enqueued** receipt and its card in host events. These are Mesh-owned retries, not controller resends. The raw observer timeout says receipt/witness missing, but both witnesses exist: the failed conjunct is exactly-one-attempt accounting. No duplicate exposure or lost obligation is established. Beta also had 19 attempts / one exposure at the first boundary, accepted by step 4. The corrected observer counts one accepted target and one exposing receipt, paired only with the exposing attempt ID; pending retries are deferral evidence. The observed retry cadence (16 refusals over about 43 seconds) is a separate observation, not the lane verdict.',
 '', 'Step 6 **NOT RUN** under stop-on-failure. Read-only post-teardown accounting finds one transport exposure for each of the six marker IDs; alpha’s final marker has a read receipt, beta’s final marker does not. This offline accounting does not substitute for the required explicit final reads. The cursor-following step-6 implementation is offline-tested, not runtime-certified here.',
 '', 'Working-window measurements: alpha **46.254 s** at the Taurhaus boundary and **44.376 s** at the Mesh boundary; beta **46.251 s** at the Mesh boundary. Beta’s original Taurhaus-boundary turn has no completed >=30-second window: the daemon restart interrupted it before python3 began, and the paced task ran after supported resume. That subclaim is **UNPROVED — harness timing**.',
 '', f"Owner epoch **2 → 3**, PID **{A['owner_census']['old_pid']} → {A['owner_census']['new_pid']}**; the old owner was gone before first delivery. Across **{A['owner_census']['samples']}** restart-window samples, maximum observed owners was **1**. However, maximum gap **{A['owner_census']['max_gap_seconds']:.3f} s** violates the ≤1-second cadence. Required owner exclusion is **UNPROVED — harness cadence**, despite no observed overlap.",
 '', '### Runtime evidence','',
 '[Controller with offline review corrections](l5-restarts/run7/controller.py), [ordered assertions](l5-restarts/run7/steps.py), [commands/RPCs and exits](l5-restarts/run7/runtime/events.jsonl), [complete daemon JSONL](l5-restarts/run7/runtime/taurhaus.log.jsonl), [host events](l5-restarts/run7/runtime/host-events.jsonl), [owner census](l5-restarts/run7/runtime/owner-observations.jsonl), [lossless snapshots](l5-restarts/run7/runtime/snapshots.json), [review-corrected audit](l5-restarts/run7/final-audit.json). Only the interpreted step-5 classification is corrected; raw journal, daemon JSONL and snapshot observations remain unchanged. Commit `775085af` retains the original verdict and executed controller sources; the linked sources include offline fixes. Snapshot filenames map to SHA-256-keyed exact payloads; `pack.unpack` restores them. Pane captures contain at most 60 lines. Acceptance, transport delivery, explicit read and model action remain separate.',
 '',f"Daemon JSONL: **{A['daemon_jsonl']['rows']} complete rows**, SHA-256 `{A['daemon_jsonl']['sha256']}`. Controller exit **{A['controller_exit']}**. Controller transient busy refusal episodes: **{len(A['transient_refusals'])}**; the controller retries only named `host member busy` / `lock busy` refusals within a 65-second deadline.",
 '', 'The step-5 implementation checks transport receipts and native witnesses plus owner exclusion; the seat’s own read is observational. The step-6 implementation (not run this attempt) executes `mesh read --name <seat> --unread --mark-read --json` with explicit root/team and unchanged filters, following `--since <cursor>` until `done: true`, then pages the journal and reconciles both boundaries. Scratch AGENTS.md also requires seats to follow empty or nonempty `done: false` pages.',
 '', '### Every measured spend','',
 'Rates: inherited packet API-equivalent estimates, $0.20/$0.02/$1.20 per million uncached input/cached input/output tokens. Reasoning is included in output. These are not invoice amounts. All generations are listed; a turn without metering is explicitly unknown.',
 '', '| Thread / turn | Generation | Input / cached / output (reasoning) | USD estimate |','|---|---:|---|---:|']
for t in A['turns']:
 if not t['generations']:rows.append(f"| `{t['thread_id']}` / `{t['turn_id']}` | — | Unreported | **Unknown** |")
 for i,g in enumerate(t['generations'],1):rows.append(f"| `{g['thread_id']}` / `{g['turn_id']}` | {i} | {g['input']} / {g['cached_input']} / {g['output']} ({g['reasoning_output']}) | {g['api_equivalent_usd']:.9f} |")
rows+=['','[Cost ledger](l5-restarts/run7/runtime/cost-ledger.json) and [native usage rows](l5-restarts/run7/runtime/usage-events.json). Build, read-only observers, export and gates add zero trial inputs. Implementer/reviewer spend is separately owned by the invoking orchestrator.',
 '', '### Red → green, gates and teardown','',
 'Four new offline acceptance checks: initial run exited **1** (two assertion failures and one missing-helper error); cursor-following behavior passed already. After the step-5 transport correction and scratch pagination instructions, all **37** offline checks passed, exit **0**. Regression comments name `e6fa7d0e`. [Red](l5-restarts/run7/red.txt), [green](l5-restarts/run7/green.txt).',
 '', '| Exact command, checkout root | Exit |','|---|---:|']
for name,g in A['gates'].items():rows.append(f"| `just {name}` | **{g['exit']}** |")
rows+=['','All gates ran **after teardown**, with credential-free scratch roots and real harness executables blocked. Cargo used one build job and this checkout’s own target directory; preflight waited in 30-second polls only when at least three Cargo processes already existed. No `src-tauri/` diff, so `just test-rust-unit` was not required.',
 '',f"[Cleanup](l5-restarts/run7/runtime/cleanup.json): survivors `{A['cleanup']['survivors']}`, private port closed `{A['cleanup']['port_closed']}`, auth copy explicitly removed `{A['cleanup']['auth_copy_removed_before_root']}`, root removed `{A['cleanup']['root_removed']}`. [Gate cleanup](l5-restarts/run7/gates/gate-cleanup.json) records reaped commands and removed scratch roots. No foreign process was signaled.",
 '', '### Offline review correction (no new paid attempt)','',
 'Verified both major findings against the retained journal and native witnesses: the corrected transport predicate proves alpha (1 attempt / 1 exposure) and beta (17 attempts / 1 exposure). Step 5 remains FAIL — harness, because the original observer stopped there; step 6 remains NOT RUN. No runtime PASS is inferred. Historical run1 scope/outcome headings are now explicit.',
 '', 'Test-first evidence: `python3 -m unittest discover -s docs/design/evidence/e2e/l5-restarts/run7 -p \'*test.py\'` initially ran 42 tests and exited 1 with six assertion failures (transport retry acceptance, step-6 retry accounting, diagnostic classification, census deadline, and direct test entrypoint). The report regression then failed separately (43 tests, exit 1), reproducing the product headline. After correction all 43 tests passed, exit 0; `python3 docs/design/evidence/e2e/l5-restarts/run7/support_test.py` ran all 27 checks, exit 0. Six new tests retain Regression comments naming `540f23ea` or `775085af`.',
 '', 'Re-run after teardown: `just check-quick` exit 0 (19.11 s), `just lint` exit 0 (6.58 s), `just test-contracts` exit 0 (8.03 s). The existing isolated gate wrapper used a credential-free home, blocked real harness executables, one Cargo job and this checkout’s own target. Each Cargo preflight found two existing processes, so no wait was required. Gate cleanup confirmed children reaped and scratch roots removed. No Rust diff; Rust unit execution was not required.',
 '', 'This review adds **0 Codex inputs / $0 seat spend**; all original measured spends and unknown costs above are retained. No paid rerun was performed in this local correction round. Deadline-based sampling reduces scan-plus-sleep drift but cannot erase or excuse the retained 1.713-second gap. The optional run3 credential-literal redaction is deferred: those historical files are untouched, and their old allowlist tests intentionally encode those literals; no token material is present.',
 '', '### Deviations and limits','']
rows+=['- '+d for d in A['deviations']]
rows+=['- No product, Mesh descriptor, installation, release, plan-ledger, or other Taurhaus checkout change. Only the specified Mesh worktree was built; no Mesh commit.','']
old=D.read_text()
section='## Run7 — seventh-attempt evidence'
if section in old:
 history=old.split(section,1)[0].split('\n',1)[1]
else:
 history='\n'+f"Run7: {S['paid_inputs']} inputs; ${S['api_equivalent_usd']:.9f} metered; {len(S['unmetered_turn_ids'])} unknown-cost inputs; {A['runtime_seconds']:.2f} seconds. All owned runtime processes and scratch authentication removed. See the [run7 packet](#run7--seventh-attempt-evidence).\n\n## Historical run6 continuation verdict\n"+old.split('\n',1)[1]
D.write_text('# Lane 5 run7 — '+A['verdict']+'\n'+history.rstrip()+'\n\n'+'\n'.join(rows))
