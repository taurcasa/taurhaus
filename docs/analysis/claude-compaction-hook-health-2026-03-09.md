# Claude Compaction Hook Health - 2026-03-09

## Scope

Validate the separate Claude Code compaction path for `SessionStart(source=compact)`:

- hook installation
- wrapper script correctness
- possible lock-file interference
- analyzer output
- app-log evidence
- recent Claude session evidence

## Files Inspected

- `~/.claude/hooks/taurhaus-session-start-compact.cmd`
- `~/.claude/settings.json`
- `~/.claude/teams/taurhaus-team/.lock`
- `~/.claude/teams/taurhaus-team/daemons/team.pid.lock`
- `~/.local/share/com.taurhaus.dev/taurhaus.log.jsonl`
- `/mnt/c/Users/mstie/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl`
- `~/.claude/projects/-home-mstie-projects-taurhaus/47fb0840-8a3e-4877-b512-72d133d44386.jsonl`
- `scripts/analyze-compaction.py`
- `src-tauri/src/coordination/claude_hooks.rs`
- `src-tauri/src/lib.rs`

## Findings

### 1. Hook installation is present and structurally correct

Installed hook script:

- `~/.claude/hooks/taurhaus-session-start-compact.cmd`

Content:

```bat
@echo off
"C:\Users\mstie\AppData\Local\taurhaus\taurhaus.exe" --claude-compact-hook
```

Hook registration in `~/.claude/settings.json` is present under:

- `hooks.SessionStart`
- matcher: `"compact"`
- command: `"\\\\wsl.localhost\\Ubuntu\\home\\mstie\\.claude\\hooks\\taurhaus-session-start-compact.cmd"`

This matches the intended bridge design in code:

- `claude_hooks.rs` writes a wrapper script that calls `taurhaus.exe --claude-compact-hook`
- `lib.rs` handles `--claude-compact-hook` by reading hook JSON from stdin and emitting a Claude hook response on stdout

Conclusion: installation and registration are in place.

### 2. The wrapper points at the right mechanism

The installed `.cmd` launches the Windows Taurhaus binary directly, which is what the bridge expects.

Relevant code path:

- `render_hook_script()` in `src-tauri/src/coordination/claude_hooks.rs`
- CLI entry `--claude-compact-hook` in `src-tauri/src/lib.rs`

The bridge behavior is:

1. Claude invokes the registered `SessionStart` compact hook.
2. The wrapper launches `taurhaus.exe --claude-compact-hook`.
3. Taurhaus reads hook payload JSON from stdin.
4. Taurhaus resolves the managed Claude member/session.
5. Taurhaus returns `hookSpecificOutput.additionalContext` JSON to Claude.

So the hook script is not miswired to the wrong binary or a stale code path.

### 3. No obvious stale lock file is blocking the hook path

Observed locks:

- `~/.claude/.update.lock`
  - old
  - contains `6069`
  - unrelated to compaction hook execution
- `~/.claude/teams/taurhaus-team/.lock`
  - empty
  - current mtime, consistent with normal team config locking
- `~/.claude/teams/taurhaus-team/daemons/team.pid.lock`
  - contains `3007504`
  - normal team-daemon lock, not a Claude hook lock

I did not find any dedicated compaction-hook lock or stale compaction-state lock that plausibly explains hook non-execution.

Conclusion: lock files do not currently look like the blocker.

### 4. Analyzer shows install health, but no fire evidence

`python3 scripts/analyze-compaction.py --team taurhaus-team --last 24h`

Claude section result:

- hook installed: yes
- compact matcher present: yes
- hook script exists: yes
- hook fire evidence: none in selected window

The analyzer marks this as:

- `UNKNOWN`: Claude compact hook installed, but no hook fire evidence in selected window

That is the correct classification from the current evidence.

### 5. App logs contain no hook-fire evidence and no hook-failure evidence

Searched both:

- `~/.local/share/com.taurhaus.dev/taurhaus.log.jsonl`
- `/mnt/c/Users/mstie/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl`

No matches for:

- `claude-compact-hook`
- `Claude compact hook bridge failed`
- `failed to serialize Claude compact hook response`
- `additionalContext`
- `hookSpecificOutput`
- `SessionStart`
- `compact hook`

Implication:

- there is no evidence that the hook bridge fired successfully
- there is also no evidence that it fired and failed

This exposes an observability gap: the current bridge logs failures, but there is no explicit success event to prove a real Claude compact-hook execution occurred.

### 6. No recent real `SessionStart(source=compact)` evidence was found for the active team-lead Claude session

Active managed Claude session file:

- `~/.claude/projects/-home-mstie-projects-taurhaus/47fb0840-8a3e-4877-b512-72d133d44386.jsonl`

Search results:

- many textual mentions of `SessionStart`, `source=compact`, and `additionalContext`
- but those are from discussion/content inside the session, not actual hook payload records
- no exact `\"source\":\"compact\"` record was found in the active team-lead session JSONL
- no direct hook-fire artifact was found for that session

Conclusion: in the inspected window, I found no evidence that a real recent Claude compaction occurred for the active `team-lead` session.

## Bottom Line

### Installed

Yes.

The Claude compact hook is installed, registered, and points at the intended Taurhaus bridge path.

### Firing

Not proven.

There is no runtime evidence in the inspected logs or recent session data that the hook actually fired.

### Delivering

Not proven.

Because there is no confirmed live hook execution in the inspected window, there is also no evidence of a successful delivered `additionalContext` response.

## Most Likely Interpretation

The current problem is not a clear installation error and not an obvious stale-lock issue.

The stronger conclusion is:

- the hook is installed
- the bridge path is wired correctly
- but there is no observed real-world `SessionStart(source=compact)` execution to prove firing
- current observability is too weak to distinguish:
  - "Claude did not compact"
  - "Claude compacted but did not invoke the hook"
  - "Claude invoked the hook, but Taurhaus success is silent"

## What Needs Fixing

1. Add explicit success logging for the Claude hook bridge.
2. Log the incoming compact hook with:
   - `session_id`
   - `cwd`
   - matched team/member
   - whether `additionalContext` was returned or skipped
3. Add a small manual smoke-test path or fixture command that can invoke the hook bridge with a synthetic `SessionStart(source=compact)` payload.

Without that, the system can only say "installed" and "no observed fires", which is not enough operationally.
