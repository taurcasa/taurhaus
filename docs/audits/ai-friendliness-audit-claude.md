# AI-Friendliness & Context Efficiency Audit (Claude)

**Date**: 2026-03-05
**Reviewer**: Claude Opus 4.6 (team lead)
**Scope**: Full taurhaus codebase post-refactoring sprint
**Perspective**: What makes this codebase easy for AI agents (Claude, Codex, Gemini) to navigate, understand, and modify safely?

---

## 1. File Sizes & Context Windows

### Top 25 Largest Source Files

| Rank | Lines | File | Flag |
|------|-------|------|------|
| 1 | 2541 | `src-tauri/src/coordination/pipelines.rs` | OVER 800 |
| 2 | 2440 | `src-tauri/src/templates/storage.rs` | OVER 800 |
| 3 | 2263 | `src-tauri/src/coordination/orchestrator/tests.rs` | OVER 800 (test) |
| 4 | **1872** | **`src/lib/components/MeshTab.svelte`** | **OVER 800 — FAILED SPLIT** |
| 5 | 1607 | `src/lib/ipc.test.js` | OVER 800 (test) |
| 6 | 1291 | `src-tauri/src/search/indexer.rs` | OVER 800 |
| 7 | 1289 | `src/lib/shell.test.js` | OVER 800 (test) |
| 8 | 1249 | `src-tauri/src/db/task_queries.rs` | OVER 800 |
| 9 | 1246 | `src-tauri/src/coordination/runtime.rs` | OVER 800 |
| 10 | 1198 | `src-tauri/src/git/commits.rs` | OVER 800 |
| 11 | 1154 | `src-tauri/src/session_scanner/mod.rs` | OVER 800 |
| 12 | 1110 | `src/Shell.svelte` | OVER 800 |
| 13 | 1091 | `src-tauri/src/templates/types.rs` | OVER 800 |
| 14 | 1071 | `src-tauri/src/commands/coordination/tests.rs` | OVER 800 (test) |
| 15 | 1041 | `src-tauri/src/coordination/backend/bridged.rs` | OVER 800 |
| 16 | 1020 | `src-tauri/src/commands/coordination.rs` | OVER 800 |
| 17 | 1018 | `src-tauri/src/task_scanner/codex.rs` | OVER 800 |
| 18 | 1018 | `src-tauri/src/coordination/stores/config.rs` | OVER 800 |
| 19 | 985 | `src/lib/components/MeshTab.test.js` | OVER 800 (test) |
| 20 | 975 | `src-tauri/src/task_scanner/claude.rs` | OVER 800 |
| 21 | 974 | `src-tauri/src/daemon/server.rs` | OVER 800 |
| 22 | 973 | `src-tauri/src/coordination/orchestrator.rs` | OVER 800 |
| 23 | 874 | `src-tauri/src/commands/templates.rs` | OVER 800 |
| 24 | 873 | `src/lib/taskBoard.test.js` | OVER 800 (test) |
| 25 | 850 | `src-tauri/src/provider/daemon_client.rs` | OVER 800 |

**Summary**: 25 files over 800 lines. 7 are test files (acceptable — tests are long by nature). 18 are production code that an AI might need to hold in context for modification.

### HIGH: MeshTab.svelte — Failed Split (1872 LOC)

Task #203 claimed "MeshTab 1864→128 lines" but the file remains at 1872 LOC. New files were created alongside:
- `MeshSetupView.svelte` (170 LOC)
- `MeshRuntimeView.svelte` (249 LOC)
- `meshTabController.svelte.js` (699 LOC)
- `meshTabUtils.js` (531 LOC)

These 1649 lines of new code are **not imported by MeshTab.svelte**. MeshTab still imports all original components directly. The new files are dead code.

**Impact**: MeshTab remains the biggest frontend god-component. An AI working on mesh UI must load 1872 lines minimum, plus understand all the sub-components it imports.

**Fix**: Either complete the extraction (make MeshTab a thin shell importing MeshSetupView/MeshRuntimeView/controller) or remove the unused split files.

### MEDIUM: Backend Coordination Module (pipelines.rs 2541, runtime.rs 1246, orchestrator.rs 973)

The coordination module has 3 files over 900 LOC that are tightly coupled. An AI working on team initialization must understand all three.

**Split suggestions**:
- `pipelines.rs` (2541): Split by lifecycle phase — `pipeline_init.rs`, `pipeline_runtime.rs`, `pipeline_teardown.rs`
- `storage.rs` (2440): Split by concern — `storage_roles.rs`, `storage_presets.rs`, `storage_git.rs`

### LOW: Shell.svelte (1110 LOC)

Shell is the app's root layout and inherently touches many concerns. Difficult to split further without creating artificial boundaries. Acceptable given its role.

---

## 2. Entry Point Clarity

For each major system area, how many lines must an AI load to make changes?

| Area | Essential Files | Total LOC | Verdict |
|------|----------------|-----------|---------|
| File watcher | 3 backend + 2 frontend | ~1,439 | Good — under threshold |
| Git operations | 2 backend + 1 frontend | ~1,668 | Good — under threshold |
| Sidebar rendering | 6 files (mostly frontend) | ~1,100 | Good — under threshold |
| Adding IPC command | 3-4 files | ~200 | Excellent — minimal context |
| Adding component | 1-3 files | ~100 | Excellent — self-contained |
| Database queries | 2-3 files | ~300-1,400 | Good — domain-scoped |
| Session scanning | 7 backend + 2 frontend | ~4,666 | **Needs reading guide** |
| Template CRUD | 3 backend + 3 frontend | ~4,820 | **Needs reading guide** |
| Daemon communication | 8 files (all backend) | ~4,384 | **Needs reading guide** |
| Mesh coordination | 8+ backend + 3 frontend | ~14,088 | **Massive — needs architecture map** |

### HIGH: Mesh Coordination Context Cost (14,088 LOC)

An AI trying to understand the full mesh/coordination system would need to read ~14K lines across 11+ files. This is unrealistic for any model's context window.

**Recommended per-task reading guides** (to add to ARCHITECTURE.md):
- "Add a message type": domain.rs + requests.rs = 700 lines
- "Add a pipeline operation": pipelines.rs + orchestrator.rs = 3,500 lines
- "Understand team lifecycle": orchestrator.rs + runtime.rs = 2,200 lines

### MEDIUM: Session Scanning, Templates, Daemon (4,000-5,000 LOC each)

These areas are large but manageable if you know which files to read for which task. A reading guide in ARCHITECTURE.md would cut entry cost by 30-40%.

---

## 3. Pattern Consistency

### Excellent — No Significant Deviations Found

| Dimension | Status | Notes |
|-----------|--------|-------|
| Rust command handlers | Consistent | All use `#[tauri::command]` + `Result<T, String>` + `.sanitize_err()` |
| Svelte components | Consistent | All use `$props()` + `$derived(themeTokens(dark))`, no legacy patterns |
| IPC domain modules | Consistent | All 12 modules follow identical `invokeOrMock(command, args, mockFn)` pattern |
| Test files | Consistent | Vitest + @testing-library + vi.mock everywhere |
| Error handling | Consistent | Trait-based: `.sanitize_err()` for most, `.ipc()` for coordination (intentional upgrade) |

**Minor deviation**: `coordination.rs` uses `.ipc()` trait method vs `.sanitize_err()` elsewhere. This is an intentional upgrade (more type-safe) but creates a "which pattern do I follow?" question for an AI adding commands.

**Recommendation**: Document the `.ipc()` vs `.sanitize_err()` distinction in CLAUDE.md code standards, or migrate all commands to `.ipc()` for uniformity.

---

## 4. Self-Documenting vs Comment-Dependent

No significant issues found. The codebase relies on:
- Clear naming conventions (IPC commands match Rust function names)
- Type signatures that document contracts
- CLAUDE.md for conventions and patterns

**One concern**: The `consume-after-capture` pattern for Svelte 5 signals (documented in CLAUDE.md) is a subtle trap. Without reading CLAUDE.md, an AI would write the "wrong" pattern naturally. This is well-documented though.

---

## 5. CLAUDE.md Effectiveness

### What's Accurate (90%)
- Design paradigms, code standards, logging chain, layout dimensions
- Build recipes, E2E test instructions, release workflow
- Svelte 5 patterns, TDD workflow, quality gates
- Architecture summary (at high level)

### What's Stale
- **Line 154**: References `MeshSetupForm` → should be `MeshSetupView`
- **Architecture Summary**: Says "Team templates: ... `TemplateCatalog` -> `TeamComposer` -> `MeshSetupForm`" — this flow has changed

### What's Missing
- **New ipc/ modular structure**: `src/lib/ipc/` directory with 12 domain modules (the biggest refactor)
- **Context API**: `src/lib/context/ProjectContext.js`, `SessionContext.js`
- **Startup module**: `src-tauri/src/startup/` (bootstrap, daemon, search, watchers)
- **Controller pattern**: `meshTabController.svelte.js`, `templateBrowserController.svelte.js`, `templateHistoryController.svelte.js`
- **New service modules**: `src-tauri/src/services/task_query.rs`, `task_sync.rs`
- **Extracted modules**: `src-tauri/src/daemon_api.rs`, `project_provider.rs`

### Recommendation
Update the Architecture Summary and Key Files sections to reflect post-refactoring structure. Add a "Module Map" section showing the new ipc/, context/, startup/ directories.

---

## 6. Grep/Search Friendliness

### Good — No Issues Found

- IPC commands use unique snake_case names that match between backend and frontend
- Component names are unique and descriptive
- No ambiguous identifier reuse across contexts
- The `invokeOrMock` pattern makes IPC calls easy to trace

---

## 7. Change Locality

### Files to Touch per Common Task

| Task | Files | Scatter |
|------|-------|---------|
| New IPC command | 3-4 (handler + lib.rs registration + ipc wrapper + optional mock) | LOW |
| New Svelte component | 1-3 (component + test + parent import) | LOW |
| New tab | 5 (tab component + Shell.svelte integration + position state) | MEDIUM |
| New DB query | 2-3 (query module + optional model + call site) | LOW |
| New daemon RPC method | 2-4 (handler + dispatcher + protocol + optional IPC) | MEDIUM |
| New session detection tool | 4-6 (cli_tool enum + idle resolver + process scanner + control) | MEDIUM-HIGH |

**Overall**: Change locality is good for the most common operations (IPC, components, DB). Only specialized areas (session detection, daemon) require touching 4+ files.

---

## 8. Dead Code — From Incomplete Splits

### HIGH: Unused MeshTab Split Files (1649 LOC)

Files created by task #203 that are not imported anywhere:
- `src/lib/components/MeshSetupView.svelte` (170 LOC)
- `src/lib/components/MeshRuntimeView.svelte` (249 LOC)
- `src/lib/components/meshTabController.svelte.js` (699 LOC)
- `src/lib/components/meshTabUtils.js` (531 LOC)

**Impact**: Dead code confuses AI agents — they see these files, assume they're in use, and may try to modify them instead of the actual MeshTab.svelte. It also inflates the codebase size and search results.

**Fix**: Complete the MeshTab extraction or remove the unused files.

---

## Summary of Findings

| # | Finding | Impact | Category |
|---|---------|--------|----------|
| 1 | MeshTab.svelte split incomplete — 1872 LOC + 1649 LOC dead code | HIGH | File size / Dead code |
| 2 | Mesh coordination requires 14K LOC context load | HIGH | Entry point clarity |
| 3 | CLAUDE.md missing post-refactoring structure (ipc/, context/, startup/) | MEDIUM | Documentation |
| 4 | pipelines.rs (2541) and storage.rs (2440) are split candidates | MEDIUM | File size |
| 5 | Session scanning, templates, daemon need reading guides | MEDIUM | Entry point clarity |
| 6 | CLAUDE.md has stale MeshSetupForm reference | LOW | Documentation |
| 7 | .ipc() vs .sanitize_err() error trait inconsistency | LOW | Pattern consistency |

### What's Working Well
- **Pattern consistency is excellent** — near-zero deviations across all dimensions
- **IPC domain split worked perfectly** — 12 uniform modules, easy to navigate
- **TemplateBrowserPanel and TemplateHistoryPanel splits completed successfully**
- **Change locality is good** — most common tasks touch 3-4 files max
- **Search friendliness is excellent** — unique identifiers, no ambiguity
- **Context API (ProjectContext/SessionContext) reduces prop drilling effectively**
