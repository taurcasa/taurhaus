/**
 * Shared helpers for Tauri e2e tests.
 */

/**
 * Wait for the app to be ready — either showing the wizard or the main shell.
 * Uses a generous timeout because the Tauri app + Svelte hydration can be slow.
 */
export async function waitForAppReady() {
  // Give the app time to start up before querying the DOM
  await browser.pause(3_000)

  // Look for the Overview tab button — present in the main shell (most common path)
  // Falls back to the wizard if this is a fresh install
  const overviewTab = await $('button=Overview')
  const wizard = await $('[data-testid="get-started-button"]')

  await browser.waitUntil(
    async () => {
      return (await overviewTab.isExisting()) || (await wizard.isExisting())
    },
    {
      timeout: 30_000,
      interval: 1_000,
      timeoutMsg: 'App did not render within 30s (checked for Overview tab and wizard)'
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
  const overviewTab = await $('button=Overview')
  if (await overviewTab.isExisting()) return true

  // Must be the wizard — navigate through it
  const getStarted = await $('[data-testid="get-started-button"]')
  if (!(await getStarted.isExisting())) return false

  // Step 1: Get started
  await getStarted.click()

  // Step 2: Scan for projects
  const input = await $('[data-testid="wizard-step-2"] input[type="text"]')
  await input.waitForExist({ timeout: 5_000 })
  await input.setValue('/home/mstie/projects')

  const scanBtn = await $('[data-testid="scan-button"]')
  await scanBtn.click()

  // Step 3: Register projects
  const registerBtn = await $('[data-testid="register-button"]')
  await registerBtn.waitForExist({ timeout: 30_000 })
  await registerBtn.click()

  // Step 4 → 5: Wait for indexing → completion → click dashboard
  const dashboardBtn = await $('[data-testid="go-to-dashboard"]')
  await dashboardBtn.waitForExist({ timeout: 120_000 })
  await dashboardBtn.click()

  // Wait for main app
  await overviewTab.waitForExist({ timeout: 15_000 })
  return true
}
