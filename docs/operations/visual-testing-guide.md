# Visual Testing Guide

This guide defines the lightweight visual testing lane for taurhaus and its boundary with unit tests and full E2E.

## Test lane overview

taurhaus has three frontend test lanes:

| Lane | Command | Scope | Rendering model |
|---|---|---|---|
| JSDOM Vitest | `bun run test` | logic, state, DOM interaction, unit assertions | simulated DOM, no real browser |
| Vitest Browser Mode | `just test-visual` | component screenshots with mocked state | real browser rendering |
| README screenshot export | `just capture-readme-screenshots` | curated marketing/docs screenshots | real browser rendering |
| WDIO + `tauri-driver` | `just test-e2e`, `just test-e2e-full` | real integration and full workflows | real app + real backend |

Use the lightest lane that answers the question.

## When to use visual tests

Use Browser Mode visual tests when the goal is appearance with controlled mock state:

- dark/light theme verification
- layout regression checks such as spacing, overflow, truncation, or row wrapping
- component states that are awkward to prove with text-only DOM assertions
- screenshots of isolated surfaces such as `MeshCanvas`, `HoverCard`, `MeshNodeDetail`, or sidebar rows

Visual tests should be driven by named fixtures in `src/test/visual/fixtures/`.

## When NOT to use visual tests

Do not use visual tests for:

- real IPC integration
- cross-component workflows
- click-handler correctness or state-transition logic
- Tauri-specific behavior such as windowing, tray, or native shell integration
- performance measurement

If the test needs the real backend or real workflow wiring, it belongs in WDIO E2E. If it needs behavior assertions more than appearance, it belongs in normal Vitest tests.

## How to add a new visual test

1. Add or extend a named fixture in `src/test/visual/fixtures/`.
2. Follow the existing scenario pattern: `{state}_{variant}_{theme}`.
3. Add a browser-mode spec in `src/test/visual/specs/`.
4. Use the shared `renderVisual` helper from `src/test/visual/renderVisual.js`.
5. Lock the viewport and theme in the spec.
6. Run `just test-visual`.
7. Review generated screenshots under `src/test/visual/__screenshots__/{component}/{scenario}.png`.

Fixture examples already in the repo:

- `runtime_fiveAgents_dark`
- `active_claudeWorking_dirty_dark`
- `active_claude_selected_dark`
- `active_claude_light`

Rules:

- fixtures are pure named scenarios, not ad hoc inline mock blobs
- keep fixtures close to the component domain
- visual specs are additive and should not replace existing unit or E2E coverage

## How to use the visual fixture host

Run:

```bash
bun run dev:visual
```

This starts the Vite visual host for rapid manual review of mock states without launching the Tauri app.

Use the fixture host when:

- iterating on CSS or layout quickly
- comparing dark/light variants by eye
- validating a new mock scenario before writing or updating screenshot specs

Relevant files:

- `visual-host.html`
- `src/visual-host/VisualHost.svelte`
- `src/visual-host/registry.js`

The fixture host is for fast manual inspection. It does not replace automated screenshots.

## README screenshot workflow

Use:

```bash
just capture-readme-screenshots
```

This exports the curated screenshot set used by `README.md` from the dedicated visual host/spec path. It is narrower than `just test-visual`: the goal is reproducible documentation imagery, not broad component coverage.

Relevant files:

- `scripts/export-readme-screenshots.sh`
- `src/test/visual/specs/ReadmeScreenshotsHost.svelte`
- `src/test/visual/fixtures/readmeScreenshots.fixtures.js`

## Boundary rules

- Visual tests are for appearance, not behavior.
- Browser Mode visual tests use mocked IPC and mocked app state by design.
- JSDOM Vitest remains the default lane for logic and interaction assertions.
- WDIO E2E remains required for real Tauri integration and end-to-end workflow coverage.
- Do not move click-flow or state-transition tests into visual specs.
- Do not remove existing JSDOM or E2E tests just because a screenshot now exists.

## Quick decision rule

- “Does it look right with this exact mock state?” -> visual test
- “Do I need to refresh the README marketing screenshots?” -> `just capture-readme-screenshots`
- “Does the component behave correctly?” -> JSDOM Vitest
- “Does the real app work end-to-end?” -> WDIO E2E
