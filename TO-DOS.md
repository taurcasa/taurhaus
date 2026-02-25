# Refactoring Tasks — v0.3.1

Reference: [docs/refactoring-v0.3.1.md](docs/refactoring-v0.3.1.md)

## Critical

- [x] **R01** — Replace hardcoded `/home/mstie/` daemon path with dynamic `dirs::home_dir()` resolution (`src-tauri/src/daemon/launcher.rs:125,165`)
- [x] **R02** — Replace hardcoded `/home/mstie/` paths in test code with generic paths (`resolver.rs`, `process.rs`, `idle.rs` test functions)
- [x] **R03** — Fix 5 svelte-check a11y warnings: change `<span>` click handlers to `<button>`, fix conditional tabindex (`Shell.svelte:1090`, `AddProjectModal.svelte:521`, `FirstRunWizard.svelte:294,337`)

## High

- [x] **R04** — Improve empty states at 1280x1440: larger icons (48-64px), larger text, actionable guidance (`TaskBoard.svelte`)
- [x] **R05** — Darken light-mode secondary text tokens to meet WCAG AA contrast (4.5:1) (`app.css`, `Shell.svelte` derived tokens)
- [x] **R06** — Add max-width constraint to Overview tab content for comfortable reading (`Shell.svelte` Overview section)
- [x] **R07** — Add eviction to FILE_CACHE and CODEX_PATH_CACHE to prevent unbounded memory growth (`idle.rs:245,449`)

## Medium

- [x] **R08** — Rebalance Git tab panel proportions: wider commit list, narrower detail pane (`Shell.svelte` Git section)
- [x] **R09** — Improve sidebar branch pill contrast: use `bg-white/10` instead of dark background (`Shell.svelte` sidebar)
- [x] **R10** — Validate WSL distro parameter against safe pattern before passing to `wsl.exe` (`launcher.rs`, `terminal.rs`)
- [x] **R11** — Extract duplicated SVG logo data to shared `src/lib/toolLogos.js` module (`sessionIndicator.js`, `sessionStore.svelte.js`)
- [x] **R12** — Fix project switch race condition with request generation counter (`Shell.svelte`) — already implemented via `_selectGeneration` counter
- [x] **R13** — Add confirmation dialog before project removal (`Shell.svelte` Remove button) — already implemented via two-click confirm in context menu
- [x] **R14** — Check for tantivy update with fixed `lru` crate, or document accepted risk

---

# Splash Screen & Logo — v0.3.2

Reference: [docs/splash-screen-design.md](docs/splash-screen-design.md)

Logo source: `docs/logo-candidates/candidate-01-keystone-gemini.jpg`

## Tasks

- [x] **S01** — Prepare logo image assets: crop/process candidate-01 to clean square PNG with transparency, generate sizes (22px, 32px, 48px, 128px, 256px, 1024px), create Windows ICO bundle (16/32/48/256)
- [x] **S02** — Create `SplashScreen.svelte` component: full-screen `bg-brand-950`, centered logo with CSS `clip-path: inset()` reveal animation (3 phases: foundation → walls → crown), "taurhaus" wordmark (Geist 18px), status text (12px, white/30), minimum 800ms display, `prefers-reduced-motion` support
- [x] **S03** — Integrate splash into boot sequence: modify `App.svelte` to show splash first, wire `daemon-status` Tauri events to drive animation phases, gate Shell rendering behind splash completion, error state with retry + "continue anyway" after 15s timeout
- [x] **S04** — Replace titlebar placeholder logo: swap the "t" square in `Shell.svelte` (line 531-533) with actual logo image at 22px
- [x] **S05** — Generate Windows app icons: update `src-tauri/icons/` with new logo PNGs and ICO, replace default Tauri icon in exe/installer
- [x] **S06** — Update design doc: finalize `docs/splash-screen-design.md` with actual implementation approach (clip-path raster reveal instead of SVG stroke animation)

---

# Pre-Publish Roadmap — v1.0

What needs to happen before taurhaus can be released publicly (open source or otherwise).

## Legal & Compliance

- [x] **P01** — Choose and add LICENSE file (MIT, Apache 2.0, or dual MIT/Apache — decide based on goals)
- [x] **P02** — Dependency license audit: verify all Cargo + npm deps are compatible with chosen license. `cargo-license` for Rust, `license-checker` for npm. Document any copyleft or problematic licenses.
- [x] **P03** — Formalize `cargo audit` suppressions: create `.cargo/audit.toml` with the 3 accepted advisories from `docs/known-advisories.md` (lru, git2, GTK3/glib)
- [x] **P04** — Run full security audit: `cargo audit` + `npm audit`, resolve or accept all findings

## Documentation

- [ ] **P05** — Update README.md: remove "Private — not yet open source" from License section, add license badge, review screenshots for accuracy
- [ ] **P06** — Create CHANGELOG.md: version history from v0.1.0 through v0.3.2 (can be reconstructed from git log and BOOTSTRAP.md)
- [ ] **P07** — Create CONTRIBUTING.md: dev environment setup, `just` recipes, code standards (Svelte 5 runes, Tailwind v4 tokens, Rust patterns), PR process, testing expectations
- [ ] **P08** — Create SECURITY.md: how to report vulnerabilities, supported versions, response timeline
- [ ] **P09** — Condense architecture for newcomers: write a concise ARCHITECTURE.md that summarizes the 22 ADRs into a readable overview (the full ADRs stay as deep-dive reference)

## App Polish — Overview Tab

The hero section anchors on session handoffs, but handoffs aren't created consistently enough to be useful. Rethink what the overview shows when there's no recent session.

- [ ] **P10** — Redesign Overview tab: replace session-first hero with something more universally useful. Candidates: recent file changes, project stats (last commit, branch, file count), quick-actions (launch session, open terminal), or a condensed activity feed. Keep README rendering as a fallback. Design first, then implement.

## App Polish — Git View

The commit detail pane (right side) is a wall of text — file list + diff content runs together without enough visual structure.

- [ ] **P11** — Improve Git commit detail pane: better visual separation between file list and diff content, collapsible file sections, syntax highlighting in diffs, change-type badges (added/modified/deleted). Take screenshot first → design → implement.

## Settings Expansion

Current settings are limited to activity thresholds and code themes. Users need control over terminal and session behavior — these are preference-driven and inconvenient when hardcoded.

- [ ] **P12** — Terminal emulator preference: let users choose between Windows Terminal (current default), or a custom command. Store in settings DB, use in `terminal.rs` launch logic.
- [ ] **P13** — Tmux pane layout preference: configurable strategy for how new CLI sessions are arranged. Options: (a) new window per session (current default), (b) horizontal split — fill panes in current window before creating new one, (c) per-project grouping — same project shares a window with splits, different projects get new windows. Store preference in settings, implement in daemon's tmux session creation.
- [ ] **P14** — Reorganize settings UI: group into logical sections (General, Display, Terminal & Sessions, Search). Current flat layout won't scale with more settings.

## Infrastructure

- [ ] **P15** — GitHub Actions CI: run `just check` on push and PR. Needs WSL2 or Linux runner for Rust tests.
- [ ] **P16** — Release pipeline: tag-triggered builds, auto-generate NSIS installer, publish to GitHub Releases with changelog excerpt
- [ ] **P17** — Clean up `.gitignore`: add `.env*`, `*.log`, ensure no secrets can accidentally be committed
