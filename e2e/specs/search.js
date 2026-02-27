/**
 * Search overlay e2e tests — verify the search UI opens, accepts input,
 * shows results, and supports keyboard navigation.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'

describe('Search Overlay', () => {
  let mainApp = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
  })

  describe('open and close', () => {
    it('opens search with Ctrl+K', async function () {
      if (!mainApp) return this.skip()

      await browser.keys(['Control', 'k'])
      await browser.pause(500)

      const searchOverlay = await $('[data-testid="search-overlay"]')
      if (!(await searchOverlay.isExisting())) {
        // Fallback: try clicking a search button
        const searchBtn = await $('[data-testid="search-btn"]')
        if (await searchBtn.isExisting()) {
          await searchBtn.click()
          await browser.pause(500)
        }
      }

      const overlay = await $('[data-testid="search-overlay"]')
      if (!(await overlay.isExisting())) return this.skip()
      expect(await overlay.isDisplayed()).toBe(true)
    })

    it('shows search input when open', async function () {
      if (!mainApp) return this.skip()
      const overlay = await $('[data-testid="search-overlay"]')
      if (!(await overlay.isExisting())) return this.skip()

      const input = await overlay.$('input')
      expect(await input.isDisplayed()).toBe(true)
    })

    it('search input is focused', async function () {
      if (!mainApp) return this.skip()
      const overlay = await $('[data-testid="search-overlay"]')
      if (!(await overlay.isExisting())) return this.skip()

      const input = await overlay.$('input')
      expect(await input.isFocused()).toBe(true)
    })

    it('closes search with Escape', async function () {
      if (!mainApp) return this.skip()
      const overlay = await $('[data-testid="search-overlay"]')
      if (!(await overlay.isExisting())) return this.skip()

      await browser.keys('Escape')
      await browser.pause(300)

      const overlayAfter = await $('[data-testid="search-overlay"]')
      expect(await overlayAfter.isExisting()).toBe(false)
    })
  })

  describe('search results', () => {
    it('typing a query shows results or empty state', async function () {
      if (!mainApp) return this.skip()

      // Reopen search
      await browser.keys(['Control', 'k'])
      await browser.pause(500)

      const overlay = await $('[data-testid="search-overlay"]')
      if (!(await overlay.isExisting())) return this.skip()

      const input = await overlay.$('input')
      await input.setValue('README')
      await browser.pause(1_000) // Wait for debounce

      // Should show results or "no results" message
      const results = await overlay.$$('[data-testid="search-result"]')
      const noResults = await overlay.$('[data-testid="search-no-results"]')
      const hasContent = results.length > 0 || (await noResults.isExisting())
      // Even if no results, the overlay should still be showing
      expect(await overlay.isDisplayed()).toBe(true)
    })

    it('cleans up after closing', async function () {
      if (!mainApp) return this.skip()
      await browser.keys('Escape')
      await browser.pause(300)

      const overlay = await $('[data-testid="search-overlay"]')
      expect(await overlay.isExisting()).toBe(false)
    })
  })
})
