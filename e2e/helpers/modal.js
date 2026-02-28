/**
 * Modal helpers for E2E tests — open/close Manage Projects modal.
 */

/**
 * Open the Manage Projects modal.
 */
export async function openManageProjects() {
  const btn = await $('[data-testid="manage-projects-btn"]')
  await btn.click()
  await browser.waitUntil(
    async () => {
      const modal = await $('[data-testid="manage-projects-modal"]')
      return await modal.isExisting()
    },
    { timeout: 5_000, interval: 300, timeoutMsg: 'Manage Projects modal did not open' }
  )
}

/**
 * Close the currently open modal.
 */
export async function closeModal() {
  const close = await $('[data-testid="modal-close"]')
  await close.click()
  await browser.waitUntil(
    async () => {
      const modal = await $('[data-testid="manage-projects-modal"]')
      return !(await modal.isExisting())
    },
    { timeout: 5_000, interval: 300, timeoutMsg: 'Modal did not close' }
  )
}

/**
 * Open the "Add project" section, switch to manual mode, and submit a path.
 * Assumes the Manage Projects modal is already open.
 * @param {string} path - The path to enter in the manual path input
 */
export async function tryAddProjectPath(path) {
  // Check if manual-path-input is already visible (already in manual mode)
  let input = await $('[data-testid="manual-path-input"]')
  if (!(await input.isExisting())) {
    // Step 1: Open the add section if not already open
    const showAdd = await $('[data-testid="show-add-section"]')
    if (await showAdd.isExisting()) {
      await showAdd.click()
    }

    // Step 2: Wait for scan to complete, then switch to manual mode
    await browser.waitUntil(
      async () => {
        const manualBtn = await $('[data-testid="enter-manual-mode"]')
        return await manualBtn.isExisting()
      },
      { timeout: 10_000, interval: 300, timeoutMsg: '"Enter path manually" button did not appear' }
    )
    const manualBtn = await $('[data-testid="enter-manual-mode"]')
    await manualBtn.click()

    input = await $('[data-testid="manual-path-input"]')
    await input.waitForExist({ timeout: 5_000 })
  }

  // Clear any previous value before setting new one
  await input.clearValue()
  await input.setValue(path)

  const addBtn = await $('[data-testid="manual-add-button"]')
  await addBtn.click()
}

/**
 * Wait for a modal identified by testid to appear.
 * @param {string} testid - The data-testid of the modal
 */
export async function waitForModal(testid) {
  await browser.waitUntil(
    async () => {
      const modal = await $(`[data-testid="${testid}"]`)
      return await modal.isExisting()
    },
    { timeout: 5_000, interval: 300, timeoutMsg: `Modal "${testid}" did not appear` }
  )
}
