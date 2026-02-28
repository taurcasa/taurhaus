/**
 * Visual review — capture screenshots of all main views.
 *
 * Resolution: 1280×1440 (half-width of 2560×1440, user's side-panel setup).
 * Screenshots saved to e2e/screenshots/.
 *
 * Run:
 *   E2E_SKIP_BUILD=1 npx wdio run e2e/wdio.conf.js --spec e2e/specs/screenshots.js
 */

import { resolve } from 'node:path'
import { mkdirSync } from 'node:fs'
import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { clickTestId, fastClick } from '../helpers/navigation.js'
import { POLL_FAST, WAIT_SHORT } from '../helpers/timing.js'

const screenshotDir = resolve(import.meta.dirname, '..', 'screenshots')

// Wait for tab-specific content to render (replaces fixed pauses)
async function waitForContent(selectors, timeout = 3_000) {
  const sels = Array.isArray(selectors) ? selectors : [selectors]
  await browser.waitUntil(
    async () => {
      for (const sel of sels) {
        const el = await $(sel)
        if (await el.isExisting()) return true
      }
      return false
    },
    { timeout, interval: POLL_FAST }
  ).catch(() => {}) // Best-effort — take screenshot regardless
}

describe('Visual Review Screenshots', () => {
  before(async () => {
    mkdirSync(screenshotDir, { recursive: true })

    // Set window size: half of 2560×1440
    await browser.setWindowSize(1280, 1440)

    await waitForAppReady()
    await ensureMainApp()

    // Wait for sidebar to settle
    await waitForContent(['[data-testid="project-item"]'])
  })

  it('01 — Overview tab (dark mode)', async () => {
    await clickTestId('tab-overview')
    await waitForContent(['[data-testid="quick-actions"]', '[data-testid="overview-readme"]'])
    await browser.saveScreenshot(resolve(screenshotDir, '01-overview-dark.png'))
  })

  it('02 — Overview tab (light mode)', async () => {
    await clickTestId('theme-light')
    await browser.waitUntil(
      async () => !(await browser.execute(() => document.documentElement.classList.contains('dark'))),
      { timeout: 1_000, interval: POLL_FAST }
    )
    await browser.saveScreenshot(resolve(screenshotDir, '02-overview-light.png'))
  })

  it('03 — Files tab (light mode)', async () => {
    await clickTestId('tab-files')
    await waitForContent(['[role="treeitem"]'])

    // Click the first file in the tree if available
    const firstFile = await $('li[role="treeitem"]:not([aria-expanded])')
    if (await firstFile.isExisting()) {
      await fastClick('li[role="treeitem"]:not([aria-expanded])')
      await waitForContent(['[data-testid="code-viewer"]', '[data-testid="markdown-content"]'])
    }

    await browser.saveScreenshot(resolve(screenshotDir, '03-files-light.png'))
  })

  it('04 — Tasks tab (light mode)', async () => {
    await clickTestId('tab-tasks')
    await waitForContent([
      '[data-testid="kanban-column"]',
      '[data-testid="tasks-empty"]',
      '[data-testid="sub-tab-list"]',
    ])
    await browser.saveScreenshot(resolve(screenshotDir, '04-tasks-light.png'))
  })

  it('05 — Git tab (light mode)', async () => {
    await clickTestId('tab-git')
    await waitForContent(['[data-testid="commit-row"]', '[data-testid="git-empty"]'])

    const firstCommit = await $('[data-testid="commit-row"]')
    if (await firstCommit.isExisting()) {
      await clickTestId('commit-row')
      await waitForContent(['[data-testid="commit-file"]'])
    }

    await browser.saveScreenshot(resolve(screenshotDir, '05-git-light.png'))
  })

  it('06 — Switch back to dark mode', async () => {
    await clickTestId('theme-dark')
    await browser.waitUntil(
      async () => await browser.execute(() => document.documentElement.classList.contains('dark')),
      { timeout: 1_000, interval: POLL_FAST }
    )
  })

  it('07 — Files tab (dark mode)', async () => {
    await clickTestId('tab-files')
    await waitForContent(['[role="treeitem"]'])

    const firstFile = await $('li[role="treeitem"]:not([aria-expanded])')
    if (await firstFile.isExisting()) {
      await fastClick('li[role="treeitem"]:not([aria-expanded])')
      await waitForContent(['[data-testid="code-viewer"]', '[data-testid="markdown-content"]'])
    }

    await browser.saveScreenshot(resolve(screenshotDir, '07-files-dark.png'))
  })

  it('08 — Tasks tab (dark mode)', async () => {
    await clickTestId('tab-tasks')
    await waitForContent([
      '[data-testid="kanban-column"]',
      '[data-testid="tasks-empty"]',
      '[data-testid="sub-tab-list"]',
    ])
    await browser.saveScreenshot(resolve(screenshotDir, '08-tasks-dark.png'))
  })

  it('09 — Git tab (dark mode)', async () => {
    await clickTestId('tab-git')
    await waitForContent(['[data-testid="commit-row"]', '[data-testid="git-empty"]'])

    const firstCommit = await $('[data-testid="commit-row"]')
    if (await firstCommit.isExisting()) {
      await clickTestId('commit-row')
      await waitForContent(['[data-testid="commit-file"]'])
    }

    await browser.saveScreenshot(resolve(screenshotDir, '09-git-dark.png'))
  })

  it('10 — Switch to different project', async () => {
    const sidebarItems = await $$('aside button')
    for (const item of sidebarItems) {
      const text = await item.getText()
      if (text && !text.includes('taurhaus') && !text.includes('Filter') &&
          !text.includes('Manage') && !text.includes('Settings')) {
        await browser.execute((el) => el.click(), item)
        await waitForContent(['[data-testid="quick-actions"]', '[data-testid="overview-readme"]'])
        break
      }
    }
    await browser.saveScreenshot(resolve(screenshotDir, '10-other-project-dark.png'))
  })
})
