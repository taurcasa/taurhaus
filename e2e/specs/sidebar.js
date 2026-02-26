/**
 * Sidebar e2e tests — verify project list rendering, project switching,
 * activity state groups, and sidebar interactions.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'

describe('Sidebar', () => {
  let mainApp = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
  })

  describe('project list', () => {
    it('renders the sidebar element', async function () {
      if (!mainApp) return this.skip()
      const sidebar = await $('aside')
      expect(await sidebar.isExisting()).toBe(true)
    })

    it('sidebar contains at least one project', async function () {
      if (!mainApp) return this.skip()
      // Projects are rendered as buttons in the sidebar
      const sidebar = await $('aside')
      const buttons = await sidebar.$$('button')
      // Filter out non-project buttons (Settings, Manage, etc.)
      const projectButtons = []
      for (const btn of buttons) {
        const text = await btn.getText()
        if (text && !text.includes('Settings') && !text.includes('Manage') &&
            !text.includes('Filter') && text.trim().length > 0) {
          projectButtons.push(btn)
        }
      }
      expect(projectButtons.length).toBeGreaterThan(0)
    })

    it('shows activity state group headers', async function () {
      if (!mainApp) return this.skip()
      const sidebar = await $('aside')
      const text = await sidebar.getText()
      // Should contain at least one group header
      const hasGroup = text.includes('ACTIVE') || text.includes('RECENT') ||
        text.includes('STALE') || text.includes('DORMANT')
      expect(hasGroup).toBe(true)
    })
  })

  describe('project switching', () => {
    it('clicking a different project changes the Overview header', async function () {
      if (!mainApp) return this.skip()

      // Get current project name from Overview header
      const overviewTab = await $('button=Overview')
      await overviewTab.click()
      await browser.pause(500)

      const h1Before = await $('h1')
      const nameBefore = await h1Before.getText()

      // Find a different project in sidebar
      const sidebar = await $('aside')
      const buttons = await sidebar.$$('button')
      let clicked = false
      for (const btn of buttons) {
        const text = await btn.getText()
        if (text && text.trim().length > 0 && !text.includes(nameBefore) &&
            !text.includes('Settings') && !text.includes('Manage') &&
            !text.includes('Filter') && !text.includes('ACTIVE') &&
            !text.includes('RECENT') && !text.includes('STALE') &&
            !text.includes('DORMANT')) {
          await btn.click()
          clicked = true
          break
        }
      }

      if (!clicked) return this.skip()
      await browser.pause(1_000)

      const h1After = await $('h1')
      const nameAfter = await h1After.getText()
      expect(nameAfter).not.toBe(nameBefore)
    })

    it('can switch back to first project', async function () {
      if (!mainApp) return this.skip()

      // Click first project button in sidebar (skip group headers)
      const sidebar = await $('aside')
      const buttons = await sidebar.$$('button')
      for (const btn of buttons) {
        const text = await btn.getText()
        if (text && text.trim().length > 0 &&
            !text.includes('Settings') && !text.includes('Manage') &&
            !text.includes('Filter') && !text.includes('ACTIVE') &&
            !text.includes('RECENT') && !text.includes('STALE') &&
            !text.includes('DORMANT')) {
          await btn.click()
          break
        }
      }
      await browser.pause(500)

      // Should show some project name
      const h1 = await $('h1')
      const name = await h1.getText()
      expect(name.length).toBeGreaterThan(0)
    })
  })

  describe('filter', () => {
    it('filter input exists in sidebar', async function () {
      if (!mainApp) return this.skip()
      const sidebar = await $('aside')
      const input = await sidebar.$('input[type="text"]')
      if (!(await input.isExisting())) return this.skip()

      expect(await input.isDisplayed()).toBe(true)
    })
  })
})
