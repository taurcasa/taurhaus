# Native eligibility — RESULT

2026-09-09. **No eligibility enabled.** The commissioned app-server framing
probe failed; Claude failed exact-build preflight; Codex composed drain is
blocked by the supplied binary's compiled activation seam. These findings are
not model-uptake passes. [Execution audit](execution-audit.md) retains isolation,
commands, hashes, cleanup and gate logs.

Candidates: Taurhaus `cadd533ebd83d75d848e289e32974a2db64875c9` (#154), protocol
26; Mesh `504b2b6bf8f9017c7a5ce7957cd59b2a0ffafc08` (embedded build metadata
matches, clean). Host: Linux/WSL2 x86_64. Work stayed in `taurhaus-elig`; the
explicitly named Mesh worktree was inspected and left unchanged.

| Ordered trial | Outcome / evidence | Model turns / spend |
|---|---|---|
| 1. Codex 0.153.4 Unix app-server; idle start and active steer | **FAIL framing**: raw NDJSON got EOF; HTTP upgrade got `101 Switching Protocols`. Start/steer/completed-steer trials stopped before a thread existed. [Wire evidence](codex-0.153.4-app-server.md) | 0 / $0.00 |
| 2a. Claude UserPromptSubmit | **NOT RUN**: installed 2.1.266 differs from pin 2.1.263. [Evidence](claude-2.1.266-UserPromptSubmit.md) | 0 / $0.00 |
| 2b. Claude Stop continuation | **NOT RUN**: same mismatch; continuation is also unsupported by the paired bridge. [Evidence](claude-2.1.266-Stop.md) | 0 / $0.00 |
| 3. Codex 0.153.4 SessionStart(compact) composition | **INCONCLUSIVE**: no scratch-enabled compiled pin; bridge filters it and owner independently refuses it. No compaction/model trial. [Evidence](codex-0.153.4-SessionStart-compact.md) | 0 / $0.00 |
| agy / Grok | Not commissioned; not run; disabled | 0 / $0.00 |

**Spend ledger:** Codex 0.153.4 total **0/5 model turns, $0.00/$2.00**, zero
generations/tokens; Claude total **0 turns, $0.00**. The planned Haiku control
was not run after the mandated version-mismatch stop. Model actually used in
trials: none. No harness credentials were accessed. Planned hook markers are
recorded in their boundary files, explicitly labeled never delivered. There
are **no model replies, session/turn IDs, bridge offers, native enqueue receipts,
or explicit-read consumption receipts** to claim as uptake.

**Descriptors flipped: none. Mesh commit IDs: none.** All compiled hook/native
pins remain disabled. No Taurhaus registry entry was added. The software needs
an independently scoped activation path for a real hook trial; Claude also
needs native-consumer exclusion and a Stop envelope/continuation contract.
Fixing Unix transport or adding those mechanisms exceeds this evidence packet.

| Verification | Exit code / result |
|---|---|
| `just build-daemon` | 101 initially (missing ignored resource); 0 after `just ensure-tauri-resources` (0) |
| Scratch daemon authenticated ping | 0; protocol 26 and exact private data root |
| `just check-quick` | 0; 150 frontend files / 2,469 tests passed, typecheck and Rust test compilation passed |
| `just lint` | 0 |
| `just test-contracts` | 0; 68 tests passed (15 renderer, 20 harness, 33 boundary) |
| `just test-rust-unit` | Not required: no `src-tauri/` diff |
| Mesh `just check-quick`, `just lint`, `just test` | Not run; exit codes N/A: their descriptor-flip condition never occurred |

All builds/gates use this checkout's own `target/`, scratch writable product
roots and inert harness shims. Cargo was checked before every recipe with the
requested process pattern; no competing Cargo was observed. No new tests or
production logic were added; runtime transport falsification preceded the
first evidence commit. Non-evidence inserted-line budget: **0/600**.

**Cleanup:** both persistent owned children were terminated and reaped. The
scratch daemon used probed port **30021**, which refused connections afterward;
no private daemon survives. No tmux server was started, and private TMUX_TMPDIR
was empty. Port 17233, operator sessions, installed daemons and real account
homes were untouched.

**Deviations / review:** no paid uptake could be established on these candidates;
all missing acceptance signals are named above. The daemon build needed the
existing resource-bootstrap recipe. No release, install, activation bypass,
transport repair or ledger edits were made. The brief's Opus evidence lens is
left to the orchestrator review route; no Opus reviewer was available in this
implementer lane, and no extra review-model spend was made.
