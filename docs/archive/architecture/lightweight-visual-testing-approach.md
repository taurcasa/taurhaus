# Lightweight Visual Testing Approach

Date: 2026-03-06  
Task: #403

## Problem

taurhaus currently has two frontend test modes:

- fast logic/unit tests in Vitest + JSDOM
- slow real-app verification in WebdriverIO + `tauri-driver`

That leaves a gap for visual regression work on components such as `MeshCanvas`, hover cards, sidebar indicators, and mesh detail panels. Those cases usually need:

- real browser rendering
- controlled mock data
- screenshots of a single component or a narrow UI surface
- no Tauri app boot, tmux side effects, or full end-to-end harness

## Current repo baseline

- Frontend tests already use `vitest` and `@testing-library/svelte`.
- The codebase already mocks IPC heavily with `vi.mock('./ipc.js', ...)`.
- Existing screenshot capture lives in the heavy WDIO layer under `e2e/specs/`.
- The app is plain Vite + Svelte 5, not SvelteKit.

That matters because the cheapest successful design is one that extends the current Vitest/Vite path instead of adding a second UI-platform stack.

## Evaluation criteria

1. Svelte 5 runes compatibility
2. Clean IPC mocking for Tauri commands
3. Real-browser screenshot quality
4. Setup and maintenance cost
5. Speed relative to full E2E
6. Ability to reproduce specific visual states
7. Fit with existing Vitest/WDIO infrastructure
8. WSL2/Linux developer fit

## Comparison table

| Option | Svelte 5 fit | IPC mocking | Screenshot quality | Setup cost | Speed | Mock-state reproduction | Infra fit | WSL2/Linux fit | Verdict |
|---|---|---|---|---|---|---|---|---|---|
| Vitest Browser Mode | Strong | Strong | Strong | Low | Fast | Strong | Strong | Strong | Best default |
| Playwright component testing | Strong, but CT is still experimental | Strong | Strong | Medium | Fast | Strong | Medium | Strong | Good fallback / future option |
| Storybook | Strong | Medium-Strong | Medium by itself, strong with Chromatic | Medium-High | Medium | Strong | Medium | Strong | Useful only if we also want a component catalog |
| Histoire | Risky today | Medium | Medium | Medium | Medium | Strong | Low | Likely fine technically, weak project fit | Do not adopt |
| Custom dev harness only | Strong | Strong | None by itself | Low-Medium | Fast for manual review | Strong | Strong | Strong | Useful as a companion, not the full solution |

## Recommendation

Adopt a two-part approach:

1. **Automated visual screenshots in Vitest Browser Mode**
2. **A tiny Vite-only visual fixture host for manual browsing of mock states**

Do not make Storybook or Histoire the default solution.

This keeps the architecture aligned with taurhaus:

- same test runner family already in use
- same `vi.mock`-based IPC strategy already in use
- real browser rendering for screenshots
- minimal new concepts for the team
- no duplication of the existing WDIO E2E role

## Why this is the right split

### Vitest Browser Mode is the lowest-friction path

Official Vitest docs support Browser Mode with browser providers and screenshot assertions. That gives taurhaus the missing layer between JSDOM logic tests and full Tauri E2E.

Why it fits this repo:

- It stays inside the existing Vitest workflow.
- It can render real Svelte components in a real browser instead of JSDOM-only layout.
- Existing IPC mocking patterns transfer directly.
- Screenshot baselines can live next to frontend tests instead of inside WDIO-only flows.
- It avoids introducing a separate story format, preview runtime, and addon ecosystem.

### A tiny fixture host solves the “configurable mock data” problem

Automated screenshots alone are not enough. Developers also need a quick way to inspect multiple visual states while iterating on CSS/layout.

The practical answer is a very small visual host page that:

- mounts one component at a time
- loads fixture variants from plain modules
- toggles light/dark mode, viewport, and named states
- runs entirely in Vite without booting the Tauri shell

This is much lighter than Storybook and is enough for taurhaus’s current needs.

## Option-by-option assessment

### 1. Vitest Browser Mode

**Assessment:** recommended

Strengths:

- Official Browser Mode support in Vitest 4 is current and active.
- Official screenshot assertions exist, including configurable screenshot paths.
- Browser providers include Playwright and WebdriverIO, so taurhaus can stay close to tools it already knows.
- Very low conceptual overhead for the team.

Weaknesses:

- Browser-mode tests are a test runner, not a browsable component catalog.
- Developers still need a small manual review surface for rapid design iteration.

Fit for taurhaus:

- Excellent for `MeshCanvas`, hover cards, sidebar badges, detail popovers, and empty/error/loading states.
- Easy to pair with pure fixture builders and IPC mocks.

### 2. Playwright component testing

**Assessment:** viable, but not the default first step

Strengths:

- Real browser rendering and mature screenshot assertions.
- Strong component mount model and debugging tools.
- Natural future path if taurhaus standardizes more of its frontend testing on Playwright.

Weaknesses:

- Official docs still label component testing as experimental.
- It introduces a second frontend test runner beside Vitest.
- taurhaus already has WDIO for E2E, so this would add another browser-testing lane instead of filling the existing Vitest gap.

Fit for taurhaus:

- Technically solid.
- Architecturally heavier than needed for the immediate problem.

### 3. Storybook

**Assessment:** good product, wrong default tradeoff

Strengths:

- Current official Storybook docs support Svelte with the `@storybook/svelte-vite` framework.
- Strong story-driven mock-state modeling.
- Good manual browsing and review UX.

Weaknesses:

- Storybook alone is mainly a component workshop, not an opinionated in-repo screenshot lane.
- Official visual testing flow is tied to Chromatic, which adds external service dependency and process overhead.
- Story maintenance becomes its own parallel UI system.

Fit for taurhaus:

- Worth reconsidering only if the team also wants a durable component catalog/design-system surface.
- Too much platform for the current problem.

### 4. Histoire

**Assessment:** reject for now

Strengths:

- Conceptually close to the desired lightweight story-browser model.
- Vite-native and simpler than Storybook in principle.

Weaknesses:

- Official docs still frame the Svelte guide around Svelte 3.
- Inference from current public project activity: Histoire appears materially less current than Storybook or Vitest for the Svelte 5 ecosystem.
- That makes it a poor foundation for a repo standard in March 2026.

Fit for taurhaus:

- The risk is not “can it render a component at all.”
- The risk is long-term tool confidence and Svelte 5 alignment.

### 5. Custom dev harness

**Assessment:** useful, but only as part of the recommended solution

Strengths:

- Smallest possible abstraction surface.
- Perfect control over mock data and fixture naming.
- No dependency on story ecosystems.

Weaknesses:

- Manual-only unless paired with a screenshot runner.
- Easy to let it sprawl into an ad hoc mini-Storybook unless disciplined.

Fit for taurhaus:

- Good as a thin manual review surface beside Vitest Browser Mode.
- Not sufficient as the only answer.

## Proposed architecture

### Core decision

Use:

- **Vitest Browser Mode** for automated screenshot regression tests
- **fixture modules** for named mock states
- **one thin visual host page** for manual inspection during development

Keep:

- **JSDOM Vitest** for logic and interaction unit tests
- **WDIO + `tauri-driver`** for real integration and end-to-end verification

### Suggested file layout

```text
src/test/visual/
  browser.setup.js
  renderVisual.js
  fixtures/
    meshCanvas.fixtures.js
    hoverCard.fixtures.js
    sidebar.fixtures.js
  specs/
    meshCanvas.visual.test.js
    hoverCard.visual.test.js
    sidebar.visual.test.js

src/visual-host/
  VisualHost.svelte
  registry.js

visual-host.html
src/visual-host.js
```

### Fixture design

Each fixture module should export named scenarios, not generic mock blobs. Example:

```js
export const meshCanvasScenarios = {
  setup_threeAgents_light: { ... },
  runtime_fiveAgents_dark: { ... },
  runtime_overflow_eightAgents_dark: { ... },
  empty_noAgents_light: { ... },
}
```

Rules:

- Fixtures should be pure data builders.
- Component-specific fixture files are better than a shared mega-fixture layer.
- Shared helpers are acceptable only for true duplication, such as common agent builders.

### IPC mocking strategy

Do not invent a second mocking architecture.

Reuse the repo’s existing frontend testing pattern:

- `vi.mock('./ipc.js', ...)` for direct module consumers
- `window.__TAURI_INTERNALS__` shims only when the component path depends on runtime Tauri detection

For visual tests, add one central helper that:

- resets mocks between cases
- applies light/dark theme
- normalizes viewport size
- waits for fonts and animation settle points before screenshot capture

### Screenshot conventions

Each screenshot test should lock:

- viewport
- theme
- fixture name
- animation state
- font readiness

Recommended naming:

```text
src/test/visual/__screenshots__/
  meshCanvas/runtime_fiveAgents_dark.png
  hoverCard/active_dirty_project_dark.png
  sidebar/project_selected_light.png
```

## Rough implementation plan

### Phase 1: establish the lane

1. Add a separate Vitest config for browser visual tests.
2. Enable Browser Mode with a Playwright provider.
3. Add a screenshot setup helper that waits for:
   - `document.fonts.ready`
   - next animation frame
   - a short deterministic settle point only when needed
4. Add one pilot spec for `MeshCanvas`.

### Phase 2: fixture discipline

1. Create `src/test/visual/fixtures/`.
2. Move mock-state creation into named scenario builders.
3. Keep fixtures close to the component domain.

### Phase 3: manual fixture host

1. Add a separate Vite entry page such as `visual-host.html`.
2. Render a simple selector for:
   - component
   - scenario
   - theme
   - viewport preset
3. Reuse the same fixture modules as the automated screenshot specs.

### Phase 4: expand coverage

Initial target list:

- `MeshCanvas`
- `HoverCard`
- `MeshNodeDetail`
- sidebar project row indicators
- agent detail popup / runtime detail surface

### Phase 5: document boundaries

Add a short testing-guide section:

- visual tests are for component appearance with mocked state
- WDIO E2E remains required for real Tauri integration and cross-surface flows
- do not use visual tests to replace workflow verification

## Example developer workflow

Example: verify a `MeshCanvas` line-routing fix.

1. Add or update a fixture:
   - `runtime_fiveAgents_dark`
   - `runtime_sixAgents_dark`
   - `runtime_eightAgents_dark`
2. Open the visual host in Vite and inspect those cases manually while adjusting layout.
3. Add or update browser screenshot tests for the same named scenarios.
4. Run the visual lane locally.
5. If the component looks correct, run the normal frontend tests.
6. If the change also affects real integration behavior, keep or add WDIO coverage separately.

This gives fast iteration for layout work without pretending that mock-state screenshots replace real-app verification.

## Why not collapse everything into Playwright or Storybook

### Why not Storybook-first

Storybook is strongest when the repo wants:

- a real component catalog
- design-system documentation
- broader non-engineer review workflows
- story-driven review as a first-class product process

taurhaus does not need that to solve the current problem. It needs a narrow, fast visual regression lane.

### Why not Playwright-first

Playwright component testing is credible, but it creates an extra frontend browser-test runtime while taurhaus already has:

- Vitest for frontend tests
- WDIO for end-to-end tests

That is more moving pieces than necessary for phase one.

## Risks and mitigations

### Risk: flaky screenshots from fonts or animation

Mitigation:

- wait for `document.fonts.ready`
- freeze or disable non-essential motion in screenshot mode
- keep viewport/theme deterministic
- generate baselines on a consistent Linux environment

### Risk: fixture sprawl

Mitigation:

- named scenarios only
- per-component fixture files
- no generic “kitchen sink” mock store

### Risk: visual lane starts replacing E2E

Mitigation:

- document the boundary explicitly
- require WDIO only for real integration, Tauri IPC wiring, and multi-surface workflows

## Final recommendation

Implement **Vitest Browser Mode plus a small shared visual fixture host**.

That is the best balance of:

- low setup cost
- high Svelte 5 confidence
- strong IPC mocking fit
- real-browser screenshot capture
- fast local iteration
- minimal architectural drift from the existing repo

Defer Storybook unless taurhaus later decides it wants a formal component catalog. Do not adopt Histoire as a project standard today. Keep Playwright component testing as a future option, not the first move.

## Sources

Official sources reviewed on 2026-03-06:

- Vitest Browser Mode: https://vitest.dev/guide/browser/
- Vitest visual regression testing: https://vitest.dev/guide/browser/visual-regression-testing
- Vitest browser config: https://vitest.dev/guide/browser/config
- Playwright component testing: https://playwright.dev/docs/test-components
- Playwright screenshot assertions: https://playwright.dev/docs/test-snapshots
- Storybook Svelte + Vite framework docs: https://storybook.js.org/docs/get-started/frameworks/svelte-vite
- Storybook visual testing docs: https://storybook.js.org/docs/writing-tests/visual-testing
- Histoire Svelte guide: https://histoire.dev/guide/svelte3/get-started.html

Inference called out explicitly:

- Histoire is a risky default for taurhaus because its public docs remain Svelte-3-oriented and its current ecosystem signal looks materially less active than Vitest, Playwright, and Storybook for Svelte 5 work.
