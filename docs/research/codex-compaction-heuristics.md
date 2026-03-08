# Codex Compaction Heuristics

Date: March 8, 2026

## Executive Summary

Yes, a viable Codex compaction heuristic exists.

The strongest signal is not tmux silence or `/proc` I/O. It is the Codex session JSONL file itself:

1. Each live Codex session writes to a dated file under `~/.codex/sessions/YYYY/MM/DD/*.jsonl`.
2. When compaction happens, Codex appends a `type:"compacted"` record followed immediately by an `event_msg` with `payload.type:"context_compacted"`.
3. In the terminal, Codex also prints a visible `Context compacted` line.

That gives Taurhaus a practical detection strategy:

- Primary detector: watch the active session JSONL for appended `compacted` or `context_compacted`.
- Secondary detector: watch the pane output for `Context compacted`.
- Do not rely on `/proc/<pid>/io` or generic silence windows except as weak corroboration.

This should be reliable enough for push-based role-card re-injection with low false positives.

## Scope and Method

This investigation used:

- real live Codex agent panes in tmux
- direct inspection of current `~/.codex/sessions/2026/03/08/*.jsonl`
- a controlled manual `/compact` run in a live Codex pane
- `/proc/<pid>/io` sampling during compaction
- historical scan of March 2026 Codex session files

I also cross-checked upstream Codex repo discussions for whether a first-party hook exists. I found no stable public hook surface for compaction.

## Current Codex Runtime Shape

On this machine, current Codex agents are running as:

- shell foreground command in tmux appears as `node`
- actual command line is `node .../bin/codex ...`
- vendor binary is a child process under the Node launcher

Example live commands:

- `codex --yolo -m gpt-5.4`
- `codex resume --last --yolo`

Current session files are not flat legacy files anymore. They are stored as:

- `~/.codex/sessions/YYYY/MM/DD/rollout-<timestamp>-<id>.jsonl`

That dated JSONL layout is important because it gives Taurhaus a concrete filesystem watch target.

## 1. Session File Behavior During Compaction

## Controlled observation

I created a fresh Codex session and captured its live session file:

- file: `~/.codex/sessions/2026/03/08/rollout-2026-03-08T14-45-23-019ccdb2-09d5-7ff0-b5b6-72b7178c7dbf.jsonl`
- writer PID: `656105`

Before compaction:

- file size: `72,280` bytes
- latest records were normal turn records:
  - `session_meta`
  - user message
  - assistant response
  - `task_complete`

I then triggered manual compaction with `/compact`.

After compaction:

- file size became `75,675` bytes
- size delta: `+3,395` bytes overall from pre-compact state
- the key compaction append burst was `+3,186` bytes in one sample step
- the file was appended to, not truncated or rewritten
- the same file path stayed active; no new session file was created

The appended records were:

- `type:"compacted"`
- `type:"event_msg"` with `payload.type:"token_count"`
- `type:"event_msg"` with `payload.type:"context_compacted"`
- `type:"event_msg"` with `payload.type:"task_complete"`

The most important record is:

- `type:"compacted"` with `payload.replacement_history`

That `replacement_history` contains:

- the pre-compaction user/assistant turns
- a `type:"compaction"` entry containing encrypted compacted content

## Historical scan

This was not a one-off artifact from the manual test.

Historical March scan results:

- `232` `compacted` records
- across `41` distinct session files

Pattern consistency:

- `payload.message` was empty in sampled records
- `replacement_history` length varied by how much history got compacted
- `context_compacted` appeared as the paired event signal

Conclusion:

- Codex compaction is externally visible in the active session JSONL.
- The session file is appended, not replaced.
- Monitoring content, not just mtime, is viable.

## 2. tmux Output Signals

## Controlled observation

During manual `/compact`, the pane showed:

- `• Context compacted`

That string is a useful secondary signal because it is:

- explicit
- short
- low ambiguity

I did not observe a richer progress line such as:

- `Compacting conversation...`
- `Summarizing history...`

Instead, the visible UX is minimal:

- slash command entered
- several seconds of silence
- final `Context compacted`

## Timing

In the controlled session:

- `task_started` for the compact turn: `2026-03-08T13:46:34.502Z`
- `compacted`: `2026-03-08T13:46:41.037Z`
- `context_compacted`: `2026-03-08T13:46:41.038Z`

Observed compact duration:

- about `6.5` seconds

That silence is real, but it is not a safe detector by itself. Normal model latency can also produce multi-second quiet periods.

## Practical value of tmux observation

tmux output is useful as:

- a fallback when the session file path is unknown
- a corroborating signal to suppress false positives

tmux output is not strong enough as the primary detector.

## 3. Process and `/proc` I/O Signals

I sampled `/proc/656105/io` while manual compaction ran.

Observed burst at the compaction step:

- `d_write_bytes: 114,688`
- `d_wchar: 91,065`
- `d_rchar: 81,920`

Earlier normal turn activity also caused smaller I/O jumps, for example:

- a normal prompt/response step produced a file append of `+209` bytes with:
  - `d_read_bytes: 126,976`
  - `d_write_bytes: 28,672`
  - `d_rchar: 3,154,770`
  - `d_wchar: 28,228`

Conclusion:

- compaction does create a noticeable I/O burst
- but I/O is too noisy and too implementation-dependent to use as the primary heuristic

Weaknesses:

- no semantic meaning
- likely to vary by model/runtime version
- easy to confuse with other large-turn or logging activity

Recommendation:

- use `/proc/<pid>/io` only as a tertiary debugging aid
- do not build product logic on it

## 4. Resume Behavior

After compaction, Codex printed:

- `To continue this session, run codex resume 019ccdb2-09d5-7ff0-b5b6-72b7178c7dbf`

That matters because:

- the session ID remains stable
- the compacted state is persisted in the same session file
- `resume` is tied back to the same canonical JSONL

The session file after compaction still contains:

- the `compacted` record
- the `context_compacted` event
- the replacement history for the compacted turn

So Taurhaus can detect:

- "this session has compacted before"

by scanning backward in the same JSONL for:

- `type:"compacted"`

What I could not prove from first-party docs:

- whether resume itself emits a dedicated post-compaction marker beyond the persisted `compacted` history

But for Taurhaus’s use case, that extra signal is not required. The existing session file already preserves enough evidence.

## 5. Periodic Injection Without Detection

This is viable as a fallback, but not as the main strategy.

## Where periodic injection helps

- task assignment time
- explicit resume time
- maybe long idle-to-active transitions

These are low-noise moments and align with user expectations.

## Where periodic injection fails

- it misses the actual compaction moment
- it can inject too late
- if done on a timer, it becomes noisy
- it can push stale role cards when nothing meaningful changed

Recommendation:

- do not replace compaction detection with periodic timers
- use periodic/task-boundary injection only as a fallback for sessions where we cannot map the active JSONL cleanly

## 6. Most Viable Heuristic

## Recommended heuristic stack

### Primary

Watch the active session JSONL for appended records:

- `type:"compacted"`
- or `type:"event_msg"` with `payload.type:"context_compacted"`

This is the best detector because it is:

- explicit
- low false-positive
- stable across many real sessions
- tied to the actual canonical session state

### Secondary

Watch the corresponding tmux pane for:

- `Context compacted`

Use this when:

- the session file mapping is not yet known
- or as confirmation before pushing a role card

### Tertiary

If needed for diagnostics only:

- look for a short silence window followed by a write burst in `/proc/<pid>/io`

Do not use this as a shipping detector.

## Proposed Taurhaus implementation

1. Map each live Codex pane to its active session JSONL.
2. Add a tail watcher on that file.
3. On appended `compacted` or `context_compacted`, enqueue a debounced role-card push to that specific agent.
4. Optionally require either:
   - the paired tmux line `Context compacted`, or
   - a matching active pane/session association
5. Record the last compact timestamp per session so repeated reads do not retrigger.

## Reliability assessment

Estimated practical reliability:

- session-file heuristic: high
- tmux-only heuristic: medium
- I/O-only heuristic: low

This is good enough for the stated goal:

- low false positives
- likely well above the requested `80%` useful detection threshold

## Edge Cases and Risks

## Session path mapping

The hardest operational problem is not detecting compaction once you know the file. It is mapping:

- pane/process -> active session JSONL

On this machine that mapping is possible via `lsof`, but Taurhaus should not depend on heavyweight polling. It should derive or cache the active session file path when the Codex process starts.

## Version drift

These signals are implementation-level, not a documented public hook contract. They are much stronger than heuristics based on silence, but weaker than Claude’s official hook lifecycle.

The most likely drift points are:

- event names
- JSONL payload shape
- terminal wording

So Taurhaus should:

- prefer JSONL content over terminal text
- treat terminal text as fallback
- keep the detector behind a tool-specific adapter boundary

## Bottom Line

Codex still has no first-party compaction hook, but it does have a workable external detection path.

Best heuristic:

- monitor the active session JSONL for appended `compacted` / `context_compacted`

Fallback:

- watch tmux output for `Context compacted`

Do not use:

- generic silence windows
- raw `/proc` I/O patterns
- timer-only periodic reinjection as the main solution

So the answer for Taurhaus is:

- yes, Codex compaction detection is viable
- the viable detector is session-file event monitoring, not process heuristics

## Sources

- OpenAI Codex repo discussion index: https://github.com/openai/codex/discussions
- OpenAI Codex discussion about missing hooks: https://github.com/openai/codex/discussions/2150
- OpenAI Codex issue discussing session/rollout history internals: https://github.com/openai/codex/issues/4972

## Local Evidence

Direct observations on this machine, March 8, 2026:

- live tmux Codex panes via `aitx`
- `ps` process trees for Codex Node launcher + vendor binary
- `lsof` mapping from live Codex PID to active JSONL session file
- manual `/compact` in a controlled session
- `/proc/<pid>/io` sampling during compaction
- historical scan of March 2026 session files showing `232` compaction events across `41` files
