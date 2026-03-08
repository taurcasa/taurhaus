/**
 * Modal helpers for E2E tests — open/close Manage Projects modal.
 *
 * PERF: Uses browser.execute() for clicks and condition checks to minimize
 * WebDriver round-trips.
 */

import { WAIT_MEDIUM, WAIT_XLONG } from './timing.js'

/**
 * Open the Manage Projects modal.
 */
export async function openManageProjects() {
  await browser.execute(() => {
    document.querySelector('[data-testid="manage-projects-btn"]')?.click()
  })
  await browser.waitUntil(
    async () => browser.execute(() =>
      document.querySelector('[data-testid="manage-projects-modal"]') !== null
    ),
    { ...WAIT_MEDIUM, timeoutMsg: 'Manage Projects modal did not open' }
  )
}

/**
 * Close the currently open modal.
 */
export async function closeModal() {
  await browser.execute(() => {
    document.querySelector('[data-testid="modal-close"]')?.click()
  })
  await browser.waitUntil(
    async () => browser.execute(() =>
      document.querySelector('[data-testid="manage-projects-modal"]') === null
    ),
    { ...WAIT_MEDIUM, timeoutMsg: 'Modal did not close' }
  )
}

/**
 * Open the "Add project" section, switch to manual mode, and submit a path.
 * Assumes the Manage Projects modal is already open.
 * @param {string} path - The path to enter in the manual path input
 */
export async function tryAddProjectPath(path) {
  // Check if manual-path-input is already visible (already in manual mode)
  const hasManualInput = await browser.execute(() =>
    document.querySelector('[data-testid="manual-path-input"]') !== null
  )

  if (!hasManualInput) {
    // Step 1: Open the add section if not already open
    await browser.execute(() => {
      document.querySelector('[data-testid="show-add-section"]')?.click()
    })

    // Step 2: Wait for scan to complete, then switch to manual mode
    await browser.waitUntil(
      async () => browser.execute(() =>
        document.querySelector('[data-testid="enter-manual-mode"]') !== null
      ),
      { ...WAIT_XLONG, timeoutMsg: '"Enter path manually" button did not appear' }
    )
    await browser.execute(() => {
      document.querySelector('[data-testid="enter-manual-mode"]')?.click()
    })

    // Wait for manual input to appear
    await browser.waitUntil(
      async () => browser.execute(() =>
        document.querySelector('[data-testid="manual-path-input"]') !== null
      ),
      { ...WAIT_MEDIUM, timeoutMsg: 'Manual path input did not appear' }
    )
  }

  // Use WebDriver for input (setValue triggers proper Svelte bind:value updates)
  const input = await $('[data-testid="manual-path-input"]')
  await input.clearValue()
  await input.setValue(path)

  // Trigger blur to run validatePath (component validates onblur)
  await browser.execute(() => {
    const el = document.querySelector('[data-testid="manual-path-input"]')
    if (el) el.dispatchEvent(new FocusEvent('blur', { bubbles: true }))
  })

  // Wait for validation to complete (validating spinner disappears)
  await browser.waitUntil(
    async () => browser.execute(() => {
      // Validation done when: error message exists, validation message exists, or add button is enabled
      return document.querySelector('[data-testid="manual-error"]') !== null ||
        document.querySelector('[data-testid="validation-message"]') !== null ||
        !document.querySelector('[data-testid="manual-add-button"]')?.disabled
    }),
    { ...WAIT_MEDIUM, timeoutMsg: 'Path validation did not complete' }
  )

  // Click add button (will only work if path is valid — for invalid paths, validation message is already shown)
  await browser.execute(() => {
    const btn = document.querySelector('[data-testid="manual-add-button"]')
    if (btn && !btn.disabled) btn.click()
  })
}

/**
 * Wait for a modal identified by testid to appear.
 * @param {string} testid - The data-testid of the modal
 */
async function waitForModal(testid) {
  await browser.waitUntil(
    async () => browser.execute(
      (id) => document.querySelector(`[data-testid="${id}"]`) !== null,
      testid
    ),
    { ...WAIT_MEDIUM, timeoutMsg: `Modal "${testid}" did not appear` }
  )
}
