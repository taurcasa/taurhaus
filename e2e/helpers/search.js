/**
 * Search helpers for E2E tests — open/close search overlay.
 *
 * PERF: Uses browser.execute() for condition checks.
 */

import { WAIT_SHORT, WAIT_MEDIUM } from './timing.js'
import { MOD_KEY } from './platform.js'

async function invokeTauri(command, args = undefined) {
  return await browser.executeAsync((payload, done) => {
    const tauri = window.__TAURI_INTERNALS__
    if (!tauri || typeof tauri.invoke !== 'function') {
      done({ ok: false, error: 'Tauri internals unavailable' })
      return
    }

    tauri
      .invoke(payload.command, payload.args)
      .then((result) => done({ ok: true, result }))
      .catch((error) => done({ ok: false, error: error?.message ?? String(error) }))
  }, { command, args })
}

export async function ensureSearchReady() {
  const status = await invokeTauri('get_index_status')
  if (status.ok && Number(status.result?.doc_count || 0) > 0) return true

  await invokeTauri('rebuild_index')

  await browser.waitUntil(
    async () => {
      const refreshed = await invokeTauri('get_index_status')
      return refreshed.ok && Number(refreshed.result?.doc_count || 0) > 0
    },
    { timeout: 12_000, interval: 200, timeoutMsg: 'Search index did not become ready' }
  )

  return true
}

/**
 * Open the search overlay via Ctrl+K.
 * Safe to call when overlay is already open (no-op).
 */
export async function openSearch() {
  const alreadyOpen = await browser.execute(() =>
    document.querySelector('[data-testid="search-overlay"]') !== null
  )
  if (alreadyOpen) return

  await browser.keys([MOD_KEY, 'k'])
  await browser.waitUntil(
    async () => browser.execute(() =>
      document.querySelector('[data-testid="search-overlay"]') !== null
    ),
    { ...WAIT_MEDIUM, timeoutMsg: 'Search overlay did not open' }
  )

  // Ensure the input exists and receives focus before test actions continue.
  await browser.waitUntil(
    async () => browser.execute(() => {
      const input = document.querySelector('[data-testid="search-input"]')
      if (!input) return false
      input.focus()
      return document.activeElement === input
    }),
    { ...WAIT_SHORT, timeoutMsg: 'Search input did not become focused' }
  )
}

/**
 * Close the search overlay and wait for it to disappear.
 */
export async function closeSearch() {
  await browser.keys('Escape').catch(() => {})
  await browser.execute(() => {
    const overlay = document.querySelector('[data-testid="search-overlay"]')
    if (!overlay) return
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  })
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
