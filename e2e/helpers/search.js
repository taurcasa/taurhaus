/**
 * Search helpers for E2E tests — open/close search overlay.
 *
 * PERF: Uses browser.execute() for condition checks.
 */

import { WAIT_SHORT, WAIT_MEDIUM } from './timing.js'

/**
 * Open the search overlay via Ctrl+K.
 * Safe to call when overlay is already open (no-op).
 */
export async function openSearch() {
  const alreadyOpen = await browser.execute(() =>
    document.querySelector('[data-testid="search-overlay"]') !== null
  )
  if (alreadyOpen) return

  await browser.keys(['Control', 'k'])
  await browser.waitUntil(
    async () => browser.execute(() =>
      document.querySelector('[data-testid="search-overlay"]') !== null
    ),
    { ...WAIT_MEDIUM, timeoutMsg: 'Search overlay did not open' }
  )
}

/**
 * Close the search overlay and wait for it to disappear.
 */
export async function closeSearch() {
  await browser.keys('Escape')
  await browser.waitUntil(
    async () => browser.execute(() =>
      document.querySelector('[data-testid="search-overlay"]') === null
    ),
    { ...WAIT_SHORT, timeoutMsg: 'Search overlay did not close' }
  )
}

/**
 * Dismiss the search overlay if it's open (no-op if already closed).
 */
export async function dismissSearch() {
  await browser.execute(() => {
    if (document.querySelector('[data-testid="search-overlay"]')) {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    }
  })
}
