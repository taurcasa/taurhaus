# taurhaus Setup Guide

## Prerequisites

taurhaus runs on Windows but relies on several components running inside WSL2. These must be available for the app to function.

| Component | Required | Managed by app | Notes |
|-----------|----------|----------------|-------|
| WSL2 | Yes | Implicitly (first `wsl.exe` call starts it) | Must be installed and configured beforehand |
| taurhaus-daemon | Yes | Auto-started on app launch | TCP server in WSL, listens on port 9000 |
| tmux server | Yes | Auto-started on app launch | Background process, no UI |
| Windows Terminal | No | Opened on demand | Only needed to view/interact with CLI sessions |

## Bootstrap Chain

When taurhaus launches, it ensures prerequisites are running in this order:

```
1. Daemon running?      → no → wsl.exe -d {distro} -- ~/.local/bin/taurhaus-daemon --port 9000
2. tmux server alive?   → no → wsl.exe -d {distro} -- tmux start-server
3. (Terminal)            → opened later, on demand, when user wants to interact with a session
```

Each step checks whether the component is already running before acting. If WSL2 isn't started yet, the first `wsl.exe` call starts it implicitly — no separate step needed.

## How tmux fits in

tmux has a server/client architecture that's central to how taurhaus works:

- **tmux server**: A background process that owns sessions, windows, and panes. Each pane contains a virtual terminal running a shell or command. The server persists regardless of whether anyone is watching.
- **tmux client**: A terminal that "attaches" to a session as a viewport. Purely for viewing and interaction. Closing the terminal doesn't affect the server or anything running in it.

When you run `tmux` with no arguments in a terminal, it does three things invisibly: starts a server (if needed), creates a session, and attaches your terminal as a client.

taurhaus only talks to the **server side** — creating windows, sending keystrokes, listing panes, reading output. CLI tools (Claude, Codex, Gemini) run as processes inside tmux panes, managed by the server, with no terminal required. The terminal is only needed when you want to watch or type into a running session.

## Installing the daemon

```bash
just install-daemon
```

This builds the daemon binary and copies it to `~/.local/bin/taurhaus-daemon`. The app's auto-start mechanism expects it at this path.

## Shell Configuration

### oh-my-zsh auto-update (required for headless sessions)

taurhaus creates tmux panes headlessly — no terminal is attached when the pane's shell starts. If oh-my-zsh is configured to prompt for updates, the shell blocks waiting for Y/N input. Any commands sent to that pane (like launching Claude) end up going into the update prompt instead.

**Fix** — set auto-update mode in `~/.zshrc`:

```zsh
zstyle ':omz:update' mode auto      # update automatically without asking
```

This tells oh-my-zsh to update silently. No prompt, no blocking.

**General principle**: Any interactive shell plugin that prompts on startup is a problem for headless tmux panes. If you add new zsh plugins, check whether they have interactive prompts and disable or auto-accept them.

### Why daemon and tmux startup aren't affected

The bootstrap chain runs binaries directly via `wsl.exe -- /path/to/binary`, which doesn't start a shell at all. No `.zshrc` is sourced, no plugins load, no prompts appear. The shell configuration issue only applies to the interactive shells that tmux spawns inside panes.

## Troubleshooting

### App doesn't detect sessions / daemon not connected

Check if the daemon is running:

```bash
# From WSL
ss -tlnp | grep 9000
```

If not running, start it manually:

```bash
taurhaus-daemon --verbose
```

Or reinstall and restart:

```bash
just install-daemon
```

### tmux not running

```bash
tmux list-sessions
```

If you get "no server running", start one:

```bash
tmux start-server
tmux new-session -d -s main
```

### CLI tool launch fails

Ensure tmux has at least one session. taurhaus creates new windows inside an existing session — it doesn't create sessions from scratch.

```bash
tmux list-sessions    # should show at least one
```

### Phone SSH + PC resolution conflict

If you SSH in from a phone and attach to the same tmux session your PC is using, the session grid resizes to the smallest client (the phone), shrinking everything on your PC.

Options:
- Attach read-only from the phone: `tmux attach -t mysession -r` (no resize)
- Use a separate session: `tmux new-session -s phone`
- Detach the phone before returning to the PC
