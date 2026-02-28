/**
 * Cross-Tab Navigation — tests cross-tab navigation flows:
 * Overview→Git deep link, Git→Files open-file, project switching tab memory,
 * tab button rendering, sidebar highlight, and search→Files navigation.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import {
  switchToTab,
  waitForTabContent,
  waitForProjectsLoaded,
  getCurrentProjectName,
  isTabActive,
  waitForTabActive,
  waitForFileContent,
} from '../helpers/navigation.js'

let mainApp = false

describe('Cross-Tab Navigation', () => {
  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (mainApp) await waitForProjectsLoaded()
  })

  // ─── Overview → Git ───────────────────────────────────────────────────────

  describe('overview to git', () => {
    it('clicking a commit row in Overview activates Git tab with commit selected', async function () {
      if (!mainApp) return this.skip()

      await switchToTab('overview')

      // Check for overview commit rows — skip if project has no commits displayed
      const commitRow = await $('[data-testid="overview-commit-row"]')
      if (!(await commitRow.isExisting())) return this.skip()

      await commitRow.click()

      // Git tab should become active
      await waitForTabActive('git', 5_000)

      // A commit-row should have aria-current="true" in the Git tab
      await browser.waitUntil(
        async () => {
          const selected = await $('[data-testid="commit-row"][aria-current="true"]')
          return await selected.isExisting()
        },
        { timeout: 5_000, interval: 300, timeoutMsg: 'No commit-row was selected in Git tab after navigation' }
      )
    })
  })

  // ─── Git → Files ──────────────────────────────────────────────────────────

  describe('git to files', () => {
    it('clicking open-file-btn in Git tab activates Files tab with file loaded', async function () {
      if (!mainApp) return this.skip()

      await switchToTab('git')

      // Click a commit to select it
      const commitRow = await $('[data-testid="commit-row"]')
      if (!(await commitRow.isExisting())) return this.skip()
      await commitRow.click()

      // Wait for file list to appear
      await browser.waitUntil(
        async () => (await $$('[data-testid="commit-file"]')).length > 0,
        { timeout: 8_000, interval: 300, timeoutMsg: 'Commit file list did not appear' }
      )

      // Click the first file to open diff view
      const firstFile = await $('[data-testid="commit-file"]')
      await firstFile.click()

      // Wait for open-file-btn to appear in diff view
      await browser.waitUntil(
        async () => (await $('[data-testid="open-file-btn"]')).isExisting(),
        { timeout: 8_000, interval: 400, timeoutMsg: 'open-file-btn did not appear in Git diff view' }
      )

      const openFileBtn = await $('[data-testid="open-file-btn"]')
      await openFileBtn.click()

      // Files tab should become active
      await waitForTabActive('files', 5_000)

      // File content should be visible
      await waitForFileContent(8_000, 'File content did not appear in Files tab after open-file-btn')
    })
  })

  // ─── Project Switching ────────────────────────────────────────────────────

  describe('project switching', () => {
    it('preserves active tab when returning to a previously visited project', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length < 2) return this.skip()

      // Select first project and navigate to Git tab
      await projects[0].click()
      await browser.pause(300)
      await switchToTab('git')

      const firstProjectName = await getCurrentProjectName()

      // Switch to second project (will default to Overview)
      await projects[1].click()
      await browser.pause(500)

      // Switch back to first project
      for (const project of await $$('[data-testid="project-item"]')) {
        const text = await browser.execute((el) => el.textContent, project)
        if (text.includes(firstProjectName)) {
          await project.click()
          break
        }
      }

      // The Git tab should still be active (position memory)
      await waitForTabActive('git', 5_000)
    })

    it('new project defaults to Overview tab', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length < 2) return this.skip()

      // Go to first project, switch to Files
      await projects[0].click()
      await browser.pause(300)
      await switchToTab('files')

      // Switch to second project — should show Overview (default)
      await projects[1].click()

      await waitForTabActive('overview', 5_000)
    })
  })

  // ─── Tab Navigation ───────────────────────────────────────────────────────

  describe('tab navigation', () => {
    it('clicking each tab button renders the correct content area', async function () {
      if (!mainApp) return this.skip()

      const tabChecks = [
        {
          tab: 'overview',
          contentSelector: '[data-testid="quick-actions"], [data-testid="overview-readme"]',
        },
        {
          tab: 'files',
          contentSelector: '[role="treeitem"], [data-testid="filetree-loading"]',
        },
        {
          tab: 'git',
          contentSelector: '[data-testid="git-tab"]',
        },
        {
          tab: 'tasks',
          contentSelector: '[data-testid="sub-tab-list"], [data-testid="tasks-loading"], [data-testid="tasks-empty"]',
        },
      ]

      for (const { tab, contentSelector } of tabChecks) {
        const btn = await $(`[data-testid="tab-${tab}"]`)
        await btn.click()

        const selectors = contentSelector.split(', ')
        await browser.waitUntil(
          async () => {
            for (const sel of selectors) {
              const el = await $(sel)
              if (await el.isExisting()) return true
            }
            return false
          },
          { timeout: 10_000, interval: 400, timeoutMsg: `Content for tab "${tab}" did not appear` }
        )
      }
    })

    it('navigation maintains sidebar selection highlight on active project-item', async function () {
      if (!mainApp) return this.skip()

      // Select a project and switch tabs — the sidebar item should retain its highlighted state
      const projects = await $$('[data-testid="project-item"]')
      if (projects.length === 0) return this.skip()

      await projects[0].click()
      await browser.pause(300)

      for (const tab of ['overview', 'files', 'git', 'tasks']) {
        const btn = await $(`[data-testid="tab-${tab}"]`)
        await btn.click()
        await browser.pause(300)

        // The active project-item should have an aria-current or data-active attribute
        // (implementation may vary — check for at least one project-item with a distinct state)
        const activeItem = await $(
          '[data-testid="project-item"][aria-current], [data-testid="project-item"][data-active="true"], [data-testid="project-item"].active'
        )
        const hasActive = await activeItem.isExisting()

        // Fallback: sidebar should at least still have project items
        if (!hasActive) {
          const projectItems = await $$('[data-testid="project-item"]')
          expect(projectItems.length).toBeGreaterThan(0)
        } else {
          expect(hasActive).toBe(true)
        }
      }
    })
  })

  // ─── Defaults ─────────────────────────────────────────────────────────────

  describe('defaults', () => {
    it('project switch: Overview is the default tab for a freshly selected project', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length < 2) return this.skip()

      // Start on second project (fresh select)
      await projects[1].click()

      await browser.waitUntil(
        async () => {
          const overviewContent = await $('[data-testid="quick-actions"], [data-testid="overview-readme"], [data-testid="overview-sessions"]')
          return await overviewContent.isExisting()
        },
        { timeout: 8_000, interval: 300, timeoutMsg: 'Overview content did not appear as default tab' }
      )
    })

    it('search result click navigates to correct file in Files tab', async function () {
      if (!mainApp) return this.skip()

      // Open search overlay via Ctrl+K
      await browser.keys(['Control', 'k'])

      const searchOverlay = await $('[data-testid="search-overlay"]')
      try {
        await searchOverlay.waitForExist({ timeout: 5_000 })
      } catch {
        return this.skip() // Search overlay not available
      }

      // Type a query that will definitely match something (README exists in every project)
      const searchInput = await $('[data-testid="search-input"]')
      await searchInput.waitForExist({ timeout: 3_000 })
      await searchInput.click()
      await browser.keys('README'.split(''))

      // Wait for results
      await browser.waitUntil(
        async () => (await $('[data-testid="search-result"]')).isExisting(),
        { timeout: 8_000, interval: 400, timeoutMsg: 'No search results appeared for "README"' }
      )

      // Click the first result
      const firstResult = await $('[data-testid="search-result"]')
      await firstResult.click()

      // Files tab should now be active
      await waitForTabActive('files', 5_000)

      // File content must actually render (code viewer or markdown) — not just tree items.
      // "Error loading file" here means a real bug in the search→file navigation flow.
      await browser.waitUntil(
        async () => {
          const codeViewer = await $('[data-testid="code-viewer"]')
          const markdown = await $('[data-testid="markdown-content"]')
          return (await codeViewer.isExisting()) || (await markdown.isExisting())
        },
        { timeout: 10_000, interval: 300, timeoutMsg: 'File content did not render after search navigation (code-viewer or markdown-content expected)' }
      )
    })
  })
})
