# Getting Started with taurhaus

This guide walks you through setting up taurhaus from scratch on a Windows machine. By the end, you'll have the app running with your projects loaded and CLI sessions visible.

## Before You Start

taurhaus is a Windows desktop app that manages AI coding sessions running inside WSL2. You'll need:

- **Windows 10 or 11** with admin access
- **10 minutes** for initial setup

If you already have WSL2, Windows Terminal, and tmux installed, skip straight to [Install taurhaus](#install-taurhaus).

## Step 1: Set Up WSL2

If you don't have WSL2 yet, open PowerShell as Administrator and run:

```powershell
wsl --install
```

This installs Ubuntu by default. Restart your computer when prompted.

After restart, open Ubuntu from the Start menu to complete the Linux user setup (username and password).

### Enable mirrored networking

taurhaus communicates with a daemon running inside WSL. For this to work, WSL must use mirrored networking mode.

Open Notepad and create (or edit) the file at `%USERPROFILE%\.wslconfig` with this content:

```ini
[wsl2]
networkingMode=mirrored
```

Then restart WSL:

```powershell
wsl --shutdown
```

**Why this matters**: Without mirrored networking, the Windows app can't connect to the WSL daemon on `localhost:17233`. This is the most common setup issue.

## Step 2: Install tmux

Open your WSL terminal (Ubuntu) and run:

```bash
sudo apt update && sudo apt install -y tmux
```

taurhaus uses tmux to manage CLI tool sessions in the background. You don't need to know tmux — the app handles everything for you.

## Step 3: Install Your AI CLI Tools

Install whichever tools you use inside WSL:

**Claude Code** (Anthropic):
```bash
curl -fsSL https://claude.ai/install.sh | bash
```

**Codex** (OpenAI):
```bash
npm install -g @openai/codex
```

**Gemini CLI** (Google):
```bash
npm install -g @google/gemini-cli
```

You need at least one installed for session management features. The app works without any CLI tools — you just won't see live sessions.

## Step 4: Shell Configuration

If you use oh-my-zsh, add this line to your `~/.zshrc`:

```bash
zstyle ':omz:update' mode auto
```

This prevents oh-my-zsh from blocking headless terminal sessions with update prompts. If you don't use oh-my-zsh, skip this step.

**General rule**: Any shell plugin that prompts for input on startup will block taurhaus from launching CLI sessions. If you use other interactive plugins, configure them to run non-interactively.

## Install taurhaus

1. Download the latest `taurhaus_x.x.x_x64-setup.exe` from the [Releases page](../../releases)
2. Run the installer — it's a standard Windows setup wizard
3. Launch taurhaus from the Start menu

On first launch, taurhaus will:
- Start the WSL daemon automatically (you may see a brief console flash — this is normal)
- Create a tmux session named "taurhaus" in WSL
- Show the First Run Wizard

## First Run Wizard

The wizard helps you discover your projects:

1. **Welcome** — Overview of what taurhaus does
2. **Browse** — Navigate to your project directories (e.g., `~/projects/`)
3. **Select** — Choose which projects to register
4. **Progress** — Projects are scanned and indexed
5. **Complete** — You're ready to go

The app scans `~/projects/` by default. You can add more directories later in Settings.

## Using taurhaus

### Sidebar

The sidebar lists all registered projects, grouped by activity:
- **Active** — Modified within the last 7 days
- **Recent** — Modified within the last 30 days
- **Stale** — Modified within the last 90 days
- **Dormant** — No activity for 90+ days

Click a project to open it. The thresholds are configurable in Settings.

### Tabs

Each project has five tabs:

| Tab | What it shows |
|-----|---------------|
| **Overview** | README, recent commits, session handoffs |
| **Files** | File tree with syntax-highlighted preview |
| **Tasks** | Aggregated tasks from Claude Code, Codex, and Gemini |
| **Git** | Commit history, diffs, and file changes |
| **Sessions** | Completed session history with commit context |

### CLI Sessions

Tool indicator icons appear next to project names in the sidebar when CLI sessions are running:

- **Green glow** = actively working (streaming output)
- **Amber outline** = idle (waiting for input)

**Launch a session**: Right-click a project, select "Launch Claude" (or Codex, or Gemini)

**Navigate to a session**: Click the tool icon to jump to that session in Windows Terminal

**Stop a session**: Right-click the tool icon and select "Stop"

### Search

Press `Ctrl+K` to open the search overlay. Search across all project files and content.

### Settings

Click the gear icon in the bottom-left corner to configure:
- Scan directories and ignore patterns
- Activity thresholds (Active / Recent / Stale / Dormant)
- Code viewer theme (separate for light and dark mode)

## Troubleshooting

### "Daemon not connected" or sessions not appearing

The daemon runs inside WSL and communicates over TCP port 17233. If sessions don't appear:

1. **Check WSL networking mode**:
   ```powershell
   # In PowerShell
   cat $env:USERPROFILE\.wslconfig
   ```
   Must contain `networkingMode=mirrored`. If you just changed it, run `wsl --shutdown` and relaunch taurhaus.

2. **Check if the daemon is running**:
   ```bash
   # In WSL
   ss -tlnp | grep 17233
   ```
   If nothing shows, restart taurhaus — it auto-starts the daemon.

3. **Check for port conflicts**:
   ```bash
   # In WSL
   ss -tlnp | grep 17233
   ```
   If another process is using port 17233, stop it or change the conflicting service's port.

4. **Manual daemon start** (for debugging):
   ```bash
   # In WSL
   ~/.local/bin/taurhaus-daemon --verbose
   ```
   This shows detailed connection and scanning logs.

### CLI tool not detected

taurhaus detects CLI tools by scanning running processes in WSL. If a tool doesn't appear:

- Make sure it's installed globally (`npm install -g ...`)
- Make sure it's running inside the `taurhaus` tmux session (launched via taurhaus, not manually)
- Check `tmux list-windows -t taurhaus` to see active windows

### Windows Terminal opens but shows nothing

The terminal tab runs `tmux attach-session -t taurhaus`. If the tmux session doesn't exist:

```bash
# In WSL
tmux new-session -d -s taurhaus
```

Then try launching a session from taurhaus again.

### App feels slow on first load

The initial project scan indexes all files for search. This is a one-time operation. Subsequent launches are fast because the index is persisted.

## Updating

Download and run the latest installer. It overwrites the previous version. Your projects, settings, and search index are preserved (stored in your user data directory, not the install directory).

The installer ships an updated daemon binary with each app release. If a local daemon update is needed, taurhaus can install the bundled daemon from the app's daemon update flow.

## Uninstalling

1. Uninstall taurhaus via Windows Settings > Apps
2. Optionally remove the daemon binary:
   ```bash
   rm ~/.local/bin/taurhaus-daemon
   ```
3. Optionally remove the tmux session:
   ```bash
   tmux kill-session -t taurhaus
   ```
4. App data is stored in `%APPDATA%\com.taurhaus.app` — remove this directory to delete all project data, settings, and search indexes
