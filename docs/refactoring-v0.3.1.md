# Refactoring Plan — v0.3.1

Findings from comprehensive audit (code quality, security, performance, UI/UX, visual design).
Each issue becomes one task. Issues verified against actual codebase — false positives removed.

## Critical

### R01 — Hardcoded `/home/mstie/` daemon path
**Source:** Code quality + Security
**Files:** `src-tauri/src/daemon/launcher.rs:125,165`
**Problem:** Daemon binary path is hardcoded to `/home/mstie/.local/bin/taurhaus-daemon`. Breaks for any other user.
**Fix:** Resolve `$HOME` dynamically or use `~/.local/bin/taurhaus-daemon` expanded at runtime. The `dirs` crate is already a dependency — use `dirs::home_dir()`.
**AC:**
- Daemon binary path derived from `dirs::home_dir()` or `$HOME` env var
- No literal `/home/mstie/` remains in non-test Rust source
- Existing tests updated or new test confirms dynamic resolution
- `just check` passes

### R02 — Hardcoded paths in test code
**Source:** Code quality
**Files:** `src-tauri/src/claude_code/resolver.rs:178,297`, `src-tauri/src/session_scanner/process.rs:325,350`, `src-tauri/src/session_scanner/idle.rs:657,1197,1207,1217`
**Problem:** Test code uses `/home/mstie/...` paths. Won't break builds (they're in `#[test]`), but is a code smell and will confuse contributors.
**Fix:** Use tempdir paths or generic `/home/testuser/...` paths in tests.
**AC:**
- No `/home/mstie/` in any Rust test code (grep confirms zero matches outside `#[cfg(test)]` is already handled by R01, so this handles the test cases)
- All tests still pass

### R03 — A11y warnings (svelte-check)
**Source:** Tooling (svelte-check)
**Files:** `src/Shell.svelte:1090`, `src/lib/AddProjectModal.svelte:521`, `src/lib/FirstRunWizard.svelte:294,337`
**Problem:** 5 warnings — noninteractive elements with tabindex, click handlers without keyboard handlers, `<span>` missing ARIA role.
**Fix:**
- Shell.svelte:1090 — `tabindex` on non-interactive indicator span when `ind.interactive` is false. Only set tabindex when interactive.
- AddProjectModal.svelte:521 — chevron `<span>` with click. Change to `<button>` or add keyboard handler + role.
- FirstRunWizard.svelte:294 — same chevron pattern. Change to `<button>`.
- FirstRunWizard.svelte:337 — `<span>` with click on root entry chevron. Change to `<button>` + add role.
**AC:**
- `npx svelte-check` reports 0 warnings, 0 errors
- No `svelte-ignore a11y_*` pragmas used (fix properly, don't suppress)
- Interactive elements use `<button>` with proper keyboard handlers

## High

### R04 — Empty states too sparse at 1280x1440
**Source:** Visual review (Gemini — score 5/10)
**Files:** `src/lib/TaskBoard.svelte` (empty state), potentially other tabs
**Problem:** Tiny icon + two lines centered in massive panel. Looks broken at tall resolutions.
**Fix:** Larger icon (48-64px), larger text, subtle descriptive guidance. Consider a dashed-border area or helpful CTA.
**AC:**
- Empty state icon is at least 48px
- Primary message uses text-lg or larger
- Secondary message provides actionable guidance
- Visual review at 1280x1440 looks intentional, not broken
- Screenshot comparison (e2e spec) captures the improved state

### R05 — Light mode secondary text contrast
**Source:** Visual review (Gemini — scores 7.5-8/10)
**Files:** `src/app.css` (design tokens), `src/Shell.svelte` (derived tokens)
**Problem:** Secondary/tertiary text in light mode too faint. Timestamps, section headers, metadata. Fails WCAG AA (4.5:1 ratio).
**Fix:** Darken light-mode `textSecondary` and `textTertiary` derived tokens. Target at minimum `text-zinc-500` → `text-zinc-600` level.
**AC:**
- All secondary text in light mode meets WCAG AA contrast (4.5:1 against white)
- Verified visually in e2e screenshots (light mode)
- No changes to dark mode tokens (already good)

### R06 — Overview content needs max-width
**Source:** Visual review (Gemini — score 7.5/10)
**Files:** `src/Shell.svelte` (Overview tab section)
**Problem:** Overview text content stretches full panel width (~1000px). Uncomfortable reading at 1280px.
**Fix:** Add `max-w-3xl` or `max-w-4xl` to the overview content container. Keep it left-aligned.
**AC:**
- Overview tab content constrained to readable width (max ~768-896px)
- Content stays left-aligned (not centered)
- No horizontal scroll at 1280px width
- Other tabs (Files, Git, Tasks) unaffected — they use full width appropriately

### R07 — FILE_CACHE never evicts (memory growth)
**Source:** Performance review
**Files:** `src-tauri/src/session_scanner/idle.rs:245` (FILE_CACHE), `idle.rs:449` (CODEX_PATH_CACHE)
**Problem:** Both static `HashMap` caches grow unbounded. Projects removed or renamed leave stale entries forever.
**Fix:** Add periodic eviction — either a max-age sweep or a max-size LRU. The Codex cache has TTL-based freshness checks but never removes entries. FILE_CACHE has no eviction at all.
**AC:**
- Both caches have a max entry count or periodic sweep
- Stale entries (project no longer in active scan) get cleaned up
- Test that confirms eviction behavior
- No performance regression in poll cycle (eviction should be cheap)

## Medium

### R08 — Git tab layout proportions
**Source:** Visual review (Gemini)
**Files:** `src/Shell.svelte` (Git tab section)
**Problem:** At 1280px, commit detail (middle) is too wide for sparse content. Commit list (right) is too cramped.
**Fix:** Adjust the flex/width split. Currently the list is a fixed-width right panel. Consider making the detail pane narrower or the list wider.
**AC:**
- Git tab at 1280x1440 shows balanced panels
- Commit list has enough room for hash + message without excessive truncation
- Commit detail doesn't have excessive empty space
- Visual verification via e2e screenshot

### R09 — Sidebar branch pills contrast
**Source:** Visual review (Gemini — score 8/10)
**Files:** `src/Shell.svelte` (sidebar project rows)
**Problem:** Branch pills use dark gray/black background that clashes with teal sidebar. Too visually heavy — pulls attention from project names.
**Fix:** Use `bg-white/10` or `bg-white/8` for the branch pills. Reduce font weight or size slightly.
**AC:**
- Branch pills use translucent background that harmonizes with teal sidebar
- Project name remains the primary visual element in each row
- Pills still readable in both light and dark sidebar states
- Visual verification via e2e screenshot

### R10 — Distro parameter validation before wsl.exe
**Source:** Security review
**Files:** `src-tauri/src/daemon/launcher.rs`, `src-tauri/src/terminal.rs`
**Problem:** The `distro` parameter from settings is passed directly to `wsl.exe -d`. Could be used for command injection if settings are tampered with.
**Fix:** Validate distro name against a safe pattern (alphanumeric + hyphens + underscores only). Reject suspicious values.
**AC:**
- Distro name validated with regex `^[a-zA-Z0-9_-]+$` before use
- Invalid distro names return a clear error message
- Test for both valid and invalid distro names
- Existing valid distro names (e.g., "Ubuntu", "Ubuntu-22.04") still work

### R11 — SVG logo dedup
**Source:** Code quality
**Files:** `src/lib/sessionIndicator.js`, `src/lib/sessionStore.svelte.js`
**Problem:** Tool logo SVG path data duplicated across files.
**Fix:** Extract shared SVG data to a single source-of-truth file (e.g., `src/lib/toolLogos.js`).
**AC:**
- SVG path data defined in one file only
- Both `sessionIndicator.js` and `sessionStore.svelte.js` import from shared source
- No visual regression (logos render identically)
- Tests still pass

### R12 — Project switch race condition
**Source:** UI/UX review
**Files:** `src/Shell.svelte` (project selection + data loading)
**Problem:** Rapid project switching can show stale data from previous project's async loads.
**Fix:** Track a request generation counter or abort signal. Discard results from outdated requests.
**AC:**
- Rapid clicking between 3+ projects always shows correct project data
- No flash of wrong-project content
- Test or manual verification procedure documented

### R13 — Silent failure on project removal
**Source:** UI/UX review
**Files:** `src/Shell.svelte` (Remove button in Project info section)
**Problem:** "Remove" button in project info has no confirmation dialog. Destructive action happens immediately.
**Fix:** Add a confirmation step — either inline "Are you sure?" or a small modal.
**AC:**
- Remove button shows confirmation before proceeding
- Confirmation clearly states what will be removed
- Cancel option available
- Confirmed removal works as before

### R14 — `lru` crate advisory (cargo audit warning)
**Source:** cargo audit (RUSTSEC-2026-0002)
**Files:** `src-tauri/Cargo.toml` (transitive via tantivy)
**Problem:** `lru` 0.12.5 has unsound `IterMut`. Transitive dependency from tantivy.
**Fix:** Check if tantivy has released a version with an updated lru. If not, document as accepted risk.
**AC:**
- If tantivy update available with fixed lru: update Cargo.toml + verify build
- If not: add `docs/known-advisories.md` documenting the accepted risk
- `cargo audit` output reviewed and documented

## Dependencies

```
R01 → R02 (fix production paths first, then clean up tests)
R04, R05, R06, R08, R09 are independent visual fixes (can be done in any order)
R03 is independent (a11y fixes)
R07 is independent (cache eviction)
R10 is independent (security validation)
R11 is independent (SVG dedup)
R12, R13 are independent UI fixes
R14 is independent (dependency check)
```
