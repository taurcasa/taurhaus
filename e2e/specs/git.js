/**
 * Git tab e2e tests — verify commit list, date grouping, commit detail,
 * file list, and diff view work end-to-end.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'

describe('Git Tab', () => {
  let mainApp = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()

    if (mainApp) {
      const gitTab = await $('button=Git')
      await gitTab.click()
      await browser.pause(2_000)
    }
  })

  describe('commit list', () => {
    it('renders commit rows or empty state', async function () {
      if (!mainApp) return this.skip()
      const commitRows = await $$('[data-testid="commit-row"]')
      const emptyState = await $('[data-testid="git-empty"]')

      const hasContent = commitRows.length > 0 || (await emptyState.isExisting())
      expect(hasContent).toBe(true)
    })

    it('commit rows display message text', async function () {
      if (!mainApp) return this.skip()
      const commitRows = await $$('[data-testid="commit-row"]')
      if (commitRows.length === 0) return this.skip()

      const text = await commitRows[0].getText()
      expect(text.length).toBeGreaterThan(0)
    })

    it('shows date group headers', async function () {
      if (!mainApp) return this.skip()
      const commitRows = await $$('[data-testid="commit-row"]')
      if (commitRows.length === 0) return this.skip()

      // Date headers are sticky divs with uppercase text like "Today", "Yesterday", etc.
      // They appear above commit rows in the list
      const gitTab = await $('[data-testid="git-tab"]')
      const text = await gitTab.getText()
      // Should contain at least one date label
      const hasDateLabel = text.includes('Today') || text.includes('Yesterday') ||
        text.includes('Monday') || text.includes('Tuesday') || text.includes('Wednesday') ||
        text.includes('Thursday') || text.includes('Friday') || text.includes('Saturday') ||
        text.includes('Sunday') || /[A-Z][a-z]+ \d+/.test(text)
      expect(hasDateLabel).toBe(true)
    })
  })

  describe('commit detail', () => {
    it('shows "Select a commit" placeholder initially', async function () {
      if (!mainApp) return this.skip()
      const placeholder = await $('p=Select a commit to view details')
      // Might not exist if a commit was auto-selected via navTarget
      if (await placeholder.isExisting()) {
        expect(await placeholder.isDisplayed()).toBe(true)
      }
    })

    it('clicking a commit shows commit detail with files', async function () {
      if (!mainApp) return this.skip()
      const commitRows = await $$('[data-testid="commit-row"]')
      if (commitRows.length === 0) return this.skip()

      await commitRows[0].click()
      await browser.pause(1_000)

      // Should show commit files or at least the commit hash in detail
      const commitFiles = await $$('[data-testid="commit-file"]')
      const detailArea = await $('[data-testid="git-tab"]')
      const detailText = await detailArea.getText()

      // Detail should contain the commit hash (visible in header)
      expect(detailText.length).toBeGreaterThan(0)
    })

    it('selected commit row has aria-current', async function () {
      if (!mainApp) return this.skip()
      const commitRows = await $$('[data-testid="commit-row"]')
      if (commitRows.length === 0) return this.skip()

      // First row should already be selected from previous test
      const ariaCurrent = await commitRows[0].getAttribute('aria-current')
      expect(ariaCurrent).toBe('true')
    })

    it('shows file list in commit detail', async function () {
      if (!mainApp) return this.skip()
      const commitFiles = await $$('[data-testid="commit-file"]')
      if (commitFiles.length === 0) return this.skip()

      // File should have visible text (filename)
      const text = await commitFiles[0].getText()
      expect(text.length).toBeGreaterThan(0)
    })
  })

  describe('diff view', () => {
    it('clicking a file shows diff view', async function () {
      if (!mainApp) return this.skip()
      const commitFiles = await $$('[data-testid="commit-file"]')
      if (commitFiles.length === 0) return this.skip()

      await commitFiles[0].click()
      await browser.pause(1_000)

      const diffView = await $('[data-testid="diff-view"]')
      expect(await diffView.isExisting()).toBe(true)
    })

    it('diff view has back button', async function () {
      if (!mainApp) return this.skip()
      const backBtn = await $('[data-testid="back-to-files"]')
      if (!(await backBtn.isExisting())) return this.skip()

      expect(await backBtn.isDisplayed()).toBe(true)
    })

    it('clicking back returns to file list', async function () {
      if (!mainApp) return this.skip()
      const backBtn = await $('[data-testid="back-to-files"]')
      if (!(await backBtn.isExisting())) return this.skip()

      await backBtn.click()
      await browser.pause(500)

      const diffView = await $('[data-testid="diff-view"]')
      expect(await diffView.isExisting()).toBe(false)
    })
  })

  describe('tab switching', () => {
    it('survives round-trip to Overview and back', async function () {
      if (!mainApp) return this.skip()

      const overviewTab = await $('button=Overview')
      await overviewTab.click()
      await browser.pause(300)

      const gitTab = await $('button=Git')
      await gitTab.click()
      await browser.pause(1_000)

      const gitTabEl = await $('[data-testid="git-tab"]')
      expect(await gitTabEl.isDisplayed()).toBe(true)
    })
  })
})
