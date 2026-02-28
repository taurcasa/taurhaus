/**
 * Settings helpers for E2E tests — open/close settings, get/set values.
 */

/**
 * Open settings view by clicking the settings toggle in the sidebar.
 */
export async function openSettings() {
  const toggle = await $('[data-testid="settings-toggle"]')
  await toggle.click()
  await browser.waitUntil(
    async () => {
      const view = await $('[data-testid="settings-view"]')
      return await view.isExisting()
    },
    { timeout: 5_000, interval: 300, timeoutMsg: 'Settings view did not open' }
  )
}

/**
 * Close settings view by clicking the back button.
 */
export async function closeSettings() {
  const back = await $('[data-testid="settings-back"]')
  await back.click()
  await browser.waitUntil(
    async () => {
      const view = await $('[data-testid="settings-view"]')
      return !(await view.isExisting())
    },
    { timeout: 5_000, interval: 300, timeoutMsg: 'Settings view did not close' }
  )
}

/**
 * Ensure settings view is open — opens it if not already visible.
 */
export async function ensureSettingsOpen() {
  const view = await $('[data-testid="settings-view"]')
  if (!(await view.isExisting())) {
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
  const tagName = await el.getTagName()
  if (tagName === 'select') {
    return await el.getValue()
  }
  return await el.getValue()
}

/**
 * Set a value on a settings input/select and trigger change.
 * @param {string} testid - The data-testid of the input/select
 * @param {string} value - The value to set
 */
export async function setSettingValue(testid, value) {
  const el = await $(`[data-testid="${testid}"]`)
  const tagName = await el.getTagName()
  if (tagName === 'select') {
    await el.selectByAttribute('value', value)
  } else {
    await el.clearValue()
    await el.setValue(value)
    // Trigger blur to commit the change
    await browser.execute((selector) => {
      const input = document.querySelector(`[data-testid="${selector}"]`)
      if (input) input.dispatchEvent(new Event('change', { bubbles: true }))
    }, testid)
  }
}
