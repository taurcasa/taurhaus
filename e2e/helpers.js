/**
 * Shared helpers for Tauri e2e tests.
 */

import {
  PAUSE_BOOT, PAUSE_BOOT_ACTION, POLL_BOOT,
  TIMEOUT_BOOT,
} from './helpers/timing.js'

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

/** Assert setup succeeded. Only first-run-wizard.js may walk onboarding. */
export async function ensureMainApp() {
  if (await $('[data-testid="get-started-button"]').isExisting()) {
    throw new Error('Required post-onboarding seed is absent; refusing a fallback wizard walk')
  }
  await $('[data-testid="tab-overview"]').waitForExist({ timeout: 30_000 })
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
