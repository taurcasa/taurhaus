# Command Center — Feature Requirements

> taurhaus as the control hub for Claude Code sessions across projects.
> The core vision: "I'm in taurhaus reading docs, I see a session is idle, I click to jump there."

## Tier 1: Awareness

**Goal**: At a glance, know which projects have active Claude Code sessions.

### Decisions

| Question | Decision | Notes |
|----------|----------|-------|
| What is "active"? | Binary: session exists or doesn't | A Claude Code CLI process running in a tmux pane, started in a registered project's directory |
| Session states | Active / Idle / Inactive | Active = Claude working. Idle = waiting for user input (actionable signal). Inactive = no session. Idle detection is stretch goal — depends on what Claude Code exposes. |
| UI placement | Sidebar badges on all projects | Every registered project gets a status indicator. Command center view comes in later tiers. |
| Freshness | 500ms polling | Effectively instant. The scan (ps + tmux + stat) costs <10ms per cycle — negligible even at 2Hz. Pure polling, no event-based complexity needed. |
| Scope | Claude Code CLI first | Build the abstraction generalized enough for Codex CLI later. Focus detection and mapping on Claude Code. |
| Multi-project | All projects at once | Sidebar badges across all projects simultaneously. Not just the selected one. |

### Detection Strategy: Process + tmux + JSONL (Researched)

Research conducted on a live Claude Code session. The detection chain is well-defined:

#### The Golden Chain: Process → /proc → tmux

```
claude process (PID)
  → /proc/PID/cwd        → project path (e.g., /home/mstie/projects/taurhaus)
  → /proc/PID/fd/0       → terminal device (e.g., /dev/pts/2)
  → tmux list-panes -a   → pane_tty=/dev/pts/2 → window:pane ID for navigation
```

**Verified working commands:**
```bash
# 1. Find all claude processes + their working directories
ps -eo pid,tty,args | grep "[c]laude"
# PID=4927 TTY=pts/2 CMD=claude --dangerously-skip-permissions

# 2. Get project path from PID
readlink /proc/4927/cwd
# /home/mstie/projects/taurhaus

# 3. Get terminal device
readlink /proc/4927/fd/0
# /dev/pts/2

# 4. Map to tmux pane
tmux list-panes -a -F '#{pane_id} #{pane_tty} #{window_index}:#{window_name} #{session_name}'
# %0 /dev/pts/2 0:claude 0

# 5. Cross-reference: PID → project → tmux window:pane (all in one)
```

This gives us **everything**: which projects have sessions, where they are in tmux, and enough info to navigate to them.

#### JSONL Transcript — Session ID + Idle Detection

Each active session writes to `~/.claude/projects/<slug>/<session-id>.jsonl`:
- **Grows continuously** while Claude is working (~1.5KB/3s observed)
- **Stops growing** when idle (waiting for user input)
- Each entry has: `sessionId`, `cwd`, `type` (user/assistant), `timestamp`, `version`
- File mtime is the simplest idle detector: `mtime > 5s ago` + process running = idle

**Key paths:**
```
~/.claude/projects/-home-mstie-projects-taurhaus/
  98931161-37bb-48cb-98e9-23107de06cf9.jsonl    ← active session transcript
  98931161-37bb-48cb-98e9-23107de06cf9/          ← session directory (subagents, tool-results)
  memory/                                         ← persistent memory files
```

#### Other Filesystem Signals Discovered

| Location | What | Useful? |
|----------|------|---------|
| `~/.claude/history.jsonl` | Global prompt history, all projects | Has `project` field per entry. Could detect recent activity per project. |
| `~/.claude/__store.db` | SQLite with `base_messages`, `assistant_messages`, `conversation_summaries` | Session-level data but appears outdated/legacy. JSONL is more current. |
| `~/.claude/debug/latest` | Symlink to current debug log | Points to session-specific debug file. Changes on session start. |
| `~/.claude/todos/` | Task files (`<session-id>-agent-<agent-id>.json`) | Claude Code's internal task tracking. Could power Tier 4 Kanban. |
| `~/.claude/plans/` | Plan mode files (named slugs like `swift-napping-rabin.md`) | Agent plan documents. |
| `~/.claude/session-env/<session-id>/` | Session environment snapshots | Mostly empty for current sessions. |
| `~/.claude/file-history/<session-id>/` | File backup history per session | Tracks which files were modified. Could power Tier 2 file tracking. |

#### Idle Detection Strategy

Two approaches, both viable:

1. **JSONL mtime** (preferred): If `mtime > N seconds ago` AND process is still running → idle. Simple, no parsing needed. Tested: file grows at ~500B/s when active, stops completely when idle.

2. **tmux pane content capture**: `tmux capture-pane -p -t %0 | tail -1` — check if the prompt is showing. More direct but fragile (depends on prompt format).

### Session Info Display

- **Minimum**: Active/inactive dot on sidebar project items
- **Achievable stretch**: Idle detection via JSONL mtime — "session is idle, you can interact"
  - Active (working): JSONL mtime < 5s, green dot
  - Idle (waiting for input): JSONL mtime > 10s + process alive, amber dot
  - Inactive: no process, grey/no dot

### Remaining Questions

- **WSL2 bridge**: extend existing daemon with a `list_sessions` RPC method, or use `wsl.exe` commands from Windows? Daemon is cleaner (one TCP call vs. multiple `wsl.exe` invocations).
- **Polling at 500ms**: The full scan (ps + readlink + tmux list-panes + stat) costs <10ms — under 1% CPU even at 2Hz. No per-project overhead; one scan covers all projects. Event-based adds complexity without meaningful benefit.
- **Project path matching**: `/proc/PID/cwd` gives Linux paths. Need to match against registered project paths which may be stored as WSL UNC paths (`\\wsl.localhost\...`). The path conversion module already handles this.

---

## Tier 2: Inspection

**Goal**: Deeper read-only info about active sessions — what's happening without switching to the terminal.

**Priority**: Lower than Tier 1 and Tier 3. Awareness + Control matter more.

### Decisions

| Question | Decision | Notes |
|----------|----------|-------|
| Inspection depth | Recent activity summary | High-level ("edited 3 files", "ran tests") not raw terminal output. Deferred — not blocking Tier 1/3 work. |
| Live terminal preview | Not prioritized | Raw tmux pane content inside taurhaus is complex and not the main ask. |
| Session transcript | Not prioritized | JSONL formatted view. Interesting but not a driver. |
| File activity tracking | Yes, live | When a session is active, surface which files Claude is modifying. Connect to existing file viewer. High synergy with what taurhaus already does. |
| Cost/token tracking | Nice-to-have | Would look at occasionally. Not a driver. Could pull from Claude Code's own tracking if available. |

### Key Feature: Live File Tracking

The standout Tier 2 feature. When Claude is actively editing files:
- Highlight modified files in the file tree
- Maybe show a "recently touched" list
- The file watcher infrastructure already exists — this may come almost for free once sessions are detected

---

## Tier 3: Control

**Goal**: Start, navigate to, and manage Claude Code sessions from taurhaus.

### Decisions

| Question | Decision | Notes |
|----------|----------|-------|
| Launch flow | New tmux window + claude | Creates a new window in the existing tmux session, cd to project dir, runs claude. Plus bootstrap: start terminal + tmux if not running. |
| Default command | `claude --dangerously-skip-permissions --continue` | Continue from last checkpoint is the default workflow. `--dangerously-skip-permissions` is almost always on. |
| Navigation | Full precision | Focus Windows Terminal → switch tmux window → select pane. Maximum precision to land exactly where the session is. |
| Send input | No | taurhaus navigates, user types. Clean separation of concerns. |
| Lifecycle | Full | Start, stop, resume, restart from taurhaus. |
| Terminal app | Windows Terminal (`wt.exe`) | Standard, has good CLI support. Add others later if needed. |

### Launch Modes

| Mode | Command | Trigger |
|------|---------|---------|
| **Continue** (default) | `claude --dangerously-skip-permissions --continue` | Primary "Start Session" button |
| **Fresh start** | `claude --dangerously-skip-permissions` | Context menu / secondary action |
| **Resume (pick checkpoint)** | `claude --dangerously-skip-permissions --resume` | Context menu — user selects checkpoint in the terminal |

### Bootstrap Sequence

When launching a session, taurhaus must handle the case where nothing is running yet:

1. **Is Windows Terminal running?** → If not, start it via `wt.exe`
2. **Is tmux running in WSL2?** → If not, start a tmux session
3. **Create new tmux window** → Named after the project
4. **cd to project directory + run claude**

Complexity assessment for bootstrapping: TBD — depends on how `wt.exe` handles launching into an existing tmux session vs. starting fresh.

### Navigation Flow

"Jump to session" from taurhaus:
1. Identify which tmux window:pane the session is in (from detection layer)
2. Send tmux command to select that window + pane (`tmux select-window -t X` + `tmux select-pane -t Y`)
3. Focus Windows Terminal window (Win32 API or `wt.exe` focus command)

### Lifecycle Management

| Action | How |
|--------|-----|
| **Start** | Bootstrap sequence above |
| **Stop** | Send `/exit` or interrupt signal to the tmux pane |
| **Resume** | Launch with `--continue` or `--resume` flag |
| **Restart** | Stop + Start sequence |

### Open Questions

- Can `wt.exe` attach to an existing tmux session reliably, or does it always start a new shell?
- How to detect if tmux is already running from Windows (via `wsl.exe tmux ls`)?
- Naming convention for tmux windows — use project name? Project ID?
- How does `--continue` behave if there's no previous session? (Graceful fallback to fresh?)

---

## Tier 4: Orchestration (Vision — Deferred)

**Goal**: Multi-project coordination and deeper Claude Code integration.

**Approach**: Build Tiers 1-3 first, use them in real workflows, let Tier 4 requirements emerge from actual friction points.

### Ideas to Revisit

- **Kanban from Claude Code task files**: Claude Code writes task state to disk (`~/.claude/`). Surface these as a per-project Kanban board — see open TODOs, in-progress work, completed items without opening a terminal.
- **Concurrent session management**: Run Claude Code on multiple projects simultaneously. taurhaus as the dashboard showing all active sessions, their status, and allowing quick switching.
- **Batch operations**: "Run security audit on all projects", "update dependencies across repos". Queue up tasks and dispatch to Claude Code sessions.
- **Cross-project coordination**: If related projects are both active, surface that relationship. Maybe even enable team/swarm workflows across projects.

### Why Defer

The user prefers to build and use the basics, then discover what's missing through real workflow experience. Premature Tier 4 design risks building features that don't match actual needs.

---

## Context

### User's tmux Setup
- Single tmux session, multiple windows (2-4 typically)
- 2 panes per window
- Cycles windows with `Ctrl+B n`
- tmux runs in WSL2, taurhaus runs on Windows

### Current Workflow
- `cd` into project directory, run `claude` or `claude --resume`
- Reads docs in taurhaus, codes in tmux — wants to bridge the gap
- `aitx` (tmux CLI wrapper) exists on PATH but rarely used manually

### User's Terminal Setup
- Windows Terminal showing WSL2/tmux
- taurhaus as a separate Windows application
- "Focus the terminal app" = bring Windows Terminal to foreground AND switch to the right tmux window
