# Codex 0.153.4 prompt readiness measurement (2026-09-10)

Base: `6398bfa3`; branch: `feat/codex-prompt-readiness`. `samples.json` is
120 rchar deltas, sampled every 500 ms for 60 s after the loaded composer and
model footer appeared and the interruptible MCP startup line disappeared.
Only the verified directory-trust screen received Enter; zero model prompts/turns.
The binary digest is recorded with the samples. Pane exports retain the last
8 lines, replacing only the scratch-root name. Initial and final panes are idle.

Isolation: private tmux socket and TMUX_TMPDIR, no inherited TMUX, empty scratch
HOME and git project, CODEX_HOME initially containing only the authorized
0600 auth.json copy. `cleanup.json` confirms process exit and auth/root removal.
The controller uses monotonic deadlines (500 ms × 120), reading only rchar.

Even after the visible prompt loaded, background startup reads persisted for
11 s (peak 779,043 B/500 ms). From 11 s onward, median reads were 416 B/500 ms;
isolated idle bursts reached 123,392 B, with a 46,848/22,784 B adjacent pair.
Those are idle-pane reads, not evidence of a model turn. All raw counts remain.

L2 run 3's complete log at the evidence checkout records 23 transitions
(12 process_io active, 11 none idle), not 23 pairs, and contains no raw rchar
series. The replay uses this newly measured series, never invented L2 counts.
No model-turn IO was captured in either trial; no empirical busy-rate claim.

Deviations: attempt 1 stopped at the trust screen. Following the user's continue
instruction, the replacement's first sampling window still included visible MCP
startup; `startup-*` retains it separately. The corrected capture above waited
for that line to disappear. All three launches used no model input and cleaned up.
The requested runtime-exclusion document is absent; Mesh's actual Record::idle
reader was inspected (activity_confidence idle; observed_at age 0–120 s).
`red.json` retains the first-scan regression failure (exit 101).

Invocation: native Codex `--yolo --no-alt-screen -m gpt-5.6-luna -c model_reasoning_effort="low" -c projects."<scratch>/project".trust_level="trusted"`; private tmux `-f /dev/null`, 140×45 pane. Trust-screen Enter was the only input.

Final verification: `gates.json` records all four required gates at exit 0, Cargo queue polls, the corrected lint/branch-count failures, and cleanup. The 90 s replay emits no activity transitions; the settled trace passes at 500/1500 ms cadence, while synthetic sustained 64 KiB/s confirms activity. The exported idle snapshot stays fresh. No real model-turn rate was measured; The round-1 review widens the original 32 KiB/s × 3-poll margin to four polls; `harness-model.md` records the outstanding real-turn calibration debt.
