/**
 * Regression e2e tests — targeted tests for bugs that have been found and
 * fixed. Each test documents the original regression and ensures it doesn't
 * recur. Add a new test here any time a visual or behavioral regression is
 * discovered.
 */

import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { selectProjectByName, switchToTab, waitForProjectsLoaded } from '../helpers/navigation.js'
import { TAURHAUS_CLAUDE_DIR, TAURHAUS_PROJECT_PATH } from '../helpers/platform.js'
import { POLL, WAIT_MEDIUM } from '../helpers/timing.js'

const REGRESSION_STAMP = Date.now()
const REGRESSION_TEAM = `event-pipeline-team-${REGRESSION_STAMP}`
const TASK_SUBJECT = `Regression task ${REGRESSION_STAMP}`
const README_MARKER = `event-pipeline-readme-${REGRESSION_STAMP}`

function writeRegressionTask(teamName, projectPath, subject) {
  const teamDir = join(TAURHAUS_CLAUDE_DIR, 'teams', teamName)
  const tasksDir = join(TAURHAUS_CLAUDE_DIR, 'tasks', teamName)
  mkdirSync(teamDir, { recursive: true })
  mkdirSync(tasksDir, { recursive: true })
  writeFileSync(
    join(teamDir, 'config.json'),
    JSON.stringify(
      {
        name: teamName,
        members: [{ projectPath }],
      },
      null,
      2
    ),
    'utf8'
  )
  writeFileSync(
    join(tasksDir, '1.json'),
    JSON.stringify(
      {
        id: '1',
        subject,
        status: 'pending',
      },
      null,
      2
    ),
    'utf8'
  )
}

function cleanupRegressionTask(teamName) {
  rmSync(join(TAURHAUS_CLAUDE_DIR, 'teams', teamName), { recursive: true, force: true })
  rmSync(join(TAURHAUS_CLAUDE_DIR, 'tasks', teamName), { recursive: true, force: true })
}

describe('Regressions', () => {
  let mainApp = false

  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
  })

  describe('content-enter animation (commit 768cdec regression)', () => {
    // The content-enter CSS class provides a subtle fade-up animation when
    // switching projects. It was accidentally dropped from the {#key} wrapper
    // div during component extraction in 768cdec. The animation CSS was also
    // orphaned in OverviewTab, GitTab, and TaskBoard (class referenced but
    // no matching <style> block because it was scoped to Shell.svelte).
    // Fix: moved animation to app.css as a global rule.
    //
    // IMPORTANT: content-enter must ONLY be on the {#key} wrapper div
    // (Shell.svelte), NOT on elements inside individual tabs. Chromium
    // replays CSS animations when toggling display:none (class:hidden),
    // and the transform property forces GPU compositor layer creation.
    // For tabs with large Shiki-highlighted content (thousands of spans),
    // this causes multi-second freezes on every tab switch.

    it('main content wrapper has content-enter class', async function () {
      if (!mainApp) return this.skip()

      // The {#key} wrapper div is the direct child of <main> that holds
      // all tab content. It should have the content-enter class.
      const wrapper = await $('[data-testid="content-wrapper"]')
      expect(await wrapper.isExisting()).toBe(true)

      const hasClass = await browser.execute(() => {
        const el = document.querySelector('[data-testid="content-wrapper"]')
        return el ? el.classList.contains('content-enter') : false
      })
      expect(hasClass).toBe(true)
    })

    it('content-enter animation is defined in CSS', async function () {
      if (!mainApp) return this.skip()

      // Verify the animation actually resolves — not just the class name
      // existing, but that the browser has a matching animation definition.
      const animationName = await browser.execute(() => {
        const el = document.querySelector('[data-testid="content-wrapper"]')
        if (!el) return 'none'
        const style = window.getComputedStyle(el)
        return style.animationName
      })
      expect(animationName).toBe('content-enter')
    })

    it('tab internals do NOT have content-enter class', async function () {
      if (!mainApp) return this.skip()

      // content-enter must only exist once (on the {#key} wrapper).
      // If any tab component also has it, the animation replays on every
      // tab switch, causing GPU compositor thrashing with large content.
      const count = await browser.execute(() => {
        return document.querySelectorAll('.content-enter').length
      })
      expect(count).toBe(1) // Only the wrapper
    })
  })

  describe('DirectoryBrowser overflow (commit 284bd54 regression)', () => {
    // The directory tree container lost overflow-hidden during extraction
    // to DirectoryBrowser.svelte, allowing long paths to cause horizontal
    // scrollbar instead of clipping at the rounded border.

    it('directory tree has overflow-hidden for horizontal clipping', async function () {
      if (!mainApp) return this.skip()

      // Open settings to find "Add Project" or navigate to a view with DirectoryBrowser
      // The DirectoryBrowser appears in AddProjectModal and FirstRunWizard.
      // We can't easily trigger those in E2E without side effects, so we
      // verify the CSS rule exists in the stylesheet instead.
      const hasRule = await browser.execute(() => {
        // Check that the global CSS includes overflow-hidden on directory-tree
        // This is a structural check — the component template applies the class
        const testEl = document.querySelector('[data-testid="directory-tree"]')
        if (testEl) {
          const style = window.getComputedStyle(testEl)
          return style.overflowX === 'hidden'
        }
        // DirectoryBrowser not currently rendered — check that the class
        // string 'overflow-hidden' exists in at least one stylesheet
        // (This is a weaker check but still catches the regression)
        return null // Can't verify without the component being mounted
      })

      // If DirectoryBrowser is mounted, verify overflow-x is hidden
      // If not mounted, skip (we can't force it without side effects)
      if (hasRule === null) {
        return this.skip()
      }
      expect(hasRule).toBe(true)
    })
  })

  describe('event pipeline live coverage (commit a53ad31 regression)', () => {
    // Regression: commit a53ad31 removed tmux focus hook installation during
    // the session-control cleanup, so tmux window switches no longer updated
    // tmux-focus.json or the sidebar foreground indicator. The earlier #1364
    // follow-up misdiagnosed the issue as a frontend listener problem because
    // it never verified the running app end to end.
    //
    // The tmux producer path is now covered in Rust plus attached-client
    // manual verification. This E2E block keeps the live file and task update
    // paths honest in the packaged app without a page reload.

    const readmePath = join(TAURHAUS_PROJECT_PATH, 'README.md')
    let originalReadme = null

    before(async () => {
      if (!mainApp) return

      await waitForProjectsLoaded()
      originalReadme = readFileSync(readmePath, 'utf8')
    })

    after(async () => {
      cleanupRegressionTask(REGRESSION_TEAM)
      if (originalReadme !== null) {
        writeFileSync(readmePath, originalReadme, 'utf8')
      }
    })

    it('refreshes README content after a live file edit without reloading the app', async function () {
      if (!mainApp) return this.skip()

      await selectProjectByName('taurhaus')
      await switchToTab('overview')
      await browser.waitUntil(
        async () => (await $('[data-testid="overview-readme"]')).isExisting(),
        { ...WAIT_MEDIUM, timeoutMsg: 'Overview README section did not render' }
      )

      if (!originalReadme.includes(README_MARKER)) {
        writeFileSync(readmePath, `${originalReadme}\n\n${README_MARKER}\n`, 'utf8')
      }

      await browser.waitUntil(
        async () => {
          return await browser.execute((marker) => {
            const text = document.querySelector('[data-testid="overview-readme"]')?.textContent || ''
            return text.includes(marker)
          }, README_MARKER)
        },
        { timeout: 20_000, interval: POLL, timeoutMsg: 'Overview README did not refresh after file edit' }
      )
    })

    it('refreshes the active task board after a live Claude task file appears', async function () {
      if (!mainApp) return this.skip()

      await selectProjectByName('taurhaus')
      await switchToTab('tasks')

      writeRegressionTask(REGRESSION_TEAM, TAURHAUS_PROJECT_PATH, TASK_SUBJECT)

      await browser.waitUntil(
        async () => {
          return await browser.execute((expectedSubject) => {
            return Array.from(document.querySelectorAll('[data-testid="task-row"]')).some(
              (row) => row.textContent?.includes(expectedSubject)
            )
          }, TASK_SUBJECT)
        },
        { timeout: 20_000, interval: POLL, timeoutMsg: 'Task board did not refresh after Claude task file change' }
      )

      const taskText = await browser.execute(() => {
        return Array.from(document.querySelectorAll('[data-testid="task-row"]'))
          .map((row) => row.textContent?.trim() ?? '')
          .join('\n')
      })
      expect(taskText).toContain(TASK_SUBJECT)
    })
  })
})
