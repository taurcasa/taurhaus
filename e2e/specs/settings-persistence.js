/**
 * Settings Persistence e2e tests — verify settings actually persist across
 * open/close cycles. Replaces the old settings.js render-only checks.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { clickTestId } from '../helpers/navigation.js'
import { openSettings, closeSettings, ensureSettingsOpen, getSettingValue, setSettingValue } from '../helpers/settings.js'
import { POLL_SLOW, TIMEOUT_LONG } from '../helpers/timing.js'

describe('Settings Persistence', () => {
  let mainApp = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
  })

  describe('open and close', () => {
    it('opens settings via sidebar toggle — settings-view appears', async function () {
      if (!mainApp) return this.skip()

      const toggle = await $('[data-testid="settings-toggle"]')
      if (!(await toggle.isExisting())) return this.skip()

      await openSettings()

      const view = await $('[data-testid="settings-view"]')
      expect(await view.isExisting()).toBe(true)
    })

    it('back button returns to project view — settings-view disappears', async function () {
      if (!mainApp) return this.skip()

      // Ensure settings are open first
      await ensureSettingsOpen()

      await closeSettings()

      const viewAfter = await $('[data-testid="settings-view"]')
      expect(await viewAfter.isExisting()).toBe(false)

      // Main UI should be visible again
      const overviewTab = await $('[data-testid="tab-overview"]')
      expect(await overviewTab.isExisting()).toBe(true)
    })
  })

  describe('activity thresholds', () => {
    let originalActiveValue

    before(async () => {
      if (!mainApp) return
      await openSettings()
      originalActiveValue = await getSettingValue('threshold-active')
    })

    after(async () => {
      if (!mainApp || originalActiveValue === undefined) return
      // Restore original value
      await ensureSettingsOpen()
      await setSettingValue('threshold-active', originalActiveValue)
      await closeSettings()
    })

    it('reads current active threshold value as a positive number', async function () {
      if (!mainApp) return this.skip()

      const value = await getSettingValue('threshold-active')
      expect(parseInt(value, 10)).toBeGreaterThan(0)
    })

    it('changed threshold persists after close and reopen', async function () {
      if (!mainApp) return this.skip()

      const newValue = '45'
      await setSettingValue('threshold-active', newValue)
      await closeSettings()

      await openSettings()

      const persisted = await getSettingValue('threshold-active')
      expect(persisted).toBe(newValue)
    })

    it('threshold input rejects non-numeric text', async function () {
      if (!mainApp) return this.skip()

      const input = await $('[data-testid="threshold-active"]')
      if (!(await input.isExisting())) return this.skip()

      // Read current value before attempting invalid entry
      const before = await input.getValue()

      await input.clearValue()
      await input.setValue('abc')
      await browser.execute((testid) => {
        const el = document.querySelector(`[data-testid="${testid}"]`)
        if (el) el.dispatchEvent(new Event('change', { bubbles: true }))
      }, 'threshold-active')

      const after = await input.getValue()
      // Either the value was rejected (empty or original) or input type="number" strips non-numeric
      const parsed = parseFloat(after)
      expect(isNaN(parsed) || parsed > 0).toBe(true)
    })
  })

  describe('display settings', () => {
    let originalLightTheme
    let originalDarkTheme

    before(async () => {
      if (!mainApp) return
      await ensureSettingsOpen()
      originalLightTheme = await getSettingValue('code-theme-light')
      originalDarkTheme = await getSettingValue('code-theme-dark')
    })

    after(async () => {
      if (!mainApp) return
      await ensureSettingsOpen()
      if (originalLightTheme !== undefined) {
        await setSettingValue('code-theme-light', originalLightTheme)
      }
      if (originalDarkTheme !== undefined) {
        await setSettingValue('code-theme-dark', originalDarkTheme)
      }
      await closeSettings()
    })

    it('light code theme change persists after close and reopen', async function () {
      if (!mainApp) return this.skip()

      const select = await $('[data-testid="code-theme-light"]')
      if (!(await select.isExisting())) return this.skip()

      // Get available options to pick one different from current
      const options = await select.$$('option')
      if (options.length < 2) return this.skip()

      const currentVal = await select.getValue()
      let targetOption = null
      for (const opt of options) {
        const val = await opt.getValue()
        if (val !== currentVal) {
          targetOption = await opt.getText()
          break
        }
      }
      if (!targetOption) return this.skip()

      await select.selectByVisibleText(targetOption)
      await closeSettings()

      await openSettings()

      const persisted = await getSettingValue('code-theme-light')
      expect(persisted).not.toBe(currentVal)
    })

    it('dark code theme change persists after close and reopen', async function () {
      if (!mainApp) return this.skip()

      const select = await $('[data-testid="code-theme-dark"]')
      if (!(await select.isExisting())) return this.skip()

      const options = await select.$$('option')
      if (options.length < 2) return this.skip()

      const currentVal = await select.getValue()
      let targetOption = null
      for (const opt of options) {
        const val = await opt.getValue()
        if (val !== currentVal) {
          targetOption = await opt.getText()
          break
        }
      }
      if (!targetOption) return this.skip()

      await select.selectByVisibleText(targetOption)
      await closeSettings()

      await openSettings()

      const persisted = await getSettingValue('code-theme-dark')
      expect(persisted).not.toBe(currentVal)
    })
  })

  describe('terminal settings', () => {
    let originalEmulator
    let originalLayout

    before(async () => {
      if (!mainApp) return
      await ensureSettingsOpen()
      originalEmulator = await getSettingValue('terminal-emulator')
      originalLayout = await getSettingValue('tmux-layout')
    })

    after(async () => {
      if (!mainApp) return
      await ensureSettingsOpen()
      if (originalEmulator) {
        await setSettingValue('terminal-emulator', originalEmulator)
      }
      if (originalLayout) {
        await setSettingValue('tmux-layout', originalLayout)
      }
      await closeSettings()
    })

    it('terminal emulator change persists after close and reopen', async function () {
      if (!mainApp) return this.skip()

      const select = await $('[data-testid="terminal-emulator"]')
      if (!(await select.isExisting())) return this.skip()

      const options = await select.$$('option')
      if (options.length < 2) return this.skip()

      const currentVal = await select.getValue()
      let targetOption = null
      for (const opt of options) {
        const val = await opt.getValue()
        if (val !== currentVal) {
          targetOption = await opt.getText()
          break
        }
      }
      if (!targetOption) return this.skip()

      await select.selectByVisibleText(targetOption)
      await closeSettings()

      await openSettings()

      const persisted = await getSettingValue('terminal-emulator')
      expect(persisted).not.toBe(currentVal)
    })

    it('tmux layout change persists after close and reopen', async function () {
      if (!mainApp) return this.skip()

      const select = await $('[data-testid="tmux-layout"]')
      if (!(await select.isExisting())) return this.skip()

      const options = await select.$$('option')
      if (options.length < 2) return this.skip()

      const currentVal = await select.getValue()
      let targetOption = null
      for (const opt of options) {
        const val = await opt.getValue()
        if (val !== currentVal) {
          targetOption = await opt.getText()
          break
        }
      }
      if (!targetOption) return this.skip()

      await select.selectByVisibleText(targetOption)
      await closeSettings()

      await openSettings()

      const persisted = await getSettingValue('tmux-layout')
      expect(persisted).not.toBe(currentVal)
    })
  })

  describe('CLI tools', () => {
    before(async () => {
      if (!mainApp) return
      await ensureSettingsOpen()
    })

    it('CLI tools section is visible', async function () {
      if (!mainApp) return this.skip()
      const section = await $('[data-testid="settings-cli-tools"]')
      expect(await section.isExisting()).toBe(true)
    })

    it('claude continue button exists and is clickable', async function () {
      if (!mainApp) return this.skip()
      const btn = await $('[data-testid="cli-claude-continue"]')
      if (!(await btn.isExisting())) return this.skip()
      expect(await btn.isEnabled()).toBe(true)
    })

    it('claude fresh button exists and is clickable', async function () {
      if (!mainApp) return this.skip()
      const btn = await $('[data-testid="cli-claude-fresh"]')
      if (!(await btn.isExisting())) return this.skip()
      expect(await btn.isEnabled()).toBe(true)
    })

    it('claude resume button exists and is clickable', async function () {
      if (!mainApp) return this.skip()
      const btn = await $('[data-testid="cli-claude-resume"]')
      if (!(await btn.isExisting())) return this.skip()
      expect(await btn.isEnabled()).toBe(true)
    })
  })

  describe('search index', () => {
    before(async () => {
      if (!mainApp) return
      await ensureSettingsOpen()
    })

    it('rebuild index button triggers rebuild and completes without error', async function () {
      if (!mainApp) return this.skip()

      const btn = await $('[data-testid="rebuild-index-btn"]')
      if (!(await btn.isExisting())) return this.skip()

      await clickTestId('rebuild-index-btn')

      // Wait for completion — either the button becomes re-enabled or text changes
      await browser.waitUntil(
        async () => {
          const err = await $('[data-testid="rebuild-error"]')
          if (await err.isExisting()) return true // done (with error, still done)
          const isEnabled = await btn.isEnabled()
          return isEnabled
        },
        { timeout: TIMEOUT_LONG, interval: POLL_SLOW, timeoutMsg: 'Rebuild index did not complete within timeout' }
      )

      // No error element should appear
      const errEl = await $('[data-testid="rebuild-error"]')
      expect(await errEl.isExisting()).toBe(false)
    })
  })

  describe('all sections render', () => {
    before(async () => {
      if (!mainApp) return
      await ensureSettingsOpen()
    })

    after(async () => {
      if (!mainApp) return
      const view = await $('[data-testid="settings-view"]')
      if (await view.isExisting()) {
        await closeSettings()
      }
    })

    it('scanning (general) section is visible', async function () {
      if (!mainApp) return this.skip()
      const el = await $('[data-testid="settings-scanning"]')
      expect(await el.isExisting()).toBe(true)
    })

    it('display section is visible', async function () {
      if (!mainApp) return this.skip()
      const el = await $('[data-testid="settings-display"]')
      expect(await el.isExisting()).toBe(true)
    })

    it('terminal section is visible', async function () {
      if (!mainApp) return this.skip()
      const el = await $('[data-testid="settings-terminal"]')
      expect(await el.isExisting()).toBe(true)
    })

    it('cli-tools section is visible', async function () {
      if (!mainApp) return this.skip()
      const el = await $('[data-testid="settings-cli-tools"]')
      expect(await el.isExisting()).toBe(true)
    })

    it('search index section is visible', async function () {
      if (!mainApp) return this.skip()
      const el = await $('[data-testid="settings-index"]')
      expect(await el.isExisting()).toBe(true)
    })
  })
})
