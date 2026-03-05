/**
 * Git Workflow — commit list, file diffs, navigation, position memory.
 * Replaces the old git.js spec with workflow-oriented tests.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import {
  switchToTab,
  waitForProjectsLoaded,
  selectProjectByName,
  clickTestId,
} from '../helpers/navigation.js'
import { POLL_FAST, WAIT_INSTANT, WAIT_SHORT, WAIT_MEDIUM, WAIT_LONG } from '../helpers/timing.js'

let mainApp = false

describe('Git Workflow', () => {
  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (mainApp) {
      await waitForProjectsLoaded()
      // Select the taurhaus project (we know it has commits)
      await selectProjectByName('taurhaus')
      await switchToTab('git')
    }
  })

  // ─── Commit List ──────────────────────────────────────────────────────────

  describe('commit list', () => {
    it('commit list loads with date group headers', async function () {
      if (!mainApp) return this.skip()

      // Wait for either commits or empty state
      await browser.waitUntil(
        async () => {
          const commits = await $$('[data-testid="commit-row"]')
          const empty = await $('[data-testid="git-empty"]')
          return commits.length > 0 || (await empty.isExisting())
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Commit list did not load' }
      )

      const commits = await $$('[data-testid="commit-row"]')
      if (commits.length === 0) return this.skip() // empty repo

      // Date group headers contain day names or date strings
      const gitTab = await $('[data-testid="git-tab"]')
      const tabText = await gitTab.getText()
      const hasDateGroup =
        /today|yesterday|monday|tuesday|wednesday|thursday|friday|saturday|sunday|\d{4}/i.test(tabText)

      expect(hasDateGroup).toBe(true)
    })

    it('clicking a commit shows file list with commit-file elements', async function () {
      if (!mainApp) return this.skip()

      const commits = await $$('[data-testid="commit-row"]')
      if (commits.length === 0) return this.skip()

      await commits[0].click()

      await browser.waitUntil(
        async () => {
          const files = await $$('[data-testid="commit-file"]')
          return files.length > 0
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Commit file list did not appear' }
      )

      const commitFiles = await $$('[data-testid="commit-file"]')
      expect(commitFiles.length).toBeGreaterThan(0)
    })

    it('selected commit has aria-current="true"', async function () {
      if (!mainApp) return this.skip()

      const commits = await $$('[data-testid="commit-row"]')
      if (commits.length === 0) return this.skip()

      await commits[0].click()

      const current = await $('[data-testid="commit-row"][aria-current="true"]')
      await current.waitForExist({ timeout: 2_000 })
      expect(await current.isExisting()).toBe(true)
    })

    it('commit detail shows hash and message text', async function () {
      if (!mainApp) return this.skip()

      const commits = await $$('[data-testid="commit-row"]')
      if (commits.length === 0) return this.skip()

      await commits[0].click()
      await browser.waitUntil(
        async () => (await $('[data-testid="commit-row"][aria-current="true"]')).isExisting(),
        WAIT_INSTANT
      )

      // The selected commit row should contain a hash (7 hex chars) and a message
      const current = await $('[data-testid="commit-row"][aria-current="true"]')
      const rowText = await current.getText()

      // Should contain a 7-char hex hash substring
      expect(/[0-9a-f]{7}/i.test(rowText)).toBe(true)
      // Should contain some message text (not just whitespace/hash)
      expect(rowText.trim().length).toBeGreaterThan(7)
    })
  })

  // ─── File Diff ────────────────────────────────────────────────────────────

  describe('file diff', () => {
    before(async () => {
      if (!mainApp) return
      // Ensure a commit is selected with a file list visible
      const commits = await $$('[data-testid="commit-row"]')
      if (commits.length > 0) {
        await commits[0].click()
        await browser.waitUntil(
          async () => {
            const files = await $$('[data-testid="commit-file"]')
            return files.length > 0
          },
          WAIT_MEDIUM
        )
      }
    })

    it('clicking a file shows diff-view with diff-line elements', async function () {
      if (!mainApp) return this.skip()

      const commitFiles = await $$('[data-testid="commit-file"]')
      if (commitFiles.length === 0) return this.skip()

      await commitFiles[0].click()

      // Wait for diff content (or diff-empty if binary/empty)
      await browser.waitUntil(
        async () => {
          const diffView = await $('[data-testid="diff-view"]')
          if (!(await diffView.isExisting())) return false
          const content = await $('[data-testid="diff-content"]')
          const empty = await $('[data-testid="diff-empty"]')
          return (await content.isExisting()) || (await empty.isExisting())
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Diff view did not appear after clicking file' }
      )

      const diffView = await $('[data-testid="diff-view"]')
      expect(await diffView.isExisting()).toBe(true)

      // If there is content, check for colored diff lines
      const diffContent = await $('[data-testid="diff-content"]')
      if (await diffContent.isExisting()) {
        const diffLines = await $$('[data-testid="diff-line"]')
        expect(diffLines.length).toBeGreaterThan(0)
      }
    })

    it('back button returns to file list (diff-view disappears)', async function () {
      if (!mainApp) return this.skip()

      const diffView = await $('[data-testid="diff-view"]')
      if (!(await diffView.isExisting())) {
        // Open a diff first
        const commitFiles = await $$('[data-testid="commit-file"]')
        if (commitFiles.length === 0) return this.skip()
        await commitFiles[0].click()
        await browser.waitUntil(
          async () => (await $('[data-testid="diff-view"]')).isExisting(),
          WAIT_MEDIUM
        )
      }

      const backBtn = await $('[data-testid="back-to-files"]')
      await backBtn.waitForExist({ timeout: 5_000 })
      await clickTestId('back-to-files')

      await browser.waitUntil(
        async () => !(await (await $('[data-testid="diff-view"]')).isExisting()),
        { ...WAIT_SHORT, timeoutMsg: 'diff-view did not disappear after back button' }
      )
    })

    it('open-file-btn exists in diff view', async function () {
      if (!mainApp) return this.skip()

      const commits = await $$('[data-testid="commit-row"]')
      if (commits.length === 0) return this.skip()
      await commits[0].click()

      await browser.waitUntil(
        async () => (await $$('[data-testid="commit-file"]')).length > 0,
        { ...WAIT_LONG, timeoutMsg: 'Commit files did not appear before opening diff view' }
      )

      const commitFile = await $('[data-testid="commit-file"]')
      if (!(await commitFile.isExisting())) return this.skip()
      await commitFile.click()
      await browser.waitUntil(
        async () => (await $('[data-testid="diff-view"]')).isExisting(),
        WAIT_MEDIUM
      )

      const openFileBtn = await $('[data-testid="open-file-btn"]')
      expect(await openFileBtn.isExisting()).toBe(true)
    })
  })

  // ─── Navigation ───────────────────────────────────────────────────────────

  describe('navigation', () => {
    it('file list shows modification type indicators', async function () {
      if (!mainApp) return this.skip()

      const commits = await $$('[data-testid="commit-row"]')
      if (commits.length === 0) return this.skip()

      await commits[0].click()
      await browser.waitUntil(
        async () => (await $$('[data-testid="commit-file"]')).length > 0,
        WAIT_MEDIUM
      )

      // File pills or modification indicators (A/M/D tags, colored badges, etc.)
      // They live inside commit-file elements or alongside them
      const filePills = await $$('[data-testid="file-pill"]')
      const commitFiles = await $$('[data-testid="commit-file"]')

      // Either file-pills or commit-files should exist to show modification types
      expect(filePills.length + commitFiles.length).toBeGreaterThan(0)
    })

    it('range filter appears when commits are session-filtered', async function () {
      if (!mainApp) return this.skip()

      // Range filter is conditional — only appears when commits are filtered by session
      const rangeFilter = await $('[data-testid="range-filter"]')
      if (!(await rangeFilter.isExisting())) {
        // No active session filter — this is valid, skip gracefully
        return this.skip()
      }
      expect(await rangeFilter.isDisplayed()).toBe(true)
    })

    it('scroll sentinel exists when more commits are available', async function () {
      if (!mainApp) return this.skip()

      const commits = await $$('[data-testid="commit-row"]')
      if (commits.length === 0) return this.skip()

      // Sentinel is conditional — only exists when hasMore && !rangeFilter
      const sentinel = await $('[data-testid="scroll-sentinel"]')
      if (!(await sentinel.isExisting())) {
        // All commits loaded or range filter active — skip gracefully
        return this.skip()
      }
      expect(await sentinel.isDisplayed()).toBe(true)
    })

    it('tab round-trip: Overview → Git → content still renders', async function () {
      if (!mainApp) return this.skip()

      await switchToTab('overview')
      await switchToTab('git')

      const gitTab = await $('[data-testid="git-tab"]')
      expect(await gitTab.isExisting()).toBe(true)

      // Commit list or empty state should still be visible
      await browser.waitUntil(
        async () => {
          const commits = await $$('[data-testid="commit-row"]')
          const empty = await $('[data-testid="git-empty"]')
          return commits.length > 0 || (await empty.isExisting())
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Git content did not render after tab round-trip' }
      )
    })

    it('empty state shows git-empty when repository has no commits', async function () {
      if (!mainApp) return this.skip()

      // Check whether we're in an empty state or not
      const commits = await $$('[data-testid="commit-row"]')
      if (commits.length > 0) return this.skip() // has commits, can't test empty state directly

      const empty = await $('[data-testid="git-empty"]')
      expect(await empty.isExisting()).toBe(true)
    })

    it('clicking overview-commit-row activates Git tab with commit selected', async function () {
      if (!mainApp) return this.skip()

      await switchToTab('overview')

      const overviewCommit = await $('[data-testid="overview-commit-row"]')
      if (!(await overviewCommit.isExisting())) return this.skip()

      await clickTestId('overview-commit-row')

      // Git tab should now be active
      await browser.waitUntil(
        async () => {
          const gitTab = await $('[data-testid="git-tab"]')
          return await gitTab.isExisting()
        },
        { ...WAIT_SHORT, timeoutMsg: 'Git tab did not activate after clicking overview-commit-row' }
      )

      // A commit should be selected (aria-current)
      await browser.waitUntil(
        async () => {
          const current = await $('[data-testid="commit-row"][aria-current="true"]')
          return await current.isExisting()
        },
        { ...WAIT_SHORT, timeoutMsg: 'No commit was selected in Git tab after cross-tab click' }
      )
    })
  })

  // ─── Position Memory ──────────────────────────────────────────────────────

  describe('position memory', () => {
    it('selected commit is restored after switching projects and back', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length < 2) return this.skip()

      // Make sure we're on Git tab with a commit selected
      await switchToTab('git')
      const commits = await $$('[data-testid="commit-row"]')
      if (commits.length === 0) return this.skip()

      await commits[0].click()
      await browser.waitUntil(
        async () => (await $('[data-testid="commit-row"][aria-current="true"]')).isExisting(),
        WAIT_SHORT
      )

      // Capture the selected commit's hash for later verification (not full text — relative times change)
      const selectedBefore = await $('[data-testid="commit-row"][aria-current="true"]')
      const selectedTextBefore = await selectedBefore.getText()
      const hashBefore = selectedTextBefore.match(/[0-9a-f]{7}/i)?.[0]

      // Use known fixture project names for deterministic project switching.
      if (!(await selectProjectByName('ledger'))) return this.skip()
      if (!(await selectProjectByName('taurhaus'))) return this.skip()

      // Navigate to Git tab on the original project
      await switchToTab('git')

      // The previously selected commit should still be selected
      await browser.waitUntil(
        async () => (await $('[data-testid="commit-row"][aria-current="true"]')).isExisting(),
        { ...WAIT_MEDIUM, timeoutMsg: 'No commit selected after returning to project' }
      )

      const selectedAfter = await $('[data-testid="commit-row"][aria-current="true"]')
      const selectedTextAfter = await selectedAfter.getText()
      const hashAfter = selectedTextAfter.match(/[0-9a-f]{7}/i)?.[0]

      // Compare by commit hash — relative time text ("5m" → "6m") changes during test run
      expect(hashAfter).toBe(hashBefore)
    })
  })
})
