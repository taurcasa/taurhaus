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

const screenshotDir = resolve(import.meta.dirname, '..', 'screenshots')

describe('Visual Review Screenshots', () => {
  before(async () => {
    mkdirSync(screenshotDir, { recursive: true })

    // Set window size: half of 2560×1440
    await browser.setWindowSize(1280, 1440)

    await waitForAppReady()
    await ensureMainApp()

    // Extra settle time for real data to load
    await browser.pause(2_000)
  })

  it('01 — Overview tab (dark mode)', async () => {
    // Default should be dark mode; click Overview to be sure
    const overviewTab = await $('button=Overview')
    await overviewTab.click()
    await browser.pause(1_000)
    await browser.saveScreenshot(resolve(screenshotDir, '01-overview-dark.png'))
  })

  it('02 — Overview tab (light mode)', async () => {
    const lightBtn = await $('button=Light')
    await lightBtn.click()
    await browser.pause(500)
    await browser.saveScreenshot(resolve(screenshotDir, '02-overview-light.png'))
  })

  it('03 — Files tab (light mode)', async () => {
    const filesTab = await $('button=Files')
    await filesTab.click()
    await browser.pause(1_500)

    // Click the first file in the tree if available
    const firstFile = await $('li[role="treeitem"]:not([aria-expanded])')
    if (await firstFile.isExisting()) {
      await firstFile.click()
      await browser.pause(1_000)
    }

    await browser.saveScreenshot(resolve(screenshotDir, '03-files-light.png'))
  })

  it('04 — Tasks tab (light mode)', async () => {
    const tasksTab = await $('button=Tasks')
    await tasksTab.click()
    await browser.pause(1_500)
    await browser.saveScreenshot(resolve(screenshotDir, '04-tasks-light.png'))
  })

  it('05 — Git tab (light mode)', async () => {
    const gitTab = await $('button=Git')
    await gitTab.click()
    await browser.pause(1_500)

    // Click the first commit if available
    const firstCommit = await $('[data-testid="commit-row"]')
    if (await firstCommit.isExisting()) {
      await firstCommit.click()
      await browser.pause(1_000)
    }

    await browser.saveScreenshot(resolve(screenshotDir, '05-git-light.png'))
  })

  it('06 — Switch back to dark mode', async () => {
    const darkBtn = await $('button=Dark')
    await darkBtn.click()
    await browser.pause(500)
  })

  it('07 — Files tab (dark mode)', async () => {
    const filesTab = await $('button=Files')
    await filesTab.click()
    await browser.pause(1_500)

    const firstFile = await $('li[role="treeitem"]:not([aria-expanded])')
    if (await firstFile.isExisting()) {
      await firstFile.click()
      await browser.pause(1_000)
    }

    await browser.saveScreenshot(resolve(screenshotDir, '07-files-dark.png'))
  })

  it('08 — Tasks tab (dark mode)', async () => {
    const tasksTab = await $('button=Tasks')
    await tasksTab.click()
    await browser.pause(1_500)
    await browser.saveScreenshot(resolve(screenshotDir, '08-tasks-dark.png'))
  })

  it('09 — Git tab (dark mode)', async () => {
    const gitTab = await $('button=Git')
    await gitTab.click()
    await browser.pause(1_500)

    const firstCommit = await $('[data-testid="commit-row"]')
    if (await firstCommit.isExisting()) {
      await firstCommit.click()
      await browser.pause(1_000)
    }

    await browser.saveScreenshot(resolve(screenshotDir, '09-git-dark.png'))
  })

  it('10 — Switch to different project', async () => {
    // Click a different project in the sidebar
    const sidebarItems = await $$('aside button')
    for (const item of sidebarItems) {
      const text = await item.getText()
      // Pick a project that isn't "taurhaus" (the first/active one)
      if (text && !text.includes('taurhaus') && !text.includes('Filter') &&
          !text.includes('Manage') && !text.includes('Settings')) {
        await item.click()
        await browser.pause(2_000)
        break
      }
    }
    await browser.saveScreenshot(resolve(screenshotDir, '10-other-project-dark.png'))
  })
})
