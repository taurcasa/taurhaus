# Getting Started with taurhaus

This guide walks you through setting up taurhaus from scratch. By the end, you'll have the app running with your projects loaded and CLI sessions visible.

Pick your platform to get started:

- [Windows Setup](#windows-setup)
- [macOS Setup](#macos-setup)
- [Install taurhaus](#install-taurhaus) (if prerequisites are already done)

## Before You Start

taurhaus is a desktop app that manages AI coding sessions (Claude Code, Codex, Antigravity CLI, Grok CLI) across your projects. It runs on **Windows** and **macOS**.

| | Windows | macOS |
|---|---------|-------|
| **Requirements** | Windows 10/11 with WSL2 | macOS 12 Monterey or later |
| **CLI tools run in** | WSL2 (Linux) | Natively |
| **Terminal** | Windows Terminal | iTerm2, Ghostty, or Terminal.app |
| **Setup time** | ~10 minutes | ~5 minutes |

---

## Windows Setup

If you already have WSL2, Windows Terminal, and tmux installed, skip to [Install Your AI CLI Tools](#install-your-ai-cli-tools).

### Step 1: Set Up WSL2

Open PowerShell as Administrator and run:

```powershell
wsl --install
```

This installs Ubuntu by default. Restart your computer when prompted.

After restart, open Ubuntu from the Start menu to complete the Linux user setup (username and password).

#### Enable mirrored networking

taurhaus communicates with its helper service running inside WSL. For this to work, WSL must use mirrored networking mode.

Open Notepad and create (or edit) the file at `%USERPROFILE%\.wslconfig` with this content:

```ini
[wsl2]
networkingMode=mirrored
```

Then restart WSL:

```powershell
wsl --shutdown
```

**Why this matters**: Without mirrored networking, the Windows app can't connect to the WSL helper service (the `taurhaus-daemon` process) on `localhost:17233`. This is the most common setup issue.

### Step 2: Install tmux

Open your WSL terminal (Ubuntu) and run:

```bash
sudo apt update && sudo apt install -y tmux
```

taurhaus uses tmux to manage CLI tool sessions in the background. You don't need to know tmux — the app handles everything for you.

Now continue to [Install Your AI CLI Tools](#install-your-ai-cli-tools).

---

## macOS Setup

### Step 1: Install Homebrew (if needed)

If you don't have Homebrew yet:

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

### Step 2: Install tmux

```bash
brew install tmux
```

taurhaus uses tmux to manage CLI tool sessions in the background. You don't need to know tmux — the app handles everything for you.

Now continue to [Install Your AI CLI Tools](#install-your-ai-cli-tools).

---

## Install Your AI CLI Tools

Install whichever tools you use. On Windows, run these inside WSL. On macOS, run them in your normal terminal.

**Claude Code** (Anthropic):
```bash
curl -fsSL https://claude.ai/install.sh | bash
```

**Codex** (OpenAI):
```bash
bun add -g @openai/codex
```

**Antigravity CLI** (Google): install the native `agy` binary from the
[Antigravity CLI project](https://github.com/google-antigravity/antigravity-cli).

**Grok CLI** (xAI): install the native `grok` binary as described in the
[Grok Build docs](https://docs.x.ai/build/overview).

You need at least one installed for session management features. The app works without any CLI tools — you just won't see live sessions.

## Shell Configuration

If you use oh-my-zsh (default shell framework on many setups, and the default shell on macOS is zsh), add this line to your `~/.zshrc`:

```bash
zstyle ':omz:update' mode auto
```

This prevents oh-my-zsh from blocking headless terminal sessions with update prompts. If you don't use oh-my-zsh, skip this step.

**General rule**: Any shell plugin that prompts for input on startup will block taurhaus from launching CLI sessions. If you use other interactive plugins, configure them to run non-interactively.

## Install taurhaus

### Windows

1. Download the latest `taurhaus_x.x.x_x64-setup.exe` from the [Releases page](https://github.com/taurcasa/taurhaus/releases)
2. Run the installer — it's a standard Windows setup wizard
3. Launch taurhaus from the Start menu

On first launch, taurhaus will:
- Start the WSL helper service automatically in the background
- Create a tmux session named "taurhaus" in WSL
- Show the First Run Wizard

### macOS

1. Download the latest `taurhaus_x.x.x_aarch64.dmg` from the [Releases page](https://github.com/taurcasa/taurhaus/releases)
2. Open the DMG and drag taurhaus to your Applications folder
3. Launch taurhaus from Applications (or Spotlight)

On first launch, taurhaus will:
- Install and start the helper service automatically at `~/.local/bin/taurhaus-daemon`
- Create a tmux session named "taurhaus"
- Show the First Run Wizard

> **macOS Gatekeeper**: Since the app is not notarized, you may see "taurhaus can't be opened because it is from an unidentified developer." Right-click the app, select Open, then click Open in the dialog. You only need to do this once.

## First Run Wizard

The wizard helps you discover your projects:

1. **Welcome** — Overview of what taurhaus does
2. **Helper service setup** — Check helper-service install status, offer install/update if needed
3. **Browse** — Navigate to your project directories (e.g., `~/projects/`)
4. **Select** — Choose which projects to register
5. **Progress** — Projects are scanned and indexed
6. **Complete** — You're ready to go

The app scans `~/projects/` by default. You can add more directories later in Settings.

## How taurhaus works

taurhaus is a desktop app that watches your project directories and detects when AI CLI tools (Claude Code, Codex, Antigravity CLI, Grok CLI) are running in tmux sessions. It gives you a single view into every project — files, commits, tasks, and active sessions — so you can switch between projects without losing context. The app watches native/local project directories itself (`startup/watchers.rs`) and stores project metadata in SQLite, indexes content for full-text search, and talks to a background helper service (the "daemon") over a local TCP connection. The daemon takes over the work that has to happen on the Linux side: watching WSL project paths when it is connected, and inspecting sessions and accounts there.

## Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+K` / `Cmd+K` | Open search overlay |
| `Tab` / `Shift+Tab` | Navigate between tabs |
| `Arrow keys` | Navigate within lists and menus |
| `Escape` | Close overlay, dialog, or context menu |
| `Shift+F10` | Open context menu (keyboard equivalent of right-click) |
| `Enter` / `Space` | Activate focused item |

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
| **Tasks** | Aggregated tasks from supported transcript-capable harnesses |
| **Mesh** | Multi-agent team setup and live roster |
| **Git** | Commit history, diffs, and file changes |

### CLI Sessions

Tool indicator icons appear next to project names in the sidebar when CLI sessions are running:

- **Green glow** = actively working (streaming output)
- **Amber outline** = idle (waiting for input)

**Launch a session**: Right-click a project and choose the tool-specific menu items. Taurhaus shows `Continue Claude` and `Continue Grok`, `New Claude Session`, `New Codex Session`, `New Antigravity Session` and `New Grok Session`, plus `Resume Claude`, `Resume Codex`, `Resume Antigravity` and `Resume Grok`. Where a tool has more than one account signed in, each launch item carries an `Account` submenu listing those accounts, with usage where the tool reports it — Grok is `usage: false`, and any account without a usage snapshot shows no meter.

**Navigate to a session**: Click the tool icon to jump to that session in your terminal. `Open in Terminal` appears when a live session has tmux coordinates; if navigation is attempted without a valid terminal target, taurhaus shows a sidebar notice instead of failing silently.

**Stop a session**: Right-click a project and use the per-tool `Stop <Tool>` or `Restart <Tool>` actions for running sessions.

### Mesh View

The Mesh tab lets you set up and manage multi-agent teams. Start from a built-in preset, user template, or blank slate; choose Claude, Codex, or Antigravity lead roles where supported by the selected preset (Grok ships as an agent role, `grok-developer`, with a `Grok Pair` preset); then launch and monitor the team from the live roster/canvas. See [Mesh view](features/mesh.md) for details.

### Search

Press `Ctrl+K` (Windows) or `Cmd+K` (macOS) to open the search overlay. Search across all project files and content.

### Settings

Click the gear icon in the bottom-left corner to configure:
- Scan directories and ignore patterns
- Activity thresholds (Active / Recent / Stale / Dormant)
- Code viewer theme (separate for light and dark mode)
- Preferred terminal app (`Windows Terminal` or `Custom` on Windows, `iTerm2`/`Ghostty`/`Terminal.app`/`Custom` on macOS, manual on Linux)

## Troubleshooting

### Windows

#### "Helper service not connected" or sessions not appearing

The helper service runs inside WSL and communicates over TCP port 17233. If sessions don't appear:

1. **Check WSL networking mode**:
   ```powershell
   # In PowerShell
   cat $env:USERPROFILE\.wslconfig
   ```
   Must contain `networkingMode=mirrored`. If you just changed it, run `wsl --shutdown` and relaunch taurhaus.

2. **Check if the helper service is running**:
   ```bash
   # In WSL
   ss -tlnp | grep 17233
   ```
   If nothing shows, restart taurhaus — it auto-starts the helper service.

3. **Check for port conflicts**:
   ```bash
   # In WSL
   ss -tlnp | grep 17233
   ```
   If another process is using port 17233, stop it or change the conflicting service's port.

4. **Manual helper-service start** (for debugging):
   ```bash
   # In WSL
   ~/.local/bin/taurhaus-daemon --verbose
   ```
   This shows detailed connection and scanning logs.

#### Windows Terminal opens but shows nothing

The terminal tab runs `tmux attach-session -t taurhaus`. If the tmux session doesn't exist:

```bash
# In WSL
tmux new-session -d -s taurhaus
```

Then try launching a session from taurhaus again.

### macOS

#### "Helper service not connected" or sessions not appearing

The helper service runs natively and communicates over TCP port 17233. If sessions don't appear:

1. **Check if the helper service is running**:
   ```bash
   lsof -i :17233
   ```
   If nothing shows, restart taurhaus — it auto-starts the helper service.

2. **Check for port conflicts**:
   ```bash
   lsof -i :17233
   ```
   If another process is using port 17233, stop it or change the conflicting service's port.

3. **Manual helper-service start** (for debugging):
   ```bash
   ~/.local/bin/taurhaus-daemon --verbose
   ```

#### Helper service crashes immediately after update

On macOS Sequoia and later, copied binaries can fail code signature validation. If the helper service won't start after an update:

```bash
codesign --force --sign - ~/.local/bin/taurhaus-daemon
```

Then restart taurhaus.

#### Terminal emulator doesn't open

taurhaus supports iTerm2, Ghostty, and Terminal.app. Check that your preferred emulator is installed in `/Applications/`. You can change the emulator in Settings.

### Both Platforms

#### CLI tool not detected

taurhaus detects CLI tools by scanning running processes. If a tool doesn't appear:

- Make sure it's installed globally (`bun add -g ...` or via the tool's installer)
- Make sure it's running inside the `taurhaus` tmux session (launched via taurhaus, not manually)
- Check `tmux list-windows -t taurhaus` to see active windows

#### App feels slow on first load

The initial project scan indexes all files for search. This is a one-time operation. Subsequent launches are fast because the index is persisted.

#### File watcher limits on Linux/WSL

taurhaus uses Linux file watchers (inotify) to detect changes in your projects. The helper service uses roughly 4-6 watcher instances, and each Mesh team member adds 2 more. The AI tools themselves (Claude Code, Codex, Antigravity) also create their own watchers independently.

The default Linux limit is 128 instances per user. If you run large Mesh teams or many projects, you may hit this limit — taurhaus will log a warning if it happens.

Check your current limit:

```bash
sysctl fs.inotify.max_user_instances
```

Raise it if needed:

```bash
sudo sysctl -w fs.inotify.max_user_instances=512
```

To make the change permanent, add `fs.inotify.max_user_instances=512` to `/etc/sysctl.conf`.

This only applies to Linux and WSL2. macOS uses a different file-watching mechanism and doesn't have this limit.

## Updating

Download and run the latest installer (Windows) or DMG (macOS). It overwrites the previous version. Your projects, settings, and search index are preserved — they're stored in your user data directory, not the install directory.

The installer ships an updated helper-service binary with each release. If a local update is needed, taurhaus can install the bundled helper service from the app's update flow.

## Uninstalling

### Windows

1. Uninstall taurhaus via Windows Settings > Apps
2. Optionally remove the helper-service binary:
   ```bash
   # In WSL
   rm ~/.local/bin/taurhaus-daemon
   ```
3. Optionally remove the tmux session:
   ```bash
   tmux kill-session -t taurhaus
   ```
4. App data is stored in `%APPDATA%\com.taurhaus.dev` — remove this directory to delete all project data, settings, and search indexes

### macOS

1. Drag taurhaus from Applications to Trash
2. Optionally remove the helper-service binary:
   ```bash
   rm ~/.local/bin/taurhaus-daemon
   ```
3. Optionally remove the tmux session:
   ```bash
   tmux kill-session -t taurhaus
   ```
4. App data is stored in `~/Library/Application Support/com.taurhaus.dev` — remove this directory to delete all project data, settings, and search indexes
