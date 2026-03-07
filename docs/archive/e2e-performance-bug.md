# E2E Performance Bug: WebDriver operations 73% slower in full suite vs subset

## Findings (2026-02-28)

The most likely root cause is **WebdriverIO command logging**, not Mocha suite registration or Tauri app state.

- WDIO 9 defaults `logLevel` to `info`.
- In that mode, every WebDriver call emits `COMMAND`, `DATA`, and `RESULT` log lines from `@wdio/utils`.
- This suite now uses many `browser.execute()` calls, which means WDIO also serializes and prints large script payloads and sometimes large DOM/text results.
- In the 14-spec persistent run, that produces thousands of synchronous writes on the same Node worker that is issuing WebDriver commands.
- The slowdown appearing from the first test fits this: the logging path is active immediately, while app-state degradation would usually accumulate over time.

I confirmed the default locally by inspecting `node_modules/@wdio/config/build/index.js`, which sets `logLevel: "info"`, and `node_modules/@wdio/utils/build/index.js`, which logs `COMMAND` and `RESULT` for each command.

## Resolution applied

`e2e/wdio.conf.js` now sets:

```javascript
const wdioLogLevel = process.env.E2E_WDIO_LOG_LEVEL || 'error'
// ...
logLevel: wdioLogLevel
```

That keeps verbose command logging available when explicitly needed:

```bash
E2E_WDIO_LOG_LEVEL=info E2E_SKIP_BUILD=1 bunx wdio run e2e/wdio.conf.js
```

## Notes

- Short timing spot-checks were noisy, so I do **not** have a clean before/after percentage for the whole suite yet.
- Even so, this change removes a clear per-command overhead source that was definitely active in the problematic configuration.
- If the suite is still materially slower after this, the next thing to measure is `tauri-driver` / WebKitWebDriver command latency with WDIO logging disabled.

## Problem

Running the full E2E suite (14 specs, 137 tests) in persistent mode takes **~4 minutes**, with individual WebDriver operations averaging **164.6ms**. Running the same specs in a smaller persistent session (3 specs, 32 tests) averages **95.2ms per operation** — 73% faster.

The slowness is present **from the very first test** in the full suite. It is not progressive degradation over time. The same spec (e.g. `overview-interactions.js`) is slow when it runs as part of the 14-spec session, but fast when it runs in a 3-spec session.

## Measured data

| Run | Specs | Tests | Wall time | Avg op | Slow ops (>=100ms) | WebDriver % of wall |
|-----|-------|-------|-----------|--------|---------------------|---------------------|
| Full suite | 14 | 137 | 252.8s | **164.6ms** | **82%** | 91% |
| 3-spec subset | 3 | 32 | 37.9s | **95.2ms** | **25%** | 66% |

Both runs use identical configuration — same `[[...]]` grouped sub-array (persistent mode), same capabilities, same `waitforTimeout`/`waitforInterval`, same hooks.

## Configuration

- **wdio**: `specs: [[...all spec files...]]` — single sub-array = single worker session = persistent app mode
- **Driver chain**: WebdriverIO → tauri-driver → WebKitWebDriver (Linux/WebKit2GTK)
- **App**: Tauri 2 + Svelte 5 + Rust (debug build)
- **Platform**: Linux (WSL2), kernel 6.6.87.2

### Full suite wdio.conf.js (simplified)

```javascript
specs: [[
  'daemon-integration.js',     // 14 spec files grouped in
  'overview-interactions.js',   // a single sub-array for
  'git-workflow.js',            // persistent mode
  // ... 11 more ...
]],
```

### 3-spec comparison config (same structure, fewer files)

```javascript
specs: [[
  'overview-interactions.js',
  'git-workflow.js',
  'cross-tab-navigation.js',
]],
```

## What we've already tried (and ruled out)

These optimizations reduced total time from 6:27 → 3:55 but did NOT close the per-operation gap between full and subset runs:

1. **Replaced XPath selectors with CSS testid selectors** — `$('button=Overview')` (~518ms) → `$('[data-testid="tab-overview"]')` (~23ms). Eliminated all text-based selectors.

2. **Replaced most `$()` calls with `browser.execute()`** — all helper functions (`navigation.js`, `modal.js`, `settings.js`, `search.js`) now use `browser.execute(() => document.querySelector(...))` for condition checks. ~3ms per call instead of ~125ms.

3. **Replaced `elementClick` with in-page clicks** — added `clickTestId(testid)` / `fastClick(selector)` helpers that do `browser.execute((id) => document.querySelector(...).click(), id)`. Eliminated ~93 WebDriver `elementClick` calls.

4. **Optimized `resetAppState()`** — collapsed 15+ sequential WebDriver round-trips into 1 `browser.execute()` call.

5. **Centralized timing constants** — all poll intervals (50-150ms), timeouts (1.5-45s), and pause values in `e2e/helpers/timing.js`.

6. **Set global `waitforInterval: 50`** in wdio config.

## Key observation

**82% of operations take >=100ms in the full suite vs 25% in the 3-spec run.** These are the same types of operations — `executeScript`, `findElement`, `findElements` — running against the same app build. The ONLY difference is how many spec files are in the `specs` array.

## Operation breakdown (full suite)

```
fast(<20ms):    115 ops,   0.7s total
medium(20-100ms): 140 ops,   8.2s total
slow(>=100ms): 1143 ops, 221.2s total  ← 96% of WebDriver time
```

## Hypotheses to investigate

1. **wdio spec loading overhead**: Does wdio do something at session init proportional to the number of specs loaded? Pre-parsing, creating mocha suites, setting up reporters for all specs at once?

2. **Mocha suite registration overhead**: All 14 spec files are `import`ed and their `describe()` blocks registered before any test runs. Could 14 describe trees cause overhead on every WebDriver call? (Mocha hooks, event listeners, etc.)

3. **WebdriverIO verbose logging (`logLevel: 'info'`)**: Default log level outputs every COMMAND/DATA/RESULT to stdout. With 14 specs generating ~16K lines (~2.9MB), could stdout buffering or log formatting slow down the event loop between operations?

4. **tauri-driver / WebKitWebDriver session state**: Does the number of spec files affect how tauri-driver or WebKitWebDriver initializes the session?

5. **Node.js event loop contention**: With all 14 specs loaded, there are more module-level imports, more closures in memory, more registered mocha hooks. Could this create GC pressure or event loop delays that add ~70ms to every async WebDriver call?

## How to reproduce

```bash
# Build once
bunx tauri build --debug --no-bundle

# Full suite (slow — ~4 min)
E2E_SKIP_BUILD=1 bunx wdio run e2e/wdio.conf.js

# 3-spec subset (fast — ~38s)
# Create /tmp/wdio-3spec.conf.js with specs: [[overview, git, cross-tab]]
E2E_SKIP_BUILD=1 bunx wdio run /tmp/wdio-3spec.conf.js
```

## Files

- `e2e/wdio.conf.js` — main config (persistent mode)
- `e2e/helpers/timing.js` — centralized timing constants
- `e2e/helpers/navigation.js` — optimized navigation helpers (browser.execute)
- `e2e/helpers/modal.js`, `settings.js`, `search.js` — other optimized helpers
- `e2e/helpers.js` — boot helpers (`waitForAppReady`, `ensureMainApp`, `resetAppState`)
- `e2e/specs/*.js` — 14 spec files

## Current test results

137 passing, 2 failing (context-menu edge cases), 10 skipped.
