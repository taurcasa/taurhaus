# taurhaus

> The house where all your projects live.

A desktop tool that gives a single, clear view into all AI-driven projects — their code, docs, progress, and history — so you never lose context between sessions.

Built with Tauri 2, Svelte 5, and Rust.

## Screenshots

| Dark mode | Light mode |
|-----------|------------|
| ![Overview — Dark](e2e/screenshots/01-overview-dark.png) | ![Overview — Light](e2e/screenshots/02-overview-light.png) |
| ![Git — Dark](e2e/screenshots/09-git-dark.png) | ![Git — Light](e2e/screenshots/05-git-light.png) |
| ![Tasks — Dark](e2e/screenshots/08-tasks-dark.png) | ![Tasks — Light](e2e/screenshots/04-tasks-light.png) |

## Features

- **Project overview** — See all your projects at a glance with activity grouping (Active / Recent / Stale / Dormant)
- **File browser** — Browse project files with syntax-highlighted preview (VS Code grammars via Shiki)
- **Git integration** — Commit history, diffs, blame — all in-app via libgit2, no CLI dependency
- **Task board** — Aggregated tasks from Claude Code, Codex, and Gemini CLI
- **Session history** — Auto-imported session handoffs with commit and file change context
- **Multi-CLI session management** — Launch, stop, and navigate Claude Code, Codex, and Gemini CLI sessions from the sidebar
- **Live activity detection** — Real-time active/idle status for running CLI sessions
- **Full-text search** — Search across all project content (powered by tantivy)
- **Relationship mapping** — Auto-detected cross-project dependencies from Cargo.toml, CLAUDE.md, and session mentions

## System Requirements

| Requirement | Version |
|-------------|---------|
| Windows | 10 or 11 |
| WSL2 | Any distribution (Ubuntu recommended) |
| Windows Terminal | Latest from Microsoft Store |
| tmux | 3.0+ (installed in WSL) |

taurhaus runs as a native Windows application. It communicates with a lightweight daemon running inside WSL2 for session detection and process management.

### Optional (for CLI session management)

At least one of these AI CLI tools installed in WSL:

- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) — `npm install -g @anthropic-ai/claude-code`
- [Codex](https://github.com/openai/codex) — `npm install -g @openai/codex`
- [Gemini CLI](https://github.com/google-gemini/gemini-cli) — `npm install -g @anthropic-ai/gemini-cli`

## Installation

### Prerequisites

1. **Enable WSL2** if not already set up:
   ```
   wsl --install
   ```

2. **Enable mirrored networking** in WSL (required for daemon communication):

   Create or edit `%USERPROFILE%\.wslconfig`:
   ```ini
   [wsl2]
   networkingMode=mirrored
   ```
   Then restart WSL: `wsl --shutdown`

3. **Install tmux** inside WSL:
   ```bash
   sudo apt install tmux
   ```

### Install taurhaus

1. Download the latest `taurhaus_x.x.x_x64-setup.exe` from [Releases](../../releases)
2. Run the installer
3. Launch taurhaus — the first-run wizard will guide you through project discovery

The app automatically manages its WSL daemon. No manual daemon setup required.

For detailed setup instructions, see the [Getting Started Guide](docs/getting-started.md).

## Quick Start

1. **First run** — The wizard scans your project directories and registers them
2. **Browse** — Click any project in the sidebar to see its overview, files, tasks, and git history
3. **Launch a session** — Right-click a project and select a CLI tool to start a new session
4. **Navigate** — Click the tool indicator icons next to a project name to jump to a running session in Windows Terminal
5. **Search** — Press `Ctrl+K` to search across all projects

## Architecture

```
┌──────────────────────────────────────────────────┐
│  taurhaus.exe (Windows native, Tauri 2 + Svelte) │
│  ├── SQLite (metadata, sessions, relationships)  │
│  ├── tantivy (full-text search index)            │
│  └── libgit2 (in-process git operations)         │
└─────────────────┬────────────────────────────────┘
                  │ TCP (localhost:9000)
┌─────────────────▼────────────────────────────────┐
│  taurhaus-daemon (WSL2, Rust)                    │
│  ├── Process scanning (/proc)                    │
│  ├── Session file watching (notify + ignore)     │
│  ├── tmux session management                     │
│  └── Activity detection (IO, TCP, mtime)         │
└──────────────────────────────────────────────────┘
```

See [`docs/phase-4-architecture.md`](docs/phase-4-architecture.md) for the full architecture (22 ADRs).

## Development

### Stack

| Layer | Technology |
|-------|------------|
| Frontend | Svelte 5 + Tailwind v4 |
| Backend | Rust (Tauri 2) |
| Storage | SQLite + tantivy + filesystem |
| Git | libgit2 via `git2` crate |
| Build | Vite + cargo |
| Tests | Vitest + Cargo test + WebdriverIO |

### Build recipes

All builds use `just` (install via `cargo install just`).

```bash
just dev              # Full Tauri dev mode (hot-reload)
just dev-frontend     # Frontend only (no Rust backend)
just build-windows    # Windows release build (NSIS installer)
just check            # Quality gate: clippy + svelte-check + all tests
just test             # All tests (Rust + frontend)
```

See the [justfile](justfile) for all available recipes.

### Running tests

```bash
just test             # Everything
just test-rust        # Rust unit tests (576 tests)
just test-frontend    # Frontend tests (373 tests)
```

## Project Status

taurhaus is in active development. Core features are implemented and functional. See [BOOTSTRAP.md](BOOTSTRAP.md) for detailed phase status.

**Current focus**: Operationalization — setup guides, daemon reliability, cross-platform groundwork.

## License

Private — not yet open source.
