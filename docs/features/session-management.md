# Session management

Session management detects live AI CLI tool sessions, maps them to projects, and surfaces real-time and historical session context in the UI.

## Overview

taurhaus session management has three layers:
- Runtime detection of active/idle CLI sessions (Claude, Codex, Gemini)
- UI surfacing in sidebar indicators, hover cards, and overview/session history views
- Handoff and activity persistence for historical context

The runtime scanner is process-based and tool-aware, with explicit hysteresis to avoid flickering state changes.

## Runtime delivery model

Session state delivery is event-driven above the daemon:
- Daemon scanner polls and classifies sessions every 500ms.
- Daemon serves versioned snapshots via `wait_session_updates` long-poll.
- App backend bridge (`start_session_updates_bridge`) long-polls daemon updates and emits frontend `sessions-updated` events.
- Frontend session store applies those events and drives sidebar/hover indicators reactively.
- On startup, frontend runs a one-shot `list_claude_sessions` hydrate to avoid waiting for first delta event.

## Supported tools

Supported CLI tools:
- Claude Code (`claude`)
- Codex CLI (`codex`)
- Gemini CLI (`gemini`)

Each tool has:
- its own process signature matcher
- tool-specific session file layout resolver
- tool-specific activity signal strategy

## Session detection pipeline

![Session Detection Pipeline](../images/session-detection.jpg)

Detection sequence:
1. Scan processes (`ps -eo pid,args`) and detect known tool command signatures.
2. Enrich each process with CWD and TTY via platform APIs.
3. Resolve tool-specific idle/activity signals.
4. Merge with process-level activity signals (tool-dependent).
5. Apply bidirectional hysteresis before reporting state.
6. Deduplicate duplicate shim/native process pairs per `(tty, cli_tool)`.

CWD matching and project association:
- Scanner records each process `project_path` from process CWD.
- Frontend session state is keyed by normalized `project_path`.
- Sidebar rows query sessions by each registered project path, effectively associating sessions to registered projects by path match.

Process dedup behavior:
- Sessions are sorted by PID descending.
- For duplicate `(tty, cli_tool)` entries, taurhaus keeps the highest PID (typically the native child process, not launcher shim).

## Mermaid flow

### 1) Scanner pipeline (all tools)

```mermaid
%%{init: {
  'theme': 'base',
  'themeVariables': {
    'fontFamily': 'Geist, system-ui, sans-serif',
    'lineColor': '#2dd4bf',
    'fontSize': '12px',
    'edgeLabelBackground': '#2dd4bf'
  },
  'flowchart': { 'curve': 'basis', 'padding': 20, 'nodeSpacing': 60, 'rankSpacing': 76, 'wrappingWidth': 280 }
}}%%
flowchart TD
    classDef entry fill:#2dd4bf,stroke:#14b8a6,stroke-width:3px,color:#042f2e
    classDef step fill:#334155,stroke:#64748b,stroke-width:2px,color:#e2e8f0
    classDef data fill:#475569,stroke:#94a3b8,stroke-width:2px,color:#f1f5f9
    classDef key fill:#0f766e,stroke:#2dd4bf,stroke-width:2px,color:#ecfeff
    classDef output fill:#042f2e,stroke:#2dd4bf,stroke-width:3px,color:#99f6e4

    A(["scan_sessions()<br/>called"]):::entry --> B["scan_processes()<br/>ps -> pid,args -> cli_tool"]:::step
    B --> C["enrich process<br/>cwd(project_path) + tty"]:::step
    C --> D[(count Codex sessions<br/>per project_path)]:::data
    D --> E["compute raw state<br/>for each process"]:::step
    E --> F["apply per-PID state<br/>hysteresis (2 polls)"]:::key
    F --> G["build ClaudeSession<br/>payload"]:::step
    G --> H["dedupe (tty, cli_tool)<br/>keep highest PID"]:::key
    H --> I[(retain active PIDs in<br/>proc_io + state trackers)]:::data
    I --> J(["return session list<br/>to frontend"]):::output

    linkStyle default stroke:#2dd4bf,stroke-width:2px
    linkStyle 3,6 stroke:#94a3b8,stroke-width:1.6px
    linkStyle 7,8 stroke:#99f6e4,stroke-width:2.4px
```

### 2) Per-tool active/idle decision

```mermaid
%%{init: {
  'theme': 'base',
  'themeVariables': {
    'fontFamily': 'Geist, system-ui, sans-serif',
    'lineColor': '#2dd4bf',
    'fontSize': '12px',
    'edgeLabelBackground': '#2dd4bf'
  },
  'flowchart': { 'curve': 'basis', 'padding': 20, 'nodeSpacing': 60, 'rankSpacing': 76, 'wrappingWidth': 280 }
}}%%
flowchart TD
    classDef input fill:#2dd4bf,stroke:#14b8a6,stroke-width:3px,color:#042f2e
    classDef decision fill:#14b8a6,stroke:#0f766e,stroke-width:2.4px,color:#042f2e
    classDef claude fill:#0f766e,stroke:#2dd4bf,stroke-width:2px,color:#ecfeff
    classDef gemini fill:#1d4ed8,stroke:#60a5fa,stroke-width:2px,color:#dbeafe
    classDef codex fill:#334155,stroke:#94a3b8,stroke-width:2px,color:#e2e8f0
    classDef result fill:#042f2e,stroke:#2dd4bf,stroke-width:3px,color:#99f6e4

    A(["Process<br/>(pid, tool, project_path)"]):::input --> B{"Tool?"}:::decision

    subgraph CLAUDE_LANE["Claude"]
      direction TB
      C["file_active = claude jsonl/<br/>subagent mtime < 5s"]:::claude
      D["proc_active = /proc io<br/>hysteresis"]:::claude
      E["raw_active = file_active<br/>OR proc_active"]:::claude
    end

    subgraph GEMINI_LANE["Gemini"]
      direction TB
      G["file_active = gemini chats<br/>mtime < 5s"]:::gemini
      H["proc_active = has<br/>ESTABLISHED :443 socket"]:::gemini
      I["raw_active = file_active<br/>OR proc_active"]:::gemini
    end

    subgraph CODEX_LANE["Codex"]
      direction TB
      K["file_active = codex session<br/>mtime < 10s (project-scoped)"]:::codex
      L["proc_active = /proc io<br/>hysteresis (per pid)"]:::codex
      M{"codex sessions for<br/>project_path > 1 ?"}:::decision
      N["raw_active = file_active<br/>OR proc_active"]:::codex
      O["raw_active = proc_active<br/>only"]:::codex
      P{"multi-codex?"}:::decision
      Q["keep session_id/jsonl_path<br/>from file resolver"]:::codex
      R["hide session_id/jsonl_path<br/>(shared, not attributable)"]:::codex
    end

    Z(["state =<br/>hysteresis(raw_active)"]):::result

    B --> C
    C --> D
    D --> E
    E --> Z

    B --> G
    G --> H
    H --> I
    I --> Z

    B --> K
    K --> L
    L --> M
    M -->|No| N
    M -->|Yes| O
    N --> P
    O --> P
    P -->|No| Q
    P -->|Yes| R
    Q --> Z
    R --> Z

    style CLAUDE_LANE fill:#134e4a,stroke:#2dd4bf,stroke-width:3px,color:#99f6e4,font-weight:bold,font-size:14px
    style GEMINI_LANE fill:#1e3a8a,stroke:#60a5fa,stroke-width:3px,color:#dbeafe,font-weight:bold,font-size:14px
    style CODEX_LANE fill:#1f2937,stroke:#94a3b8,stroke-width:3px,color:#e2e8f0,font-weight:bold,font-size:14px

    linkStyle default stroke:#2dd4bf,stroke-width:2px
    linkStyle 1,5,9,12,13,16,17 stroke:#14b8a6,stroke-width:1.4px,stroke-dasharray:5 5
    linkStyle 4,8,18,19 stroke:#99f6e4,stroke-width:2.4px
```

## Platform process inspection

Process inspection uses platform-specific implementations:

| Platform | Process CWD/TTY/IO strategy |
|------|------------------------------|
| Linux | `/proc` (`/proc/<pid>/cwd`, `/proc/<pid>/fd/0`, `/proc/<pid>/io`, `/proc/<pid>/net/tcp*`) |
| macOS | `libproc` + `lsof` fallback (CWD/TTY/socket checks) |
| Windows | Native scan is no-op; session detection is handled by WSL daemon path |

## Per-tool idle/activity detection

SessionResolver-based file detection:

| Tool | Session file source | Activity threshold | Extra notes |
|------|---------------------|--------------------|------------|
| Claude | `~/.claude/projects/<slug>/*.jsonl` + `<session>/subagents/*.jsonl` | 5s mtime | Subagent mtime keeps compaction work marked active |
| Codex | `~/.codex/sessions/YYYY/MM/DD/*.jsonl` matched by `session_meta.payload.cwd` | 10s mtime | 7-day lookback supports resumed sessions stored in older date dirs |
| Gemini | `~/.gemini/tmp/<dir-or-hash>/chats/*.json` | 5s mtime | Supports both newer directory-name layout and older path-hash layout |

Process-level supplemental signals:
- Claude: `/proc` IO (`rchar` delta threshold) with consecutive-poll hysteresis
- Gemini: ESTABLISHED TCP socket to remote `:443` indicates active API call
- Codex: `/proc` IO (`rchar` delta threshold) with consecutive-poll hysteresis; file mtime is kept as fallback only when the project has a single Codex session

## Bidirectional hysteresis

Two hysteresis layers reduce state flicker:
- IO hysteresis (Claude/Codex process signals): requires two consecutive above-threshold polls for active confirmation
- Session state hysteresis (all tools): reported state changes only after two consecutive raw polls agree on the new state

Polling cadence:
- Daemon scanner cadence is `500ms` (`SessionActivityHub`).
- Tauri UI path is event-driven (daemon long-poll -> `sessions-updated`), not frontend IPC polling.
- Frontend-only mock mode still uses a `500ms` polling loop for local development.
- State transition confirmation requires sustained agreement across consecutive scanner polls.

## Sidebar indicators and hover details

Sidebar behavior:
- Live sessions are shown as tool-specific status pills/icons on project rows.
- State colors: green for active, amber for idle.
- Indicators are clickable when tmux metadata exists, enabling jump-to-session navigation.

HoverCard behavior:
- On row hover, card shows:
  - project status (activity state, dirty flag)
  - recent commits
  - per-tool live session summary (working vs waiting)
  - duration and active-time percentages
  - technical metadata (session id, tmux target, pid)
  - historical aggregated activity stats

## Session handoffs and import flow

Handoff format:
- Markdown handoff file with YAML frontmatter (`date`, `summary`, `next_steps`, `open_questions`, etc.)
- Optional `.meta.json` sidecar for structured metadata

Import behavior:
- File watcher classifies `docs/sessions/session-*.md` create/modify events as session files.
- Event processor imports via `services::session_import::import_handoff`.
- Dedup is file-path based (already-imported files are skipped).
- Session ID precedence: frontmatter `session_id` -> sidecar `session_id` -> generated UUID.
- Successful import emits `session-imported` and updates search index.

Authoring model:
- Session handoffs are expected from Claude Code `SessionEnd` hook output.
- `/handoff` skill is documented as a manual fallback path.

## Session history and summaries

Overview tab session summary:
- Uses `get_latest_session` and `list_sessions`.
- Shows latest summary, next steps, open questions, and timeline-style recent summaries.

Session History timeline view:
- Uses `get_archived_sessions` grouped by session id.
- Shows tasks, commit counts, file counts, source tools, and expandable lazy-loaded commit/file detail.
- Supports navigation to commit, file, or commit-range filtered Git view.

Activity statistics:
- Frontend tracks per-session active/total ticks per session-store update tick.
- In Tauri, update ticks are event-driven daemon snapshots; in mock mode, ticks come from frontend polling.
- On session disappearance, it persists session activity metrics.
- HoverCard reads aggregated project activity stats for historical context.

## Key files

| File | Purpose |
|------|---------|
| `src/lib/sessionStore.svelte.js` | Session snapshot store, event-apply path, mock-mode polling, per-session runtime metrics, activity persistence trigger |
| `src/lib/sessionIndicator.js` | Tool indicator semantics, active/idle coloring, row tinting |
| `src/lib/toolLogos.js` | Shared SVG logos + display names for Claude/Codex/Gemini |
| `src/lib/Sidebar.svelte` | Session badges in project list, tmux jump interactions, hover-card entry point |
| `src/lib/HoverCard.svelte` | Live session detail card + historical activity preview |
| `src/lib/SessionHistory.svelte` | Archived session timeline with task/commit/file drill-down |
| `src-tauri/src/session_scanner/mod.rs` | Scanner orchestration, dedup logic, global bidirectional hysteresis |
| `src-tauri/src/session_scanner/process.rs` | Process discovery and CLI tool detection from `ps` output |
| `src-tauri/src/session_scanner/proc_io.rs` | Claude/Codex IO activity heuristic + Gemini TCP activity checks |
| `src-tauri/src/session_scanner/idle/mod.rs` | SessionResolver abstraction and shared detection helpers |
| `src-tauri/src/session_scanner/idle/claude.rs` | Claude session file + subagent mtime logic |
| `src-tauri/src/session_scanner/idle/codex.rs` | Codex date-tree lookup + CWD matching + 7-day lookback |
| `src-tauri/src/session_scanner/idle/gemini.rs` | Gemini chats directory/hash resolution + mtime logic |
| `src-tauri/src/daemon/session_activity.rs` | Daemon-owned global scanner hub, versioned snapshots, long-poll waiters |
| `src-tauri/src/daemon/session_listener.rs` | App-side daemon long-poll client for `wait_session_updates` |
| `src-tauri/src/daemon_lifecycle.rs` | Session updates bridge (`sessions-updated`) and daemon reconnect flow |
| `src-tauri/src/platform/linux.rs` | Linux `/proc` process/IO/socket inspection |
| `src-tauri/src/platform/darwin.rs` | macOS `libproc` + `lsof` process/socket inspection |
| `src-tauri/src/session/parser.rs` | Handoff markdown/frontmatter + sidecar parser |
| `src-tauri/src/services/session_import.rs` | Handoff import and dedup into sessions table |
| `src-tauri/src/commands/sessions.rs` | Session summary/list/detail IPC for overview |

## Related documents

- [Command center](./command-center.md) — session launch/stop/navigation controls
- [Git integration](./git-integration.md) — commit-range integration used by session history
- [Project management](./project-management.md) — where session context is surfaced in sidebar/overview
