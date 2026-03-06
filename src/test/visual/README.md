# Visual Test Infrastructure

This directory contains the lightweight browser-mode screenshot lane described in
`docs/architecture/lightweight-visual-testing-approach.md`.

## Layout

- `fixtures/` — named visual states rendered by screenshot specs
- `specs/` — browser-mode Vitest specs
- `__screenshots__/` — generated PNG artifacts
- `renderVisual.js` — shared setup helper for viewport, theme, font settling, and IPC mock reset
- `ipcVisualMocks.js` — centralized visual-test IPC mock registry

## Usage

Run only the visual browser lane:

```bash
just test-visual
```

Run the manual fixture host in a plain Vite browser session:

```bash
bun run dev:visual
```

## Rules

- Keep fixtures as pure, named scenarios.
- Reset IPC mocks before every visual render via `renderVisual`.
- Lock viewport and theme inside the spec; do not rely on browser defaults.
- Use this lane for component appearance, not for full Tauri workflow verification.
