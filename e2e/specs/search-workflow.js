/**
 * Search Workflow e2e tests — open/close, result navigation, keyboard nav.
 * Replaces old search.js with workflow-focused tests.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForFileContent, clickTestId } from '../helpers/navigation.js'
import { openSearch, closeSearch, dismissSearch } from '../helpers/search.js'
import { WAIT_INSTANT, WAIT_SHORT, WAIT_MEDIUM, TIMEOUT_LONG } from '../helpers/timing.js'

describe('Search Workflow', () => {
  let mainApp = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
  })

  afterEach(async () => {
    // Ensure search overlay is closed between tests
    await dismissSearch()
  })

  describe('open and close', () => {
    it('Ctrl+K opens search overlay', async function () {
      if (!mainApp) return this.skip()

      await openSearch()

      const overlay = await $('[data-testid="search-overlay"]')
      expect(await overlay.isDisplayed()).toBe(true)
    })

    it('search input is focused on open', async function () {
      if (!mainApp) return this.skip()

      await openSearch()

      const input = await $('[data-testid="search-input"]')
      expect(await input.isFocused()).toBe(true)
    })

    it('Escape closes the overlay', async function () {
      if (!mainApp) return this.skip()

      await openSearch()

      await closeSearch()

      const overlayAfter = await $('[data-testid="search-overlay"]')
      expect(await overlayAfter.isExisting()).toBe(false)
    })

    it('reopening clears prior input and results', async function () {
      if (!mainApp) return this.skip()

      // Open, type something, close, reopen
      await openSearch()
      const input = await $('[data-testid="search-input"]')
      await input.setValue('stale query')

      await closeSearch()
      await openSearch()

      const freshInput = await $('[data-testid="search-input"]')
      const value = await freshInput.getValue()
      expect(value).toBe('')

      const results = await $$('[data-testid="search-result"]')
      expect(results.length).toBe(0)

      await closeSearch()
    })

    it('Ctrl+K toggles overlay open and closed', async function () {
      if (!mainApp) return this.skip()

      await openSearch()

      const overlay = await $('[data-testid="search-overlay"]')
      expect(await overlay.isDisplayed()).toBe(true)

      await closeSearch()

      const overlayAfter = await $('[data-testid="search-overlay"]')
      expect(await overlayAfter.isExisting()).toBe(false)
    })
  })

  describe('search and navigate', () => {
    it('typing "README" shows results with non-empty titles', async function () {
      if (!mainApp) return this.skip()

      await openSearch()

      const input = await $('[data-testid="search-input"]')
      await input.setValue('README')

      // Wait for results to appear
      await browser.waitUntil(
        async () => {
          const results = await $$('[data-testid="search-result"]')
          return results.length > 0
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Search results for "README" did not appear' }
      )

      const results = await $$('[data-testid="search-result"]')
      expect(results.length).toBeGreaterThan(0)

      // Each result should have non-empty text
      const firstText = await results[0].getText()
      expect(firstText.trim().length).toBeGreaterThan(0)
    })

    it('clicking a result closes overlay and loads file in Files tab', async function () {
      if (!mainApp) return this.skip()

      const overlay = await $('[data-testid="search-overlay"]')
      if (!(await overlay.isExisting())) {
        await openSearch()
        const input = await $('[data-testid="search-input"]')
        await input.setValue('README')
        await browser.waitUntil(
          async () => (await $$('[data-testid="search-result"]')).length > 0,
          { ...WAIT_MEDIUM, timeoutMsg: 'No results to click' }
        )
      }

      const firstResult = await $('[data-testid="search-result"]')
      if (!(await firstResult.isExisting())) return this.skip()

      await clickTestId('search-result')

      // Overlay must close
      await browser.waitUntil(
        async () => {
          const o = await $('[data-testid="search-overlay"]')
          return !(await o.isExisting())
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Overlay did not close after clicking result' }
      )

      // Files tab should now be active — look for file content
      await waitForFileContent(TIMEOUT_LONG, 'File content did not load after clicking search result')
    })

    it('gibberish query shows no-results state', async function () {
      if (!mainApp) return this.skip()

      await openSearch()

      // Type gibberish using keyboard to ensure oninput fires
      await clickTestId('search-input')
      await browser.keys('xyzzy999qqq'.split(''))

      // Wait for "No matches found" to appear in the results container
      await browser.waitUntil(
        async () => {
          const container = await $('[data-testid="search-results"]')
          if (!(await container.isExisting())) return false
          const text = await browser.execute(
            (el) => el.textContent,
            container
          )
          return text.includes('No matches')
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'No-results state did not appear for gibberish query' }
      )

      // Overlay should still be open (no crash)
      const overlay = await $('[data-testid="search-overlay"]')
      expect(await overlay.isExisting()).toBe(true)

      // Should have zero result buttons
      const results = await $$('[data-testid="search-result"]')
      expect(results.length).toBe(0)
    })
  })

  describe('keyboard navigation', () => {
    it('ArrowDown highlights first result', async function () {
      if (!mainApp) return this.skip()

      await openSearch()

      const input = await $('[data-testid="search-input"]')
      await input.setValue('README')

      await browser.waitUntil(
        async () => (await $$('[data-testid="search-result"]')).length > 0,
        { ...WAIT_MEDIUM, timeoutMsg: 'No results for ArrowDown test' }
      )

      await browser.keys('ArrowDown')

      // First result should be highlighted via CSS class (bg-zinc-800 dark / bg-zinc-100 light)
      const results = await $$('[data-testid="search-result"]')
      if (results.length === 0) return this.skip()

      const firstResult = results[0]
      const className = await firstResult.getAttribute('class') ?? ''

      const isHighlighted =
        className.includes('bg-zinc-800') ||
        className.includes('bg-zinc-100')

      expect(isHighlighted).toBe(true)
    })

    it('Enter on highlighted result navigates and closes overlay', async function () {
      if (!mainApp) return this.skip()

      const overlay = await $('[data-testid="search-overlay"]')
      if (!(await overlay.isExisting())) {
        await openSearch()
        const input = await $('[data-testid="search-input"]')
        await input.setValue('README')
        await browser.waitUntil(
          async () => (await $$('[data-testid="search-result"]')).length > 0,
          { ...WAIT_MEDIUM, timeoutMsg: 'No results for Enter test' }
        )
        await browser.keys('ArrowDown')
      }

      await browser.keys('Enter')

      // Overlay must close
      await browser.waitUntil(
        async () => {
          const o = await $('[data-testid="search-overlay"]')
          return !(await o.isExisting())
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Overlay did not close after Enter' }
      )

      // File content must load
      await waitForFileContent(TIMEOUT_LONG, 'File did not load after Enter on result')
    })
  })
})
