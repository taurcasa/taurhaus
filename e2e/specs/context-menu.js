/**
 * Context Menu — right-click on project items, menu actions, dismiss behaviors.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded, openContextMenu, dismissContextMenu } from '../helpers/navigation.js'

let mainApp = false

describe('Context Menu', () => {
  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (mainApp) await waitForProjectsLoaded()
  })

  // ─── Open and Close ───────────────────────────────────────────────────────

  describe('open and close', () => {
    afterEach(async () => {
      await dismissContextMenu()
    })

    it('right-click on project item opens context menu', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length === 0) return this.skip()

      await openContextMenu(projects[0])

      const menu = await $('[data-testid="context-menu"]')
      expect(await menu.isExisting()).toBe(true)
    })

    it('context menu contains at least one menu item', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length === 0) return this.skip()

      await openContextMenu(projects[0])

      // Look for any menu item — data-testid starts with "menu-item-"
      const items = await $$('[data-testid^="menu-item-"]')
      expect(items.length).toBeGreaterThan(0)
    })

    it('Escape closes the context menu', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length === 0) return this.skip()

      await openContextMenu(projects[0])
      await dismissContextMenu()

      const menu = await $('[data-testid="context-menu"]')
      expect(await menu.isExisting()).toBe(false)
    })

    it('clicking outside closes the context menu', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length === 0) return this.skip()

      await openContextMenu(projects[0])

      // Click on main content area to dismiss
      const main = await $('main')
      if (await main.isExisting()) {
        await main.click()
      } else {
        // Fallback: click far corner
        await browser.execute(() => document.body.click())
      }

      await browser.waitUntil(
        async () => !(await (await $('[data-testid="context-menu"]')).isExisting()),
        { timeout: 5_000, interval: 200, timeoutMsg: 'Context menu did not close after clicking outside' }
      )

      const menu = await $('[data-testid="context-menu"]')
      expect(await menu.isExisting()).toBe(false)
    })
  })

  // ─── Actions ──────────────────────────────────────────────────────────────

  describe('actions', () => {
    it('Copy Path menu item copies to clipboard (if available)', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length === 0) return this.skip()

      await openContextMenu(projects[0])

      const copyItem = await $('[data-testid="menu-item-copy-path"]')
      if (!(await copyItem.isExisting())) {
        // Copy Path not present — skip gracefully
        await browser.keys(['Escape'])
        return this.skip()
      }

      await copyItem.click()

      // Clipboard read may not be available in WebKit test environment — wrap in try/catch
      try {
        const clipText = await browser.execute(() => navigator.clipboard.readText())
        // If we got a value, it should look like a path
        if (clipText) {
          expect(typeof clipText).toBe('string')
          expect(clipText.length).toBeGreaterThan(0)
        }
      } catch {
        // Clipboard API unavailable in test context — skip clipboard assertion
      }
    })

    it('menu disappears after clicking an action', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length === 0) return this.skip()

      await openContextMenu(projects[0])

      // Click the first available menu item (non-destructive)
      const items = await $$('[data-testid^="menu-item-"]')
      if (items.length === 0) {
        await browser.keys(['Escape'])
        return this.skip()
      }

      // Avoid "remove" actions — pick a safe item
      let safeItem = null
      for (const item of items) {
        const tid = await item.getAttribute('data-testid')
        if (!tid.includes('remove') && !tid.includes('delete')) {
          safeItem = item
          break
        }
      }

      if (!safeItem) {
        await browser.keys(['Escape'])
        return this.skip()
      }

      await safeItem.click()

      await browser.waitUntil(
        async () => !(await (await $('[data-testid="context-menu"]')).isExisting()),
        { timeout: 5_000, interval: 200, timeoutMsg: 'Context menu did not close after clicking action' }
      )

      const menu = await $('[data-testid="context-menu"]')
      expect(await menu.isExisting()).toBe(false)
    })
  })

  // ─── Remove ───────────────────────────────────────────────────────────────

  describe('remove', () => {
    it('context menu has a remove option', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length === 0) return this.skip()

      await openContextMenu(projects[0])

      // Look for a remove-related menu item — "remove", "unregister", "delete"
      const menuText = await browser.execute(() => {
        const menu = document.querySelector('[data-testid="context-menu"]')
        return menu ? menu.textContent.toLowerCase() : ''
      })

      const hasRemoveOption =
        menuText.includes('remove') ||
        menuText.includes('unregister') ||
        menuText.includes('delete')

      // Dismiss without acting
      await browser.keys(['Escape'])

      expect(hasRemoveOption).toBe(true)
    })
  })
})
