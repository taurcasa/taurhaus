/**
 * Tasks Workflow e2e tests — tests actual interactions on the Tasks tab:
 * loading, empty state, kanban board, task detail panel, sub-tabs, session history.
 *
 * Replaces the old taskBoard.js which only verified static rendering.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { switchToTab, clickTestId } from '../helpers/navigation.js'
import { WAIT_SHORT, WAIT_MEDIUM, WAIT_LONG } from '../helpers/timing.js'

describe('Tasks Workflow', () => {
  let mainApp = false
  let hasTasks = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()

    if (mainApp) {
      await switchToTab('tasks')
    }
  })

  describe('loading', () => {
    it('Tasks tab loads and loading completes within timeout', async function () {
      if (!mainApp) return this.skip()

      await browser.waitUntil(
        async () => {
          const loading = await $('[data-testid="tasks-loading"]')
          const empty = await $('[data-testid="tasks-empty"]')
          const columns = await $$('[data-testid="kanban-column"]')
          return (
            !(await loading.isExisting()) ||
            (await empty.isExisting()) ||
            columns.length > 0
          )
        },
        { ...WAIT_LONG, timeoutMsg: 'Tasks loading did not complete within timeout' }
      )

      // Record whether tasks exist for subsequent conditional tests
      const rows = await $$('[data-testid="task-row"]')
      hasTasks = rows.length > 0
    })
  })

  describe('empty state', () => {
    it('shows tasks-empty message when no tasks exist', async function () {
      if (!mainApp) return this.skip()
      if (hasTasks) return this.skip()

      const empty = await $('[data-testid="tasks-empty"]')
      expect(await empty.isExisting()).toBe(true)
      const text = await empty.getText()
      expect(text.trim().length).toBeGreaterThan(0)
    })
  })

  describe('kanban board', () => {
    it('renders kanban columns when tasks exist', async function () {
      if (!mainApp) return this.skip()
      if (!hasTasks) return this.skip()

      const columns = await $$('[data-testid="kanban-column"]')
      expect(columns.length).toBeGreaterThan(0)
    })

    it('task rows show non-empty subject text', async function () {
      if (!mainApp) return this.skip()
      if (!hasTasks) return this.skip()

      const rows = await $$('[data-testid="task-row"]')
      const firstRow = rows[0]
      const text = await firstRow.getText()
      expect(text.trim().length).toBeGreaterThan(0)
    })

    it('task rows contain a tool icon SVG', async function () {
      if (!mainApp) return this.skip()
      if (!hasTasks) return this.skip()

      const rows = await $$('[data-testid="task-row"]')
      const firstRow = rows[0]
      const svg = await firstRow.$('svg')
      expect(await svg.isExisting()).toBe(true)
    })
  })

  describe('task detail', () => {
    it('clicking a task row opens the detail panel', async function () {
      if (!mainApp) return this.skip()
      if (!hasTasks) return this.skip()

      const rows = await $$('[data-testid="task-row"]')
      await browser.execute((el) => el.click(), rows[0])

      await browser.waitUntil(
        async () => {
          const panel = await $('[data-testid="task-detail-panel"]')
          return await panel.isExisting()
        },
        { ...WAIT_SHORT, timeoutMsg: 'task-detail-panel did not appear after click' }
      )

      const panel = await $('[data-testid="task-detail-panel"]')
      expect(await panel.isExisting()).toBe(true)
    })

    it('detail panel shows description or sections content', async function () {
      if (!mainApp) return this.skip()
      if (!hasTasks) return this.skip()

      // Detail panel should already be open from previous test; if not, open it
      const panel = await $('[data-testid="task-detail-panel"]')
      if (!(await panel.isExisting())) {
        const rows = await $$('[data-testid="task-row"]')
        if (rows.length === 0) return this.skip()
        await browser.execute((el) => el.click(), rows[0])
        await panel.waitForExist({ timeout: 3_000 })
      }

      // Wait for detail loading to complete
      await browser.waitUntil(
        async () => {
          const loading = await $('[data-testid="detail-loading"]')
          return !(await loading.isExisting())
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Detail panel loading did not finish' }
      )

      // Either sections or description element should be present
      const sections = await $('[data-testid="detail-sections"]')
      const description = await $('[data-testid="detail-description"]')
      const hasContent = (await sections.isExisting()) || (await description.isExisting())
      expect(hasContent).toBe(true)
    })

    it('detail close button dismisses the panel', async function () {
      if (!mainApp) return this.skip()
      if (!hasTasks) return this.skip()

      // Ensure panel is open
      let panel = await $('[data-testid="task-detail-panel"]')
      if (!(await panel.isExisting())) {
        const rows = await $$('[data-testid="task-row"]')
        if (rows.length === 0) return this.skip()
        await browser.execute((el) => el.click(), rows[0])
        await panel.waitForExist({ timeout: 3_000 })
      }

      await clickTestId('detail-close')

      await browser.waitUntil(
        async () => {
          panel = await $('[data-testid="task-detail-panel"]')
          return !(await panel.isExisting())
        },
        { ...WAIT_SHORT, timeoutMsg: 'task-detail-panel did not close after clicking detail-close' }
      )

      panel = await $('[data-testid="task-detail-panel"]')
      expect(await panel.isExisting()).toBe(false)
    })
  })

  describe('sub-tabs', () => {
    it('clicking sub-tab-history shows history content', async function () {
      if (!mainApp) return this.skip()

      const historyTab = await $('[data-testid="sub-tab-history"]')
      if (!(await historyTab.isExisting())) return this.skip()

      await clickTestId('sub-tab-history')

      await browser.waitUntil(
        async () => {
          const content = await $('[data-testid="history-tab-content"]')
          return await content.isExisting()
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'history-tab-content did not appear after switching to history sub-tab' }
      )

      const content = await $('[data-testid="history-tab-content"]')
      expect(await content.isExisting()).toBe(true)
    })

    it('clicking sub-tab-active returns to active/kanban view', async function () {
      if (!mainApp) return this.skip()

      const activeTab = await $('[data-testid="sub-tab-active"]')
      if (!(await activeTab.isExisting())) return this.skip()

      await clickTestId('sub-tab-active')

      // Active view shows kanban columns or empty state — not history content
      await browser.waitUntil(
        async () => {
          const columns = await $$('[data-testid="kanban-column"]')
          const empty = await $('[data-testid="tasks-empty"]')
          const historyContent = await $('[data-testid="history-tab-content"]')
          return (
            columns.length > 0 ||
            (await empty.isExisting()) ||
            !(await historyContent.isExisting())
          )
        },
        { ...WAIT_SHORT, timeoutMsg: 'Active sub-tab did not restore active view' }
      )

      const historyContent = await $('[data-testid="history-tab-content"]')
      // History content should be gone (or kanban visible)
      const columns = await $$('[data-testid="kanban-column"]')
      const empty = await $('[data-testid="tasks-empty"]')
      const backOnActive = columns.length > 0 || (await empty.isExisting()) || !(await historyContent.isExisting())
      expect(backOnActive).toBe(true)
    })
  })

  describe('session history', () => {
    it('history tab shows session headers or empty state', async function () {
      if (!mainApp) return this.skip()

      // Switch to history sub-tab to check content
      const historyTab = await $('[data-testid="sub-tab-history"]')
      if (!(await historyTab.isExisting())) return this.skip()

      await clickTestId('sub-tab-history')
      await $('[data-testid="history-tab-content"]').waitForExist({ timeout: 5_000 })

      // Wait for history loading to resolve
      await browser.waitUntil(
        async () => {
          const loading = await $('[data-testid="history-loading"]')
          return !(await loading.isExisting())
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'History loading did not finish' }
      )

      const sessionHeaders = await $$('[data-testid="session-header"]')
      const emptyState = await $('[data-testid="history-empty"]')
      const hasContent = sessionHeaders.length > 0 || (await emptyState.isExisting())
      expect(hasContent).toBe(true)

      // Return to active sub-tab
      const activeTab = await $('[data-testid="sub-tab-active"]')
      if (await activeTab.isExisting()) await clickTestId('sub-tab-active')
    })
  })

  describe('resilience', () => {
    it('tab round-trip Overview → Tasks still renders content', async function () {
      if (!mainApp) return this.skip()

      await switchToTab('overview')
      await switchToTab('tasks')

      // After round-trip, Tasks should show some content
      await browser.waitUntil(
        async () => {
          const loading = await $('[data-testid="tasks-loading"]')
          const empty = await $('[data-testid="tasks-empty"]')
          const columns = await $$('[data-testid="kanban-column"]')
          const subTabs = await $('[data-testid="sub-tab-list"]')
          return (
            !(await loading.isExisting()) ||
            (await empty.isExisting()) ||
            columns.length > 0 ||
            (await subTabs.isExisting())
          )
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Tasks did not recover after tab round-trip' }
      )

      const loading = await $('[data-testid="tasks-loading"]')
      const empty = await $('[data-testid="tasks-empty"]')
      const columns = await $$('[data-testid="kanban-column"]')
      const subTabs = await $('[data-testid="sub-tab-list"]')
      const hasContent =
        !(await loading.isExisting()) ||
        (await empty.isExisting()) ||
        columns.length > 0 ||
        (await subTabs.isExisting())
      expect(hasContent).toBe(true)
    })
  })
})
