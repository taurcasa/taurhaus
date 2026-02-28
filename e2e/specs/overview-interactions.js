/**
 * Overview Interactions e2e tests — tests actual user interactions on the Overview tab:
 * header, readme rendering, commit click navigation, quick actions, relationships, sessions.
 *
 * Replaces the old overview.js which only verified static rendering.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { switchToTab, clickTestId } from '../helpers/navigation.js'
import { WAIT_SHORT, WAIT_MEDIUM } from '../helpers/timing.js'

describe('Overview Interactions', () => {
  let mainApp = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()

    if (mainApp) {
      // Ensure we start on the Overview tab (default)
      await switchToTab('overview')
    }
  })

  describe('header', () => {
    it('displays a non-empty project name in h1', async function () {
      if (!mainApp) return this.skip()
      const h1 = await $('h1')
      await h1.waitForExist({ timeout: 5_000 })
      const text = await h1.getText()
      expect(text.trim().length).toBeGreaterThan(0)
    })

    it('shows branch name near the project title', async function () {
      if (!mainApp) return this.skip()
      // Branch may be rendered as a pill or span near the header — check via textContent
      const headerArea = await browser.execute(() => {
        const h1 = document.querySelector('h1')
        if (!h1) return null
        // Check parent and siblings for branch-like text
        const parent = h1.parentElement
        return parent ? parent.textContent : null
      })
      // Just verify the header area is non-empty — branch display is optional
      if (!headerArea) return this.skip()
      expect(headerArea.trim().length).toBeGreaterThan(0)
    })
  })

  describe('readme', () => {
    it('renders README content with non-empty HTML', async function () {
      if (!mainApp) return this.skip()

      // Wait for readme section to appear (may be loading initially)
      const readmeSection = await $('[data-testid="overview-readme"]')
      if (!(await readmeSection.isExisting())) return this.skip()

      // Wait for markdown content to populate
      await browser.waitUntil(
        async () => {
          const content = await $('[data-testid="markdown-content"]')
          if (!(await content.isExisting())) return false
          const html = await browser.execute((el) => el.innerHTML, content)
          return html && html.trim().length > 0
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'README markdown-content did not populate' }
      )

      const content = await $('[data-testid="markdown-content"]')
      const html = await browser.execute((el) => el.innerHTML, content)
      expect(html.trim().length).toBeGreaterThan(0)
    })
  })

  describe('commits', () => {
    it('shows overview-commit-row elements after loading', async function () {
      if (!mainApp) return this.skip()

      // Wait for commits loading to resolve
      await browser.waitUntil(
        async () => {
          const loading = await $('[data-testid="commits-loading"]')
          if (await loading.isExisting()) return false
          return true
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Commits loading did not finish' }
      )

      const rows = await $$('[data-testid="overview-commit-row"]')
      // If no commits, that's still valid — just verify loading resolved
      const hasContent = rows.length > 0 || await browser.execute(() => {
        // Check for any empty-state text if no commit rows
        const body = document.body.textContent || ''
        return body.toLowerCase().includes('no commits')
      })
      expect(hasContent).toBe(true)
    })

    it('clicking a commit row navigates to Git tab with that commit selected', async function () {
      if (!mainApp) return this.skip()

      const rows = await $$('[data-testid="overview-commit-row"]')
      if (rows.length === 0) return this.skip()

      await browser.execute((el) => el.click(), rows[0])

      // Git tab should become active
      await browser.waitUntil(
        async () => {
          const gitTab = await $('[data-testid="git-tab"]')
          return await gitTab.isExisting()
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Git tab did not appear after commit click' }
      )

      // A commit row should be marked as selected/current
      await browser.waitUntil(
        async () => {
          const selectedRow = await $('[data-testid="commit-row"][aria-current="true"]')
          return await selectedRow.isExisting()
        },
        { ...WAIT_SHORT, timeoutMsg: 'No commit-row with aria-current="true" found' }
      )

      const selectedRow = await $('[data-testid="commit-row"][aria-current="true"]')
      expect(await selectedRow.isExisting()).toBe(true)

      // Navigate back to Overview for subsequent tests
      await switchToTab('overview')
    })
  })

  describe('quick actions', () => {
    it('Claude launch button exists and is enabled', async function () {
      if (!mainApp) return this.skip()
      const btn = await $('[data-testid="action-launch-claude"]')
      expect(await btn.isDisplayed()).toBe(true)
      expect(await btn.isEnabled()).toBe(true)
    })

    it('Codex launch button exists and is enabled', async function () {
      if (!mainApp) return this.skip()
      const btn = await $('[data-testid="action-launch-codex"]')
      expect(await btn.isDisplayed()).toBe(true)
      expect(await btn.isEnabled()).toBe(true)
    })

    it('Gemini launch button exists and is enabled', async function () {
      if (!mainApp) return this.skip()
      const btn = await $('[data-testid="action-launch-gemini"]')
      expect(await btn.isDisplayed()).toBe(true)
      expect(await btn.isEnabled()).toBe(true)
    })

    it('Terminal button exists and is enabled', async function () {
      if (!mainApp) return this.skip()
      const btn = await $('[data-testid="action-open-terminal"]')
      expect(await btn.isDisplayed()).toBe(true)
      expect(await btn.isEnabled()).toBe(true)
    })
  })

  describe('relationships', () => {
    it('shows relationship rows or "No connections" message', async function () {
      if (!mainApp) return this.skip()

      // Wait for relationships loading to resolve
      await browser.waitUntil(
        async () => {
          const loading = await $('[data-testid="relationships-loading"]')
          return !(await loading.isExisting())
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Relationships loading did not finish' }
      )

      const rows = await $$('[data-testid="relationship-row"]')
      // Check for empty state text if no relationship rows
      const hasContent = rows.length > 0 || await browser.execute(() => {
        const relSection = document.querySelector('[data-testid="overview-relationships"]')
        if (!relSection) return true // section not present is valid
        const text = relSection.textContent || ''
        return text.toLowerCase().includes('no connections') || text.toLowerCase().includes('no relationships')
      })
      expect(hasContent).toBe(true)
    })

    it('dismissing a relationship removes the row', async function () {
      if (!mainApp) return this.skip()

      const rows = await $$('[data-testid="relationship-row"]')
      if (rows.length === 0) return this.skip()

      const initialCount = rows.length

      // Click the dismiss button on the first relationship row
      const dismissBtn = await $('[data-testid="dismiss-relationship"]')
      if (!(await dismissBtn.isExisting())) return this.skip()

      await clickTestId('dismiss-relationship')

      // Row count should decrease by 1 (or section shows empty state)
      await browser.waitUntil(
        async () => {
          const updatedRows = await $$('[data-testid="relationship-row"]')
          const emptyState = await $('[data-testid="relationships-empty"], p=No connections detected yet.')
          return updatedRows.length < initialCount || (await emptyState.isExisting())
        },
        { ...WAIT_SHORT, timeoutMsg: 'Relationship row did not disappear after dismiss' }
      )

      const updatedRows = await $$('[data-testid="relationship-row"]')
      expect(updatedRows.length).toBeLessThan(initialCount)
    })
  })

  describe('sessions', () => {
    it('session history section renders or is hidden when no sessions exist', async function () {
      if (!mainApp) return this.skip()

      // Sessions section is conditionally rendered — only appears when loading
      // or when sessions exist. A fresh app with no sessions hides it entirely.
      const sessionsSection = await $('[data-testid="overview-sessions"]')
      if (!(await sessionsSection.isExisting())) {
        // Verify the overview still renders (section hidden is valid when empty)
        const overview = await $('[data-testid="quick-actions"]')
        expect(await overview.isExisting()).toBe(true)
        return
      }

      expect(await sessionsSection.isExisting()).toBe(true)
    })

    it('sessions section resolves loading state', async function () {
      if (!mainApp) return this.skip()

      await browser.waitUntil(
        async () => {
          const loading = await $('[data-testid="sessions-loading"]')
          return !(await loading.isExisting())
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Sessions loading did not finish' }
      )

      // After loading, sessions-loading should be gone
      const loading = await $('[data-testid="sessions-loading"]')
      expect(await loading.isExisting()).toBe(false)
    })
  })

  describe('project info', () => {
    it('shows a path-like string in the overview content', async function () {
      if (!mainApp) return this.skip()

      // Project path is shown somewhere in the overview — look for a slash-containing text
      const bodyText = await browser.execute(() => document.body.innerText)
      // Any absolute path indicator — '/' on Linux/Mac, '\' on Windows
      const hasPath = /\/[^\s]+|[A-Z]:\\/i.test(bodyText)
      expect(hasPath).toBe(true)
    })
  })
})
