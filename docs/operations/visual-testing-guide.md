# Visual Testing Guide

This guide defines the lightweight visual testing lane for taurhaus and its boundary with unit tests and full E2E.

## Test lane overview

taurhaus has three frontend test lanes:

| Lane | Command | Scope | Rendering model |
|---|---|---|---|
| JSDOM Vitest | `bun run test` | logic, state, DOM interaction, unit assertions | simulated DOM, no real browser |
| Vitest Browser Mode | `just test-visual` | component screenshots with mocked state | real browser rendering |
| Window-size shot | `just visual-shot C S [V] [T]` | one fixture at a real window size | real browser at 1920/1366/1024 |
| README screenshot export | `just capture-readme-screenshots` | curated marketing/docs screenshots | real browser rendering |
| WDIO + `tauri-driver` | `just test-e2e`, `just test-e2e-full` | real integration and full workflows | real app + real backend |

`just visual-shot` is the lane for anything positioned against the *viewport* —
overlays, anchored menus, submenus. Browser Mode renders into a fixed 960×640
test page, so a popup that measures the window cannot be judged there. See
[`testing-guide.md`](./testing-guide.md#full-window-screenshots-just-visual-shot)
for flags, the port/pid rules, and where the PNGs land.

Use the lightest lane that answers the question.

## Which browser the lane launches

`just test-visual` does not hardcode a browser. `vitest.visual.config.js` asks
`scripts/visual-browser.mjs` for one, in this order:

1. `PLAYWRIGHT_CHROME_PATH`, when it is set. It is an override, not a hint: a
   path that does not exist fails the run immediately with an error naming the
   path, rather than falling through to another browser.
2. `/usr/bin/google-chrome`, when that file exists.
3. Playwright's managed Chromium — the revision the installed `playwright`
   package points at, else the newest `chromium-<revision>` actually present in
   the browser cache (`PLAYWRIGHT_BROWSERS_PATH`, else the platform default).
   Install one with `bunx playwright install chromium`.

The resolved path is passed to Playwright as `executablePath` and printed as
the run's first line — `[visual] browser: <path> (<source>)` — so the browser
reported is always the browser that launched. `scripts/visual-browser.test.mjs`
covers the order against a fake filesystem.

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

- `runtime_fiveAgents_dark` (`meshCanvas.fixtures.js`)
- `active_claudeWorking_dirty_dark` (`hoverCard.fixtures.js`)
- `active_claude_selected_dark` (`sidebar.fixtures.js`)
- `active_claude_light`, `idle_agy_dark`, `idle_grok_dark`, `cross_project_agy_light` (`meshNodeDetail.fixtures.js`)
- `account.fixtures.js` and `shellPopups.fixtures.js` cover the account chooser, chip menu and usage meters — the surfaces `just visual-shot` exists for; `grok-two-accounts-light` is the tool that shows identities with no usage meter at all

The fixture roster is a per-tool matrix, and it has to stay one: the mesh
builder cycles all four tools — `toolCycle = ['codex', 'agy', 'claude', 'grok']`
in `createAgentMembers` (`src/test/visual/fixtures/builders.js:71-74`, consumed
by `meshCanvas.fixtures.js` and `readmeScreenshots.fixtures.js`) — while
`sidebar.fixtures.js` uses no builder and maintains its own four-tool scenario,
`active_multiTool_dark`, session by session (`:225-250`). When a harness joins the registry, add
its scenario to the mesh canvas, mesh node detail, sidebar, roster and account
fixtures in the same PR — otherwise its surfaces are shot by no one.

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
