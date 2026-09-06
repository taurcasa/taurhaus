/**
 * First-run wizard e2e tests — cover the onboarding flow from welcome
 * through dashboard handoff.
 */

import { waitForAppReady } from '../helpers.js'
import { PROJECTS_DIR } from '../helpers/platform.js'
import { POLL_WIZARD, WAIT_LONG } from '../helpers/timing.js'
import { assertOnboardedProjects, invokeApp } from '../helpers/onboarding.js'

describe('First-run wizard', () => {
  before(async () => {
    await waitForAppReady()
  })

  it('guides first launch from welcome to completion', async () => {
    const getStarted = await $('[data-testid="get-started-button"]')
    await getStarted.waitForExist({ timeout: 10_000 })
    expect(await (await $('[data-testid="wizard-step-1"]')).isExisting()).toBe(true)

    await getStarted.click()

    const daemonStep = await $('[data-testid="wizard-step-2"]')
    // Regression: c3d5ea841 asserted display immediately after DOM insertion,
    // racing the wizard transition under full-suite load.
    await daemonStep.waitForDisplayed({ timeout: 5_000 })

    await browser.waitUntil(
      async () => {
        const browseStep = await $('[data-testid="wizard-step-3"]')
        if (await browseStep.isExisting()) return true

        const skipButton = await $('[data-testid="daemon-skip-button"]')
        if (await skipButton.isExisting()) {
          await skipButton.click()
        }

        return await browseStep.isExisting()
      },
      {
        timeout: 15_000,
        interval: POLL_WIZARD,
        timeoutMsg: 'Wizard did not reach the browse step from daemon setup'
      }
    )

    const browseStep = await $('[data-testid="wizard-step-3"]')
    expect(await browseStep.isDisplayed()).toBe(true)

    const pathInput = await $('[data-testid="wizard-step-3"] input[type="text"]')
    await pathInput.waitForExist({ timeout: 5_000 })
    await pathInput.setValue(PROJECTS_DIR)
    expect(await pathInput.getValue()).toBe(PROJECTS_DIR)

    const scanButton = await $('[data-testid="scan-button"]')
    await scanButton.click()

    const selectionStep = await $('[data-testid="wizard-step-4"]')
    await selectionStep.waitForExist({ timeout: 30_000 })
    expect(await selectionStep.isDisplayed()).toBe(true)

    await browser.waitUntil(
      async () => {
        const registerButton = await $('[data-testid="register-button"]')
        if (!(await registerButton.isExisting())) return false
        if (await registerButton.isEnabled()) return true

        const selectAllButton = await browser.execute(() => {
          return Array.from(document.querySelectorAll('[data-testid="wizard-step-4"] button')).find(
            (button) => button.textContent?.trim() === 'Select all'
          ) !== undefined
        })

        if (selectAllButton) {
          await browser.execute(() => {
            const button = Array.from(document.querySelectorAll('[data-testid="wizard-step-4"] button')).find(
              (candidate) => candidate.textContent?.trim() === 'Select all'
            )
            button?.click()
          })
        }

        return await registerButton.isEnabled()
      },
      {
        ...WAIT_LONG,
        timeout: 10_000,
        interval: POLL_WIZARD,
        timeoutMsg: 'Register button stayed disabled after scanning fixture projects'
      }
    )

    const selectionText = await browser.execute(() => {
      const step = document.querySelector('[data-testid="wizard-step-4"]')
      return step?.textContent ?? ''
    })
    expect(selectionText).toContain('Found 2 repositories')
    expect(selectionText).toContain('taurhaus')
    expect(selectionText).toContain('ledger')

    const registerButton = await $('[data-testid="register-button"]')
    const registerLabel = await registerButton.getText()
    expect(registerLabel).toContain('Register 2 projects')

    await browser.execute(() => {
      const seen = { step5: false, step6: false }
      const markSeen = () => {
        if (document.querySelector('[data-testid="wizard-step-5"]')) seen.step5 = true
        if (document.querySelector('[data-testid="wizard-step-6"]')) seen.step6 = true
      }

      markSeen()

      const existingObserver = window.__taurhausWizardObserver
      if (existingObserver) existingObserver.disconnect()

      const observer = new MutationObserver(() => {
        markSeen()
      })
      observer.observe(document.body, { childList: true, subtree: true, attributes: true })

      window.__taurhausWizardSeen = seen
      window.__taurhausWizardObserver = observer
    })

    await registerButton.click()

    // Regression: c3d5ea841 stopped waiting as soon as step 5 appeared, then
    // disconnected the observer before the asynchronous completion step rendered.
    await browser.waitUntil(
      async () => {
        const completionStep = await $('[data-testid="wizard-step-6"]')
        if (await completionStep.isExisting()) return true

        const overviewTab = await $('[data-testid="tab-overview"]')
        return await overviewTab.isExisting()
      },
      {
        timeout: 120_000,
        interval: POLL_WIZARD,
        timeoutMsg: 'Wizard did not complete project registration or reach the dashboard'
      }
    )

    const seenWizardSteps = await browser.execute(() => {
      window.__taurhausWizardObserver?.disconnect?.()
      return window.__taurhausWizardSeen ?? { step5: false, step6: false }
    })
    expect(seenWizardSteps.step5).toBe(true)
    expect(seenWizardSteps.step6).toBe(true)

    const completionText = await browser.execute(() => {
      const step = document.querySelector('[data-testid="wizard-step-6"]')
      return step?.textContent ?? ''
    })
    if (completionText) {
      expect(completionText).toContain('2 projects registered')
      expect(completionText).toContain("You're all set.")
    }

    await browser.waitUntil(
      async () => {
        const overviewTab = await $('[data-testid="tab-overview"]')
        return await overviewTab.isExisting()
      },
      { timeout: 30_000, interval: POLL_WIZARD, timeoutMsg: 'Overview tab did not appear after finishing the wizard' }
    )

    const dashboardButton = await $('[data-testid="go-to-dashboard"]')
    if (await dashboardButton.isExisting()) {
      await dashboardButton.click()
    }

    const projectItems = await $$('[data-testid="project-item"]')
    expect(projectItems.length).toBeGreaterThanOrEqual(2)
    assertOnboardedProjects(await invokeApp('list_projects'), await invokeApp('is_first_run'))
    await browser.refresh()
    await $('[data-testid="tab-overview"]').waitForExist({ timeout: 30_000 })
    expect(await $('[data-testid="first-run-wizard"]').isExisting()).toBe(false)
    assertOnboardedProjects(await invokeApp('list_projects'), await invokeApp('is_first_run'))
  })
})
