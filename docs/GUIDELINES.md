# Documentation Guidelines

Standards for writing and maintaining taurhaus documentation.

## Principles

1. **Current over comprehensive** — A short accurate doc beats a long outdated one. Update docs when you change the code they describe.
2. **Feature perspective, not code walkthrough** — Describe what the system does and why, not line-by-line code. Include code snippets only when they clarify a non-obvious pattern or API contract.
3. **One source of truth** — Each concept lives in one place. Cross-reference, don't duplicate. If something is already documented well in CLAUDE.md, link to it rather than restating.
4. **Scannable** — Headers, tables, and short paragraphs. Developers skim docs — make that work.

## Document Categories

| Category | Location | Purpose | Examples |
|----------|----------|---------|----------|
| **Project root** | `/*.md` | Entry points and contributor guides | README, ARCHITECTURE, CONTRIBUTING, SECURITY, CHANGELOG |
| **Architecture** | `docs/architecture/` | Technical deep dives into system design | Data model, IPC reference, daemon protocol |
| **Features** | `docs/features/` | Per-feature documentation (what it does, how it works) | Project management, git integration, search |
| **UI** | `docs/ui/` | Visual design system and layout documentation | Design tokens, layout structure |
| **Operations** | `docs/operations/` | Build, test, deploy, release procedures | Build guide, testing strategy |
| **Security** | `docs/security/` | Audit reports, risk register, security model | Audit reports, risk register |
| **Mesh** | `docs/` (top-level) | Multi-CLI coordination design docs (in-flight) | coordination-architecture, mesh-view-design |

## Document Structure Template

Every document should follow this structure. Sections are optional where marked — use judgment.

```markdown
# Title

One-paragraph summary of what this document covers and who it's for.

## Overview

What this subsystem/feature does at a high level. 2-3 paragraphs max.

## [Core sections — varies by category]

Architecture docs: design decisions, component diagram, data flow
Feature docs: capabilities, user-facing behavior, configuration
Operations docs: prerequisites, step-by-step procedures

## Key Files

| File | Purpose |
|------|---------|
| `path/to/file.rs` | Brief description |

## Related documents

- [Link to related doc](relative-path.md) — one-line description
```

## Writing Style

- **Tense**: Present tense ("The daemon listens on port 9000"), not future ("The daemon will listen").
- **Voice**: Direct and technical. No filler ("In order to", "It should be noted that"). Just state it.
- **Headings**: Sentence case ("Data model overview"), not title case ("Data Model Overview").
- **Code snippets**: Only when they clarify something non-obvious. Annotate with comments explaining *why*, not *what*. Never paste entire files.
- **Tables**: Prefer tables over bullet lists for structured data (commands, files, config options).
- **Links**: Use relative paths for internal links (`../architecture/data-model.md`). Absolute URLs for external only.

## File Naming

- Lowercase, hyphen-separated: `data-model.md`, `build-and-release.md`
- Feature docs match the feature name as users know it, not internal module names
- No version numbers in filenames (use git history)

## Keeping Docs Current

- When you change code that a doc describes, update the doc in the same commit
- Stale docs are worse than no docs — if you notice something outdated, fix it or flag it
- Audit reports and investigation docs (e.g., `e2e-performance-bug.md`) are point-in-time records — they don't need updating, but should be clearly dated

## Cross-referencing

- CLAUDE.md is the authoritative source for build recipes, code standards, and development workflow. Don't restate these — link to the relevant section.
- ARCHITECTURE.md is the entry point for technical contributors. Feature docs can assume the reader has seen it.
- Each feature doc should link to related architecture docs and vice versa.
