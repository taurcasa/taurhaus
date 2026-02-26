/**
 * Shared helpers for Tauri e2e tests.
 */

/**
 * Wait for the app to be ready — handles splash screen, wizard, or main shell.
 *
 * The boot sequence is: Splash Screen → (Wizard | Main Shell).
 * The splash screen waits for the daemon, then fades to the app.
 * If the daemon doesn't start within 15s, the splash shows "Continue anyway."
 * We handle all three entry points.
 */
export async function waitForAppReady() {
  // Give the app time to start up before querying the DOM
  await browser.pause(3_000)

  await browser.waitUntil(
    async () => {
      // Check if we're already past the splash into main app
      const overviewTab = await $('button=Overview')
      if (await overviewTab.isExisting()) return true

      // Check for wizard (first-run)
      const wizard = await $('[data-testid="get-started-button"]')
      if (await wizard.isExisting()) return true

      // Splash might be showing "Continue anyway" — click it to proceed
      const continueBtn = await $('button=Continue anyway')
      if (await continueBtn.isExisting()) {
        await continueBtn.click()
        await browser.pause(500)
        return false // Will check again on next iteration
      }

      return false
    },
    {
      timeout: 45_000,
      interval: 1_000,
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
  const overviewTab = await $('button=Overview')
  if (await overviewTab.isExisting()) return true

  // Must be the wizard — navigate through it
  const getStarted = await $('[data-testid="get-started-button"]')
  if (!(await getStarted.isExisting())) return false

  // Step 1: Get started
  await getStarted.click()

  // Step 2: Daemon setup — auto-proceeds if installed, otherwise skip
  const daemonStep = await $('[data-testid="wizard-step-2"]')
  if (await daemonStep.isExisting()) {
    // Wait for auto-proceed (daemon already installed) or skip
    const skipBtn = await $('[data-testid="daemon-skip-button"]')
    const browseStep = await $('[data-testid="wizard-step-3"]')
    await browser.waitUntil(
      async () => {
        if (await browseStep.isExisting()) return true
        if (await skipBtn.isExisting()) {
          await skipBtn.click()
          return true
        }
        return false
      },
      { timeout: 10_000, interval: 500 }
    )
  }

  // Step 3: Scan for projects
  const input = await $('[data-testid="wizard-step-3"] input[type="text"]')
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
