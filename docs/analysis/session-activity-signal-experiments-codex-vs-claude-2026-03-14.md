# Session Activity Signal Experiments: Codex vs Claude

**Date:** 2026-03-14
**Task:** #1283

## Scope

This experiment compares one live Codex session and one live Claude session
attached to the same project:

- project: `/home/mstie/projects/taurhaus`
- Codex PID: `4078018`
- Claude PID: `4080053`

Both processes resolved to the Taurhaus project cwd, which makes the comparison
useful because project identity is held constant while the observable signals
change by tool.

## Session Samples

### Codex sample

- PID: `4078018`
- cwd: `/home/mstie/projects/taurhaus`
- tty: `/dev/pts/7`
- command:
  - `codex --yolo -m gpt-5.4`
- directly open transcript fd:
  - `/home/mstie/.codex/sessions/2026/03/13/rollout-2026-03-13T00-14-38-019ce454-a61a-7740-9740-ebc847ae4666.jsonl`
- transcript session metadata:
  - session id `019ce454-a61a-7740-9740-ebc847ae4666`
  - cwd `/home/mstie/projects/taurhaus`
  - cli version `0.113.0`

### Claude sample

- PID: `4080053`
- cwd: `/home/mstie/projects/taurhaus`
- tty: `/dev/pts/10`
- command:
  - `claude --dangerously-skip-permissions --team-name taurhaus-team --agent-name design-taurhaus --agent-id design-taurhaus@taurhaus-team --agent-type general-purpose`
- directly open transcript fd:
  - none for a project-scoped transcript
- open history fd:
  - `/home/mstie/.claude/history.jsonl`

## Candidate Observable Signals

### Codex

Candidate signals visible on this host:

- process alive (`/proc/<pid>`)
- project cwd (`/proc/<pid>/cwd`)
- attached tty (`/proc/<pid>/fd/0`)
- direct open transcript file in `/proc/<pid>/fd/*`
- transcript `session_meta` record with project cwd and session id
- transcript mtime
- `/proc/<pid>/io` `rchar` deltas
- established remote `:443` TCP sockets

### Claude

Candidate signals visible on this host:

- process alive (`/proc/<pid>`)
- project cwd (`/proc/<pid>/cwd`)
- attached tty (`/proc/<pid>/fd/0`)
- CLI team/agent flags from argv
- `/proc/<pid>/io` `rchar` deltas
- shared `~/.claude/history.jsonl` file descriptor and mtime
- latest project transcript mtime under `~/.claude/projects/<slug>/`
- latest subagent transcript mtime under `<session>/subagents/`

## Point-in-Time Measurements

### `/proc/<pid>/io` `rchar` deltas over 500 ms intervals

Codex PID `4078018`:

- samples: `38509977725`, `38512222314`, `38512222314`, `38512222314`
- deltas: `2244589`, `0`, `0`

Claude PID `4080053`:

- samples: `2381742189`, `2381742236`, `2381752765`, `2381752812`
- deltas: `47`, `10529`, `47`

Interpretation:

- both tools show useful burst-style IO signals
- Claude's burst/noise separation matches the current heuristic well
- Codex also shows large bursts, but they are more episodic and need a second
  signal when multiple Codex sessions share a project

### Transcript and history mtimes

Codex transcript:

- path:
  - `/home/mstie/.codex/sessions/2026/03/13/rollout-2026-03-13T00-14-38-019ce454-a61a-7740-9740-ebc847ae4666.jsonl`
- age at observation:
  - about `0.09s`

Claude shared history:

- path:
  - `/home/mstie/.claude/history.jsonl`
- age at observation:
  - about `168s`

Claude latest Taurhaus project transcript:

- path:
  - `/home/mstie/.claude/projects/-home-mstie-projects-taurhaus/a395eb67-a7a8-4403-aebb-4443400c21d0.jsonl`
- age at observation:
  - about `43565s`

Claude latest Taurhaus subagent transcript:

- path:
  - `/home/mstie/.claude/projects/-home-mstie-projects-taurhaus/a395eb67-a7a8-4403-aebb-4443400c21d0/subagents/agent-a8d9801efee880a4a.jsonl`
- age at observation:
  - about `46344s`

Interpretation:

- Codex exposes a near-real-time project transcript that is both open by the
  live process and updating continuously
- the sampled Claude process did not expose a currently updating project
  transcript, even though its `/proc` IO showed a clear active burst
- `~/.claude/history.jsonl` is live, but it is global history rather than a
  strong per-project transcript binding

### TCP socket observations

Codex PID `4078018` had two established remote `:443` TCP connections during
the sample window.

Claude PID `4080053` did not surface established remote `:443` TCP
connections in the same check.

Interpretation:

- Codex socket presence is a weak activity signal because it remains present at
  idle due to keep-alive behavior
- Claude does not benefit from the Codex/Gemini socket strategy here

## Signal Quality Assessment

### Codex

Strong signals:

- direct transcript fd mapping
- transcript `session_meta` with cwd + session id
- transcript mtime
- `rchar` bursts

Weak signals:

- socket presence alone

Assessment:

- Codex has the best direct session identity binding in this experiment
- the exact transcript file is discoverable from the live process itself
- transcript mtime is high-quality for single-session projects
- for multi-session projects, per-PID IO remains necessary to decide which
  Codex process is actively doing work right now

### Claude

Strong signals:

- `rchar` bursts from `/proc/<pid>/io`
- argv team/agent flags for identity

Medium signals:

- project cwd
- tty identity

Weak signals:

- shared `history.jsonl`
- latest project transcript mtime
- latest subagent transcript mtime for this sampled process

Assessment:

- Claude currently has the best activity signal at the process layer, not the
  file layer
- project transcript discovery is weaker than Codex for a live process because
  the sampled Claude process did not keep an open project transcript fd and its
  latest project transcript files were stale while process IO still showed
  active bursts
- team/agent argv flags help identify *which* Claude member the process is, but
  they do not by themselves show whether the process is actively working

## Working Conclusion

For the sampled live sessions:

- **Codex wins on session identity binding**
  - exact transcript file, session id, project cwd, and live mtime are all easy
    to recover
- **Claude wins on process-level activity confidence**
  - `/proc/<pid>/io` gave a clean active burst even when project transcript
    files looked stale

The practical implication for Taurhaus is:

- Codex should continue to lean on transcript binding plus per-PID IO for
  "which session is active right now"
- Claude should continue to treat `/proc` IO as the primary active/idle signal,
  with transcript mtimes as supplementary evidence rather than the main source
  of truth
