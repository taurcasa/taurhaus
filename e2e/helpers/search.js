/**
 * Search helpers for E2E tests — open/close search overlay.
 */

/**
 * Open the search overlay via Ctrl+K.
 * Safe to call when overlay is already open (no-op).
 */
export async function openSearch() {
  // If already open, don't toggle it closed
  const existing = await $('[data-testid="search-overlay"]')
  if (await existing.isExisting()) return

  await browser.keys(['Control', 'k'])
  await browser.waitUntil(
    async () => {
      const overlay = await $('[data-testid="search-overlay"]')
      return await overlay.isExisting()
    },
    { timeout: 5_000, interval: 300, timeoutMsg: 'Search overlay did not open' }
  )
}

/**
 * Close the search overlay and wait for it to disappear.
 */
export async function closeSearch() {
  await browser.keys('Escape')
  await browser.waitUntil(
    async () => {
      const overlay = await $('[data-testid="search-overlay"]')
      return !(await overlay.isExisting())
    },
    { timeout: 3_000, interval: 200, timeoutMsg: 'Search overlay did not close' }
  )
}

/**
 * Dismiss the search overlay if it's open (no-op if already closed).
 */
export async function dismissSearch() {
  const overlay = await $('[data-testid="search-overlay"]')
  if (await overlay.isExisting()) {
    await closeSearch()
  }
}
