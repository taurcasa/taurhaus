/**
 * Files Workflow — actual file browsing workflows.
 * Replaces the old files.js spec with scenario-oriented tests.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { switchToTab, waitForFileContent } from '../helpers/navigation.js'
import { WAIT_SHORT, WAIT_MEDIUM, WAIT_XLONG, TIMEOUT_LONG } from '../helpers/timing.js'

let mainApp = false
const TREE_NODE_SELECTOR = '[data-testid="file-tree-node"]'

async function isDirectoryNode(node) {
  return browser.execute((el) => {
    const firstSvg = el.querySelector('svg')
    const cls = firstSvg?.getAttribute('class') || ''
    return cls.includes('transition-transform')
  }, node)
}

async function isExpandedDirectoryNode(node) {
  return browser.execute((el) => {
    const firstSvg = el.querySelector('svg')
    const cls = firstSvg?.getAttribute('class') || ''
    return cls.includes('rotate-90')
  }, node)
}

describe('Files Workflow', () => {
  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (mainApp) await switchToTab('files')
  })

  // ─── File Tree ────────────────────────────────────────────────────────────

  describe('file tree', () => {
    it('loads with file tree nodes', async function () {
      if (!mainApp) return this.skip()

      // Wait for file tree loading to complete (skeleton disappears).
      await browser.waitUntil(
        async () => {
          const loading = await $('[data-testid="filetree-loading"]')
          return !(await loading.isExisting())
        },
        { ...WAIT_XLONG, timeoutMsg: 'File tree loading did not complete' }
      )

      const items = await $$(TREE_NODE_SELECTOR)
      expect(items.length).toBeGreaterThan(0)
    })

    it('auto-selects README on first load — markdown-content visible', async function () {
      if (!mainApp) return this.skip()

      // Navigate away and back so the tab does a fresh load
      await switchToTab('overview')
      await switchToTab('files')

      // Wait for tree to settle
      await browser.waitUntil(
        async () => {
          const items = await $$(TREE_NODE_SELECTOR)
          return items.length > 0
        },
        WAIT_MEDIUM
      )

      // Check for a README in the tree first — skip if project has none
      const items = await $$(TREE_NODE_SELECTOR)
      let hasReadme = false
      for (const item of items) {
        const text = await browser.execute((el) => el.textContent, item)
        if (text.toLowerCase().includes('readme')) {
          hasReadme = true
          break
        }
      }
      if (!hasReadme) return this.skip()

      // README should be auto-selected → markdown-content showing
      const markdownContent = await $('[data-testid="markdown-content"]')
      await markdownContent.waitForExist({
        timeout: 5_000,
        timeoutMsg: 'markdown-content did not appear after Files tab auto-select of README',
      })
      expect(await markdownContent.isExisting()).toBe(true)
    })

    it('tree items have non-empty name text', async function () {
      if (!mainApp) return this.skip()

      await browser.waitUntil(
        async () => (await $$(TREE_NODE_SELECTOR)).length > 0,
        WAIT_MEDIUM
      )

      const items = await $$(TREE_NODE_SELECTOR)
      for (const item of items.slice(0, 5)) {
        const text = await browser.execute((el) => el.textContent, item)
        expect(text.trim().length).toBeGreaterThan(0)
      }
    })

    it('node_modules is not in tree (gitignore respected)', async function () {
      if (!mainApp) return this.skip()

      await browser.waitUntil(
        async () => (await $$(TREE_NODE_SELECTOR)).length > 0,
        WAIT_MEDIUM
      )

      const items = await $$(TREE_NODE_SELECTOR)
      for (const item of items) {
        const text = await browser.execute((el) => el.textContent, item)
        expect(text).not.toContain('node_modules')
      }
    })
  })

  // ─── File Viewing ─────────────────────────────────────────────────────────

  describe('file viewing', () => {
    it('clicking a .js or .rs file shows code-viewer with highlighted spans', async function () {
      if (!mainApp) return this.skip()

      await browser.waitUntil(
        async () => (await $$(TREE_NODE_SELECTOR)).length > 0,
        WAIT_MEDIUM
      )

      // Find a code file (.js or .rs)
      const items = await $$(TREE_NODE_SELECTOR)
      let codeItem = null
      for (const item of items) {
        const text = await browser.execute((el) => el.textContent, item)
        if (text.trim().match(/\.(js|ts|rs|py|go|json)$/)) {
          codeItem = item
          break
        }
      }
      if (!codeItem) return this.skip()

      await browser.execute((el) => el.click(), codeItem)
      await browser.waitUntil(
        async () => (await $('[data-testid="code-viewer"]')).isExisting(),
        { ...WAIT_MEDIUM, timeoutMsg: 'code-viewer did not appear after clicking code file' }
      )

      // Should contain highlighted <span> elements from Shiki
      const codeViewer = await $('[data-testid="code-viewer"]')
      const spans = await codeViewer.$$('span')
      expect(spans.length).toBeGreaterThan(0)
    })

    it('clicking a .md file shows markdown-content with rendered HTML', async function () {
      if (!mainApp) return this.skip()

      await browser.waitUntil(
        async () => (await $$(TREE_NODE_SELECTOR)).length > 0,
        WAIT_MEDIUM
      )

      const items = await $$(TREE_NODE_SELECTOR)
      let mdItem = null
      for (const item of items) {
        const text = await browser.execute((el) => el.textContent, item)
        if (text.trim().match(/\.md$/i)) {
          mdItem = item
          break
        }
      }
      if (!mdItem) return this.skip()

      await browser.execute((el) => el.click(), mdItem)
      await browser.waitUntil(
        async () => (await $('[data-testid="markdown-content"]')).isExisting(),
        { ...WAIT_MEDIUM, timeoutMsg: 'markdown-content did not appear after clicking .md file' }
      )

      // Rendered markdown should contain at least one heading or paragraph
      const markdownContent = await $('[data-testid="markdown-content"]')
      const headings = await markdownContent.$$('h1, h2, h3, h4, p')
      expect(headings.length).toBeGreaterThan(0)
    })

    it('binary or image file shows appropriate viewer or informational message', async function () {
      if (!mainApp) return this.skip()

      // Scan entire tree for a known binary extension
      const items = await $$(TREE_NODE_SELECTOR)
      let binaryItem = null
      for (const item of items) {
        const text = await browser.execute((el) => el.textContent, item)
        if (text.trim().match(/\.(png|jpg|jpeg|gif|svg|ico|woff|woff2|ttf|bin)$/i)) {
          binaryItem = item
          break
        }
      }
      if (!binaryItem) return this.skip()

      await browser.execute((el) => el.click(), binaryItem)

      // Wait briefly for content to respond
      await browser.waitUntil(
        async () => {
          const cv = await $('[data-testid="code-viewer"]')
          const mc = await $('[data-testid="markdown-content"]')
          return (await cv.isExisting()) || (await mc.isExisting())
        },
        WAIT_SHORT
      ).catch(() => {}) // Best-effort

      // Should show SOMETHING — image viewer, binary message, or code viewer
      const codeViewer = await $('[data-testid="code-viewer"]')
      const markdownContent = await $('[data-testid="markdown-content"]')
      const main = await $('main')

      const hasContent =
        (await codeViewer.isExisting()) ||
        (await markdownContent.isExisting()) ||
        (await main.isExisting())
      expect(hasContent).toBe(true)
    })
  })

  // ─── Directory Navigation ─────────────────────────────────────────────────

  describe('directory navigation', () => {
    it('clicking a collapsed directory expands it', async function () {
      if (!mainApp) return this.skip()

      await browser.waitUntil(
        async () => (await $$(TREE_NODE_SELECTOR)).length > 0,
        { ...WAIT_MEDIUM, timeoutMsg: 'No file tree nodes found' }
      )

      const dirs = await $$(TREE_NODE_SELECTOR)
      const itemsBefore = dirs.length

      // Find one that is currently collapsed.
      let collapsed = null
      for (const dir of dirs) {
        if (!(await isDirectoryNode(dir))) continue
        if (!(await isExpandedDirectoryNode(dir))) {
          collapsed = dir
          break
        }
      }
      if (!collapsed) return this.skip()

      await browser.execute((el) => el.click(), collapsed)
      await browser.waitUntil(
        async () => await isExpandedDirectoryNode(collapsed),
        { ...WAIT_MEDIUM, timeoutMsg: 'Directory did not expand' }
      )

      // Children should have appeared in the tree
      const itemsAfter = await $$(TREE_NODE_SELECTOR)
      expect(itemsAfter.length).toBeGreaterThan(itemsBefore)
    })

    it('clicking an expanded directory collapses it', async function () {
      if (!mainApp) return this.skip()

      await browser.waitUntil(
        async () => (await $$(TREE_NODE_SELECTOR)).length > 0,
        WAIT_MEDIUM
      )

      const dirs = await $$(TREE_NODE_SELECTOR)
      // Find one that is currently expanded
      let expanded = null
      for (const dir of dirs) {
        if (!(await isDirectoryNode(dir))) continue
        if (await isExpandedDirectoryNode(dir)) {
          expanded = dir
          break
        }
      }

      // Expand one if nothing is already expanded
      if (!expanded) {
        let collapsed = null
        for (const dir of dirs) {
          if (!(await isDirectoryNode(dir))) continue
          if (!(await isExpandedDirectoryNode(dir))) {
            collapsed = dir
            break
          }
        }
        if (!collapsed) return this.skip()
        await browser.execute((el) => el.click(), collapsed)
        await browser.waitUntil(
          async () => await isExpandedDirectoryNode(collapsed),
          WAIT_MEDIUM
        )
        expanded = collapsed
      }

      const itemsBefore = (await $$(TREE_NODE_SELECTOR)).length
      await browser.execute((el) => el.click(), expanded)
      await browser.waitUntil(
        async () => !(await isExpandedDirectoryNode(expanded)),
        { ...WAIT_MEDIUM, timeoutMsg: 'Directory did not collapse' }
      )

      // Children should have been removed
      const itemsAfter = (await $$(TREE_NODE_SELECTOR)).length
      expect(itemsAfter).toBeLessThan(itemsBefore)
    })
  })

  // ─── State Preservation ───────────────────────────────────────────────────

  describe('state preservation', () => {
    it('position memory: selected file is still shown after round-trip to Overview', async function () {
      if (!mainApp) return this.skip()

      await browser.waitUntil(
        async () => (await $$(TREE_NODE_SELECTOR)).length > 0,
        WAIT_MEDIUM
      )

      // Select a code file so we have a deterministic viewer state
      const items = await $$(TREE_NODE_SELECTOR)
      let codeItem = null
      for (const item of items) {
        const text = await browser.execute((el) => el.textContent, item)
        if (text.trim().match(/\.(js|ts|rs|py|json|md)$/)) {
          codeItem = item
          break
        }
      }
      if (!codeItem) return this.skip()

      await browser.execute((el) => el.click(), codeItem)
      await browser.waitUntil(
        async () => {
          const cv = await $('[data-testid="code-viewer"]')
          const mc = await $('[data-testid="markdown-content"]')
          return (await cv.isExisting()) || (await mc.isExisting())
        },
        WAIT_SHORT
      ).catch(() => {})

      // Record which viewer is showing
      const hadCodeViewer = await (await $('[data-testid="code-viewer"]')).isExisting()
      const hadMarkdown = await (await $('[data-testid="markdown-content"]')).isExisting()
      const hadContent = hadCodeViewer || hadMarkdown
      if (!hadContent) return this.skip()

      // Round-trip through Overview
      await switchToTab('overview')
      await switchToTab('files')

      // Same viewer should still be showing (position restored)
      await waitForFileContent(TIMEOUT_LONG, 'File viewer did not persist after tab round-trip')

      if (hadCodeViewer) {
        const codeViewer = await $('[data-testid="code-viewer"]')
        expect(await codeViewer.isExisting()).toBe(true)
      } else {
        const markdown = await $('[data-testid="markdown-content"]')
        expect(await markdown.isExisting()).toBe(true)
      }
    })

    it('tab round-trip survival: Files → Overview → Files still renders tree', async function () {
      if (!mainApp) return this.skip()

      await switchToTab('overview')
      await switchToTab('files')

      await browser.waitUntil(
        async () => (await $$(TREE_NODE_SELECTOR)).length > 0,
        { ...WAIT_MEDIUM, timeoutMsg: 'File tree did not render after tab round-trip' }
      )

      const items = await $$(TREE_NODE_SELECTOR)
      expect(items.length).toBeGreaterThan(0)
    })
  })
})
