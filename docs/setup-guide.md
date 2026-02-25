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
1. Daemon running?            → no → wsl.exe -d {distro} -- ~/.local/bin/taurhaus-daemon --port 9000
2. tmux "taurhaus" session?   → no → wsl.exe -d {distro} -- tmux new-session -d -s taurhaus
3. (Terminal)                  → opened later, on demand, via wt.exe with tmux attach
```

Each step checks whether the component is already running before acting. If WSL2 isn't started yet, the first `wsl.exe` call starts it implicitly — no separate step needed.

The app uses a dedicated tmux session named `taurhaus` for all CLI tool windows. This avoids interfering with the user's own tmux sessions. The daemon-side code also creates this session on demand if it doesn't exist when launching a tool.

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

### tmux not running / taurhaus session missing

```bash
tmux list-sessions
```

You should see a session named `taurhaus`. If not, the app creates it automatically on startup. To create it manually:

```bash
tmux new-session -d -s taurhaus
```

### CLI tool launch fails

The app creates tool windows inside the `taurhaus` tmux session. Check that it exists:

```bash
tmux has-session -t taurhaus    # exit code 0 = exists
```

### Phone SSH + PC resolution conflict

If you SSH in from a phone and attach to the same tmux session your PC is using, the session grid resizes to the smallest client (the phone), shrinking everything on your PC.

Options:
- Attach read-only from the phone: `tmux attach -t mysession -r` (no resize)
- Use a separate session: `tmux new-session -s phone`
- Detach the phone before returning to the PC
