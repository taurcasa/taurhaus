# taurhaus

> The house where all your projects live.

A desktop companion for developers who have far too many AI sessions running at once and have lost track of which project Claude is refactoring, which one Codex is "thinking" about, whether Gemini has finished, and when it is time to spin up a coordinated Mesh team.

If you've ever stared at six terminal tabs wondering "wait, did I already start that migration?" — this is for you.

![taurhaus Overview](docs/screenshot-overview.png)

### What this is

- A **side panel** that sits next to your terminal and tells you what all your AI tools are doing — so you don't have to tab through twelve tmux panes to find out
- A **context window** for the deeply unwise workflow of running Claude Code, Codex, and Gemini CLI across multiple projects simultaneously (we're not here to judge, we're here to help)
- A way to **see what's running**, jump between sessions, and pick up where you left off after your ambition briefly exceeded your working memory
- A **Mesh View** to initialize and run multi-agent teams across CLI tools, including live roster status and hot-add onboarding
- Built by someone with the same problem. You're among friends here.

### What this isn't

- Not an IDE — VS Code, Zed, Cursor, and friends are excellent at editing code. We just watch from the sidelines.
- Not a terminal emulator — you still type your commands where you've always typed them
- Not a wrapper that runs AI tools for you — the tools run in your terminal, we just keep an eye on them so you don't have to
- Not a productivity system that will make you more disciplined about how many things you start at once. If anything, it enables the problem.

## Features

- **Project overview** — All projects at a glance, grouped by activity (Active / Recent / Stale / Dormant)
- **Project onboarding** — Scan/register existing repos or create a brand-new git project (initialized on `main`) from the Add Project modal
- **File browser** — Browse and preview files with VS Code-grade syntax highlighting (Shiki)
- **Git integration** — Commit history and inline diffs — all in-app via libgit2, no CLI dependency
- **Task board** — Aggregated tasks from Claude Code, Codex, and Gemini CLI in one view
- **Multi-CLI session management** — Launch, stop, and jump to Claude Code, Codex, and Gemini CLI sessions from the sidebar
- **Mesh View (multi-agent coordination)** — Create teams, initialize agent sessions, track live team status, and hot-add/re-onboard members
- **Team templates** — Built-in role/preset catalog (including `codex-architect` and `standard-team`) with compose/apply flow
- **Live activity detection** — Real-time active/idle status for running CLI sessions
- **Full-text search** — Search across all project content with Ctrl+K (powered by tantivy)
- **Session handoffs** — Auto-imported session summaries so you can pick up where you left off
- **Relationship mapping** — Auto-detected cross-project dependencies from Cargo.toml, CLAUDE.md, and session mentions

| Git tab | Files tab |
|---------|-----------|
| ![Git](docs/screenshot-git.png) | ![Files](docs/screenshot-files.png) |

## Setup

> For a detailed walkthrough with troubleshooting, see the [Getting Started guide](docs/getting-started.md).

### Windows

**Prerequisites** — install these before running taurhaus:

1. **WSL2** with any Linux distribution (Ubuntu recommended):
   ```
   wsl --install
   ```

2. **tmux** inside WSL:
   ```bash
   sudo apt update && sudo apt install tmux
   ```

3. **WSL2 mirrored networking** — create or edit `%USERPROFILE%\.wslconfig`:
   ```ini
   [wsl2]
   networkingMode=mirrored
   ```
   Then restart WSL: `wsl --shutdown`

4. **At least one AI CLI tool** inside WSL:
   - [Claude Code](https://docs.anthropic.com/en/docs/claude-code): `curl -fsSL https://claude.ai/install.sh | bash`
   - [Codex](https://github.com/openai/codex): `npm install -g @openai/codex`
   - [Gemini CLI](https://github.com/google-gemini/gemini-cli): `npm install -g @google/gemini-cli`

5. **Mesh CLI** (required for Mesh View team orchestration):
   - The Mesh tab can install/update bundled Mesh automatically when supported
   - Manual check: `mesh --help`

**Install**: Download `taurhaus_x.x.x_x64-setup.exe` from [Releases](../../releases) and run the installer. The app bundles its own WSL daemon and installs it automatically on first launch.

### macOS

**Prerequisites**:

1. **Homebrew** (if not already installed):
   ```bash
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
   ```

2. **tmux**:
   ```bash
   brew install tmux
   ```

3. **At least one AI CLI tool**:
   - [Claude Code](https://docs.anthropic.com/en/docs/claude-code): `brew install --cask claude-code`
   - [Codex](https://github.com/openai/codex): `brew install --cask codex`
   - [Gemini CLI](https://github.com/google-gemini/gemini-cli): `npm install -g @google/gemini-cli`

4. **Mesh CLI** (required for Mesh View team orchestration):
   - The Mesh tab can install/update bundled Mesh automatically when supported
   - Manual check: `mesh --help`

**Install**: Download the DMG from [Releases](../../releases) — choose `universal` for any Mac, or `aarch64` (Apple Silicon) / `x64` (Intel) for a smaller download. Drag to Applications.

### First launch

1. The setup wizard scans your home directory for project folders and registers them
2. The daemon installs automatically — into WSL on Windows, or `~/.local/bin/` on macOS
3. Right-click any project in the sidebar to launch a CLI tool session

### Quick start

- **Browse** — Click any project in the sidebar to see its overview, files, tasks, and git history
- **Launch a session** — Right-click a project to start a CLI tool session
- **Start a team** — Open the Mesh tab to initialize a multi-agent team for the selected project
- **Navigate** — Click tool indicator icons next to a project name to jump to a running session
- **Search** — Press `Ctrl+K` to search across all projects

## Architecture

taurhaus is a native desktop application built with Tauri 2 (Svelte 5 frontend + Rust backend with SQLite, tantivy, and libgit2).

![System Architecture](docs/images/system-architecture.jpg)

A lightweight companion daemon handles process scanning, file watching, and tmux management. On **Windows** it runs inside WSL2, on **macOS** it runs natively as a subprocess. Both communicate with the app over TCP using a JSON-line protocol.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full technical overview.

## Development

| Layer | Technology |
|-------|------------|
| Frontend | Svelte 5 + Tailwind v4 |
| Backend | Rust (Tauri 2) |
| Storage | SQLite + tantivy + filesystem |
| Git | libgit2 via `git2` crate |
| Tests | Vitest + Cargo test + WebdriverIO |

All builds use [`just`](https://github.com/casey/just) recipes:

```bash
just dev              # Full Tauri dev mode (hot-reload)
just build-windows    # Windows release build (NSIS installer)
just build-macos      # macOS release build (DMG)
just check            # Quality gate: clippy + svelte-check + all tests
just test             # All tests (Rust + frontend)
just bump 0.4.0       # Bump version everywhere
just release          # Create GitHub Release with artifacts
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

MIT — see [LICENSE](LICENSE) for details.
