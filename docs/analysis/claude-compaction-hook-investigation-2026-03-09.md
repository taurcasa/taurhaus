# Claude Compaction Hook Investigation — 2026-03-09

## Question

Is the Claude `SessionStart(source=compact)` hook working but simply not triggering, or is there a registration/execution problem?

## Conclusion

Both of these are true:

1. The hook bridge implementation itself works when invoked with a valid `SessionStart(source=compact)` payload.
2. The active `team-lead` Claude session does not appear to have compacted recently, so there may genuinely have been no live trigger in the observed windows.
3. There is also a real Windows registration/execution risk: the installed hook command points at a WSL UNC `.cmd` wrapper path, and manual invocation of that exact wrapper path failed.

So the best current diagnosis is:

- **not proven to be firing in real life because there has been no recent trigger**
- **not safe to call “fully healthy” because the registered wrapper path is likely wrong for Windows hook execution**

## Current Claude Code docs

Official Anthropic hook docs currently say:

- `SessionStart` supports matcher `compact`
- hook commands are user-defined **shell commands**
- hook input is JSON on stdin
- `SessionStart` stdout is added as context for Claude

Sources:

- Anthropic hooks reference: https://docs.anthropic.com/en/docs/claude-code/hooks
- Anthropic hooks guide: https://docs.anthropic.com/en/docs/claude-code/hooks-guide

Relevant documented behavior:

- `SessionStart` matchers include:
  - `startup`
  - `resume`
  - `clear`
  - `compact`
- hooks execute shell commands automatically

So the installed matcher value `compact` is correct for the current docs.

## Local installation state

Claude Code version:

- `2.1.71 (Claude Code)`

Installed hook registration in [settings.json](/home/mstie/.claude/settings.json):

- event: `SessionStart`
- matcher: `compact`
- command:
  - `"\\\\wsl.localhost\\Ubuntu\\home\\mstie\\.claude\\hooks\\taurhaus-session-start-compact.cmd"`

Installed wrapper:

- [taurhaus-session-start-compact.cmd](/home/mstie/.claude/hooks/taurhaus-session-start-compact.cmd)

Content:

```bat
@echo off
"C:\Users\mstie\AppData\Local\taurhaus\taurhaus.exe" --claude-compact-hook
```

Installed app binary exists:

- [taurhaus.exe](/mnt/c/Users/mstie/AppData/Local/taurhaus/taurhaus.exe)

## Manual smoke tests

### 1. Direct hook bridge smoke with correct payload shape

Using the installed Windows binary directly with a synthetic Claude payload:

```json
{
  "hookEventName": "SessionStart",
  "sessionId": "47fb0840-8a3e-4877-b512-72d133d44386",
  "source": "compact",
  "cwd": "/home/mstie/projects/taurhaus"
}
```

Command path that worked:

- `cmd.exe /c C:\Users\mstie\AppData\Local\taurhaus\taurhaus.exe --claude-compact-hook`

Result:

- returned a real `hookSpecificOutput.additionalContext`
- logged `compaction.injected` for `team-lead`
- emitted a reinjection card for:
  - team: `taurhaus-team`
  - member: `team-lead`
  - task: `#669`

So the **hook bridge code path itself works**.

### 2. Wrapper path smoke

Manual invocation of the exact registered UNC wrapper path failed:

- `cmd.exe /c \\wsl.localhost\Ubuntu\home\mstie\.claude\hooks\taurhaus-session-start-compact.cmd`

Observed result:

- `The system cannot find the path specified.`

This is important because the hook registration currently points at that UNC wrapper path, not at a Windows-local command target.

## Team-lead session trigger check

Transcript inspected:

- [47fb0840-8a3e-4877-b512-72d133d44386.jsonl](/home/mstie/.claude/projects/-home-mstie-projects-taurhaus/47fb0840-8a3e-4877-b512-72d133d44386.jsonl)

Observed recent compaction count:

- `0`

I found no recent:

- `type: "compacted"`
- `event_msg.payload.type: "context_compacted"`

So for the current active `team-lead` Claude session, there is no evidence that a compact event actually happened recently.

That means the absence of `compaction.claude_hook.*` fire evidence is at least partly explained by lack of a real trigger.

## What failed in the earlier synthetic test

An earlier manual test failed with:

- `invalid Claude SessionStart hook payload: missing field hookEventName`

That was not a product bug. My synthetic test used `hook_event_name` instead of the expected Claude `camelCase` field:

- correct: `hookEventName`
- wrong: `hook_event_name`

## Interpretation

### What is working

- matcher `compact` is correct per current docs
- hook bridge binary entrypoint works
- valid Claude payloads produce the expected `additionalContext`
- the installed `taurhaus.exe` exists and can serve the hook

### What is not proven

- no real live `SessionStart(source=compact)` fire was observed for `team-lead`
- therefore there is still no end-to-end “Claude compact really happened and the registered hook fired” proof

### What looks wrong

- the registered command path is a UNC WSL `.cmd` wrapper
- manual invocation of that exact wrapper path failed on Windows
- because Claude hooks are documented as shell commands, this makes the current registration path suspicious even if the inner bridge executable is good

## Best current diagnosis

The main reason we have never seen real hook-fire evidence is likely:

- **the active team-lead Claude session has not compacted recently**

But there is also a likely latent registration issue:

- **the hook command target should probably be a Windows-local path, not a WSL UNC wrapper path**

## Recommended next steps

1. Re-register the Claude compact hook to a Windows-local command target.
   - Best candidate: point the settings command directly at a Windows-local wrapper or directly at:
     - `C:\Users\mstie\AppData\Local\taurhaus\taurhaus.exe --claude-compact-hook`
2. Keep the matcher as `compact`.
3. Trigger a real Claude `/compact` on the `team-lead` session and then verify:
   - `compaction.claude_hook.received`
   - `compaction.claude_hook.resolved`
   - `compaction.claude_hook.injected` or corresponding delivery evidence
4. If re-registration is changed, re-open Claude or refresh hooks as needed because Claude snapshots hooks for the session at startup.

## Bottom line

- The hook bridge is **working**.
- The current matcher is **correct**.
- The current active Claude session appears **untriggered**.
- The current Windows hook command registration is **likely wrong/risky** because it points at a UNC `.cmd` wrapper that failed manual invocation.
