# Path Handling Audit — 2026-03-08

Task: `#717`
Owner: `architect`
Scope: Windows/WSL/Linux path flows, correctness, and maintainability

## Executive Summary

Taurhaus has one good path-normalization nucleus in `src-tauri/src/provider/path.rs`, but the overall system still does not have a single authoritative path policy. New features regularly re-implement path discovery, normalization, or platform bridging locally. That is the real reason the same bug class keeps recurring.

The highest-risk findings are:

1. High: path resolution is still fragmented across backend modules, frontend helpers, and standalone scripts, so correctness depends on each feature author remembering the same cross-platform rules.
2. High: the compaction analyzer's default log discovery can still select the wrong log file in mixed WSL/Windows setups because it infers the active app data root heuristically instead of consuming the same source of truth as the app.
3. High: Windows display scanning currently takes a daemon shortcut that bypasses local Codex compaction processing entirely, which shows how easy it is for a platform-specific path/view branch to skip critical logic.
4. Medium: multiple path identities are serialized for the same concept (`project_path`, `projectPath`, `cwd`), which is pragmatic for compatibility but increases round-trip ambiguity and comparison drift.
5. Medium: both backend and frontend duplicate project-path normalization rules. They are close, but they are still duplicated logic.

The most effective next step is not another one-off fix. It is a small platform-aware path authority layer that every feature must use for:
- app data root
- Claude/team state root
- project identity normalization
- Windows/WSL conversion
- active log discovery
- tool-specific session roots

## Phase 1: Inventory of Path Flows

| Flow | Origin | Transformations | Platform Boundary | Current Authority / Strategy | Key References |
|---|---|---|---|---|---|
| Project identity / matching | user-selected project path, session scanner output, team config | normalize slashes, trim trailing separators, convert `\\wsl$` / `\\wsl.localhost` to Linux, convert `D:\...` to `/mnt/d/...` | Windows <-> WSL | `normalize_project_path()` in backend; mirrored by frontend `normalizeProjectPath()` | `src-tauri/src/provider/path.rs`, `src/lib/pathUtils.js`, `src/lib/sessionStore.svelte.js`, `src/Shell.svelte` |
| WSL UNC <-> Linux conversion | Windows-facing or WSL-facing project paths | `wsl_unc_to_linux()`, `linux_to_wsl_unc()` | Windows <-> WSL | centralized in backend path provider; duplicated in frontend helper | `src-tauri/src/provider/path.rs`, `src/lib/pathUtils.js` |
| Windows drive <-> Linux mount conversion | Windows project paths, Windows app-data/tmux hook paths | `windows_drive_to_linux()`, `linux_mount_to_windows()` | Windows <-> WSL | centralized in backend path provider; duplicated in frontend helper | `src-tauri/src/provider/path.rs`, `src/lib/pathUtils.js` |
| App data root / JSONL log sink | Tauri `app_data_dir()` or `TAURHAUS_DATA_DIR` override | append `taurhaus.log.jsonl`, rotate siblings, create parent dirs | native OS path only, but consumed from WSL scripts too | backend logging module is canonical for writing | `src-tauri/src/commands/logging.rs` |
| Log analysis path discovery | CLI script runtime | optional `TAURHAUS_DATA_DIR`; otherwise heuristic candidate list across Linux/macOS/Windows | WSL reading Windows app data | heuristic selection by mtime/size | `scripts/analyze-compaction.py` |
| Team state root | `TAURHAUS_CLAUDE_DIR`, Windows mesh bridge lookup, or `$HOME/.claude/teams` | choose override, then `.join("teams")`; Windows fallback may resolve mesh-managed path | Windows <-> WSL possible | `default_teams_dir()` in coordination state | `src-tauri/src/coordination/state.rs` |
| Team config document paths | `teams/<team>/config.json` | tmp write + rename, compatibility serialization of path fields | same filesystem as teams dir | `TeamConfigStore` | `src-tauri/src/coordination/stores/config.rs` |
| Runtime member state paths | `teams/<team>/runtime/<member>.json` and related state subdirs | direct store read/write | same filesystem as teams dir | runtime/config stores | `src-tauri/src/coordination/stores/runtime.rs`, `src-tauri/src/coordination/stores/` |
| Compaction operational state | `teams/<team>/state/compaction/...` and operational snapshots | direct store read/write | same filesystem as teams dir | coordination stores | `src-tauri/src/coordination/stores/` |
| Claude hook installation | inferred Claude dir from `teams_dir.parent()` | write hook script in `~/.claude/hooks/`, patch `~/.claude/settings.json` | native OS path for Claude home | feature-local logic in hook bridge | `src-tauri/src/coordination/claude_hooks.rs` |
| tmux focus hook state file | `TAURHAUS_DATA_DIR` + focus filename | convert path to Linux shell path before embedding in tmux hook commands | Windows app path -> WSL/tmux shell path | session-scanner control path uses backend converter | `src-tauri/src/session_scanner/control.rs` |
| Session scanner display path | running app decides between daemon display view vs local scan | on Windows, may return daemon sessions early | Windows app <-> WSL daemon | feature-local branch in scanner | `src-tauri/src/session_scanner/mod.rs` |
| Runtime session view | runtime-only daemon path or local resolution | retains session metadata needed for reinjection / runtime bookkeeping | Windows app <-> WSL daemon | separate runtime scan path after `#703` | `src-tauri/src/session_scanner/mod.rs`, `src-tauri/src/daemon/handlers.rs`, `src-tauri/src/daemon/protocol.rs` |
| Codex session JSONL discovery | project path + `~/.codex/sessions/YYYY/MM/DD/` | scan recent date dirs, parse first line `session_meta.payload.cwd`, compare normalized path | Linux/WSL filesystem | tool-local session resolver | `src-tauri/src/session_scanner/idle/codex.rs`, `src-tauri/src/task_scanner/codex.rs` |
| Claude task/session discovery | project path + Claude project/session dirs under home | normalize project path and compare against live session or transcript cwd | Linux/WSL/native | tool-local scanner logic, reusing backend normalizer | `src-tauri/src/task_scanner/claude.rs` |
| Gemini task/session discovery | project path + home-based Gemini chats/TODO sources | path joins, filename/session derivation | native OS path | tool-local scanner logic | `src-tauri/src/task_scanner/gemini.rs` |
| Daemon binary resolution | current platform + optional WSL distro | native: `~/.local/bin/taurhaus-daemon`; Windows: resolve WSL `$HOME`, then append path | Windows -> WSL or native | daemon launcher owns it | `src-tauri/src/daemon/launcher.rs` |
| Mesh binary / teams-path resolution | runtime/process checks and mesh integration helpers | resolve current mesh binary, Windows mesh teams dir, pidfile paths | Windows <-> WSL possible | spread across coordination runtime + mesh CLI helpers | `src-tauri/src/coordination/runtime.rs`, `src-tauri/src/coordination/state.rs`, `src-tauri/src/coordination/mesh_cli.rs` |
| Frontend relative file rendering paths | rendered markdown/file path + relative asset path | resolve relative path by string path operations only | no OS crossing; project-root-relative only | frontend-only helper | `src/lib/pathUtils.js` |

## Phase 2: Correctness Audit

### High 1: Path policy is fragmented, not authoritative

Severity: High

The backend has a solid normalization module in `src-tauri/src/provider/path.rs`, but many path-sensitive flows still resolve their own roots or platform conversions locally:
- analyzer log discovery in `scripts/analyze-compaction.py`
- Claude hook install path logic in `src-tauri/src/coordination/claude_hooks.rs`
- daemon binary discovery in `src-tauri/src/daemon/launcher.rs`
- tmux focus hook path conversion in `src-tauri/src/session_scanner/control.rs`
- frontend project-path normalization in `src/lib/pathUtils.js`

This is not just duplication. It means new features can be locally correct and still systemically wrong because they consulted a different path authority.

Why this matters:
- bugs recur in the same class rather than the same file
- mixed Windows/WSL setups are especially vulnerable
- fixes do not automatically propagate across tools, scripts, and UI

Assessment:
- Current behavior is partially correct, but brittle.
- The risk is structural, not accidental.

### High 2: Analyzer default log discovery is still heuristic and can pick the wrong active root

Severity: High

`scripts/analyze-compaction.py` uses `resolve_default_log_path()` to choose from:
- `TAURHAUS_DATA_DIR` if present
- Linux default under `~/.local/share/com.taurhaus.dev/taurhaus.log.jsonl`
- macOS default under `~/Library/Application Support/com.taurhaus.dev/taurhaus.log.jsonl`
- Windows candidates under `/mnt/c/Users/*/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl`

That is better than a fixed hardcode, but it is still heuristic discovery, not consumption of the app's real active data root. In the real `#714` investigation, the script initially analyzed the stale Linux-side log instead of the active Windows AppData log.

Why this matters:
- it can produce a false negative audit
- it undermines trust in the analyzer during exactly the kind of cross-platform investigation it is meant to support

Assessment:
- Correct only when the freshest file heuristic happens to match the active app instance.
- Not authoritative enough for operational diagnostics.

### High 3: Windows display-session shortcut bypasses compaction processing

Severity: High

On Windows, `scan_sessions_for_display()` returns daemon display sessions early when available. That early return currently bypasses the local path where Codex compaction processing runs.

Observed consequence from `#714`:
- real Codex compactions existed in raw JSONL
- the app emitted no `compaction.detected` / `compaction.injected` / related events
- the selected scan path never executed `compaction::process_codex_compaction_events(&sessions)`

This is not purely a session-scanner bug. It is a path/view-boundary correctness bug: a Windows-specific path to data skipped logic that the local Linux/WSL path executes.

Assessment:
- Incorrect in current Windows behavior for compaction detection.
- Also a maintainability warning: platform branches are not consistently preserving feature semantics.

### Medium 4: Config serialization preserves multiple synonymous project path fields

Severity: Medium

`TeamConfigStore` writes both snake_case and camelCase path forms and also writes `cwd`:
- `project_path`
- `projectPath`
- `cwd`

This is understandable for compatibility with mesh and older consumers, but it increases ambiguity about which field is canonical.

Risks:
- drift between fields if future edits touch only one representation
- accidental comparison against the wrong field
- harder reasoning during migrations and debugging

Current state:
- The serializer currently writes them from one source, so immediate drift is limited.
- The maintainability cost is still real.

### Medium 5: Frontend and backend duplicate project-path normalization logic

Severity: Medium

The frontend `src/lib/pathUtils.js` and backend `src-tauri/src/provider/path.rs` both implement:
- WSL UNC conversion
- Windows drive conversion
- slash/trailing-separator normalization

They are intentionally aligned, but they are still separate implementations.

Risks:
- subtle semantic drift over time
- tests can pass on one side and fail on the other
- new edge-case fixes must be remembered in both places

This is especially relevant because frontend grouping/session matching relies on normalized project identity.

### Medium 6: Session resolution and comparison correctness varies by tool path

Severity: Medium

Claude and Codex task/session discovery are better than before because they normalize project paths before comparison. But the normalization boundary is not identical everywhere:
- `session_scanner/idle/codex.rs` still compares by trimmed trailing slash only in its `cwd` matcher instead of using the shared backend normalizer
- `task_scanner/codex.rs` does use `crate::provider::path::normalize_project_path()`
- `task_scanner/claude.rs` also uses the shared normalizer

This means one tool path has already partly diverged from the canonical policy.

Assessment:
- no confirmed production break from this specific divergence today
- but it is a real latent bug source, especially for Windows/UNC Codex session matching

### Low 7: Some path roots are inferred from neighboring directories instead of typed ownership

Severity: Low

Examples:
- Claude hook installation infers the Claude dir from `teams_dir.parent()`
- tmux focus path infers app-data root from `TAURHAUS_DATA_DIR`
- daemon bootstrap logging carries `log_path` separately rather than consulting a shared runtime paths object

These are reasonable shortcuts, but they make the code harder to audit because the ownership graph of paths is implicit.

### Round-trip / comparison observations

What is currently good:
- backend normalization handles both `\\wsl$` and `\\wsl.localhost`
- backend normalization handles Windows drive paths and `/mnt/<drive>` conversion
- frontend tests already cover key WSL and drive-path matching scenarios in `src/lib/sessionStore.test.js`
- task scanning for Claude/Codex mostly normalizes before comparing

What is still weak:
- no single golden test suite asserts the same path corpus across backend + frontend + scripts
- path comparisons remain mostly string-based after normalization rather than typed path identity
- case-sensitivity expectations are not explicitly documented for Windows-drive-origin project paths

### TOCTOU / race observations

No severe path-specific TOCTOU bug was confirmed in this audit, but there are two moderate correctness patterns worth noting:
- analyzer root selection depends on current file mtimes/sizes, so the chosen log can change based on runtime timing rather than explicit app identity
- session/path decisions that branch between daemon-returned data and local scanner data can silently change which downstream logic runs, as seen in the Windows compaction path

## Phase 3: Maintainability Recommendations

Ranked by impact first, then effort.

### 1. Create a single backend `PlatformPaths` authority

Impact: Very high
Effort: Medium

Introduce one authoritative backend module or struct that resolves and exposes:
- active app data root
- canonical log path
- canonical Claude dir / teams dir
- tmux focus file path
- tool session roots (`.codex`, Claude projects/tasks, Gemini chats)
- daemon binary path
- hook script/settings paths

Rule: feature code should ask this authority for roots, not reconstruct them.

This would eliminate the current pattern where every feature invents its own root-discovery logic.

### 2. Make scripts consume app path policy instead of guessing

Impact: High
Effort: Medium

The compaction analyzer should not infer the active log path from a candidate list when the app/runtime can expose the actual path.

Good options:
- add a tiny IPC/CLI command that prints current resolved data roots
- or emit the effective log path and teams root in one stable machine-readable runtime file

The goal is simple: operational scripts must consume the same authoritative roots as the app.

### 3. Separate typed path domains instead of passing raw strings everywhere

Impact: High
Effort: Medium to high

Not every path is the same thing. Distinguish at the type/API level between:
- project identity path
- display path for UI
- app data root
- Claude state root
- Windows path for interop
- Linux path for runtime/daemon/tmux

Even lightweight wrapper types or disciplined helper functions would reduce accidental misuse.

### 4. Remove local normalization drift by routing all project matching through one function

Impact: High
Effort: Low to medium

Concrete fix candidates:
- make `session_scanner/idle/codex.rs` use `crate::provider::path::normalize_project_path()` instead of local trailing-slash-only comparison
- audit remaining callers that still compare raw `cwd` / `project_path` strings after partial cleanup
- keep frontend normalization behavior covered by a shared fixture corpus copied from the backend rules

### 5. Rename and split path-sensitive data views more aggressively

Impact: Medium to high
Effort: Low

The recent runtime/display session confusion is a path/view problem as much as an API problem. Keep the split explicit:
- display-safe paths and display-safe sessions
- runtime-authoritative paths and runtime-authoritative sessions

If a Windows/daemon path is display-only, the name should say so. That lowers the chance of bypassing critical logic accidentally.

### 6. Add a cross-platform golden test corpus for path normalization

Impact: Medium
Effort: Low

Create one shared corpus of path cases covering:
- `\\wsl$\Distro\...`
- `\\wsl.localhost\Distro\...`
- `C:\...`, `D:\...`
- `/mnt/c/...`
- Linux native `/home/...`
- trailing slash variants
- repeated separators
- mixed slash input

Use that corpus to test:
- backend normalizer
- frontend normalizer
- any script helper that compares project identity

That is the fastest way to catch future drift.

### 7. Collapse config path aliases behind one documented canonical field

Impact: Medium
Effort: Low to medium

Keep compatibility fields if needed, but document one canonical internal source of truth for member project identity. Prefer writing compatibility aliases from that source only, never reading them as peers unless required for migration.

### 8. Document path-boundary rules for new feature work

Impact: Medium
Effort: Low

Add a short engineering note to architecture/contributing docs:
- when to use `provider/path.rs`
- when a path must be Linux-native
- when a path may remain Windows-native
- when a value is for identity comparison vs display only
- how scripts should discover active roots

This is cheaper than rediscovering the same constraints in every feature.

## Concrete Recommended Next Actions

1. Fix the current Windows compaction bypass by ensuring the Windows display-scan path still executes compaction processing and completion logging.
2. Replace analyzer heuristic log discovery with an app-authoritative resolved log path source.
3. Unify Codex session path comparison on `normalize_project_path()` everywhere.
4. Introduce a small backend `PlatformPaths` authority and migrate new path-sensitive features to it first.
5. Add a shared golden test corpus for frontend/backend normalization parity.

## Bottom Line

Taurhaus does not primarily have isolated path bugs. It has a path-governance problem.

The normalization primitives are mostly good. The system problem is that path discovery, path identity, and platform-boundary conversion are still owned by too many different modules. Until those responsibilities are centralized, every new Windows/WSL/Linux feature will continue to risk the same class of failure.
