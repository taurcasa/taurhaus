# Run 4d — FAIL at step 2; route to Taurhaus activity/idle signal

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
