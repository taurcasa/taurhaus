/**
 * Daemon Integration — Tier 2 tests requiring the taurhaus daemon to be running.
 * All tests skip if the daemon is not connected.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded, switchToTab } from '../helpers/navigation.js'

let mainApp = false
let daemonConnected = false

describe('Daemon Integration', () => {
  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()

    if (mainApp) {
      await waitForProjectsLoaded()

      // Check daemon status indicator
      const status = await $('[data-testid="daemon-status"]')
      if (await status.isExisting()) {
        const text = await status.getText()
        daemonConnected = text.toLowerCase().includes('connected')
      }
    }
  })

  // ─── Status ───────────────────────────────────────────────────────────────

  describe('status', () => {
    it('daemon-status indicator shows "connected"', async function () {
      if (!mainApp || !daemonConnected) return this.skip()

      const status = await $('[data-testid="daemon-status"]')
      expect(await status.isExisting()).toBe(true)

      const text = (await status.getText()).toLowerCase()
      expect(text).toContain('connected')
    })
  })

  // ─── Session Management ───────────────────────────────────────────────────

  describe('session management', () => {
    it('session tool logos may appear in sidebar when sessions are active', async function () {
      if (!mainApp || !daemonConnected) return this.skip()

      // Session indicators (tool logos) appear only when CLI tools are running.
      // We check that the sidebar exists and has project items — logos are optional.
      const projects = await $$('[data-testid="project-item"]')
      expect(projects.length).toBeGreaterThan(0)

      // If any session logos are present, they should be SVG elements
      const logos = await $$('[data-testid^="tool-logo-"]')
      for (const logo of logos) {
        // Each logo should be visible
        expect(await logo.isDisplayed()).toBe(true)
      }
      // 0 logos is valid (no CLI tools running) — not a failure
    })

    it('daemon update banner can be dismissed if present', async function () {
      if (!mainApp || !daemonConnected) return this.skip()

      const banner = await $('[data-testid="daemon-update-banner"]')
      if (!(await banner.isExisting())) {
        // No update banner — daemon is up to date. Skip.
        return this.skip()
      }

      // Banner is present — dismiss it
      const dismissBtn = await $('[data-testid="daemon-update-dismiss"]')
      expect(await dismissBtn.isExisting()).toBe(true)

      await dismissBtn.click()

      await browser.waitUntil(
        async () => !(await (await $('[data-testid="daemon-update-banner"]')).isExisting()),
        { timeout: 5_000, interval: 300, timeoutMsg: 'Daemon update banner did not dismiss' }
      )

      expect(await banner.isExisting()).toBe(false)
    })
  })

  // ─── Resilience ───────────────────────────────────────────────────────────

  describe('resilience', () => {
    it('tabs navigate normally with daemon connected', async function () {
      if (!mainApp || !daemonConnected) return this.skip()

      // Navigate through all four tabs — each should render without error
      for (const tabName of ['overview', 'files', 'git', 'tasks']) {
        const tabBtn = await $(`[data-testid="tab-${tabName}"]`)
        if (!(await tabBtn.isExisting())) continue

        await tabBtn.click()
        await browser.pause(500)

        // Tab button should still be present (no crash/redirect)
        expect(await tabBtn.isExisting()).toBe(true)
      }
    })

    it('settings view is accessible with daemon running', async function () {
      if (!mainApp || !daemonConnected) return this.skip()

      const toggle = await $('[data-testid="settings-toggle"]')
      if (!(await toggle.isExisting())) return this.skip()

      await toggle.click()

      await browser.waitUntil(
        async () => await (await $('[data-testid="settings-view"]')).isExisting(),
        { timeout: 5_000, interval: 300, timeoutMsg: 'Settings view did not open with daemon running' }
      )

      const settings = await $('[data-testid="settings-view"]')
      expect(await settings.isExisting()).toBe(true)

      // Close settings
      await toggle.click()
    })

    it('Overview tab loads content with daemon connected', async function () {
      if (!mainApp || !daemonConnected) return this.skip()

      const overviewBtn = await $('[data-testid="tab-overview"]')
      if (!(await overviewBtn.isExisting())) return this.skip()

      await overviewBtn.click()

      await browser.waitUntil(
        async () => {
          const quickActions = await $('[data-testid="quick-actions"]')
          const readme = await $('[data-testid="overview-readme"]')
          return (await quickActions.isExisting()) || (await readme.isExisting())
        },
        { timeout: 10_000, interval: 500, timeoutMsg: 'Overview content did not load with daemon running' }
      )
    })

    it('sidebar project list remains populated with daemon connected', async function () {
      if (!mainApp || !daemonConnected) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      expect(projects.length).toBeGreaterThan(0)

      // Sidebar skeleton should be gone
      const skeleton = await $('[data-testid="sidebar-skeleton"]')
      expect(await skeleton.isExisting()).toBe(false)
    })
  })
})
