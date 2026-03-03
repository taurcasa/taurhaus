# Command center

The command center is taurhaus's session control layer for launching, stopping, and navigating CLI tool sessions inside tmux. It connects sidebar/context-menu actions to backend orchestration and platform-specific terminal behavior.

## Overview

Command center responsibilities:
- launch tool sessions for a selected project
- stop running sessions gracefully
- navigate to a specific tmux pane/window/session
- ensure terminal visibility/focus based on user intent
- expose configurable per-tool launch commands and tmux layout settings

## Launch modes

Supported launch modes:
- `Continue` — continue work in existing context
- `Fresh` — start a new session
- `Resume` — resume prior session context

Mode handling:
- Frontend passes mode + tool to `launch_claude_session` IPC.
- Backend resolves project path, then resolves command from settings per `(tool, mode)`.
- Session launches in tmux using selected layout strategy.

## Per-tool launch commands

Default commands (configurable in Settings):

| Tool | Continue | Fresh | Resume |
|------|----------|-------|--------|
| Claude | `claude --dangerously-skip-permissions --continue` | `claude --dangerously-skip-permissions` | `claude --dangerously-skip-permissions --resume` |
| Codex | `codex --yolo` | `codex --yolo` | `codex resume --last --yolo` |
| Gemini | `gemini --yolo --resume` | `gemini --yolo` | `gemini --yolo --resume` |

Notes:
- Commands are editable per tool/mode in `Settings -> Terminal & Sessions / CLI Tools`.
- Override commands are validated for safety before execution.

## Stop behavior

Stopping is pane-targeted (`stop_claude_session(tmux_pane, cli_tool)`), then backend sends exit input to the tool process and cleans up the pane.

Current tool behavior:
- Claude: sends `/exit` to tmux pane
- Gemini: sends `/exit` to tmux pane
- Codex: sends `Ctrl+C` (key signal)

After exit signal:
- backend polls pane command until shell is detected (or timeout)
- pane is killed to clean up (window closes automatically if last pane)

## tmux integration

tmux is the execution substrate for all command-center launches.

Launch workflow:
- ensure dedicated tmux session exists (`taurhaus`)
- choose layout strategy (`new_window`, `split`, `per_project`)
- create/split pane and run command in project directory
- return tmux coordinates (`tmux_session`, `tmux_window`, `tmux_pane`)

Navigation workflow:
- select tmux window
- select tmux pane
- optionally trigger terminal focus/open behavior

## Terminal decision tree

Terminal handling uses two intents:
- `FocusOnly` — focus an already-running preferred terminal
- `EnsureOpen` — if terminal is running, focus; otherwise launch and attach tmux

Unified flow:
1. Resolve emulator preference from settings
2. If `FocusOnly`: activate if running
3. If `EnsureOpen`:
   - if running and tmux has client: activate
   - else: launch with `tmux attach-session -t <session>`
4. `custom` emulator runs user command template with placeholders

## Platform support

| Platform | Emulator options | Default | Behavior |
|----------|------------------|---------|----------|
| Windows | `windows_terminal`, `custom` | `windows_terminal` | 3-state WT detection (`Focused`/`Running`/`NotRunning`), launch via `wt.exe` + `wsl.exe ... tmux attach` |
| macOS | `iterm2`, `ghostty`, `terminal_app`, `custom` | `iterm2` | Resolve preferred app, activate existing or launch with tmux attach |
| Linux | user-managed | n/a | terminal handler is a no-op (tmux still used for session control) |

Windows-specific detail:
- Windows Terminal detection handles WinUI window-handle quirks with process + window-enumeration fallback.

## Context-menu integration

Right-click project menu exposes command-center actions:
- Continue/New/Resume for Claude, Codex, Gemini
- Open in Terminal (when tmux metadata exists)
- Per-running-session Restart/Stop actions

Interaction model:
- Stop action includes confirmation timeout
- Restart does stop then continue launch for that tool
- Session badges in sidebar support click-to-navigate to tmux pane

## Settings integration

Settings provides user control for command-center behavior:
- terminal emulator selection
- custom terminal command template
- tmux layout strategy
- per-tool per-mode launch commands

This allows consistent command-center UX with environment-specific tooling without changing backend code.

## Daemon vs fallback path

Command center prefers daemon-backed execution when connected:
- daemon handles launch/stop/navigate requests
- results are normalized to tmux metadata for frontend use

If daemon is unavailable:
- backend falls back to direct in-process tmux control
- behavior remains functionally equivalent for launch/stop/navigation

## Key files

| File | Purpose |
|------|---------|
| `src/lib/Sidebar.svelte` | Project context-menu actions that trigger launch/stop/restart/navigation |
| `src/lib/ContextMenu.svelte` | Menu interaction component used by command-center entry points |
| `src/lib/Settings.svelte` | Emulator, tmux layout, and per-tool command configuration UI |
| `src-tauri/src/commands/command_center.rs` | IPC commands for list/launch/stop/navigate and activity metrics |
| `src-tauri/src/session_scanner/control.rs` | tmux launch/stop/navigation core logic + command resolution |
| `src-tauri/src/terminal.rs` | Cross-platform terminal focus/open decision tree |
| `src-tauri/src/session_scanner/cli_tool.rs` | Tool config (`exit_command`, tool identity) |
| `src-tauri/src/claude_code/mod.rs` | Claude Code integration module root referenced by command-center flows |

## Related documents

- [Platform abstraction](../platform-abstraction.md) — cross-platform runtime and terminal integration strategy
- [Session management](./session-management.md) — session detection and live-state model consumed by command-center actions
- [Project management](./project-management.md) — where users trigger command-center actions in the main UI
