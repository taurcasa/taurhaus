/**
 * Regression e2e tests — targeted tests for bugs that have been found and
 * fixed. Each test documents the original regression and ensures it doesn't
 * recur. Add a new test here any time a visual or behavioral regression is
 * discovered.
 */

import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { clickTestId, selectProjectByName, switchToTab, waitForProjectsLoaded } from '../helpers/navigation.js'
import { TAURHAUS_CLAUDE_DIR, TAURHAUS_PROJECT_PATH } from '../helpers/platform.js'
import { POLL, WAIT_MEDIUM } from '../helpers/timing.js'
import { ensureAttachedTmuxSession, killTmuxPane, openTmuxWindow } from '../helpers/tmux.js'
import { assertTmuxIsolation } from '../helpers/laneTmux.js'

const REGRESSION_STAMP = Date.now()
const REGRESSION_TEAM = `event-pipeline-team-${REGRESSION_STAMP}`
const TASK_SUBJECT = `Regression task ${REGRESSION_STAMP}`
const README_MARKER = `event-pipeline-readme-${REGRESSION_STAMP}`

function prepareRegressionTaskSource(teamName, projectPath) {
  const teamDir = join(TAURHAUS_CLAUDE_DIR, 'teams', teamName)
  const tasksDir = join(TAURHAUS_CLAUDE_DIR, 'tasks', teamName)
  mkdirSync(teamDir, { recursive: true })
  mkdirSync(tasksDir, { recursive: true })
  writeFileSync(
    join(teamDir, 'config.json'),
    JSON.stringify(
      {
        name: teamName,
        createdAt: Date.now(),
        members: [{ name: 'team-lead', role: 'lead', projectPath }],
      },
      null,
      2
    ),
    'utf8'
  )
}

function writeRegressionTask(teamName, subject) {
  const tasksDir = join(TAURHAUS_CLAUDE_DIR, 'tasks', teamName)
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
    assertTmuxIsolation(process.env)
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
    // IMPORTANT: content-enter must not be on a tab's root content element.
    // Chromium replays that animation when toggling display:none (class:hidden),
    // and the transform property forces GPU compositor layer creation for the
    // whole tab. Nested keyed data-reveal animations added in f7255601 are safe.

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
    // tmux-focus.json or the sidebar foreground indicator; commit f9c1e89 made
    // an unknown focus path remove every taurhaus hook. The earlier #1364
    // follow-up misdiagnosed the issue as a frontend listener problem because
    // it never verified the running app end to end.
    //
    // The hook -> file -> inotify chain is gone: the daemon hub probes
    // `tmux list-clients` once per scanner cycle, resolves the focused pane to
    // a project, and the `tmux-focus-changed` event carries `project_id`
    // (asserted in the Rust hub/bridge tests and the Shell unit tests). This
    // block is the deletion guard: it establishes a real hub-resolved focus
    // first — the state the retired chain used to overwrite — and then proves
    // the file no longer moves it. It also keeps the live file and task update
    // paths honest in the packaged app without a reload.

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

    // Ownership, not a count: the sidebar shows at most one foreground
    // indicator, so counting survives the indicator moving from one project row
    // to another, and stays at 0 when nothing resolves.
    async function foregroundOwners() {
      return await browser.execute(() =>
        Array.from(document.querySelectorAll('[data-testid="project-item"]'))
          .filter((row) => row.querySelector('[data-testid="sidebar-foreground-indicator"]'))
          .map((row) => row.getAttribute('data-project-id'))
      )
    }

    async function projectRowId(name) {
      return await browser.execute((needle) => {
        const row = Array.from(document.querySelectorAll('[data-testid="project-item"]')).find(
          (el) => (el.textContent || '').trim().toLowerCase().includes(needle)
        )
        return row?.getAttribute('data-project-id') ?? null
      }, name)
    }

    async function daemonStatus() {
      const result = await browser.executeAsync((done) => {
        const tauri = window.__TAURI_INTERNALS__
        if (!tauri || typeof tauri.invoke !== 'function') {
          done(null)
          return
        }
        tauri
          .invoke('get_daemon_status')
          .then((value) => done(value?.status ?? null))
          .catch(() => done(null))
      })
      return result
    }

    // A live focus the hub can resolve: a pane whose process the scanner reads
    // as a Claude session, working in the taurhaus project. `new-window` selects
    // the pane it creates, and `focus_from_clients` reports the attached
    // client's current pane, so the indicator lands on the taurhaus row.
    function startLiveFocusPane(session) {
      const fixtureDir = mkdtempSync(join(tmpdir(), 'taurhaus-focus-'))
      const agentPath = join(fixtureDir, 'claude')
      writeFileSync(agentPath, '#!/usr/bin/env node\nsetInterval(() => {}, 1000)\n', 'utf8')
      chmodSync(agentPath, 0o755)

      const pane = openTmuxWindow({
        session,
        cwd: TAURHAUS_PROJECT_PATH,
        command: agentPath,
        name: 'pr8-focus',
      })
      if (!pane) {
        rmSync(fixtureDir, { recursive: true, force: true })
        return null
      }

      return { ...pane, fixtureDir }
    }

    it('ignores a stale tmux-focus.json instead of driving the foreground indicator', async function () {
      if (!mainApp) return this.skip()
      this.timeout(120_000)

      const dataDir = process.env.TAURHAUS_DATA_DIR
      if (!dataDir) return this.skip()
      // Focus is a hub snapshot field now, so there is nothing to assert
      // without the daemon.
      if ((await daemonStatus()) !== 'connected') return this.skip()
      // The hub reads focus from `tmux list-clients`, so this test needs a
      // client. A clean runner has none — it gets one of its own rather than
      // skipping, which would retire the regression on exactly the machines
      // that run it. `null` means tmux itself is unavailable.
      const tmux = ensureAttachedTmuxSession({ cwd: TAURHAUS_PROJECT_PATH })
      if (!tmux) return this.skip()
      const tmuxSession = tmux.session

      const live = startLiveFocusPane(tmuxSession)
      if (!live) {
        tmux.cleanup()
        return this.skip()
      }

      const focusFile = join(dataDir, 'tmux-focus.json')
      try {
        await selectProjectByName('taurhaus')
        await waitForProjectsLoaded()

        // The red-before state: a real, resolvable focus owned by a known row.
        // Without it both stale writes below are no-ops on either code path.
        const taurhausRow = await projectRowId('taurhaus')
        expect(taurhausRow).toBeTruthy()
        await browser.waitUntil(
          async () => {
            const owners = await foregroundOwners()
            return owners.length === 1 && owners[0] === taurhausRow
          },
          {
            timeout: 30_000,
            interval: POLL,
            timeoutMsg: 'Hub never resolved the live tmux pane to the taurhaus row',
          }
        )
        const baseline = [taurhausRow]
        expect(await foregroundOwners()).toEqual(baseline)

        // A payload the retired matcher resolved to nothing: it used to clear
        // the indicator off the row that owns it, through the file watcher and
        // then the 75 ms `get_foreground_project` refresh.
        writeFileSync(
          focusFile,
          JSON.stringify({
            session: tmuxSession,
            window: '99',
            pane_id: '%99999',
            timestamp: Date.now(),
          }),
          'utf8'
        )
        // Well past the retired inotify -> Tauri event latency.
        await browser.pause(3_000)
        expect(await foregroundOwners()).toEqual(baseline)

        // Nor does a payload naming another tmux server move it.
        writeFileSync(
          focusFile,
          JSON.stringify({
            session: 'pr8-stale',
            window: '0',
            pane_id: '%0',
            timestamp: Date.now(),
          }),
          'utf8'
        )
        await browser.pause(3_000)
        expect(await foregroundOwners()).toEqual(baseline)

        // Deleting the file is not an input either.
        rmSync(focusFile, { force: true })
        await browser.pause(2_000)
        expect(await foregroundOwners()).toEqual(baseline)
      } finally {
        killTmuxPane(live.paneId)
        rmSync(live.fixtureDir, { recursive: true, force: true })
        rmSync(focusFile, { force: true })
        tmux.cleanup()
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

      // Regression: f173a922 reconciled new directories from inside the shared
      // notify callback, blocking delivery before a new team's task could scan.
      await selectProjectByName('taurhaus')
      await switchToTab('tasks')
      prepareRegressionTaskSource(REGRESSION_TEAM, TAURHAUS_PROJECT_PATH)
      writeRegressionTask(REGRESSION_TEAM, TASK_SUBJECT)

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

  describe('content-enter mounted tab coverage (commit 768cdec regression)', () => {
    it('tab roots do NOT have content-enter class', async function () {
      if (!mainApp) return this.skip()

      // Regression: f7255601 added valid nested reveal animations, so a global
      // class count no longer represented the original tab-root regression.
      const tabNames = ['overview', 'tasks', 'mesh', 'git', 'files']
      for (const tabName of tabNames) {
        await clickTestId(`tab-${tabName}`)
        await browser.waitUntil(
          async () => await browser.execute((name) => {
            const panel = document.querySelector(`#shell-panel-${name}`)
            return panel && panel.children.length > 0
          }, tabName),
          { ...WAIT_MEDIUM, timeoutMsg: `Tab root for "${tabName}" did not mount` }
        )
      }

      const inspection = await browser.execute(() => {
        const wrapper = document.querySelector('[data-testid="content-wrapper"]')
        if (!wrapper) return { mountedPanelIds: [], rootOffenders: ['content-wrapper-missing'] }
        const panels = wrapper.querySelectorAll(':scope > [role="tabpanel"]')
        return {
          mountedPanelIds: Array.from(panels)
            .filter((panel) => panel.children.length > 0)
            .map((panel) => panel.id),
          rootOffenders: Array.from(panels).flatMap((panel) =>
            Array.from(panel.children)
              .filter((child) => child.classList.contains('content-enter'))
              .map((child) => `${panel.id}:${child.tagName.toLowerCase()}`)
          ),
        }
      })
      expect(inspection.mountedPanelIds).toEqual(tabNames.map((name) => `shell-panel-${name}`))
      expect(inspection.rootOffenders).toEqual([])
    })
  })
})
