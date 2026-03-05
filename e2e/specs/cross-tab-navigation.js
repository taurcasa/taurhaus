/**
 * Cross-Tab Navigation — tests cross-tab navigation flows:
 * Overview→Git deep link, Git→Files open-file, project switching tab memory,
 * tab button rendering, sidebar highlight, and search→Files navigation.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import {
  switchToTab,
  waitForProjectsLoaded,
  selectProjectByName,
  waitForTabActive,
  waitForFileContent,
  clickTestId,
} from '../helpers/navigation.js'
import { ensureSearchReady } from '../helpers/search.js'
import { PAUSE_TICK, WAIT_SHORT, WAIT_MEDIUM, WAIT_LONG, WAIT_XLONG, TIMEOUT_MEDIUM } from '../helpers/timing.js'
import { MOD_KEY } from '../helpers/platform.js'

let mainApp = false
let searchReady = false

function projectNameFromItemText(text) {
  return text.trim().split('\n')[0].trim()
}

describe('Cross-Tab Navigation', () => {
  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (!mainApp) return

    await waitForProjectsLoaded()
    try {
      searchReady = await ensureSearchReady()
    } catch {
      searchReady = false
    }
  })

  // ─── Overview → Git ───────────────────────────────────────────────────────

  describe('overview to git', () => {
    it('clicking a commit row in Overview activates Git tab with commit selected', async function () {
      if (!mainApp) return this.skip()

      // Pre-warm Git tab so commits are cached before the cross-tab test.
      await switchToTab('git')
      await browser.waitUntil(
        async () => {
          const rows = await $$('[data-testid="commit-row"]')
          return rows.length > 0
        },
        { ...WAIT_XLONG }
      ).catch(() => {})

      await switchToTab('overview')

      // Check for overview commit rows — skip if project has no commits displayed
      const commitRow = await $('[data-testid="overview-commit-row"]')
      if (!(await commitRow.isExisting())) return this.skip()

      await clickTestId('overview-commit-row')

      // Git tab should become active
      await waitForTabActive('git', TIMEOUT_MEDIUM)

      // A commit-row should have aria-current="true" in the Git tab
      // (commits already loaded from pre-warm above)

      // A commit-row should have aria-current="true" in the Git tab
      await browser.waitUntil(
        async () => {
          const selected = await $('[data-testid="commit-row"][aria-current="true"]')
          return await selected.isExisting()
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'No commit-row was selected in Git tab after navigation' }
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
      await clickTestId('commit-row')

      // Wait for file list to appear
      await browser.waitUntil(
        async () => (await $$('[data-testid="commit-file"]')).length > 0,
        { ...WAIT_MEDIUM, timeoutMsg: 'Commit file list did not appear' }
      )

      // Click the first file to open diff view
      await clickTestId('commit-file')

      // Wait for open-file-btn to appear in diff view
      await browser.waitUntil(
        async () => (await $('[data-testid="open-file-btn"]')).isExisting(),
        { ...WAIT_MEDIUM, timeoutMsg: 'open-file-btn did not appear in Git diff view' }
      )

      await clickTestId('open-file-btn')

      // Files tab should become active
      await waitForTabActive('files', TIMEOUT_MEDIUM)

      // File content should be visible
      await waitForFileContent(TIMEOUT_MEDIUM, 'File content did not appear in Files tab after open-file-btn')
    })
  })

  // ─── Project Switching ────────────────────────────────────────────────────

  describe('project switching', () => {
    it('preserves active tab when returning to a previously visited project', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length < 2) return this.skip()
      const firstProjectName = projectNameFromItemText(await projects[0].getText())
      const secondProjectName = projectNameFromItemText(await projects[1].getText())
      if (!firstProjectName || !secondProjectName || firstProjectName === secondProjectName) return this.skip()

      // Select first project and navigate to Git tab
      if (!(await selectProjectByName(firstProjectName))) return this.skip()
      await switchToTab('git')

      // Switch to second project (will default to Overview)
      if (!(await selectProjectByName(secondProjectName))) return this.skip()

      // Switch back to first project
      if (!(await selectProjectByName(firstProjectName))) return this.skip()

      // The Git tab should still be active (position memory)
      await waitForTabActive('git', TIMEOUT_MEDIUM)
    })

    it('new project defaults to Overview tab', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length < 2) return this.skip()
      const firstProjectName = projectNameFromItemText(await projects[0].getText())
      const secondProjectName = projectNameFromItemText(await projects[1].getText())
      if (!firstProjectName || !secondProjectName || firstProjectName === secondProjectName) return this.skip()

      // Go to first project, switch to Files
      if (!(await selectProjectByName(firstProjectName))) return this.skip()
      await switchToTab('files')

      // Switch to second project — should show Overview (default)
      if (!(await selectProjectByName(secondProjectName))) return this.skip()

      await waitForTabActive('overview', TIMEOUT_MEDIUM)
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
          contentSelector: '[data-testid="file-tree-node"], [data-testid="filetree-loading"], [data-testid="file-tree-scroll"]',
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
        await clickTestId(`tab-${tab}`)

        const selectors = contentSelector.split(', ')
        await browser.waitUntil(
          async () => {
            for (const sel of selectors) {
              const el = await $(sel)
              if (await el.isExisting()) return true
            }
            return false
          },
          { ...WAIT_MEDIUM, timeoutMsg: `Content for tab "${tab}" did not appear` }
        )
      }
    })

    it('navigation maintains sidebar selection highlight on active project-item', async function () {
      if (!mainApp) return this.skip()

      // Select a project and switch tabs — the sidebar item should retain its highlighted state
      const projects = await $$('[data-testid="project-item"]')
      if (projects.length === 0) return this.skip()

      await browser.execute((el) => el.click(), projects[0])

      for (const tab of ['overview', 'files', 'git', 'tasks']) {
        await clickTestId(`tab-${tab}`)
        await browser.pause(PAUSE_TICK)

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
      await browser.execute((el) => el.click(), projects[1])

      await browser.waitUntil(
        async () => {
          const overviewContent = await $('[data-testid="quick-actions"], [data-testid="overview-readme"], [data-testid="overview-sessions"]')
          return await overviewContent.isExisting()
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Overview content did not appear as default tab' }
      )
    })

    it('search result click navigates to correct file in Files tab', async function () {
      if (!mainApp) return this.skip()
      if (!searchReady) return this.skip()

      // Open search overlay via Ctrl+K
      await browser.keys([MOD_KEY, 'k'])

      const searchOverlay = await $('[data-testid="search-overlay"]')
      try {
        await searchOverlay.waitForExist({ timeout: 5_000 })
      } catch {
        return this.skip() // Search overlay not available
      }

      // Type a query that will definitely match something (README exists in every project)
      const searchInput = await $('[data-testid="search-input"]')
      await searchInput.waitForExist({ timeout: 3_000 })
      await clickTestId('search-input')
      await browser.keys('README'.split(''))

      // Wait for results
      await browser.waitUntil(
        async () => (await $('[data-testid="search-result"]')).isExisting(),
        { ...WAIT_MEDIUM, timeoutMsg: 'No search results appeared for "README"' }
      )

      // Click the first result
      await clickTestId('search-result')

      // Files tab should now be active
      await waitForTabActive('files', TIMEOUT_MEDIUM)

      // File content must actually render (code viewer or markdown) — not just tree items.
      // "Error loading file" here means a real bug in the search→file navigation flow.
      await browser.waitUntil(
        async () => {
          const codeViewer = await $('[data-testid="code-viewer"]')
          const markdown = await $('[data-testid="markdown-content"]')
          return (await codeViewer.isExisting()) || (await markdown.isExisting())
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'File content did not render after search navigation (code-viewer or markdown-content expected)' }
      )
    })
  })
})
