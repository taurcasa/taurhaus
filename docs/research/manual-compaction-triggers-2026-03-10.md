# Manual compaction triggers in Claude Code and Codex CLI

Date: 2026-03-10
Task: #894

## Executive summary

We do have a viable path to make compaction reinjection testable on demand, but the Claude and Codex paths are very different.

- Claude Code has first-class compaction lifecycle support.
  - `PreCompact` is a documented hook.
  - `SessionStart` with matcher/source `compact` is the documented post-compaction reinjection point.
  - There is no documented `PostCompact` hook.
  - `/compact` is a real built-in slash command and can also be sent through the Claude Agent SDK.
- Codex CLI has no documented hook system for compaction.
  - Compaction is still real and externally observable.
  - The installed CLI and live session JSONLs show `compacted` plus `context_compacted` records.
  - External detection via transcript JSONL is the practical path.
- For end-to-end testing, the simplest reliable route is:
  - Claude: operator- or SDK-driven `/compact` in a real managed session, assert `PreCompact` and `SessionStart(source=compact)` effects.
  - Codex: tmux-driven `/compact` in a managed session, assert transcript boundary + Taurhaus signal + inbox delivery.

## Scope and evidence sources

This assessment combines:

- official Claude Code docs
- official Claude Agent SDK docs
- official/local Codex CLI inspection
- local empirical runs against installed `claude` and `codex`
- our current Taurhaus hook bridge implementation in `src-tauri/src/coordination/claude_hooks.rs`

Key local artifacts reviewed:

- `src-tauri/src/coordination/claude_hooks.rs`
- `~/.claude/settings.json`
- `~/.claude/hooks/taurhaus-session-start-compact.sh`
- `~/.claude/cache/changelog.md`
- `~/.codex/sessions/**/*.jsonl`

## Claude Code findings

### 1. Does `/compact` fire `PreCompact` and `PostCompact` hooks?

Answer:

- `/compact` does fire `PreCompact`.
- There is no documented `PostCompact` hook.
- The documented post-compaction lifecycle point is `SessionStart` with matcher/source `compact`.

Evidence:

- Claude hooks docs list `PreCompact` as a hook event and say it runs "before Claude Code is about to run a compact operation".
- The same docs explicitly map matcher `manual` to `/compact` and matcher `auto` to automatic compaction.
- The hooks docs list `SessionStart` matcher `compact` and say `source` is `"compact"` after compaction.
- The hooks docs do not document any `PostCompact` event.

Official references:

- Claude Code hooks reference: https://code.claude.com/docs/en/hooks
- `PreCompact` matcher and input schema: `manual` -> `/compact`, `auto` -> auto-compact
- `SessionStart` matcher/source `compact`

Implication for Taurhaus:

- Our current architecture is aligned with the documented lifecycle.
- The right bridge is not `PostCompact`; it is `SessionStart(source=compact)`.

### 2. If a team lead tells a Claude teammate "run /compact", does it execute and fire hooks?

Answer:

- Not as a supported or reliable model-invoked path.
- The safe assumption is **no** for plain natural-language delegation.
- `/compact` is a CLI slash command, not a normal free-form model action.

Why:

- Claude docs classify `/compact` as a built-in command.
- The slash-command docs explicitly distinguish built-in commands from skills.
- The skills docs also say built-in commands like `/compact` are not available through the Skill tool.
- That means a normal agent/model path does not have a documented tool route for invoking `/compact` just because it read text saying to do so.

Official references:

- Slash commands / skills docs: https://code.claude.com/docs/en/slash-commands
- Built-in commands like `/compact` are separate from skills.
- Built-in commands are not available through the Skill tool.

Practical conclusion:

- If a human/operator types `/compact`, hooks can fire.
- If a team lead merely messages a Claude teammate "please run /compact", that is not a robust test/control path and should not be used as automation.

### 3. Can compaction be triggered programmatically via SDK/API?

Answer:

- Yes, via the Claude Agent SDK slash-command interface.
- This is the cleanest programmatic Claude path currently available.

Evidence:

- The Claude Agent SDK docs have a dedicated "Slash Commands in the SDK" page.
- They explicitly show sending `prompt: "/compact"` through `query(...)`.
- They also document a `system` message subtype `compact_boundary` for completed compaction, with `compact_metadata` including `trigger`.

Official reference:

- Agent SDK slash commands: https://platform.claude.com/docs/en/agent-sdk/slash-commands

Important nuance:

- That proves programmatic compaction exists in the SDK world.
- It does **not** mean every Claude Code terminal session can be remotely forced to compact through our existing team-messaging layer.
- For Taurhaus end-to-end testing, it is still easier to drive a real managed session than to mix CLI/SDK session models unless we explicitly build a test harness around the SDK.

### 4. Is there a CLI flag or environment variable to force lower compaction thresholds?

Answer:

- I found no documented CLI flag or env var that lowers the compaction threshold on demand.
- Official docs and `claude --help` do not expose such a control.

What I checked:

- `claude --help`
- Claude Code hooks docs
- Claude Agent SDK overview/slash-command docs
- local Claude changelog at `~/.claude/cache/changelog.md`

What I found instead:

- changelog references for compaction behavior improvements
- changelog note that automatic compaction exists and has warning thresholds
- no supported threshold override surfaced in docs/help

Local evidence:

- `~/.claude/cache/changelog.md` contains compaction-related entries like:
  - auto-compact warning threshold changes
  - `/compact` fixes
  - `PreCompact` hook introduction
- but not a public operator flag like `--compact-threshold 50%`

Conclusion:

- The supported manual trigger is `/compact`, not threshold tuning.

### 5. What does the hook lifecycle and payload shape look like?

Documented Claude lifecycle relevant to compaction:

1. `PreCompact`
   - matcher: `manual` or `auto`
   - fires before the compact operation
   - payload includes:
     - `session_id`
     - `transcript_path`
     - `cwd`
     - `permission_mode`
     - `hook_event_name = "PreCompact"`
     - `trigger = "manual" | "auto"`
     - `custom_instructions`
2. actual compaction happens
3. `SessionStart`
   - matcher/source `compact`
   - payload includes:
     - `session_id`
     - `transcript_path`
     - `cwd`
     - `permission_mode`
     - `hook_event_name = "SessionStart"`
     - `source = "compact"`
     - `model`
     - optionally `agent_type`
   - hook can return `hookSpecificOutput.additionalContext`

Official reference:

- https://code.claude.com/docs/en/hooks

Our current bridge expectations:

From `src-tauri/src/coordination/claude_hooks.rs`, Taurhaus currently accepts both modern snake_case and legacy camelCase fields:

- `hook_event_name` / `hookEventName`
- `session_id` / `sessionId`
- `transcript_path` / `transcriptPath`
- `cwd`
- `source`
- optional `permission_mode`, `model`, `agent_type`

That matches the documented `SessionStart(source=compact)` contract well.

### Empirical Claude test result

I ran:

```bash
claude --debug hooks --debug-file /tmp/claude-compact-print.log -p '/compact'
```

Observed result:

- Claude parsed `/compact` as a real built-in command path.
- The debug log showed hook matching for `SessionStart` with query `startup` at process start.
- The command then failed with:

```text
Error: No messages to compact
```

Interpretation:

- `/compact` is recognized as a built-in command in headless/print mode.
- But a no-history headless session is not a useful reinjection test because there is nothing to compact.
- Real end-to-end testing still needs an actual session with accumulated history.

## Codex CLI findings

### 1. Does Codex have any compaction/context-management command or mechanism?

Answer:

- Yes, compaction exists in practice.
- But I did not find a documented public command/reference page for it in current official docs/help.

Evidence:

- The installed `codex` CLI does not mention compaction in `codex --help` or `codex debug --help`.
- However, live/local Codex session JSONL files contain explicit compaction records.
- Historical and recent local sessions show:
  - `type: "compacted"`
  - followed by `event_msg.payload.type: "context_compacted"`
- Previous local investigation also recorded terminal output `Context compacted`.

Local examples:

- `~/.codex/sessions/2026/03/07/rollout-2026-03-07T20-15-25-019cc9b9-d6ba-7371-ae5f-dbcc6276cc94.jsonl`
- `~/.codex/sessions/2026/03/08/rollout-2026-03-08T17-35-53-019cce4e-23ec-7051-a0e7-30c756dae113.jsonl`

Conclusion:

- Compaction is real.
- The stable operator-facing API/docs surface for it is weak.
- For Taurhaus, transcript observation remains the reliable external contract.

### 2. How does Codex handle context overflow?

Answer:

- Based on live transcript evidence, Codex compacts rather than silently truncating or hard-restarting the session.

Evidence:

- Session JSONLs retain continuity and append compaction boundary events.
- The same session file remains active before and after compaction.
- Local historical evidence shows:
  - boundary record `type:"compacted"`
  - follow-up `event_msg.payload.type:"context_compacted"`
- That behavior is consistent with in-place session compaction, not session restart.

Caveat:

- I did not find a current official Codex reference page that formally documents overflow semantics.
- This answer is empirical, not docs-backed.

### 3. Is there a hook system in Codex for context-management events?

Answer:

- Not today, at least not in the current public CLI surface.

Evidence:

- `codex --help` and `codex debug --help` expose no hook configuration.
- Current upstream repo discussions/issues still ask for hooks as a feature, which implies they are not yet present.

Primary upstream references:

- https://github.com/openai/codex/discussions/2150
- https://github.com/openai/codex/issues/2109

Conclusion:

- Taurhaus cannot rely on first-party Codex hook callbacks.
- External transcript-based detection remains the right design.

### 4. Can Codex compaction be detected from outside?

Answer:

- Yes, reliably enough for our use case.

Best signal:

- watch the active session JSONL file for appended compaction records:
  - `type:"compacted"`
  - `event_msg.payload.type:"context_compacted"`

Why this is the right boundary:

- it is what real live Codex sessions already emit
- it is persistent and auditable
- it works even though Codex has no hook system

This matches the Taurhaus approach we already use in the Codex compaction pipeline.

## Feasibility assessment

### Simplest workable path to trigger compaction on demand

#### Claude Code

Best path:

- use a real managed Claude session with enough history
- have an operator or a dedicated SDK harness send `/compact`
- verify:
  - `PreCompact`
  - `SessionStart(source=compact)`
  - Taurhaus bridge output / inbox delivery

Preferred trigger options, in order:

1. operator sends `/compact` inside the real session pane
2. SDK harness sends `prompt: "/compact"` to a controlled Claude test session
3. natural-language instruction to the model: **not recommended**

Reason:

- options 1 and 2 are supported control surfaces
- option 3 is not a reliable slash-command execution path

#### Codex CLI

Best path:

- use a real managed Codex session in tmux
- inject `/compact` via tmux send-keys as an operator command
- verify transcript boundary + Taurhaus signal + inbox delivery

Reason:

- Codex lacks a first-party hook path
- transcript observation is our dependable external contract

### Could we build `just test-compaction`?

Answer:

- Yes, but it should be split by tool and should not pretend the two systems are the same.

Recommended shape:

- `just test-compaction-claude`
- `just test-compaction-codex`

Optional umbrella wrapper:

- `just test-compaction` runs both

Why split it:

- Claude uses hook-based reinjection.
- Codex uses transcript-detected reinjection.
- Failure modes and observability differ materially.

### Limitations

1. Claude `/compact` only proves useful in a session that actually has enough history to compact.
2. Claude natural-language delegation is not a reliable way to invoke slash commands.
3. Codex has no first-party compaction hook, so all validation is external/heuristic by design.
4. For both tools, proving "message was delivered" is easier than proving "model consumed it correctly".
5. A headless no-history `/compact` run is a parser check, not a real reinjection test.

## Recommended follow-up tasks

### Task 1: Claude compaction integration harness

Objective:

- build a reproducible Claude test harness that fills a managed Claude test session, triggers `/compact`, and asserts `PreCompact` + `SessionStart(source=compact)` + Taurhaus delivery.

Why:

- this is the cleanest path to end-to-end proof for the Claude hook bridge.

### Task 2: Codex manual compaction test harness

Objective:

- build a tmux-driven Codex test that sends `/compact`, watches the active session JSONL, and asserts Taurhaus signal creation plus inbox delivery.

Why:

- Codex has no hook system; transcript-driven validation is the right end-to-end test.

### Task 3: Analyzer support for explicit manual-trigger runs

Objective:

- add run labeling and manual-trigger checkpoints to `scripts/analyze-compaction.py` so a test run can clearly answer:
  - boundary seen?
  - transport delivered?
  - wake/surfacing observed?

Why:

- otherwise manual compaction tests stay too forensic and slow to interpret.

### Task 4: Operator documentation for supported compaction triggers

Objective:

- document the supported operator-only trigger methods:
  - Claude: real `/compact` or SDK slash-command harness
  - Codex: tmux/operator `/compact` + transcript verification

Why:

- it prevents the team from reusing unreliable paths like "tell the agent to run /compact".

## Final answers by question

### Claude Code

1. Does `/compact` fire `PreCompact` and `PostCompact`?
   - `PreCompact`: yes.
   - `PostCompact`: no documented hook.
   - post-compaction lifecycle uses `SessionStart(source=compact)`.
2. If a team lead says "run /compact"?
   - not a supported or reliable automation path.
3. Programmatic/API trigger?
   - yes, through the Claude Agent SDK slash-command interface.
4. Threshold override flag/env?
   - none found in docs/help.
5. Exact lifecycle?
   - `PreCompact` -> compaction -> `SessionStart(source=compact)`.

### Codex CLI

1. Compaction mechanism?
   - yes, empirically present.
2. Overflow behavior?
   - empirically compacts in-place, not a hard restart.
3. Hook system?
   - no public/current hook system found.
4. External detection?
   - yes, by watching active session JSONL for `compacted` and `context_compacted`.

## Recommendation

For end-to-end reinjection testing, stop trying to find one universal trigger.

Use the tool-native control path:

- Claude: slash-command compaction via real session or SDK harness
- Codex: slash-command compaction via tmux/operator path, verified from transcript JSONL

That is the shortest path to reliable automated testing with the least architectural guesswork.
