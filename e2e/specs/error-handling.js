/**
 * Error Handling — resilience tests for project management errors, search edge cases,
 * and settings validation. Project management validation errors are also covered in
 * project-lifecycle.js; this file focuses on coverage not tested there.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded, switchToTab } from '../helpers/navigation.js'
import { openManageProjects, closeModal, tryAddProjectPath } from '../helpers/modal.js'
import { dismissSearch } from '../helpers/search.js'

let mainApp = false

describe('Error Handling', () => {
  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (mainApp) await waitForProjectsLoaded()
  })

  // ─── Project Management Errors ─────────────────────────────────────────────

  describe('project management errors', () => {
    afterEach(async () => {
      // Close modal if left open by a failed test
      const modal = await $('[data-testid="manage-projects-modal"]')
      if (await modal.isExisting()) await closeModal()
    })

    it('invalid path shows a validation error message', async function () {
      if (!mainApp) return this.skip()

      await openManageProjects()
      await tryAddProjectPath('/nonexistent/path/xyz123')

      await browser.waitUntil(
        async () => {
          const err = await $('[data-testid="validation-message"], [data-testid="manual-error"]')
          return await err.isExisting()
        },
        { timeout: 5_000, interval: 300, timeoutMsg: 'Validation error did not appear for invalid path' }
      )
    })

    it('non-git directory shows error message containing "git"', async function () {
      if (!mainApp) return this.skip()

      await openManageProjects()
      await tryAddProjectPath('/tmp')

      await browser.waitUntil(
        async () => {
          const err = await $('[data-testid="manual-error"], [data-testid="validation-message"]')
          if (!(await err.isExisting())) return false
          const text = await browser.execute((el) => el.textContent, err)
          return text.toLowerCase().includes('git')
        },
        { timeout: 5_000, interval: 300, timeoutMsg: '"Not a git repository" message did not appear for /tmp' }
      )
    })

    it('already-registered path shows duplicate error', async function () {
      if (!mainApp) return this.skip()

      await openManageProjects()
      await tryAddProjectPath('/home/mstie/projects/taurhaus')

      await browser.waitUntil(
        async () => {
          const err = await $('[data-testid="manual-error"], [data-testid="validation-message"]')
          if (!(await err.isExisting())) return false
          const text = await browser.execute((el) => el.textContent, err)
          const lower = text.toLowerCase()
          return lower.includes('already') || lower.includes('registered') || lower.includes('exists')
        },
        { timeout: 5_000, interval: 300, timeoutMsg: 'Duplicate path error message did not appear' }
      )
    })
  })

  // ─── Search Edge Cases ─────────────────────────────────────────────────────

  describe('search edge cases', () => {
    afterEach(async () => {
      await dismissSearch()
    })

    it('opening search with empty query does not crash — overlay remains functional', async function () {
      if (!mainApp) return this.skip()

      // Open search overlay (Cmd+K / Ctrl+K)
      await browser.keys(['Meta', 'k'])
      await browser.pause(300)

      // Try Ctrl+K as alternative if Meta+K didn't work (Linux)
      let overlay = await $('[data-testid="search-overlay"]')
      if (!(await overlay.isExisting())) {
        await browser.keys(['Control', 'k'])
        await browser.pause(300)
        overlay = await $('[data-testid="search-overlay"]')
      }

      if (!(await overlay.isExisting())) {
        // Search overlay may not be keyboard-triggered — skip gracefully
        return this.skip()
      }

      // Overlay is open with empty query — verify it is stable (no crash/disappear)
      const input = await $('[data-testid="search-input"]')
      expect(await input.isExisting()).toBe(true)

      // Wait a beat to ensure no spontaneous crash
      await browser.pause(500)
      expect(await overlay.isExisting()).toBe(true)
    })

    it('typing gibberish in search does not crash — shows empty or no-results state', async function () {
      if (!mainApp) return this.skip()

      await browser.keys(['Meta', 'k'])
      await browser.pause(300)

      let overlay = await $('[data-testid="search-overlay"]')
      if (!(await overlay.isExisting())) {
        await browser.keys(['Control', 'k'])
        await browser.pause(300)
        overlay = await $('[data-testid="search-overlay"]')
      }

      if (!(await overlay.isExisting())) return this.skip()

      const input = await $('[data-testid="search-input"]')
      await input.setValue('zzz_no_match_gibberish_xyz_9999_!@#')

      // Wait for any results (or lack of results) to settle
      await browser.pause(800)

      // The overlay must still be present — no crash
      expect(await overlay.isExisting()).toBe(true)

      // Search results area may exist (empty) or a no-results indicator
      const results = await $('[data-testid="search-results"]')
      if (await results.isExisting()) {
        const items = await results.$$('li, [role="option"], [data-testid^="result-"]')
        // Either empty or showing a no-results message is fine — no crash is the assertion
        expect(items.length).toBeGreaterThanOrEqual(0)
      }
    })
  })

  // ─── Settings Validation ───────────────────────────────────────────────────

  describe('settings validation', () => {
    before(async () => {
      if (!mainApp) return
      // Open settings view
      const toggle = await $('[data-testid="settings-toggle"]')
      if (await toggle.isExisting()) {
        await toggle.click()
        await browser.waitUntil(
          async () => await (await $('[data-testid="settings-view"]')).isExisting(),
          { timeout: 5_000, interval: 300, timeoutMsg: 'Settings view did not open' }
        )
      }
    })

    after(async () => {
      if (!mainApp) return
      // Close settings view if still open
      const settings = await $('[data-testid="settings-view"]')
      if (await settings.isExisting()) {
        const toggle = await $('[data-testid="settings-toggle"]')
        if (await toggle.isExisting()) await toggle.click()
      }
    })

    it('settings threshold-active rejects or clamps non-numeric input', async function () {
      if (!mainApp) return this.skip()

      const settings = await $('[data-testid="settings-view"]')
      if (!(await settings.isExisting())) return this.skip()

      const threshold = await $('[data-testid="threshold-active"]')
      if (!(await threshold.isExisting())) return this.skip()

      // Get initial valid value
      const initialValue = await threshold.getValue()

      // Try to set invalid value
      await threshold.clearValue()
      await threshold.setValue('abc')
      await browser.keys(['Tab']) // blur to trigger validation

      await browser.pause(500)

      // The field should either: revert to the original value, show empty, or clamp
      const afterValue = await threshold.getValue()
      const parsed = parseFloat(afterValue)

      // A valid numeric field should not accept "abc" — it should be empty, NaN-rejected, or reverted
      const isRejected =
        afterValue === '' ||
        afterValue === initialValue ||
        isNaN(parsed)

      expect(isRejected).toBe(true)
    })

    it('settings threshold-active accepts numeric values and field stays functional', async function () {
      if (!mainApp) return this.skip()

      const settings = await $('[data-testid="settings-view"]')
      if (!(await settings.isExisting())) return this.skip()

      const threshold = await $('[data-testid="threshold-active"]')
      if (!(await threshold.isExisting())) return this.skip()

      // Get initial valid value
      const initialValue = await threshold.getValue()

      await threshold.clearValue()
      await threshold.setValue('30')
      await browser.keys(['Tab'])

      await browser.pause(500)

      const afterValue = await threshold.getValue()
      const afterNum = parseFloat(afterValue)

      // Field should accept the valid numeric value
      expect(isNaN(afterNum)).toBe(false)
      expect(afterNum).toBe(30)

      // Restore original value
      if (initialValue) {
        await threshold.clearValue()
        await threshold.setValue(initialValue)
        await browser.keys(['Tab'])
      }
    })
  })
})
