# Native eligibility — RESULT

2026-09-09. **No eligibility enabled.** The compact trial now includes a
scratch-only Mesh rebuild and real Codex model/compaction observations. Uptake
and recovery-card composition remain unproved. [Execution audit](execution-audit.md)
retains the initial run plus the corrective controllers, regression checks and gates.
This revision carries the round-2 compact, framing and Claude corrections.

Pair: Taurhaus `cadd533ebd83d75d848e289e32974a2db64875c9` (#154), protocol 26;
Mesh `504b2b6bf8f9017c7a5ce7957cd59b2a0ffafc08`. Host: Linux/WSL2 x86_64.
Only the disposable Mesh source archive/binary enabled a pin; the Mesh worktree
and its supplied release-candidate binary remain unchanged.

| Ordered trial | Outcome / evidence | Turns / spend |
|---|---|---|
| 1. Codex 0.153.4 Unix app-server | **FAIL framing**: NDJSON EOF; WebSocket upgrade 101. Taurhaus `src-tauri/src/coordination/hosted_process.rs:41` and its raw Rpc cannot initialize this build; its Python fake models the refuted framing. Mesh `src/delivery/app_server/rpc.rs` and `capabilities.rs::TRANSPORT` need paired repair. Start/steer stopped before a thread existed. [Evidence](codex-0.153.4-app-server.md) | 0 / $0.00 |
| 2a. Claude UserPromptSubmit | **NOT RUN**: Mesh categorically refuses `harness == "claude"`; a descriptor flip cannot lift it. Pinned 2.1.263 is locally available and verified runnable; original preflight selected 2.1.266. [Evidence](claude-2.1.266-UserPromptSubmit.md) | 0 / $0.00 |
| 2b. Claude Stop continuation | **NOT RUN**: same categorical refusal, plus Taurhaus excludes Stop and v1 permits no continuation. Build procurement is not a blocker. [Evidence](claude-2.1.266-Stop.md) | 0 / $0.00 |
| 3. Codex 0.153.4 SessionStart(compact) | **INCONCLUSIVE, configuration-confounded; no valid uptake test**: the only compacting run had `trustStatus: "untrusted"` and invalid bridge identity. Hook trust was not established under the pinned app-server host; this is the eligibility refusal, not evidence against the boundary. Corrected TUI fixture never compacted; TUI is off-pin. No real offer or intact card join. [Evidence and turn IDs](codex-0.153.4-SessionStart-compact.md) | 3 user turns + 1 compaction; metered API-equivalent $0.00531472; combined conservative estimate < $0.17; billed USD/compaction usage unreported |
| agy / Grok | Not commissioned; not run; disabled | 0 / $0.00 |

**Spend ledger:** Codex 0.153.4: **3/5 user turns**, three visible generations
plus one compaction (four conservatively accounted generation slots); no further
paid turn. Reported ordinary usage: 36,572 input, including 11,776 cached; 100
output, including 77 reasoning. All user turns used `gpt-5.6-luna`, low effort.
Dollar billing and compaction token usage were not emitted. The linked evidence
shows the rate calculation and its assumptions: **$0.166959 conservative estimate
versus $2 authorized**, not a fabricated exact charge. Claude: **0 turns / $0.00**;
Haiku control omitted because the paired software categorically refuses the drains.
Round-2 prose correction: **0 additional turns / $0.00**. Budget is not exhausted;
trust resolution and a valid pinned-host compaction trial remain for the
orchestrator's re-commission decision (review option b).

**Descriptors flipped: none. Mesh commit IDs: none. Mesh gate exit codes: N/A**
(`just check-quick`, `just lint`, `just test` conditional on a passing flip).
Scratch-only `just build-release`: **0**. No Taurhaus registry entry or production
code changed. The compact flag was a removable setup prerequisite, not an
independent product refusal. Both inbox markers remained explicitly readable;
final real read-back showed both read, but `outcomes: []` and no hook offers.

| Round-2 verification rerun | Exit / result |
|---|---|
| Evidence regression checks (embedded in audit) | 0; 3 tests after observed red |
| Confound regression checks (embedded in compact evidence) | 0; 3 tests after red with 11 failed assertions/subtests |
| `just check-quick` | 0; Rust test compilation, typecheck, 150 frontend files / 2,469 tests |
| `just lint` | 0 |
| `just test-contracts` | 0; 68 tests passed |
| `just test-rust-unit` | Not required: no `src-tauri/` diff |

Gates used separate credential-free scratch homes, inert harness/tmux shims and
this checkout's own `target/`. Cargo preflight polled every 30 seconds before
each recipe and waited for competing work. Non-evidence inserted lines: **0/600**.
This revision's logs and exit metadata are under
`/tmp/elig-r2-gates-u4iw7oaa/evidence/gate-{check-quick,lint,test-contracts}.{log,json}`:
all three Cargo preflights exited 1 (no competing Cargo), followed by recipe exits
0 in 18.22s, 8.21s and 5.38s respectively. No Rust files changed.

**Cleanup:** owned Bubblewrap/PID namespaces ended; private Mesh owners, daemon
and tmux servers stopped. Ports **25817/22077**, tmux sockets `private.sock` and
`tui.sock` all refused connections (errno 111); copied auth.json removed. Initial
run cleanup on 30021 remains recorded. No operator process was killed or contacted.
The round-2 correction started no harness, daemon or tmux server. Its final
`/proc` scratch-environment scan found no surviving processes from the corrective
trial or current gates; the new gate root contained no sockets or auth.json.

**Deviations / remaining limits:** the initial packet's removable compact-pin
blocker and avoidable Claude version rationale are corrected. The hook trial used
stdio, then a named-session TUI continuation, without reopening the stopped Unix
trial. Scratch runtime/snapshot fixtures do not prove daemon launch/publication;
TUI never reached compaction. Unreported compaction billing limits exact spend
verification. No transport repair, new activation feature, release, install,
ledger edit or descriptor broadening. The transport-repair lane must cover both
shipped clients and the fake app-server, not just the contract document.
