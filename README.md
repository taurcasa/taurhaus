# taurhaus

> The house where all your projects live.

A desktop companion for developers who have far too many AI sessions running at once and have lost track of which project Claude is refactoring, which one Codex is "thinking" about, and whether Gemini has finished or just gone quiet.

If you've ever stared at six terminal tabs wondering "wait, did I already start that migration?" — this is for you.

![taurhaus Overview](docs/screenshot-overview.png)

### What this is

- A **side panel** that sits next to your terminal and tells you what all your AI tools are doing — so you don't have to tab through twelve tmux panes to find out
- A **context window** for the deeply unwise workflow of running Claude Code, Codex, and Gemini CLI across multiple projects simultaneously (we're not here to judge, we're here to help)
- A way to **see what's running**, jump between sessions, and pick up where you left off after your ambition briefly exceeded your working memory
- Built by someone with the same problem. You're among friends here.

### What this isn't

- Not an IDE — VS Code, Zed, Cursor, and friends are excellent at editing code. We just watch from the sidelines.
- Not a terminal emulator — you still type your commands where you've always typed them
- Not a wrapper that runs AI tools for you — the tools run in your terminal, we just keep an eye on them so you don't have to
- Not a productivity system that will make you more disciplined about how many things you start at once. If anything, it enables the problem.

## Features

- **Project overview** — All projects at a glance, grouped by activity (Active / Recent / Stale / Dormant)
- **File browser** — Browse and preview files with VS Code-grade syntax highlighting (Shiki)
- **Git integration** — Commit history, inline diffs, blame — all in-app via libgit2, no CLI dependency
- **Task board** — Aggregated tasks from Claude Code, Codex, and Gemini CLI in one view
- **Multi-CLI session management** — Launch, stop, and jump to Claude Code, Codex, and Gemini CLI sessions from the sidebar
- **Live activity detection** — Real-time active/idle status for running CLI sessions
- **Full-text search** — Search across all project content with Ctrl+K (powered by tantivy)
- **Session handoffs** — Auto-imported session summaries so you can pick up where you left off
- **Relationship mapping** — Auto-detected cross-project dependencies from Cargo.toml, CLAUDE.md, and session mentions

| Git tab | Files tab |
|---------|-----------|
| ![Git](docs/screenshot-git.png) | ![Files](docs/screenshot-files.png) |

## Architecture

taurhaus is a native desktop application built with Tauri 2 (Svelte 5 frontend + Rust backend with SQLite, tantivy, and libgit2).

![System Architecture](docs/system-architecture.jpg)

On **Windows**, a lightweight daemon inside WSL2 handles process scanning, file watching, and tmux session management. On **macOS**, the app inspects processes directly via libproc — no daemon needed.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full technical overview.

## Getting Started

### Requirements

**Windows**

| Requirement | Notes |
|-------------|-------|
| Windows 10/11 | Native desktop app |
| WSL2 | Any distribution (Ubuntu recommended) |
| Windows Terminal | Latest from Microsoft Store |
| tmux 3.0+ | Installed inside WSL |

WSL2 networking: Create or edit `%USERPROFILE%\.wslconfig` with `networkingMode=mirrored` under `[wsl2]`, then restart WSL (`wsl --shutdown`).

**macOS**

| Requirement | Notes |
|-------------|-------|
| macOS 10.15+ | Apple Silicon or Intel |
| tmux 3.0+ | `brew install tmux` |

At least one AI CLI tool installed: [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), or [Gemini CLI](https://github.com/google-gemini/gemini-cli).

### Install

1. Download the latest installer from [Releases](../../releases)
2. Run the installer — the app manages its own WSL daemon automatically
3. On first launch, the wizard scans your project directories and registers them

### Quick start

- **Browse** — Click any project in the sidebar to see its overview, files, tasks, and git history
- **Launch a session** — Right-click a project to start a CLI tool session
- **Navigate** — Click tool indicator icons next to a project name to jump to a running session
- **Search** — Press `Ctrl+K` to search across all projects

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
just check            # Quality gate: clippy + svelte-check + all tests
just test             # All tests (Rust + frontend)
# macOS: cargo tauri build (arm64) or --target universal-apple-darwin
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

MIT — see [LICENSE](LICENSE) for details.
