# taurhaus

> The house where all your projects live.

A desktop companion for developers running multiple AI coding sessions at once. See what Claude Code, Codex, and Gemini CLI are doing across all your projects — without tabbing through a dozen tmux panes to find out.

![taurhaus Overview](docs/screenshot-overview.png)

## Features

- **Live session dashboard** — Real-time active/idle status for all running CLI sessions, grouped by project activity
- **Multi-CLI management** — Launch, stop, and jump to Claude Code, Codex, and Gemini CLI sessions from the sidebar
- **File browser** — Browse and preview files with VS Code-grade syntax highlighting (Shiki)
- **Git integration** — Commit history, diffs, blame — all in-app via libgit2
- **Task board** — Aggregated tasks from all three CLI tools in one view
- **Full-text search** — Search across all project content with Ctrl+K
- **Session handoffs** — Auto-imported session summaries so you can pick up where you left off

| Git tab | Files tab |
|---------|-----------|
| ![Git](docs/screenshot-git.png) | ![Files](docs/screenshot-files.png) |

## Setup

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

**Install**: Download the DMG from [Releases](../../releases) — choose `universal` for any Mac, or `aarch64` (Apple Silicon) / `x64` (Intel) for a smaller download. Drag to Applications.

### First launch

1. The setup wizard scans your home directory for project folders and registers them
2. The daemon installs automatically — into WSL on Windows, or `~/.local/bin/` on macOS
3. Right-click any project in the sidebar to launch a CLI tool session

## Architecture

Tauri 2 desktop app — Svelte 5 frontend, Rust backend with SQLite, tantivy, and libgit2.

A lightweight companion daemon handles process scanning, file watching, and tmux management. On **Windows** it runs inside WSL2, on **macOS** it runs natively as a subprocess. Both communicate with the app over TCP using a JSON-line protocol.

![System Architecture](docs/system-architecture.jpg)

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full technical overview.

## Development

All builds use [`just`](https://github.com/casey/just) recipes:

```bash
just dev              # Full Tauri dev mode (hot-reload)
just build-windows    # Windows release build (NSIS installer)
just build-macos      # macOS release build (DMG)
just check            # Quality gate: clippy + svelte-check + all tests
just test             # All tests (Rust + frontend)
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

MIT — see [LICENSE](LICENSE) for details.
