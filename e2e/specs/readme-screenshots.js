/**
 * README screenshots — high-quality captures for the project README.
 *
 * Resolution: 1400×900 (conventional desktop proportions).
 * Dark mode only. Saves to docs/ (not gitignored).
 *
 * Run:
 *   E2E_SKIP_BUILD=1 npx wdio run e2e/wdio.conf.js --spec e2e/specs/readme-screenshots.js
 */

import { resolve } from 'node:path'
import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { clickTestId, fastClick } from '../helpers/navigation.js'
import { POLL_FAST } from '../helpers/timing.js'

const docsDir = resolve(import.meta.dirname, '..', '..', 'docs')

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
  ).catch(() => {})
}

describe('README Screenshots', () => {
  before(async () => {
    // Wider, shorter window for conventional desktop proportions
    await browser.setWindowSize(1400, 900)

    await waitForAppReady()
    await ensureMainApp()

    // Ensure dark mode
    const darkBtn = await $('[data-testid="theme-dark"]')
    if (await darkBtn.isExisting()) {
      await clickTestId('theme-dark')
      await browser.waitUntil(
        async () => await browser.execute(() => document.documentElement.classList.contains('dark')),
        { timeout: 1_000, interval: POLL_FAST }
      )
    }

    // Wait for sidebar to settle
    await waitForContent(['[data-testid="project-item"]'])
  })

  it('hero — Overview tab (ledger project)', async () => {
    // Navigate to "ledger" project for a clean README that isn't self-referential
    const sidebarItems = await $$('[data-testid="project-item"]')
    for (const item of sidebarItems) {
      const text = await browser.execute((el) => el.textContent, item)
      if (text && text.toLowerCase().includes('ledger')) {
        await item.scrollIntoView()
        await browser.execute((el) => el.click(), item)
        await waitForContent(['[data-testid="quick-actions"]', '[data-testid="overview-readme"]'])
        break
      }
    }

    await clickTestId('tab-overview')
    await waitForContent(['[data-testid="quick-actions"]', '[data-testid="overview-readme"]'])
    await browser.saveScreenshot(resolve(docsDir, 'screenshot-overview.png'))
  })

  it('git — Git tab with diff', async () => {
    await clickTestId('tab-git')
    await waitForContent(['[data-testid="commit-row"]', '[data-testid="git-empty"]'])

    // Click the first commit to show the diff panel
    const firstCommit = await $('[data-testid="commit-row"]')
    if (await firstCommit.isExisting()) {
      await clickTestId('commit-row')
      await waitForContent(['[data-testid="commit-file"]'])
    }

    await browser.saveScreenshot(resolve(docsDir, 'screenshot-git.png'))
  })

  it('files — Files tab with code preview', async () => {
    await clickTestId('tab-files')
    await waitForContent(['[role="treeitem"]'])

    // Click a source file to show syntax highlighting
    const firstFile = await $('li[role="treeitem"]:not([aria-expanded])')
    if (await firstFile.isExisting()) {
      await fastClick('li[role="treeitem"]:not([aria-expanded])')
      await waitForContent(['[data-testid="code-viewer"]', '[data-testid="markdown-content"]'])
    }

    await browser.saveScreenshot(resolve(docsDir, 'screenshot-files.png'))
  })
})
