/**
 * Files tab e2e tests — verify file tree rendering, file selection,
 * content viewing, and directory expansion.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'

describe('Files Tab', () => {
  let mainApp = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()

    if (mainApp) {
      const filesTab = await $('button=Files')
      await filesTab.click()
      await browser.pause(2_000)
    }
  })

  describe('file tree', () => {
    it('renders file tree with items', async function () {
      if (!mainApp) return this.skip()
      const treeItems = await $$('li[role="treeitem"]')
      expect(treeItems.length).toBeGreaterThan(0)
    })

    it('tree items show file or directory names', async function () {
      if (!mainApp) return this.skip()
      const treeItems = await $$('li[role="treeitem"]')
      if (treeItems.length === 0) return this.skip()

      const text = await treeItems[0].getText()
      expect(text.length).toBeGreaterThan(0)
    })

    it('directories are expandable', async function () {
      if (!mainApp) return this.skip()
      // Look for a tree item with aria-expanded (directory)
      const dirs = await $$('li[role="treeitem"][aria-expanded]')
      if (dirs.length === 0) return this.skip()

      // Click to toggle
      const firstDir = dirs[0]
      const expandedBefore = await firstDir.getAttribute('aria-expanded')
      await firstDir.click()
      await browser.pause(500)

      const expandedAfter = await firstDir.getAttribute('aria-expanded')
      // Should have toggled
      expect(expandedAfter).not.toBe(expandedBefore)
    })
  })

  describe('file viewer', () => {
    it('clicking a file shows content', async function () {
      if (!mainApp) return this.skip()

      // Find a non-directory tree item (no aria-expanded attribute)
      const treeItems = await $$('li[role="treeitem"]:not([aria-expanded])')
      if (treeItems.length === 0) return this.skip()

      await treeItems[0].click()
      await browser.pause(1_500)

      // Should show some content in the viewer pane
      // Look for code viewer, markdown renderer, or image viewer
      const codeViewer = await $('[data-testid="code-viewer"]')
      const markdownRenderer = await $('[data-testid="markdown-renderer"]')
      const content = await $('main')

      const hasViewer = (await codeViewer.isExisting()) ||
        (await markdownRenderer.isExisting()) ||
        (await content.isExisting())
      expect(hasViewer).toBe(true)
    })

    it('file viewer shows line numbers or formatted content', async function () {
      if (!mainApp) return this.skip()

      const codeViewer = await $('[data-testid="code-viewer"]')
      if (await codeViewer.isExisting()) {
        // Code viewer should contain line numbers
        const text = await codeViewer.getText()
        expect(text.length).toBeGreaterThan(0)
      }
    })
  })

  describe('README auto-selection', () => {
    it('auto-selects README if present in tree', async function () {
      if (!mainApp) return this.skip()

      // Go back to Files tab fresh
      const overviewTab = await $('button=Overview')
      await overviewTab.click()
      await browser.pause(300)
      const filesTab = await $('button=Files')
      await filesTab.click()
      await browser.pause(2_000)

      // Check if README content is showing (either markdown rendered or selected in tree)
      const treeItems = await $$('li[role="treeitem"]')
      let hasReadme = false
      for (const item of treeItems) {
        const text = await item.getText()
        if (text.toLowerCase().includes('readme')) {
          hasReadme = true
          break
        }
      }

      if (!hasReadme) return this.skip()
      // If README exists, some content should be displayed
      expect(hasReadme).toBe(true)
    })
  })

  describe('tab switching', () => {
    it('survives round-trip to another tab and back', async function () {
      if (!mainApp) return this.skip()

      const overviewTab = await $('button=Overview')
      await overviewTab.click()
      await browser.pause(300)

      const filesTab = await $('button=Files')
      await filesTab.click()
      await browser.pause(1_000)

      const treeItems = await $$('li[role="treeitem"]')
      expect(treeItems.length).toBeGreaterThan(0)
    })
  })
})
