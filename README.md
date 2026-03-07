# taurhaus

A desktop operations surface for developers running multiple AI tools, multiple projects, and multi-agent Mesh teams at the same time.

`taurhaus` gives you one place to see what Claude Code, Codex, Gemini CLI, and Mesh agents are doing right now, recover project context without terminal archaeology, and coordinate team workflows without giving up your editor or terminal.

![taurhaus hero overview](docs/screenshots/readme-hero-overview.png)

## Why taurhaus

AI-assisted development breaks down when the work stops being linear. One project is in a Codex session, another has a Claude refactor in flight, Gemini owns a TODO-driven cleanup, and a Mesh team is halfway through a coordinated change. The terminal can run that workflow, but it does not help you supervise it.

`taurhaus` is built for that exact operating mode:

- **Watch live work** across projects with session indicators, activity groups, hover previews, and direct session controls.
- **Recover context quickly** through README previews, recent commits, task history, handoff summaries, and cross-project search.
- **Coordinate Mesh teams** with setup, runtime visibility, hot-add/remove flows, and recovery paths after restart or degraded state.

## Core workflows

### Watch live work

The sidebar is more than a project list. It groups repositories by activity, shows live session state for Claude, Codex, and Gemini, and lets you jump directly to tmux-backed sessions when work is already in motion.

The command-center layer also supports session launch, resume, stop, restart, and terminal focus behavior from inside the app, so taurhaus can supervise work and initiate it when needed.

![Sidebar live supervision](docs/screenshots/readme-sidebar-live-supervision.png)

### Recover context fast

When you come back to a project, taurhaus gives you a compact project memory surface: README preview, recent commits, task board state, session handoffs, relationship cues, and full-text search across documents, sessions, and commits.

That makes taurhaus useful even when you are not coordinating a team. It shortens the time between “what was happening here?” and “I can act again.”

| Task and history context | Cross-project search |
|---|---|
| ![Task board context](docs/screenshots/readme-task-board-context.png) | ![Search overlay](docs/screenshots/readme-search-overlay.png) |

### Inspect code and change history

taurhaus keeps project inspection close to runtime context. You can move from an active session or handoff summary into syntax-highlighted file browsing, commit history, and diffs without switching tools just to reconstruct the recent state of a repository.

![Git context inspection](docs/screenshots/readme-git-context-inspection.png)

## Mesh teams

Mesh is a first-class taurhaus workflow, not a side feature. The Mesh tab turns a multi-agent terminal ritual into a visible team lifecycle with prerequisites, setup, runtime control, and recovery.

What taurhaus covers in Mesh:

- role and preset-driven team composition
- one-click initialize flow with progress reporting
- runtime canvas with per-member state and detail actions
- hot-add, remove, and re-onboard actions for running teams
- resume flows for offline members and full team cold-restart recovery
- disband and cleanup behavior for managed team resources

### Compose and launch

The setup flow gives you a structured way to define a lead plus mixed-tool agents across projects, instead of manually assembling panes and hoping the coordination state stays coherent.

![Mesh setup composition](docs/screenshots/readme-mesh-setup-composition.png)

### Monitor the team at runtime

Once running, taurhaus shows the roster as an operational surface: runtime status, focus targets, node detail actions, and team controls are all in one view.

![Mesh runtime canvas](docs/screenshots/readme-mesh-runtime-canvas.png)

### Recover after restart or degraded state

The Mesh workflow also covers the non-ideal path. Taurhaus can detect recovery situations and surface resume affordances instead of treating a restart or stale runtime state as manual cleanup work.

![Mesh recovery and resume](docs/screenshots/readme-mesh-recovery-resume.png)

For deeper Mesh details, see:

- [Mesh view](docs/features/mesh.md)
- [Team templates](docs/team-templates.md)
- [Coordination architecture](docs/coordination-architecture.md)

## Install and prerequisites

For the full setup walkthrough and troubleshooting, use the [Getting Started guide](docs/getting-started.md).

### Supported user platforms

- **Windows**: primary release target, with CLI tools running inside WSL2
- **macOS**: native app + native daemon path

### Required tools

Before installing taurhaus, you need:

1. **`tmux`** on the environment where your AI CLIs run
2. **At least one supported AI CLI**
   - [Claude Code](https://docs.anthropic.com/en/docs/claude-code)
   - [Codex CLI](https://github.com/openai/codex)
   - [Gemini CLI](https://github.com/google-gemini/gemini-cli)
3. **Mesh CLI** if you want team orchestration
   - taurhaus can install or update the bundled Mesh binary when supported

### Windows prerequisites

On Windows, taurhaus expects the CLI environment to live in **WSL2**.

1. Install WSL2:
   ```powershell
   wsl --install
   ```
2. Install `tmux` inside WSL:
   ```bash
   sudo apt update && sudo apt install -y tmux
   ```
3. Enable mirrored networking in `%USERPROFILE%\.wslconfig`:
   ```ini
   [wsl2]
   networkingMode=mirrored
   ```
4. Restart WSL:
   ```powershell
   wsl --shutdown
   ```

Mirrored networking matters because the Windows app talks to the daemon running inside WSL over localhost.

### macOS prerequisites

On macOS:

1. Install Homebrew if needed:
   ```bash
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
   ```
2. Install `tmux`:
   ```bash
   brew install tmux
   ```
3. Install whichever AI CLIs you use

### Install taurhaus

- **Windows**: download `taurhaus_x.x.x_x64-setup.exe` from [Releases](../../releases) and run the installer.
- **macOS**: download the DMG from [Releases](../../releases), move taurhaus to Applications, and launch it.

On first launch, taurhaus installs or starts its daemon automatically, creates the managed tmux session it needs, and then opens the first-run flow.

> macOS note: if Gatekeeper blocks the app on first launch, right-click it in Applications, choose **Open**, and confirm once.

### Shell environment note

If your shell startup scripts prompt for input, they can block automated session launches. The most common case is `oh-my-zsh` update prompts. If you use interactive shell plugins, configure them to run non-interactively for headless session starts.

## First launch and quick start

### First launch

The first-run wizard walks through:

1. daemon install/update check
2. project discovery
3. project selection
4. registration progress
5. transition into the main shell

### Quick start

Once taurhaus is open:

1. Select a project in the sidebar to load its overview.
2. Launch or resume a CLI session from the project context menu.
3. Use the Tasks tab, README preview, and recent commits to recover context.
4. Press `Ctrl+K` / `Cmd+K` to search across projects.
5. Open the Mesh tab when you want a coordinated multi-agent workflow.

## Development

### Stack

| Layer | Technology |
|---|---|
| Desktop shell | Tauri 2 |
| Frontend | Svelte 5 + Tailwind v4 |
| Backend | Rust |
| Storage | SQLite + Tantivy + filesystem |
| Git integration | `git2` / libgit2 |
| Tests | Vitest + Cargo test + WebdriverIO |

### Workflow

Taurhaus uses **Bun-only** JavaScript workflows and `just` recipes for build, test, and release commands.

```bash
just dev              # full Tauri development mode
just check-quick      # fast implementation gate
just test             # full non-E2E test lane
just test-visual      # browser-mode visual screenshot lane
just build-windows    # native Windows installer build
just build-macos      # native macOS DMG build
just release          # create GitHub release from current version
```

Contributor notes:

- use `just check-quick` during implementation
- treat `just check` as the serialized full gate for release or team-lead validation
- run Vitest from the project root, not `src-tauri/`

Further reading:

- [CONTRIBUTING.md](CONTRIBUTING.md)
- [ARCHITECTURE.md](ARCHITECTURE.md)
- [docs/README.md](docs/README.md)
- [Build and release](docs/operations/build-and-release.md)
- [Testing guide](docs/operations/testing-guide.md)
- [Visual testing guide](docs/operations/visual-testing-guide.md)

## Architecture at a glance

Taurhaus is a native desktop app with a split runtime model:

- a Tauri application for UI, storage, git, and search
- a lightweight companion daemon for process scanning, file watching, and tmux/session orchestration
- platform-specific daemon placement: **WSL2 on Windows**, **native subprocess on macOS**

That architecture lets taurhaus stay close to the real local developer environment instead of wrapping work in a separate cloud control plane.

![System architecture](docs/images/system-architecture.jpg)

For deeper technical detail, see:

- [ARCHITECTURE.md](ARCHITECTURE.md)
- [IPC reference](docs/architecture/ipc-reference.md)
- [Daemon protocol](docs/architecture/daemon-protocol.md)
- [Data model](docs/architecture/data-model.md)

## Documentation

Key product and implementation references:

- [Getting Started](docs/getting-started.md)
- [Project management](docs/features/project-management.md)
- [Session management](docs/features/session-management.md)
- [Task board](docs/features/task-board.md)
- [Command center](docs/features/command-center.md)
- [Search](docs/features/search.md)
- [Mesh view](docs/features/mesh.md)
- [Documentation index](docs/README.md)

## License

MIT. See [LICENSE](LICENSE) for details.
