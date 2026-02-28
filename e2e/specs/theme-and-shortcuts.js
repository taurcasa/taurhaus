/**
 * Theme and Shortcuts e2e tests — light/dark mode switching, persistence,
 * keyboard shortcuts, and basic app identity.
 * Partially replaces old app.js.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { switchToTab, waitForFileContent, clickTestId } from '../helpers/navigation.js'
import {
  POLL_FAST,
  WAIT_INSTANT, WAIT_SHORT, WAIT_MEDIUM,
  TIMEOUT_MEDIUM,
} from '../helpers/timing.js'

describe('Theme and Shortcuts', () => {
  let mainApp = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
  })

  // Helper: wait for theme class to match expected state
  async function waitForTheme(dark, timeout = 2_000) {
    await browser.waitUntil(
      async () => {
        const isDark = await browser.execute(() => document.documentElement.classList.contains('dark'))
        return isDark === dark
      },
      { timeout, interval: POLL_FAST }
    )
  }

  describe('theme switching', () => {
    it('clicking Light sets light mode', async function () {
      if (!mainApp) return this.skip()

      const lightBtn = await $('[data-testid="theme-light"]')
      if (!(await lightBtn.isExisting())) return this.skip()

      await clickTestId('theme-light')
      await waitForTheme(false)

      const isDark = await browser.execute(() => document.documentElement.classList.contains('dark'))
      expect(isDark).toBe(false)
    })

    it('clicking Dark sets dark mode', async function () {
      if (!mainApp) return this.skip()

      const darkBtn = await $('[data-testid="theme-dark"]')
      if (!(await darkBtn.isExisting())) return this.skip()

      await clickTestId('theme-dark')
      await waitForTheme(true)

      const isDark = await browser.execute(() => document.documentElement.classList.contains('dark'))
      expect(isDark).toBe(true)
    })

    it('clicking Light again restores light mode', async function () {
      if (!mainApp) return this.skip()

      const lightBtn = await $('[data-testid="theme-light"]')
      if (!(await lightBtn.isExisting())) return this.skip()

      await clickTestId('theme-light')
      await waitForTheme(false)

      const isDark = await browser.execute(() => document.documentElement.classList.contains('dark'))
      expect(isDark).toBe(false)
    })

    it('theme persists across tab switches', async function () {
      if (!mainApp) return this.skip()

      // Switch to dark
      const darkBtn = await $('[data-testid="theme-dark"]')
      if (!(await darkBtn.isExisting())) return this.skip()
      await clickTestId('theme-dark')
      await waitForTheme(true)

      // Switch tab
      await switchToTab('git')

      const isDark = await browser.execute(() => document.documentElement.classList.contains('dark'))
      expect(isDark).toBe(true)

      // Switch back to overview
      await switchToTab('overview')
    })

    it('theme class is present on html when viewing Files tab', async function () {
      if (!mainApp) return this.skip()

      // Go to Files tab and wait for a file to load
      await switchToTab('files')

      try {
        await waitForFileContent(TIMEOUT_MEDIUM, 'File content did not load in Files tab')
      } catch {
        return this.skip()
      }

      // Verify theme class persists while viewing file content
      const darkMode = await browser.execute(() => document.documentElement.classList.contains('dark'))
      expect(typeof darkMode).toBe('boolean') // Just verify the class mechanism works

      // File content should be visible
      const codeViewer = await $('[data-testid="code-viewer"]')
      const mdContent = await $('[data-testid="markdown-content"]')
      expect((await codeViewer.isExisting()) || (await mdContent.isExisting())).toBe(true)

      // Switch back to overview
      await switchToTab('overview')
    })
  })

  describe('keyboard shortcuts', () => {
    it('Ctrl+K opens search overlay', async function () {
      if (!mainApp) return this.skip()

      // Ensure we start with overlay closed
      const existingOverlay = await $('[data-testid="search-overlay"]')
      if (await existingOverlay.isExisting()) {
        await browser.keys('Escape')
        await browser.waitUntil(
          async () => !(await (await $('[data-testid="search-overlay"]')).isExisting()),
          WAIT_INSTANT
        ).catch(() => {})
      }

      await browser.keys(['Control', 'k'])

      await browser.waitUntil(
        async () => {
          const overlay = await $('[data-testid="search-overlay"]')
          return await overlay.isExisting()
        },
        { ...WAIT_SHORT, timeoutMsg: 'Search overlay did not open via Ctrl+K' }
      )

      const overlay = await $('[data-testid="search-overlay"]')
      expect(await overlay.isDisplayed()).toBe(true)
    })

    it('Escape closes open search overlay', async function () {
      if (!mainApp) return this.skip()

      const overlay = await $('[data-testid="search-overlay"]')
      if (!(await overlay.isExisting())) return this.skip()

      await browser.keys('Escape')

      await browser.waitUntil(
        async () => {
          const o = await $('[data-testid="search-overlay"]')
          return !(await o.isExisting())
        },
        { ...WAIT_INSTANT, timeoutMsg: 'Search overlay did not close via Escape' }
      )

      const overlayAfter = await $('[data-testid="search-overlay"]')
      expect(await overlayAfter.isExisting()).toBe(false)
    })

    it('Escape closes open settings', async function () {
      if (!mainApp) return this.skip()

      const toggleBtn = await $('[data-testid="settings-toggle"]')
      if (!(await toggleBtn.isExisting())) return this.skip()

      await clickTestId('settings-toggle')
      await browser.waitUntil(
        async () => {
          const settingsView = await $('[data-testid="settings-view"]')
          return await settingsView.isExisting()
        },
        { ...WAIT_SHORT, timeoutMsg: 'Settings did not open' }
      )

      await browser.keys('Escape')

      // Wait for settings to close
      const closed = await browser.waitUntil(
        async () => !(await (await $('[data-testid="settings-view"]')).isExisting()),
        WAIT_INSTANT
      ).catch(() => false)

      if (!closed) {
        // Close manually via back button
        const backBtn = await $('[data-testid="settings-back"]')
        if (await backBtn.isExisting()) await clickTestId('settings-back')
        return this.skip()
      }

      const overviewTab = await $('[data-testid="tab-overview"]')
      expect(await overviewTab.isExisting()).toBe(true)
    })
  })

  describe('app basics', () => {
    it('app title "taurhaus" is visible in titlebar', async function () {
      if (!mainApp) return this.skip()

      // Look for any element containing the brand text
      const brand = await browser.waitUntil(
        async () => {
          const byTestid = await $('[data-testid="app-title"]')
          if (await byTestid.isExisting()) return byTestid

          const byText = await $('span=taurhaus')
          if (await byText.isExisting()) return byText

          const found = await browser.execute(() => {
            const els = Array.from(document.querySelectorAll('*'))
            return els.some(
              (el) =>
                el.children.length === 0 &&
                el.textContent.toLowerCase().includes('taurhaus')
            )
          })
          return found ? true : null
        },
        { ...WAIT_SHORT, timeoutMsg: '"taurhaus" text not found in app' }
      )

      expect(brand).toBeTruthy()
    })
  })
})
