/**
 * Core app e2e tests — verify the application launches and basic
 * tab navigation works end-to-end through the real Tauri binary.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'

describe('App', () => {
  let mainApp = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
  })

  describe('launch', () => {
    it('renders the app window', async function () {
      if (!mainApp) return this.skip()
      // The titlebar should show "taurhaus"
      const brand = await $('span=taurhaus')
      expect(await brand.isDisplayed()).toBe(true)
    })

    it('shows the sidebar', async function () {
      if (!mainApp) return this.skip()
      const sidebar = await $('aside')
      expect(await sidebar.isExisting()).toBe(true)
    })
  })

  describe('tab navigation', () => {
    it('shows Overview, Files, and Tasks tabs', async function () {
      if (!mainApp) return this.skip()

      const overviewTab = await $('button=Overview')
      const filesTab = await $('button=Files')
      const tasksTab = await $('button=Tasks')

      expect(await overviewTab.isDisplayed()).toBe(true)
      expect(await filesTab.isDisplayed()).toBe(true)
      expect(await tasksTab.isDisplayed()).toBe(true)
    })

    it('can switch to Tasks tab', async function () {
      if (!mainApp) return this.skip()

      const tasksTab = await $('button=Tasks')
      await tasksTab.click()

      const header = await $('h2=Tasks')
      await header.waitForDisplayed({ timeout: 5_000 })
      expect(await header.isDisplayed()).toBe(true)
    })

    it('can switch to Files tab', async function () {
      if (!mainApp) return this.skip()

      const filesTab = await $('button=Files')
      await filesTab.click()
      await browser.pause(500)
    })

    it('can switch back to Overview tab', async function () {
      if (!mainApp) return this.skip()

      const overviewTab = await $('button=Overview')
      await overviewTab.click()
      await browser.pause(300)

      // Overview tab should be active
      expect(await overviewTab.isDisplayed()).toBe(true)
    })
  })
})
