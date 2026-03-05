/**
 * Settings helpers for E2E tests — open/close settings, get/set values.
 */

import { WAIT_MEDIUM, WAIT_SHORT } from './timing.js'

const SETTINGS_CLOSE_TIMEOUT = 1_000

async function isSettingsVisible() {
  const view = await $('[data-testid="settings-view"]')
  if (!(await view.isExisting())) return false
  return await view.isDisplayed()
}

async function isSettingsSaving() {
  return browser.execute(() => {
    const view = document.querySelector('[data-testid="settings-view"]')
    if (!view) return false
    return Array.from(view.querySelectorAll('p')).some((node) => node.textContent?.trim() === 'Saving...')
  })
}

/**
 * Open settings view by clicking the settings toggle in the sidebar.
 */
export async function openSettings() {
  if (await isSettingsVisible()) return true

  const toggle = await $('[data-testid="settings-toggle"]')
  if (await toggle.isExisting()) {
    await toggle.click()
  }
  await browser.waitUntil(
    async () => await isSettingsVisible(),
    { ...WAIT_MEDIUM, timeoutMsg: 'Settings view did not open' }
  )
  // Wait for settings data to load (sections render only after IPC completes)
  await browser.waitUntil(
    async () => (await $('[data-testid="settings-scanning"]')).isExisting(),
    { ...WAIT_MEDIUM, timeoutMsg: 'Settings content did not load (still showing skeleton)' }
  )
}

async function waitForSaveIdle({ required = false } = {}) {
  if (!(await isSettingsSaving())) return true

  const idle = await browser.waitUntil(
    async () => !(await isSettingsSaving()),
    { ...WAIT_SHORT, timeoutMsg: 'Settings save did not settle' }
  ).catch(() => false)

  if (!idle && required) {
    throw new Error('Settings save did not settle')
  }
  return idle
}

/**
 * Close settings view by clicking the back button.
 */
export async function closeSettings() {
  if (!(await isSettingsVisible())) return true

  await waitForSaveIdle()

  const waitClosed = async (timeout = SETTINGS_CLOSE_TIMEOUT) => browser.waitUntil(
    async () => !(await isSettingsVisible()),
    { timeout, interval: WAIT_SHORT.interval, timeoutMsg: 'Settings view did not close' }
  )

  const closedViaBack = await browser.execute(() => {
    const back = document.querySelector('[data-testid="settings-back"]')
    if (!back) return false
    back.click()
    return true
  }).catch(() => false)
  if (closedViaBack) {
    const closedAfterBack = await waitClosed().catch(() => false)
    if (closedAfterBack) return true
  }

  const closedViaToggle = await browser.execute(() => {
    const toggle = document.querySelector('[data-testid="settings-toggle"]')
    if (!toggle) return false
    toggle.click()
    return true
  }).catch(() => false)
  if (closedViaToggle) {
    const closedAfterToggle = await waitClosed().catch(() => false)
    if (closedAfterToggle) return true
  }

  await browser.keys('Escape')
  const closedAfterEscape = await waitClosed().catch(() => false)
  if (closedAfterEscape) return true

  await browser.execute(() => {
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  }).catch(() => {})

  return await waitClosed().catch(() => false)
}

/**
 * Ensure settings view is open — opens it if not already visible.
 */
export async function ensureSettingsOpen() {
  if (!(await isSettingsVisible())) {
    await openSettings()
  }
}

/**
 * Get the current value of a settings input/select.
 * @param {string} testid - The data-testid of the input/select
 * @returns {Promise<string>}
 */
export async function getSettingValue(testid) {
  const el = await $(`[data-testid="${testid}"]`)
  if (!(await el.isExisting())) return ''
  return await el.getValue()
}

/**
 * Set a value on a settings input/select and trigger save.
 * For inputs: uses native value setter + blur (Settings.svelte saves on blur).
 * For selects: uses WebDriver selectByAttribute (needs proper option selection).
 * @param {string} testid - The data-testid of the input/select
 * @param {string} value - The value to set
 */
export async function setSettingValue(testid, value) {
  const el = await $(`[data-testid="${testid}"]`)
  if (!(await el.isExisting())) return

  const tagName = (await el.getTagName())?.toLowerCase()
  if (tagName === 'select') {
    await el.selectByAttribute('value', value)
    await waitForSaveIdle()
  } else {
    await el.click()
    await el.clearValue()
    await el.setValue(value)
    await browser.keys('Tab')
    await browser.waitUntil(
      async () => (await el.getValue()) === value,
      { ...WAIT_MEDIUM, timeoutMsg: `Setting "${testid}" did not update to "${value}"` }
    )
    await waitForSaveIdle()
  }
}
