/**
 * Shared helpers for Tauri e2e tests.
 */

import {
  PAUSE_BOOT, PAUSE_BOOT_ACTION, POLL_BOOT, POLL_WIZARD,
  TIMEOUT_BOOT,
} from './helpers/timing.js'
import { PROJECTS_DIR } from './helpers/platform.js'

/**
 * Wait for the app to be ready — handles splash screen, wizard, or main shell.
 *
 * The boot sequence is: Splash Screen → (Wizard | Main Shell).
 * The splash screen waits for the daemon, then fades to the app.
 * If the daemon doesn't start within 15s, the splash shows "Continue anyway."
 * We handle all three entry points.
 */
export async function waitForAppReady() {
  // Quick check — if the app is already loaded (persistent session), reset and continue
  const alreadyReady = await $('[data-testid="tab-overview"]')
  if (await alreadyReady.isExisting()) {
    await resetAppState()
    return
  }

  // First launch: give the app time to start up before querying the DOM
  await browser.pause(PAUSE_BOOT)

  await browser.waitUntil(
    async () => {
      // Check if we're already past the splash into main app
      const overviewTab = await $('[data-testid="tab-overview"]')
      if (await overviewTab.isExisting()) return true

      // Check for wizard (first-run)
      const wizard = await $('[data-testid="get-started-button"]')
      if (await wizard.isExisting()) return true

      // Splash might be showing "Continue anyway" — click it to proceed
      const continueBtn = await $('[data-testid="continue-anyway-btn"]')
      if (await continueBtn.isExisting()) {
        await continueBtn.click()
        await browser.pause(PAUSE_BOOT_ACTION)
        return false // Will check again on next iteration
      }

      return false
    },
    {
      timeout: TIMEOUT_BOOT,
      interval: POLL_BOOT,
      timeoutMsg: 'App did not render within 45s (checked splash, wizard, and Overview tab)'
    }
  )
}

/**
 * Ensure we're in the main app (past the wizard).
 * If the wizard is showing, navigate through it.
 *
 * @returns {Promise<boolean>} true if main app is now visible
 */
export async function ensureMainApp() {
  // Check if we're already in the main app
  const overviewTab = await $('[data-testid="tab-overview"]')
  if (await overviewTab.isExisting()) return true

  // Must be the wizard — navigate through it
  const getStarted = await $('[data-testid="get-started-button"]')
  if (!(await getStarted.isExisting())) return false

  // Step 1: Get started
  await getStarted.click()

  // Step 2: Daemon setup — auto-proceeds if installed, otherwise skip
  const daemonStep = await $('[data-testid="wizard-step-2"]')
  if (await daemonStep.isExisting()) {
    const browseStep = await $('[data-testid="wizard-step-3"]')
    await browser.waitUntil(
      async () => {
        if (await browseStep.isExisting()) return true

        const checking = await $('[data-testid="daemon-checking"]')
        if (await checking.isExisting()) return false

        const skipBtn = await $('[data-testid="daemon-skip-button"]')
        if (await skipBtn.isExisting()) {
          await skipBtn.click()
        }
        return await browseStep.isExisting()
      },
      {
        timeout: 15_000,
        interval: POLL_WIZARD,
        timeoutMsg: 'Wizard did not reach scan step from daemon setup'
      }
    )
  }

  // Step 3: Scan for projects
  const input = await $('[data-testid="wizard-step-3"] input[type="text"]')
  await input.waitForExist({ timeout: 5_000 })
  await input.setValue(PROJECTS_DIR)

  const scanBtn = await $('[data-testid="scan-button"]')
  await scanBtn.click()

  // Step 3: Register projects
  const registerBtn = await $('[data-testid="register-button"]')
  await registerBtn.waitForExist({ timeout: 30_000 })
  await browser.waitUntil(
    async () => browser.execute(() => {
      const register = document.querySelector('[data-testid="register-button"]')
      if (!register) return false
      if (!register.disabled) return true

      // If nothing is selected, force-select all discovered projects.
      const selectAll = Array.from(
        document.querySelectorAll('[data-testid="wizard-step-4"] button')
      ).find((button) => button.textContent?.trim() === 'Select all')
      if (selectAll) selectAll.click()

      return !register.disabled
    }),
    { timeout: 10_000, interval: POLL_WIZARD, timeoutMsg: 'Register button stayed disabled in wizard step 4' }
  )

  await registerBtn.click()

  // Step 4 → 5: Wait for indexing → completion → click dashboard
  const dashboardBtn = await $('[data-testid="go-to-dashboard"]')
  await dashboardBtn.waitForExist({ timeout: 120_000 })
  await dashboardBtn.click()

  await browser.waitUntil(
    async () => {
      const tab = await $('[data-testid="tab-overview"]')
      return await tab.isExisting()
    },
    { timeout: 30_000, interval: POLL_WIZARD, timeoutMsg: 'Overview tab did not appear after wizard completion' }
  )
  return true
}

/**
 * Reset the app to a clean state — call between spec files in persistent mode.
 * Closes overlays/modals, returns to first project on Overview tab, resets theme.
 *
 * Optimized: uses a single browser.execute() to check all state and perform
 * in-page cleanup, minimizing WebDriver round-trips (~1-2 calls instead of ~15).
 */
async function resetAppState() {
  // Close search overlay and the Projects takeover if they are open.
  const searchOverlay = await $('[data-testid="search-overlay"]')
  if (await searchOverlay.isExisting()) await browser.keys('Escape')
  const projectsTakeover = await $('[data-testid="projects-takeover"]')
  if (await projectsTakeover.isExisting()) await browser.keys('Escape')

  // Leave settings if needed.
  const settingsBack = await $('[data-testid="settings-back"]')
  if (await settingsBack.isExisting()) await settingsBack.click()

  // Reset to dark mode and select first project.
  const darkBtn = await $('[data-testid="theme-dark"]')
  if (await darkBtn.isExisting()) await darkBtn.click()
  const firstProject = await $('[data-testid="project-item"]')
  if (await firstProject.isExisting()) await firstProject.click()

  // Return to the default Overview tab.
  const overviewBtn = await $('[data-testid="tab-overview"]')
  if (await overviewBtn.isExisting()) await overviewBtn.click()
}
