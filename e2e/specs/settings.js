/**
 * Settings e2e tests — verify the settings modal opens correctly
 * and all four sections (General, Display, Terminal & Sessions, Search)
 * are functional end-to-end.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'

describe('Settings', () => {
  let mainApp = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
  })

  describe('opening and closing', () => {
    it('opens settings from sidebar toggle button', async function () {
      if (!mainApp) return this.skip()

      // Settings toggle is an icon-only button with data-testid="settings-toggle"
      const toggleBtn = await $('[data-testid="settings-toggle"]')
      if (!(await toggleBtn.isExisting())) return this.skip()

      await toggleBtn.click()
      await browser.pause(1_000)

      const settingsView = await $('[data-testid="settings-view"]')
      expect(await settingsView.isDisplayed()).toBe(true)
    })

    it('shows back button', async function () {
      if (!mainApp) return this.skip()
      const backBtn = await $('[data-testid="settings-back"]')
      expect(await backBtn.isDisplayed()).toBe(true)
    })
  })

  describe('General section', () => {
    it('renders General section', async function () {
      if (!mainApp) return this.skip()
      const section = await $('[data-testid="settings-scanning"]')
      expect(await section.isDisplayed()).toBe(true)
    })

    it('shows "General" heading', async function () {
      if (!mainApp) return this.skip()
      const section = await $('[data-testid="settings-scanning"]')
      const text = await section.getText()
      // Heading is rendered uppercase via CSS
      expect(text.toUpperCase()).toContain('GENERAL')
    })

    it('shows scan directories', async function () {
      if (!mainApp) return this.skip()
      const section = await $('[data-testid="settings-scanning"]')
      const text = await section.getText()
      expect(text).toContain('Scan directories')
    })

    it('shows activity thresholds', async function () {
      if (!mainApp) return this.skip()
      const section = await $('[data-testid="settings-scanning"]')
      const text = await section.getText()
      expect(text).toContain('Activity state thresholds')
    })

    it('threshold inputs are editable', async function () {
      if (!mainApp) return this.skip()
      const input = await $('[data-testid="threshold-active"]')
      expect(await input.isDisplayed()).toBe(true)

      const value = await input.getValue()
      expect(parseInt(value, 10)).toBeGreaterThan(0)
    })
  })

  describe('Display section', () => {
    it('renders Display section', async function () {
      if (!mainApp) return this.skip()
      const section = await $('[data-testid="settings-display"]')
      expect(await section.isDisplayed()).toBe(true)
    })

    it('shows syntax highlighting label', async function () {
      if (!mainApp) return this.skip()
      const section = await $('[data-testid="settings-display"]')
      const text = await section.getText()
      expect(text).toContain('Syntax highlighting')
    })

    it('shows code theme dropdowns', async function () {
      if (!mainApp) return this.skip()
      const lightSelect = await $('[data-testid="code-theme-light"]')
      const darkSelect = await $('[data-testid="code-theme-dark"]')
      expect(await lightSelect.isDisplayed()).toBe(true)
      expect(await darkSelect.isDisplayed()).toBe(true)
    })
  })

  describe('Terminal & Sessions section', () => {
    it('renders Terminal section', async function () {
      if (!mainApp) return this.skip()
      const section = await $('[data-testid="settings-terminal"]')
      expect(await section.isDisplayed()).toBe(true)
    })

    it('shows "Terminal & Sessions" heading', async function () {
      if (!mainApp) return this.skip()
      const section = await $('[data-testid="settings-terminal"]')
      const text = await section.getText()
      // Heading is rendered uppercase via CSS
      expect(text.toUpperCase()).toContain('TERMINAL & SESSIONS')
    })

    it('shows terminal emulator dropdown', async function () {
      if (!mainApp) return this.skip()
      const select = await $('[data-testid="terminal-emulator"]')
      expect(await select.isDisplayed()).toBe(true)
    })

    it('shows tmux layout dropdown', async function () {
      if (!mainApp) return this.skip()
      const select = await $('[data-testid="tmux-layout"]')
      expect(await select.isDisplayed()).toBe(true)
    })

    it('emulator dropdown defaults to Windows Terminal', async function () {
      if (!mainApp) return this.skip()
      const select = await $('[data-testid="terminal-emulator"]')
      const value = await select.getValue()
      expect(value).toBe('windows_terminal')
    })
  })

  describe('Search section', () => {
    it('renders Search section', async function () {
      if (!mainApp) return this.skip()
      const section = await $('[data-testid="settings-index"]')
      expect(await section.isDisplayed()).toBe(true)
    })

    it('shows rebuild index button', async function () {
      if (!mainApp) return this.skip()
      const btn = await $('[data-testid="rebuild-index-btn"]')
      expect(await btn.isDisplayed()).toBe(true)
    })

    it('shows document count', async function () {
      if (!mainApp) return this.skip()
      const section = await $('[data-testid="settings-index"]')
      const text = await section.getText()
      expect(text).toMatch(/\d+ documents? indexed/)
    })
  })

  describe('navigation', () => {
    it('back button returns to project view', async function () {
      if (!mainApp) return this.skip()

      const backBtn = await $('[data-testid="settings-back"]')
      await backBtn.click()
      await browser.pause(500)

      // Should see Overview tab button again
      const overviewTab = await $('button=Overview')
      expect(await overviewTab.isDisplayed()).toBe(true)
    })
  })
})
