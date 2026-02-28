/**
 * Settings helpers for E2E tests — open/close settings, get/set values.
 *
 * PERF: Uses browser.execute() for clicks and condition checks.
 */

import { WAIT_MEDIUM } from './timing.js'

/**
 * Open settings view by clicking the settings toggle in the sidebar.
 */
export async function openSettings() {
  await browser.execute(() => {
    document.querySelector('[data-testid="settings-toggle"]')?.click()
  })
  await browser.waitUntil(
    async () => browser.execute(() =>
      document.querySelector('[data-testid="settings-view"]') !== null
    ),
    { ...WAIT_MEDIUM, timeoutMsg: 'Settings view did not open' }
  )
  // Wait for settings data to load (sections render only after IPC completes)
  await browser.waitUntil(
    async () => browser.execute(() =>
      document.querySelector('[data-testid="settings-scanning"]') !== null
    ),
    { ...WAIT_MEDIUM, timeoutMsg: 'Settings content did not load (still showing skeleton)' }
  )
}

/**
 * Close settings view by clicking the back button.
 */
export async function closeSettings() {
  await browser.execute(() => {
    document.querySelector('[data-testid="settings-back"]')?.click()
  })
  await browser.waitUntil(
    async () => browser.execute(() =>
      document.querySelector('[data-testid="settings-view"]') === null
    ),
    { ...WAIT_MEDIUM, timeoutMsg: 'Settings view did not close' }
  )
}

/**
 * Ensure settings view is open — opens it if not already visible.
 */
export async function ensureSettingsOpen() {
  const isOpen = await browser.execute(() =>
    document.querySelector('[data-testid="settings-view"]') !== null
  )
  if (!isOpen) await openSettings()
}

/**
 * Get the current value of a settings input/select.
 * @param {string} testid - The data-testid of the input/select
 * @returns {Promise<string>}
 */
export async function getSettingValue(testid) {
  return await browser.execute((id) => {
    const el = document.querySelector(`[data-testid="${id}"]`)
    return el ? el.value : ''
  }, testid)
}

/**
 * Set a value on a settings input/select and trigger save.
 * For inputs: uses native value setter + blur (Settings.svelte saves on blur).
 * For selects: uses WebDriver selectByAttribute (needs proper option selection).
 * @param {string} testid - The data-testid of the input/select
 * @param {string} value - The value to set
 */
export async function setSettingValue(testid, value) {
  const tagName = await browser.execute(
    (id) => document.querySelector(`[data-testid="${id}"]`)?.tagName?.toLowerCase(),
    testid
  )
  if (tagName === 'select') {
    const el = await $(`[data-testid="${testid}"]`)
    await el.selectByAttribute('value', value)
  } else {
    // Set value via native setter + dispatch input + blur (Svelte bindings + onblur handler)
    await browser.execute((id, val) => {
      const input = document.querySelector(`[data-testid="${id}"]`)
      if (!input) return
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
      setter.call(input, val)
      input.dispatchEvent(new Event('input', { bubbles: true }))
      input.dispatchEvent(new Event('change', { bubbles: true }))
      input.dispatchEvent(new FocusEvent('blur', { bubbles: true }))
    }, testid, value)
  }
}
