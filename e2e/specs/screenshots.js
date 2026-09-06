/** Native theme transitions retained from the general capture lane. */
import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { clickTestId } from '../helpers/navigation.js'
import { POLL_FAST } from '../helpers/timing.js'

describe('Native theme transitions', () => {
  before(async () => {
    await waitForAppReady()
    await ensureMainApp()
  })

  it('02 — Overview tab (light mode)', async () => {
    await clickTestId('theme-light')
    await browser.waitUntil(
      async () => !(await browser.execute(() => document.documentElement.classList.contains('dark'))),
      { timeout: 1_000, interval: POLL_FAST }
    )
  })

  it('06 — Switch back to dark mode', async () => {
    await clickTestId('theme-dark')
    await browser.waitUntil(
      async () => await browser.execute(() => document.documentElement.classList.contains('dark')),
      { timeout: 1_000, interval: POLL_FAST }
    )
  })

})
