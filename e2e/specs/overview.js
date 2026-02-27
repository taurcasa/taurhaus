/**
 * Overview tab e2e tests — verify the Overview tab renders correctly
 * with project header, quick actions, commits, and relationships.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'

describe('Overview Tab', () => {
  let mainApp = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()

    if (mainApp) {
      // Ensure we're on the Overview tab
      const overviewTab = await $('button=Overview')
      await overviewTab.click()
      await browser.pause(1_000)
    }
  })

  describe('project header', () => {
    it('displays project name', async function () {
      if (!mainApp) return this.skip()
      const h1 = await $('h1')
      const text = await h1.getText()
      expect(text.length).toBeGreaterThan(0)
    })

    it('shows branch name', async function () {
      if (!mainApp) return this.skip()
      // Branch appears as a monospace span near the project name
      const branchSpan = await $('h1 + span')
      expect(await branchSpan.isExisting()).toBe(true)
    })
  })

  describe('quick actions', () => {
    it('shows Claude launch button', async function () {
      if (!mainApp) return this.skip()
      const btn = await $('[data-testid="action-launch-claude"]')
      expect(await btn.isDisplayed()).toBe(true)
    })

    it('shows Codex launch button', async function () {
      if (!mainApp) return this.skip()
      const btn = await $('[data-testid="action-launch-codex"]')
      expect(await btn.isDisplayed()).toBe(true)
    })

    it('shows Gemini launch button', async function () {
      if (!mainApp) return this.skip()
      const btn = await $('[data-testid="action-launch-gemini"]')
      expect(await btn.isDisplayed()).toBe(true)
    })

    it('shows Terminal button', async function () {
      if (!mainApp) return this.skip()
      const btn = await $('[data-testid="action-open-terminal"]')
      expect(await btn.isDisplayed()).toBe(true)
    })
  })

  describe('last commit', () => {
    it('shows last commit section', async function () {
      if (!mainApp) return this.skip()
      // "Last commit" label should be visible
      const label = await $('span=Last commit')
      expect(await label.isExisting()).toBe(true)
    })

    it('shows commit hash or "No commits" message', async function () {
      if (!mainApp) return this.skip()
      const commitRow = await $('[data-testid="overview-last-commit"]')
      const noCommits = await $('p=No commits found.')

      const hasContent = (await commitRow.isExisting()) || (await noCommits.isExisting())
      expect(hasContent).toBe(true)
    })
  })

  describe('recent activity', () => {
    it('shows recent activity section', async function () {
      if (!mainApp) return this.skip()
      const label = await $('span=Recent activity')
      expect(await label.isExisting()).toBe(true)
    })
  })

  describe('relationships', () => {
    it('shows relationships section', async function () {
      if (!mainApp) return this.skip()
      const label = await $('span=Relationships')
      expect(await label.isExisting()).toBe(true)
    })

    it('shows "No connections" or relationship rows', async function () {
      if (!mainApp) return this.skip()
      const noConns = await $('p=No connections detected yet.')
      const relRows = await $$('[data-testid="relationship-row"]')
      const hasContent = (await noConns.isExisting()) || relRows.length > 0
      expect(hasContent).toBe(true)
    })
  })

  describe('session history', () => {
    it('shows session history section', async function () {
      if (!mainApp) return this.skip()
      const label = await $('span=Session history')
      expect(await label.isExisting()).toBe(true)
    })
  })

  describe('project info', () => {
    it('shows project path', async function () {
      if (!mainApp) return this.skip()
      const pathLabel = await $('span=Path')
      expect(await pathLabel.isExisting()).toBe(true)
    })
  })
})
