/**
 * TaskBoard e2e tests — verify the Tasks tab renders correctly
 * and displays task data from the real backend.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'

describe('TaskBoard', () => {
  let mainApp = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()

    if (mainApp) {
      const tasksTab = await $('button=Tasks')
      await tasksTab.click()
      // Wait for the Tasks header to appear
      const header = await $('h2=Tasks')
      await header.waitForDisplayed({ timeout: 5_000 })
    }
  })

  it('renders the Tasks header', async function () {
    if (!mainApp) return this.skip()

    const header = await $('h2=Tasks')
    expect(await header.getText()).toBe('Tasks')
  })

  it('completes loading without hanging', async function () {
    if (!mainApp) return this.skip()

    await browser.waitUntil(
      async () => {
        const loading = await $('[data-testid="tasks-loading"]')
        return !(await loading.isExisting())
      },
      { timeout: 10_000, timeoutMsg: 'Task loading did not complete' }
    )
  })

  it('shows empty state or task rows after loading', async function () {
    if (!mainApp) return this.skip()

    const empty = await $('[data-testid="tasks-empty"]')
    const taskRows = await $$('[data-testid="task-row"]')
    const hasContent = (await empty.isExisting()) || taskRows.length > 0
    expect(hasContent).toBe(true)
  })

  it('displays correct empty state message when no tasks', async function () {
    if (!mainApp) return this.skip()

    const empty = await $('[data-testid="tasks-empty"]')
    if (await empty.isExisting()) {
      const text = await empty.getText()
      expect(text).toContain('No tasks tracked')
    }
  })

  it('renders task rows with SVG icons when tasks exist', async function () {
    if (!mainApp) return this.skip()

    const taskRows = await $$('[data-testid="task-row"]')
    if (taskRows.length > 0) {
      const firstRow = taskRows[0]
      const svg = await firstRow.$('svg')
      expect(await svg.isExisting()).toBe(true)
    }
  })

  it('survives tab switching round-trip', async function () {
    if (!mainApp) return this.skip()

    // Switch to Overview
    const overviewTab = await $('button=Overview')
    await overviewTab.click()
    await browser.pause(300)

    // Switch back to Tasks
    const tasksTab = await $('button=Tasks')
    await tasksTab.click()

    // Header should reappear
    const header = await $('h2=Tasks')
    await header.waitForDisplayed({ timeout: 5_000 })
    expect(await header.getText()).toBe('Tasks')
  })
})
