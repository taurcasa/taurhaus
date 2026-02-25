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
