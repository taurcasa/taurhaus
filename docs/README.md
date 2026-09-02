# taurhaus Documentation

Documentation index for the taurhaus project — a desktop tool for AI project management built with Tauri 2 + Svelte 5 + Rust.

These references match the current shipped behavior. Start here for the docs that are most up to date.

## Quick Links

| I want to... | Go to |
|--------------|-------|
| Install and set up taurhaus | [Getting Started](getting-started.md) |
| Understand the system architecture | [ARCHITECTURE.md](../ARCHITECTURE.md) |
| Understand how taurhaus relates to the CLIs it runs | [Harness model](architecture/harness-model.md) |
| Read what the experiment taught | [Project retrospective](RETROSPECTIVE.md) |
| Understand Mesh runtime and recovery | [Mesh feature guide](features/mesh.md) |
| Understand current testing lanes | [Testing guide](operations/testing-guide.md) |
| Configure settings and preferences | [First run and settings](features/first-run-and-settings.md) |
| Learn the code standards and build recipes | [CLAUDE.md](../CLAUDE.md) |
| Contribute code | [CONTRIBUTING.md](../CONTRIBUTING.md) |
| Report a security issue | [SECURITY.md](../SECURITY.md) |
| See what changed recently | [CHANGELOG.md](../CHANGELOG.md) |

## Architecture

Deep dives into system design and technical decisions.

| Document | Description |
|----------|-------------|
| [ARCHITECTURE.md](../ARCHITECTURE.md) | System overview — storage, watcher ownership, IPC surface, platform abstraction, logging, and data flow |
| [Harness model](architecture/harness-model.md) | What taurhaus owns versus what the CLIs own: capability slices per harness, accounts and usage, app/daemon pairing, stability rules |
| [Data architecture](architecture/data-architecture.md) | Authoritative inventory of live stores, ownership boundaries, and derived state |
| [Data model](architecture/data-model.md) | SQLite schema, tantivy index, filesystem layout |
| [Path handling guide](architecture/path-handling-guide.md) | Root authority, normalization, and Windows/WSL/Linux path boundaries |
| [Compaction pipeline flow](architecture/compaction-pipeline-flow.md) | Hook and transcript reinjection paths, owner selection, per-harness delivery |
| [IPC reference](architecture/ipc-reference.md) | All Tauri IPC commands — parameters, return types, usage |
| [Daemon protocol](architecture/daemon-protocol.md) | TCP JSON-line protocol between app and WSL/macOS daemon |
| [Logging guidelines](architecture/log-level-guidelines.md) | Structured logging policy and event-level guidance |
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
| [Mesh view](features/mesh.md) | Multi-agent team setup, live roster, coordination, and recovery |
| [First run and settings](features/first-run-and-settings.md) | Onboarding wizard, scan/ignore settings, and application preferences |

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
| [Visual testing guide](operations/visual-testing-guide.md) | Browser-mode screenshot lane, `just visual-shot`, and fixture-host workflow |
| [Compaction testing](operations/compaction-testing.md) | How to trigger a real compaction per harness and verify delivery |
| [Infographic regeneration](operations/infographics.md) | `.env` setup and `just infographics` for regenerating the documentation images from the manifest prompts |

## Multi-CLI coordination (mesh)

Documentation for the multi-agent team orchestration feature.

| Document | Description |
|----------|-------------|
| [Coordination architecture](coordination-architecture.md) | Backend design, domain model, design decisions |
| [Mesh feature guide](features/mesh.md) | User-facing setup, runtime, recovery, and team actions |
| [Team delivery standard](team-delivery-standard.md) | Work-kind defaults, assignment contract, message conventions, and result artifacts |
| [Team templates guide](team-templates.md) | Role/preset templates, composition flow, history, diff, and revert |

## Security

| Document | Description |
|----------|-------------|
| [Security policy](../SECURITY.md) | Vulnerability reporting and security model |
| [Risk register](security/risk-register.md) | Accepted baseline risks |
| [Archive guide](archive/README.md) | Historical audits, design studies, and planning artifacts |

## Design plans

Current execution plans and their per-PR ledgers live under [docs/design](design/README.md); the raw agent research behind their facts tables is in [docs/design/research](design/research/).

## Archive

Historical audits, design explorations, migration notes, and superseded planning docs live under [docs/archive](archive/README.md).

## Documentation standards

See [GUIDELINES.md](GUIDELINES.md) for writing style, structure templates, and conventions.
