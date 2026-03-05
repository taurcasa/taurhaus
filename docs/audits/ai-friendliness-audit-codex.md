# AI-Friendliness + Context Efficiency Audit (Codex)

**Date**: 2026-03-05
**Reviewer**: Codex (developer1, GPT-5.3)
**Scope**: Full taurhaus codebase post-refactoring sprint
**Perspective**: What makes this codebase easy for AI agents (Claude, Codex, Gemini) to navigate, understand, and modify safely?

---

## Executive Summary

- Biggest AI-efficiency drag is context volume concentrated in a few files (multiple 1k-2.5k LOC files in core paths).
- Entry-point clarity is decent for startup wiring, but expensive for real changes: common tasks need 1.5k-5.8k LOC loaded before safe edits.
- Predictability is hurt most by duplicated/parallel patterns (especially Mesh UI logic existing in multiple places, with only one path actually used).
- CLAUDE.md is mostly useful, but it is stale in at least one key flow and misses critical refactor-era routing guidance.

---

## 1. File Sizes + Split Guidance (Top 15)

| Rank | Lines | File | >800 | Split direction |
|---|---:|---|---|---|
| 1 | 2541 | src-tauri/src/coordination/pipelines.rs | YES | Split by pipeline stage around initialize/add/resume boundaries (`:38`, `:300`, `:461`), move shared helpers (`:1477+`) to dedicated module |
| 2 | 2440 | src-tauri/src/templates/storage.rs | YES | Split into `role_ops`, `preset_ops`, `git_ops`, `flush/state` (major public-method blocks start `:415`, `:653`, `:910`, `:1008`) |
| 3 | 2263 | src-tauri/src/coordination/orchestrator/tests.rs | YES | Split test suites by lifecycle/reconcile/delivery groups (e.g., around `:472`, `:910`, `:1450`) |
| 4 | 1872 | src/lib/components/MeshTab.svelte | YES | Keep markup only; move domain transforms (`:179-607`) and async handlers (`:609+`) to controller/util modules |
| 5 | 1607 | src/lib/ipc.test.js | YES | Split by IPC domain (`projects`, `git`, `files`, `sessions`, `templates`, `coordination`) from current monolith (`:39+` many describes) |
| 6 | 1291 | src-tauri/src/search/indexer.rs | YES | Split schema/open/index/update/test utilities (`:31`, `:253`, `:491`, `:658`) |
| 7 | 1289 | src/lib/shell.test.js | YES | Split by concern (`markdown`, `tabs`, `project selection`, `daemon`) from grouped describes (`:173`, `:354`, `:549`, `:941`) |
| 8 | 1249 | src-tauri/src/db/task_queries.rs | YES | Separate read queries, write/upsert, archival, decode helpers (`:56`, `:117`, `:328`, `:457`) |
| 9 | 1246 | src-tauri/src/coordination/runtime.rs | YES | Separate runtime trait/system runtime/tmux command executor/test runtime (`:26`, `:170`, `:696`, `:990`) |
| 10 | 1198 | src-tauri/src/git/commits.rs | YES | Split range query engine/cache/diff formatting/tests (`:186`, `:246`, `:507`, `:591`) |
| 11 | 1154 | src-tauri/src/session_scanner/mod.rs | YES | Move cache/hysteresis/scan orchestration into submodules (public scan entry at `:356`) |
| 12 | 1110 | src/Shell.svelte | YES | Extract event wiring + data loaders + markdown nav helpers (script ends `:820`, markup starts `:822`) |
| 13 | 1091 | src-tauri/src/templates/types.rs | YES | Split schema types from validation/resolution logic (`RoleTemplate impl :116`, `TeamPreset impl :370`) |
| 14 | 1071 | src-tauri/src/commands/coordination/tests.rs | YES | Split transport shape tests vs command-flow tests (`~:108-1000`) |
| 15 | 1041 | src-tauri/src/coordination/backend/bridged.rs | YES | Split binary/preflight checks (`:161+`) from backend impl (`:432+`) and tests (`:501+`) |

---

## 2. Entry-Point Clarity / Context Loading Cost

| Area | Typical first files | Files | Approx LOC |
|---|---|---:|---:|
| Add IPC command (existing domain) | commands/projects.rs, lib.rs:167, ipc/projects.js, ipc/index.js, consumer | 5 | ~2360 |
| Add IPC command (coordination) | commands/coordination.rs:47, coordination_types.rs, lib.rs:243+, ipc/coordination.js, MeshTab.svelte | 5 | ~3494 |
| Fix local watcher behavior | startup/mod.rs:108, startup/watchers.rs, fs/watcher.rs, event_processor.rs | 4 | ~1479 |
| Fix daemon watch bridge/reconnect | daemon_lifecycle.rs, daemon/event_listener.rs, daemon/server.rs, daemon/watch.rs + startup/event processor | 6 | ~2961 |
| Modify session detection | session_scanner/mod.rs:356, control.rs, process.rs, tmux.rs, daemon/session_activity.rs, commands/command_center.rs, sessionStore.svelte.js | 7 | ~3989 |
| Modify search indexing | search/indexer.rs, commands/search.rs, event_processor.rs, startup/search.rs, SearchOverlay.svelte | 5 | ~2282 |
| Modify task ingest/sync | task_scanner/{claude,codex,gemini}.rs, services/task_sync.rs, db/task_queries.rs, commands/tasks.rs, TaskBoard.svelte | 7 | ~4614 |
| Modify template composition/storage | templates/{storage,types,composition}.rs, commands/templates.rs, TemplateBrowserPanel, TeamCustomizerPanel | 6 | ~5758 |
| Modify Mesh runtime flow | MeshTab.svelte, ipc/coordination.js, commands/coordination.rs, coordination/{pipelines,runtime,orchestrator}.rs | 6 | ~7764 |
| Add top-level frontend feature/tab | Shell.svelte, ipc.js, shell.test.js | 3 | ~2400 |

---

## 3. Findings

### HIGH Impact

**H1. Over-concentrated core files create context-window bottlenecks.**
- Multiple mission-critical files are >1000 LOC (pipelines.rs, storage.rs, MeshTab.svelte, Shell.svelte).
- Agents must load very large mixed-responsibility files before making safe edits, increasing omission risk and stale-assumption risk.
- Fix: Enforce soft caps (~400-600 LOC for prod files, ~800 max for tests) and split by behavior seams.

**H2. Mesh UI logic exists in duplicate forms; only one path is active.**
- MeshTab.svelte contains large in-file domain logic (`buildTeamConfigFromPreset` at `:369`, `composeConfigFromPayload` at `:528`), while parallel controller/util implementations exist (`meshTabController.svelte.js:31`, `meshTabUtils.js:148`) and are not imported by MeshTab.svelte.
- High wrong-edit risk: agent can patch dead/unused path and believe fix is complete.
- Fix: Choose one architecture and delete the other.

**H3. IPC extension path is manually scattered across backend registration + frontend wrappers.**
- Adding a command requires synchronized edits across lib.rs (manual registration list), command module, frontend wrapper (ipc/*.js), and exports (ipc/index.js).
- Fix: Add a command checklist template and generation/lint check that fails if backend command exists without frontend wrapper (or vice versa).

**H4. Highest-change areas also have highest context cost.**
- Git history hotspots include lib.rs, MeshTab.svelte, pipelines.rs, commands/coordination.rs — these are also among largest files.
- Fix: Prioritize decomposition of hotspots first.

### MEDIUM Impact

**M5. Pattern consistency is mixed across modules and tests.**
- Some Rust modules use inline tests (search/indexer.rs:658, fs/watcher.rs:280) while others use external test modules (commands/coordination.rs:1020 → tests.rs).
- Fix: Define per-layer test placement convention and codify in CLAUDE.md.

**M6. Data-shape normalization is duplicated across layers and naming styles.**
- Frontend repeatedly normalizes camel/snake variants (MeshTab.svelte:223-243, :269-275, :747-753), while backend types carry aliases (templates/types.rs:40, :291).
- Fix: Standardize one wire casing at IPC boundary, keep aliases only in explicit compatibility adapters.

**M7. Some behavior is comment-dependent and relies on magic sentinels.**
- Special ID `"__claude_tasks__"` is relied on across files (daemon_lifecycle.rs:99-103, startup/watchers.rs:136, event_processor.rs:510), and watcher rebuild is acknowledged but not implemented (event_processor.rs:501).
- Fix: Introduce shared constants and typed enum for internal watch channels.

**M8. CLAUDE.md is mostly useful but partially stale.**
- References `MeshSetupForm` (CLAUDE.md:154) which doesn't exist.
- Describes `src/lib/ipc.js` as command implementation (CLAUDE.md:166) but it's now a 1-line re-export.
- Fix: Update with "first file to read by task" map and remove stale references.

### LOW Impact

**L9. Grep/search is generally good but weaker for generic basenames.**
- IPC names are globally unique and grep-friendly.
- 23 `mod.rs` files and duplicate "runtime/state/types" filenames raise navigation overhead.
- Fix: Add module-level README stubs in high-density folders for quick routing.

**L10. Svelte 5 rune usage is consistent — positive for predictability.**
- No legacy reactive syntax found. Style consistency lowers false assumptions.

---

## 4. Focused Recommendations (Ordered)

1. Decompose Mesh surface first: MeshTab.svelte + delete/merge duplicate controller/utils path.
2. Decompose coordination/pipelines.rs into stage modules + shared helpers.
3. Add IPC "scatter guardrail": command checklist + static validation for backend/frontend parity.
4. Standardize test placement conventions and document them in CLAUDE.md.
5. Refresh CLAUDE.md for current component map and IPC module layout.
6. Replace magic sentinels (`__claude_tasks__`) with typed constants/contracts.
