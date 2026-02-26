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

const docsDir = resolve(import.meta.dirname, '..', '..', 'docs')

describe('README Screenshots', () => {
  before(async () => {
    // Wider, shorter window for conventional desktop proportions
    await browser.setWindowSize(1400, 900)

    await waitForAppReady()
    await ensureMainApp()

    // Ensure dark mode
    const darkBtn = await $('button=Dark')
    if (await darkBtn.isExisting()) {
      await darkBtn.click()
      await browser.pause(500)
    }

    // Extra settle time for real data to load
    await browser.pause(2_000)
  })

  it('hero — Overview tab', async () => {
    const overviewTab = await $('button=Overview')
    await overviewTab.click()
    await browser.pause(1_500)
    await browser.saveScreenshot(resolve(docsDir, 'screenshot-overview.png'))
  })

  it('git — Git tab with diff', async () => {
    const gitTab = await $('button=Git')
    await gitTab.click()
    await browser.pause(1_500)

    // Click the first commit to show the diff panel
    const firstCommit = await $('[data-testid="commit-row"]')
    if (await firstCommit.isExisting()) {
      await firstCommit.click()
      await browser.pause(1_500)
    }

    await browser.saveScreenshot(resolve(docsDir, 'screenshot-git.png'))
  })

  it('files — Files tab with code preview', async () => {
    const filesTab = await $('button=Files')
    await filesTab.click()
    await browser.pause(1_500)

    // Click a source file to show syntax highlighting
    const firstFile = await $('li[role="treeitem"]:not([aria-expanded])')
    if (await firstFile.isExisting()) {
      await firstFile.click()
      await browser.pause(1_500)
    }

    await browser.saveScreenshot(resolve(docsDir, 'screenshot-files.png'))
  })
})
