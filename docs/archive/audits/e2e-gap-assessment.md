# E2E Gap Assessment: taurhaus vs tauri-driver + WebKitWebDriver Best Practices

Date: 2026-03-05  
Scope: Linux Tauri 2 E2E stability (WebdriverIO -> tauri-driver -> WebKitWebDriver)  
Mode: Read-only analysis (no test/code changes)

## Executive Summary

taurhaus is already aligned on several important stability basics:

- Single-worker execution (`maxInstances: 1`)
- Aggressive process cleanup and signal handling
- Isolated per-session app data/temp roots
- Reduced WDIO logging overhead (`logLevel: error` by default)
- Batched spec groups to avoid one giant persistent session

The highest-value gaps are:

1. Deterministic driver/runtime pinning (tauri-driver + webkit2gtk-driver + explicit native driver path)
2. Better session resilience (startup readiness handshake, retry strategy, optional shorter session boundaries for high-churn specs)
3. Reduced fragile WebDriver interaction patterns (fewer fixed pauses and direct `.click()` on Linux WebKit paths)
4. CI parity and canarying (version drift detection before local/dev breakage)

## Sources Used

- Tauri v2 WebDriver docs (overview, WDIO example, CI):
  - https://tauri.app/develop/tests/webdriver/
  - https://tauri.app/develop/tests/webdriver/example/webdriverio/
  - https://tauri.app/develop/tests/webdriver/ci/
- Official Tauri WebDriver example repo (v2):
  - https://github.com/tauri-apps/webdriver-example
- CrabNebula tauri-driver package README:
  - https://www.npmjs.com/package/@crabnebula/tauri-driver
- WebdriverIO docs:
  - https://webdriver.io/docs/configuration/
  - https://webdriver.io/docs/bestpractices/
  - https://webdriver.io/docs/retry/
- WebKitWebDriver docs/man/help:
  - `man WebKitWebDriver` (local)
  - `WebKitWebDriver --help` (local)
- Tauri issue tracker (webdriver scope, known instability/workarounds):
  - https://github.com/tauri-apps/tauri/issues/6541
  - https://github.com/tauri-apps/tauri/issues/7667
  - https://github.com/tauri-apps/tauri/issues/8828
  - https://github.com/tauri-apps/tauri/issues/10670
  - https://github.com/tauri-apps/tauri/issues/7415

## Current taurhaus Setup (Snapshot)

From `e2e/wdio.conf.js` and E2E helpers/specs:

- Runner/session
  - `maxInstances: 1`
  - `connectionRetryTimeout: 12_000`
  - `connectionRetryCount: 0`
  - `bail` defaults to `1`, `mochaOpts.bail` defaults to `true`
- Grouping/lifecycle
  - 5 fixed spec groups (3 specs each), one app instance per group
  - `beforeSession`: cleans old artifacts, creates temp fixture repos, spawns `tauri-driver`
  - `afterSession` + `onComplete`: cleanup
  - signal cleanup handlers present
- Driver startup
  - Spawns `tauri-driver --port ... --native-port ...`
  - Readiness check only verifies TCP port open (not full session create handshake)
- Logging
  - Default `logLevel: error`
  - output dir configured for WDIO logs
- Interaction patterns
  - Extensive `waitUntil` usage
  - Still uses fixed `browser.pause(...)` in key paths
  - Still many direct element `.click()` calls in helpers/specs (plus some `browser.execute` click usage)
- Versioning/tooling
  - WDIO packages are modern (`^9.24.0`)
  - `tauri-driver` is invoked from environment (`tauri-driver` on PATH), no explicit per-run version/path pin in config

## Gap Table

| Area | Current taurhaus | Best-practice target | Stability impact | Effort |
|---|---|---|---|---|
| Driver version pinning | `tauri-driver` resolved from PATH at runtime; Linux `WebKitWebDriver` package version not pinned in project policy | Pin/test against explicit `tauri-driver` + `webkit2gtk-driver` versions in CI image and docs/runbook | High | Small-Medium |
| Native driver path determinism | No `--native-driver` in `tauri-driver` spawn; relies on ambient PATH | Provide explicit native driver path per environment to remove PATH ambiguity | Medium | Small |
| Startup readiness handshake | Waits for open port only (`waitForWebDriverReady`) | Wait for driver protocol readiness (e.g., helper like `waitTauriDriverReady` or explicit `/status`/session probe) before tests | Medium | Small |
| Session replacement hygiene | No use of WebKit `--replace-on-new-session` path | Evaluate remote-native mode with `REMOTE_WEBDRIVER_URL` + WebKit `--replace-on-new-session` to recover stale sessions faster | Medium | Medium-Large |
| Retry strategy | No spec retries configured (`specFileRetries` absent; transport retries disabled by default) | Introduce targeted retry policy for known transient bridge failures (spec-level retries + small delay) | Medium | Small |
| Session boundary risk | 3 specs share a session; failures can leave invalid state for remainder of group | Split highest-churn specs into smaller groups or enforce periodic session recycle for fragile groups | High | Medium |
| Command pacing | No global pacing/throttling policy beyond waits; still bursty in UI-heavy specs | Add minimal paced wrappers for high-risk operations; prefer condition-driven waits over rapid command bursts | Medium-High | Medium |
| Click/input robustness on Linux WebKit | Many direct `.click()` calls remain; known ecosystem history of click/setValue instability | Centralize resilient click/set helpers (fallback to JS click/input event dispatch where needed) | High | Medium |
| Fixed sleeps | Multiple `browser.pause(...)` calls remain | Replace fixed sleeps with explicit state waits wherever possible (`waitUntil`, element state, app signals) | Medium | Small-Medium |
| CI parity/canarying | No dedicated E2E matrix workflow in-repo for Linux/Windows driver drift checks | Add CI canary lane to validate driver/toolchain updates before merging | High | Medium-Large |
| Failure artifacts | WDIO logs exist; no standardized failure bundle policy (driver/native logs, app logs, screenshots, trace metadata) | Collect consistent artifacts on failure for root-cause speed | Medium | Medium |
| Upstream issue awareness | Known webdriver fragility tracked ad hoc | Track and periodically review critical upstream issues + known-good matrix in docs | Medium | Small |

## Detailed Gap Notes

### 1) Driver Version Pinning and Native Path Control

What we do now:
- Rely on `tauri-driver` and `WebKitWebDriver` availability in PATH.

Best-practice:
- Keep versions deterministic and explicit, especially in CI.
- Use explicit `--native-driver` where possible.

Why this matters:
- Multiple open Tauri webdriver issues show version-sensitive behavior; unpinned runtime surfaces increase nondeterministic failures.

### 2) Session Hygiene and Recovery

What we do now:
- Strong process cleanup exists.
- Session readiness check only validates socket open.

Best-practice:
- Validate driver protocol readiness before test start.
- Consider session replacement strategies for stale sessions.
- Add bounded spec-level retries for transient transport failures.

Why this matters:
- Socket-open is weaker than protocol-ready.
- Mid-session failures are expensive; better restart/retry boundaries reduce wasted run time.

### 3) Command Robustness (Clicks, Pauses, Pacing)

What we do now:
- Good use of `waitUntil` and some `browser.execute`.
- Still uses fixed pauses and many direct `.click()` calls.

Best-practice:
- Minimize hard sleeps.
- Use deterministic state waits and resilient action wrappers for Linux WebKit.
- Pace high-burst sequences when bridge instability is observed.

Why this matters:
- WebKit/Linux click and interaction edge cases are a known pain area in tauri webdriver issue history.

### 4) CI and Drift Detection

What we do now:
- Local recipes are strong, but no explicit in-repo webdriver canary matrix.

Best-practice:
- Run a lean CI matrix (at least Linux) with pinned system deps and artifact collection.
- Add controlled update cadence for webdriver stack.

Why this matters:
- Catches regressions from driver/toolchain changes before they become multi-hour local debugging events.

## Ranked Recommendations

### Priority 1 (High impact, low-medium effort)

1. Add deterministic driver policy:
   - Pin `tauri-driver` and `webkit2gtk-driver` in CI and document known-good versions.
   - Set explicit native driver path in test startup.
2. Upgrade startup handshake:
   - Replace port-open check with protocol-ready check.
3. Remove avoidable fixed sleeps:
   - Replace `browser.pause` in non-bootstrap paths with state waits.

### Priority 2 (High impact, medium effort)

1. Add resilient action wrappers:
   - Centralize click/input helpers with WebKit-safe fallbacks.
2. Tune session boundaries:
   - Break high-churn groups further or recycle session after select specs.
3. Introduce targeted retry policy:
   - Use spec retries for transport-level transient failures, not for product logic masking.

### Priority 3 (Medium impact, medium-large effort)

1. Add E2E CI canary lane with artifacts.
2. Evaluate remote native driver mode using WebKit `--replace-on-new-session`.

## Items Already Well Aligned

- Single-worker execution (`maxInstances: 1`)
- Per-session temp roots and fixture isolation
- Aggressive process cleanup on `afterSession`, `onComplete`, and signals
- Reduced WDIO logging overhead (defaulting away from verbose command logging)
- Explicit grouped-session architecture based on measured performance data

## Bottom Line

taurhaus is not fundamentally misconfigured; it already implements several non-trivial stability controls. The largest remaining wins are deterministic driver/runtime pinning, stronger startup/session resilience, and reducing fragile interaction patterns (fixed pauses + direct click dependence) on the Linux WebKit bridge.
