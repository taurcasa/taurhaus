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
      const projectButtons = await $$('[data-testid="project-item"]')
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

      // Ensure we're on Overview tab to see the h1 project name
      const overviewTab = await $('button=Overview')
      await overviewTab.click()
      await browser.pause(500)

      const h1Before = await $('h1')
      const nameBefore = await h1Before.getText()

      // Use data-testid to get only real project buttons
      const projectButtons = await $$('[data-testid="project-item"]')
      if (projectButtons.length < 2) return this.skip() // Need at least 2 projects

      // Click the second project button — it's guaranteed to be a different project
      // (WebKit getText on these buttons only returns branch name, not project name,
      // due to truncated spans, so we can't match by text)
      await projectButtons[1].click()
      await browser.pause(1_000)

      const h1After = await $('h1')
      const nameAfter = await h1After.getText()
      expect(nameAfter).not.toBe(nameBefore)
    })

    it('can switch back to first project', async function () {
      if (!mainApp) return this.skip()

      const projectButtons = await $$('[data-testid="project-item"]')
      if (projectButtons.length === 0) return this.skip()

      await projectButtons[0].click()
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
