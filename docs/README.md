# taurhaus Documentation

Documentation index for the taurhaus project — a desktop tool for AI project management built with Tauri 2 + Svelte 5 + Rust.

## Quick Links

| I want to... | Go to |
|--------------|-------|
| Set up the project for development | [Getting Started](getting-started.md) |
| Understand the system architecture | [ARCHITECTURE.md](../ARCHITECTURE.md) |
| Learn the code standards and build recipes | [CLAUDE.md](../CLAUDE.md) |
| Contribute code | [CONTRIBUTING.md](../CONTRIBUTING.md) |
| Report a security issue | [SECURITY.md](../SECURITY.md) |
| See what changed recently | [CHANGELOG.md](../CHANGELOG.md) |

## Architecture

Deep dives into system design and technical decisions.

| Document | Description |
|----------|-------------|
| [ARCHITECTURE.md](../ARCHITECTURE.md) | System overview — storage, IPC, platform abstraction, data flow |
| [Data model](architecture/data-model.md) | SQLite schema, tantivy index, filesystem layout |
| [IPC reference](architecture/ipc-reference.md) | All Tauri IPC commands — parameters, return types, usage |
| [Daemon protocol](architecture/daemon-protocol.md) | TCP JSON-line protocol between app and WSL/macOS daemon |
| [Platform abstraction](platform-abstraction.md) | Linux/macOS/Windows dispatch layer |
| [File rendering pipeline](file-rendering-pipeline.md) | File classification, IPC, caching, and rendering |

## Features

Per-feature documentation describing capabilities and behavior.

| Document | Description |
|----------|-------------|
| [Project management](features/project-management.md) | Registration, sidebar, activity groups, relationships |
| [File browser](features/file-browser.md) | Directory tree, file viewer, syntax highlighting, assets |
| [Git integration](features/git-integration.md) | Commit history, diffs, blame, status, range filtering |
| [Search](features/search.md) | Full-text search across all projects via tantivy |
| [Session management](features/session-management.md) | CLI session detection, handoffs, activity tracking |
| [Task board](features/task-board.md) | Cross-tool task aggregation and kanban display |
| [Command center](features/command-center.md) | CLI tool launch, stop, resume, and terminal integration |
| [First run and settings](features/first-run-and-settings.md) | Onboarding wizard and application preferences |

## UI

Visual design system and layout documentation.

| Document | Description |
|----------|-------------|
| [Design system](ui/design-system.md) | Colors, typography, tokens, dark/light mode |
| [Layout and navigation](ui/layout-and-navigation.md) | Shell structure, sidebar, tabs, position memory |

## Operations

Build, test, and release procedures.

| Document | Description |
|----------|-------------|
| [Build and release](operations/build-and-release.md) | Platform builds (Windows, macOS, Linux), release workflow |
| [Testing guide](operations/testing-guide.md) | Test strategy, test lanes, E2E setup, regression policy |

## Multi-CLI coordination (mesh)

Design documents for the multi-agent team orchestration feature (in development).

| Document | Description |
|----------|-------------|
| [Coordination architecture](coordination-architecture.md) | Backend design, domain model, design decisions |
| [Mesh view design](mesh-view-design.md) | UI design for team management tab |
| [Mesh setup vision](mesh-setup-vision.md) | UX concept for team onboarding |

## Security

| Document | Description |
|----------|-------------|
| [Security policy](../SECURITY.md) | Vulnerability reporting and security model |
| [Risk register](security/risk-register.md) | Accepted baseline risks |
| [Audit 2026-02-27](security/audit-2026-02-27.md) | Full security audit v0.3.2 |
| [Audit 2026-03-03 (directed)](security/sec-auditor-audit-2026-03-03.md) | Directed audit — IPC, file paths, search |
| [Audit 2026-03-03 (TaurSec)](security/team-lead-audit-2026-03-03.md) | Framework-driven TaurSec v1 audit |

## Documentation standards

See [GUIDELINES.md](GUIDELINES.md) for writing style, structure templates, and conventions.
