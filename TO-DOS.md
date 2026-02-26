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

- [x] **P05** — Update README.md: remove "Private — not yet open source" from License section, add license badge, review screenshots for accuracy
- [x] **P06** — Create CHANGELOG.md: version history from v0.1.0 through v0.3.2 (can be reconstructed from git log and BOOTSTRAP.md)
- [x] **P07** — Create CONTRIBUTING.md: dev environment setup, `just` recipes, code standards (Svelte 5 runes, Tailwind v4 tokens, Rust patterns), PR process, testing expectations
- [x] **P08** — Create SECURITY.md: how to report vulnerabilities, supported versions, response timeline
- [x] **P09** — Condense architecture for newcomers: write a concise ARCHITECTURE.md that summarizes the 22 ADRs into a readable overview (the full ADRs stay as deep-dive reference)

## App Polish — Overview Tab

The hero section anchors on session handoffs, but handoffs aren't created consistently enough to be useful. Rethink what the overview shows when there's no recent session.

- [x] **P10** — Redesign Overview tab: replace session-first hero with something more universally useful. Candidates: recent file changes, project stats (last commit, branch, file count), quick-actions (launch session, open terminal), or a condensed activity feed. Keep README rendering as a fallback. Design first, then implement.

## App Polish — Git View

The commit detail pane (right side) is a wall of text — file list + diff content runs together without enough visual structure.

- [x] **P11** — Improve Git commit detail pane: better visual separation between file list and diff content, collapsible file sections, syntax highlighting in diffs, change-type badges (added/modified/deleted). Take screenshot first → design → implement.

## Settings Expansion

Current settings are limited to activity thresholds and code themes. Users need control over terminal and session behavior — these are preference-driven and inconvenient when hardcoded.

- [x] **P12** — Terminal emulator preference: let users choose between Windows Terminal (current default), or a custom command. Store in settings DB, use in `terminal.rs` launch logic.
- [x] **P13** — Tmux pane layout preference: configurable strategy for how new CLI sessions are arranged. Options: (a) new window per session (current default), (b) horizontal split — fill panes in current window before creating new one, (c) per-project grouping — same project shares a window with splits, different projects get new windows. Store preference in settings, implement in daemon's tmux session creation.
- [x] **P14** — Reorganize settings UI: group into logical sections (General, Display, Terminal & Sessions, Search). Current flat layout won't scale with more settings.

## Infrastructure

- [ ] **P15** *(pending — needs CI infrastructure)* — GitHub Actions CI: run `just check` on push and PR. Needs WSL2 or Linux runner for Rust tests.
- [ ] **P16** *(pending — needs CI infrastructure)* — Release pipeline: tag-triggered builds, auto-generate NSIS installer, publish to GitHub Releases with changelog excerpt
- [x] **P17** — Clean up `.gitignore`: add `.env*`, `*.log`, ensure no secrets can accidentally be committed

---

# Beta Readiness — v1.1

Three topics that must be completed before beta distribution.

## Topic 1: Daemon Auto-Install via Wizard

Currently the WSL daemon must be pre-installed manually (`just install-daemon`). Beta users shouldn't need a dev environment. The Windows installer should handle daemon setup automatically.

**Approach**: Bundle the pre-built Linux daemon binary inside the Windows app resources. The FirstRunWizard detects if the daemon is missing/outdated and offers one-click installation into WSL.

- [x] **B01** — Design doc: daemon auto-install flow. Document: (a) how the daemon binary gets bundled in Tauri resources, (b) wizard step UX (detect → prompt → install → verify), (c) version mismatch handling (app update ships newer daemon), (d) error states (no WSL, permission denied, wrong distro). Write to `docs/daemon-install.md`.
- [x] **B02** — Bundle daemon binary in Tauri resources. Add Linux release binary to `tauri.conf.json` `bundle.resources`. Update `justfile` to build the daemon before the Windows build and copy it to the resources directory. Verify the binary appears in the installed app's resource folder.
- [x] **B03** — Add daemon detection IPC command. New Rust command `check_daemon_status` that: (a) checks if `~/.local/bin/taurhaus-daemon` exists in WSL, (b) if it exists, runs it with `--version` to get the version, (c) compares against the bundled version, (d) returns `{ installed: bool, version?: string, bundled_version: string, needs_update: bool }`.
- [x] **B04** — Add daemon install IPC command. New Rust command `install_daemon` that: (a) resolves the bundled binary path from Tauri resources, (b) copies it to WSL `~/.local/bin/taurhaus-daemon` via `wsl.exe`, (c) sets executable permissions (`chmod +x`), (d) returns success/error. Must handle: WSL not installed, permission errors, distro detection.
- [x] **B05** — Add daemon setup step to FirstRunWizard. New step between welcome and project scan: (a) calls `check_daemon_status`, (b) if daemon missing/outdated shows install prompt with explanation, (c) "Install" button calls `install_daemon` with progress feedback, (d) on success shows green checkmark and proceeds, (e) on failure shows error with manual install instructions as fallback. Skip step entirely if daemon already current.
- [x] **B06** — Add daemon update detection to app startup. Outside the wizard, on normal app launch: (a) check daemon version vs bundled version, (b) if outdated, show a non-blocking banner "Daemon update available" with one-click update button, (c) update uses same `install_daemon` command, (d) after update, restart daemon automatically.
- [x] **B07** — Test daemon install end-to-end. Uninstall daemon from WSL, run app, verify wizard offers install, install succeeds, daemon starts, app connects. Test version mismatch update flow. Test error paths (no WSL, wrong permissions). Add E2E regression test.

## Topic 2: macOS Port

taurhaus currently targets Windows (native exe) + WSL2 (daemon). macOS support requires: platform abstraction for `/proc`-dependent process detection, native daemon launching (no WSL), macOS terminal integration, and a `.dmg` build pipeline.

**Approach**: `PlatformProbe` trait with `LinuxProbe` and `DarwinProbe` implementations behind `#[cfg(target_os)]`. Daemon runs natively on macOS. Build and test on a remote macOS machine (Scaleway/MacStadium).

### Phase 1 — Platform Abstraction Design

- [x] **M01** — Write platform abstraction design doc. Map every `/proc` dependency to its macOS equivalent. Document: process CWD (`/proc/PID/cwd` → libproc), IO activity (`/proc/PID/io` → `proc_pid_rusage`), TCP sockets (`/proc/PID/net/tcp` → lsof/libproc), TTY detection (`/proc/PID/fd/0` → ttyname). Define `PlatformProbe` trait API. Write to `docs/platform-abstraction.md`.
- [x] **M02** — Create `platform` module structure. New `src-tauri/src/platform/` with: `mod.rs` (trait definitions + compile-time dispatch), `linux.rs` (existing `/proc` code extracted), `macos.rs` (stubs that return `None`/`Err`). Project compiles on both targets with stubs.

### Phase 2 — Extract Linux Code

- [x] **M03** — Extract process detection to `LinuxProbe`. Move `/proc/PID/cwd` readlink, `/proc/PID/fd/0` TTY detection, and `ps` parsing from `process.rs` into `platform/linux.rs`. `process.rs` calls `PlatformProbe` methods instead of reading `/proc` directly. All existing tests pass.
- [x] **M04** — Extract IO activity to `LinuxProbe`. Move `proc_io.rs` `read_rchar()` function into `platform/linux.rs`. `idle.rs` Claude activity detection calls the trait method. All existing tests pass.
- [x] **M05** — Extract TCP socket detection to `LinuxProbe`. Move `proc_io.rs` `has_api_connections()`, `collect_socket_inodes()`, `has_established_443()` into `platform/linux.rs`. Gemini idle detection calls the trait method. All existing tests pass.
- [ ] **M06** — Extract inotify handling to platform module. Move inotify-specific error detection and watch limit logic behind `#[cfg]`. macOS uses FSEvents (handled by `notify` crate already, but error handling differs).

### Phase 3 — macOS Implementation

- [ ] **M07** — Implement `DarwinProbe` process detection. Use `libproc` crate for: process CWD via `proc_pidpath()`, process list via `listpids()`. Fallback to `lsof -p PID | grep cwd` if libproc is insufficient. Write macOS-specific tests (gated with `#[cfg(target_os = "macos")]`).
- [ ] **M08** — Implement `DarwinProbe` IO activity detection. Use `libproc` `proc_pidinfo()` with `PROC_PIDTASKINFO` for IO counters. If raw byte counters aren't available, evaluate alternatives: `rusage`, CPU time deltas, or `dtrace`-based approach. Must detect Claude streaming activity reliably.
- [ ] **M09** — Implement `DarwinProbe` TCP socket detection. Use `lsof -p PID -i TCP -s TCP:ESTABLISHED` parsing or `libproc` `proc_pidinfo(PROC_PIDLISTFDS)` + `proc_pidfdinfo()`. Must detect Gemini's :443 connections.
- [ ] **M10** — Implement macOS daemon launcher. No WSL layer — daemon is a native binary. Modify `launcher.rs` with `#[cfg(target_os = "macos")]` path: spawn daemon directly, resolve home dir natively. Handle launchd integration if appropriate (daemon auto-start on login).
- [ ] **M11** — Implement macOS terminal integration. Replace Windows Terminal logic with macOS equivalents: Terminal.app via AppleScript (`osascript`), iTerm2 via its AppleScript API. Add terminal preference option for macOS (Terminal.app / iTerm2 / custom). Update Settings UI to show macOS-relevant options when running on macOS.
- [ ] **M12** — macOS icon and bundle configuration. Generate `.icns` icon file from existing logo PNGs. Update `tauri.conf.json` with macOS bundle settings (identifier, category, entitlements). Configure DMG installer appearance.

### Phase 4 — Build & Test on macOS

- [ ] **M13** — Set up remote macOS build environment. Provision a cloud Mac (Scaleway M1 or MacStadium). Install Rust toolchain, Node.js, Tauri CLI prerequisites. Clone repo, verify `cargo build` succeeds.
- [ ] **M14** — Build and run taurhaus on macOS. Build with `cargo tauri build`. Test: app launches, splash screen works, wizard completes, projects register, sidebar renders, tabs work. Fix any build/runtime issues.
- [ ] **M15** — Test process detection on macOS. Install Claude Code, Codex CLI, Gemini CLI on the Mac. Run sessions, verify taurhaus detects them: process appears in sidebar, activity state (active/idle) is correct, session files are found. Fix any detection issues.
- [ ] **M16** — Test terminal integration on macOS. Verify "Open terminal" and "Launch session" work with both Terminal.app and iTerm2. Verify tmux sessions attach correctly.
- [ ] **M17** — Universal binary build. Build universal binary with `cargo tauri build --target universal-apple-darwin` (ARM + Intel). Verify the DMG installer works on both architectures.
- [ ] **M18** — Run full test suite on macOS. Run `just check` equivalent on macOS. All Rust tests pass (with platform-gated tests). All frontend tests pass. Fix any platform-specific failures.

## Topic 3: README Positioning

Clarify what taurhaus is and isn't before beta distribution. Tabled until Topics 1-2 are complete — the macOS port may shift the positioning (e.g., no longer "Windows + WSL2 only").

- [ ] **B08** — Write clear positioning statement for README. Add "What this is / What this isn't" section. Key points: companion tool (not IDE replacement), context window for AI CLI workflows, not a code editor or CI dashboard, for developers using Claude Code / Codex / Gemini CLI. Reflect final platform support (Windows + macOS).

---

# Testing — Pre-Beta

Add unit tests for new P10-P14 code and build comprehensive E2E test suite with WebdriverIO + tauri-driver.

## Phase 1 — Unit Tests for New Code

- [x] **T01** — Write OverviewTab unit tests: quick actions, callbacks, last commit, session, relationships, dark mode (`src/lib/overviewTab.test.js`)
- [x] **T02** — Expand Settings unit tests for P12/P13/P14: terminal dropdown, tmux layout, custom command conditional, section structure (`src/lib/settings.test.js`)
- [x] **T03** — Verify GitTab tests cover P11 date grouping: date headers, author avatars, getDateLabel/authorInitial helpers (`src/lib/gitTab.test.js`)

## Phase 2 — E2E Tests (WebdriverIO + tauri-driver)

- [x] **T04** — E2E: Overview tab — project header, quick actions, last commit, relationships, README
- [x] **T05** — E2E: Git tab — commit list with date groups, commit detail, file list, diff view, back navigation
- [x] **T06** — E2E: Settings — all 4 sections, terminal/tmux dropdowns, rebuild index, escape/back
- [x] **T07** — E2E: Sidebar — project list, activity groups, project switching, filter, branch pills
- [x] **T08** — E2E: Search overlay — open/close, input, grouped results, keyboard navigation
- [x] **T09** — E2E: Files tab — file tree, expand/collapse, file viewer, README auto-select, syntax highlighting
- [x] **T10** — Run full test suite (unit + E2E) and commit
