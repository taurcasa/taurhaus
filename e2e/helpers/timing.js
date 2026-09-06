/**
 * Shared timing constants for E2E tests.
 *
 * Change values here to tune wait behavior across the entire suite.
 * This is a LOCAL desktop app — no network latency. UI updates happen within
 * one frame (~16ms), so 50ms polling catches them on the next check.
 * Timeouts are safety nets, not expected wait times.
 */

// ── Fixed Pauses ──────────────────────────────────────────────────────────
// browser.pause() values. Use sparingly — prefer waitUntil with conditions.

/** Brief settle between rapid sequential clicks (e.g. project switch → tab click). */
export const PAUSE_CLICK_SETTLE = 100

/** Minimal settle between iterations in tight click loops (e.g. cycling tabs). */
export const PAUSE_TICK = 50

/** Boot: initial wait for app process to start before querying DOM. */
export const PAUSE_BOOT = 1_000

/** Boot: after clicking "Continue anyway" on splash screen. */
export const PAUSE_BOOT_ACTION = 150

// ── Poll Intervals ─────────────────────────────────────────────────────────
// How often waitUntil checks its condition.

/** Fast polling for instant UI reactions (theme toggle, menu close). */
export const POLL_FAST = 50

/** Standard polling for most element waits. */
export const POLL = 100

/** Slower polling for operations that genuinely take time (search index, file load). */
export const POLL_SLOW = 150

/** Boot sequence polling — slow because we're waiting for process startup. */
export const POLL_BOOT = 1_000

/** Wizard step polling — moderate because steps auto-proceed. */
export const POLL_WIZARD = 500

// ── Timeouts ───────────────────────────────────────────────────────────────
// Max time to wait before failing. These are safety nets — actual waits
// should resolve in <100ms for UI ops, <1s for content loads.

/** Very fast UI — element should appear within 1-2 frames. */
const TIMEOUT_INSTANT = 1_500

/** Standard UI operation — tab switch, overlay open/close. */
const TIMEOUT_SHORT = 2_000

/** Content loading — file tree, commit list, search results. */
export const TIMEOUT_MEDIUM = 2_500

/** Heavy operation — search index rebuild, tasks loading, initial app boot. */
export const TIMEOUT_LONG = 4_000

/** Extra-heavy operation — full index rebuild, large file tree. */
export const TIMEOUT_XLONG = 7_000

/** First-launch only — wizard, initial indexing, and slow splash-to-shell transitions. */
export const TIMEOUT_BOOT = 45_000

// ── Presets ────────────────────────────────────────────────────────────────
// Common { timeout, interval } combos to spread into waitUntil options.

/** For instant UI feedback (theme change, menu dismiss). */
export const WAIT_INSTANT = { timeout: TIMEOUT_INSTANT, interval: POLL_FAST }

/** For standard UI operations (tab content, overlay, modal). */
export const WAIT_SHORT = { timeout: TIMEOUT_SHORT, interval: POLL }

/** For content loading (file tree, commits, search). */
export const WAIT_MEDIUM = { timeout: TIMEOUT_MEDIUM, interval: POLL }

/** For heavy operations (index rebuild, long loads). */
export const WAIT_LONG = { timeout: TIMEOUT_LONG, interval: POLL_SLOW }

/** For extra-heavy operations (full index rebuild, large file tree). */
export const WAIT_XLONG = { timeout: TIMEOUT_XLONG, interval: POLL_SLOW }

// Regression: 430e09ee budgeted recovery state as a 20/25s one-off wait.
// daemon/session_activity.rs: idle scanner cadence = 1500ms (active = 500ms).
// meshTabGate.svelte.js: live runtime UI cadence = 2000ms.
// 4 scanner cycles + 2 UI polls + 20s scheduling/IPC margin under suite
// contention = 4 * 1500 + 2 * 2000 + 20000 = 30000ms. The margin permits
// slow process probes and queued IPC; it is not another scanner cadence.
// Use only for scanner/roster propagation, not local UI or command completion.
export const WAIT_MESH_PROPAGATION = {
  timeout: 4 * 1_500 + 2 * 2_000 + 20_000,
  interval: POLL_SLOW,
}
