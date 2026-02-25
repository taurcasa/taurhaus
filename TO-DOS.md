# Refactoring Tasks — v0.3.1

Reference: [docs/refactoring-v0.3.1.md](docs/refactoring-v0.3.1.md)

## Critical

- [x] **R01** — Replace hardcoded `/home/mstie/` daemon path with dynamic `dirs::home_dir()` resolution (`src-tauri/src/daemon/launcher.rs:125,165`)
- [x] **R02** — Replace hardcoded `/home/mstie/` paths in test code with generic paths (`resolver.rs`, `process.rs`, `idle.rs` test functions)
- [x] **R03** — Fix 5 svelte-check a11y warnings: change `<span>` click handlers to `<button>`, fix conditional tabindex (`Shell.svelte:1090`, `AddProjectModal.svelte:521`, `FirstRunWizard.svelte:294,337`)

## High

- [x] **R04** — Improve empty states at 1280x1440: larger icons (48-64px), larger text, actionable guidance (`TaskBoard.svelte`)
- [ ] **R05** — Darken light-mode secondary text tokens to meet WCAG AA contrast (4.5:1) (`app.css`, `Shell.svelte` derived tokens)
- [ ] **R06** — Add max-width constraint to Overview tab content for comfortable reading (`Shell.svelte` Overview section)
- [ ] **R07** — Add eviction to FILE_CACHE and CODEX_PATH_CACHE to prevent unbounded memory growth (`idle.rs:245,449`)

## Medium

- [ ] **R08** — Rebalance Git tab panel proportions: wider commit list, narrower detail pane (`Shell.svelte` Git section)
- [ ] **R09** — Improve sidebar branch pill contrast: use `bg-white/10` instead of dark background (`Shell.svelte` sidebar)
- [ ] **R10** — Validate WSL distro parameter against safe pattern before passing to `wsl.exe` (`launcher.rs`, `terminal.rs`)
- [ ] **R11** — Extract duplicated SVG logo data to shared `src/lib/toolLogos.js` module (`sessionIndicator.js`, `sessionStore.svelte.js`)
- [ ] **R12** — Fix project switch race condition with request generation counter (`Shell.svelte`)
- [ ] **R13** — Add confirmation dialog before project removal (`Shell.svelte` Remove button)
- [ ] **R14** — Check for tantivy update with fixed `lru` crate, or document accepted risk
