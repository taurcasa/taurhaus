/**
 * Project Lifecycle — manage projects modal, sidebar filtering, project switching.
 * Replaces the old sidebar.js spec with workflow-oriented tests.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import {
  waitForProjectsLoaded,
  getCurrentProjectName,
  clickTestId,
} from '../helpers/navigation.js'
import { openManageProjects, closeModal, tryAddProjectPath } from '../helpers/modal.js'
import { PAUSE_CLICK_SETTLE, WAIT_SHORT, WAIT_MEDIUM, WAIT_LONG } from '../helpers/timing.js'
import { NONEXISTENT_PATH, NON_GIT_DIR, TAURHAUS_PROJECT_PATH } from '../helpers/platform.js'

let mainApp = false

describe('Project Lifecycle', () => {
  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (mainApp) await waitForProjectsLoaded()
  })

  // ─── Manage Projects Modal ────────────────────────────────────────────────

  describe('manage projects modal', () => {
    afterEach(async () => {
      // Close modal if it was left open by a failed test
      const modal = await $('[data-testid="manage-projects-modal"]')
      if (await modal.isExisting()) await closeModal()
    })

    it('opens via manage-projects-btn', async function () {
      if (!mainApp) return this.skip()

      await openManageProjects()
      const modal = await $('[data-testid="manage-projects-modal"]')
      expect(await modal.isExisting()).toBe(true)
    })

    it('modal shows registered projects list', async function () {
      if (!mainApp) return this.skip()

      await openManageProjects()

      // Wait for modal's project loading to finish (registered-list or no-projects appears)
      await browser.waitUntil(
        async () => {
          const list = await $('[data-testid="registered-list"]')
          const noProjects = await $('[data-testid="no-projects"]')
          return (await list.isExisting()) || (await noProjects.isExisting())
        },
        { ...WAIT_LONG, timeoutMsg: 'Registered projects did not finish loading' }
      )

      const noProjects = await $('[data-testid="no-projects"]')
      if (await noProjects.isExisting()) return this.skip()

      const list = await $('[data-testid="registered-list"]')
      const items = await list.$$('[data-testid^="remove-"]')
      expect(items.length).toBeGreaterThan(0)
    })

    it('manual path entry: invalid path shows validation error', async function () {
      if (!mainApp) return this.skip()

      await openManageProjects()
      await tryAddProjectPath(NONEXISTENT_PATH)

      await browser.waitUntil(
        async () => {
          const msg = await $('[data-testid="validation-message"], [data-testid="manual-error"]')
          return await msg.isExisting()
        },
        { ...WAIT_SHORT, timeoutMsg: 'Validation error did not appear for invalid path' }
      )
    })

    it('manual path entry: non-git directory shows "Not a git repository" message', async function () {
      if (!mainApp) return this.skip()

      await openManageProjects()
      await tryAddProjectPath(NON_GIT_DIR)

      await browser.waitUntil(
        async () => {
          const msg = await $('[data-testid="manual-error"], [data-testid="validation-message"]')
          if (!(await msg.isExisting())) return false
          const text = await browser.execute((el) => el.textContent, msg)
          return text.toLowerCase().includes('git')
        },
        { ...WAIT_SHORT, timeoutMsg: '"Not a git repository" message did not appear' }
      )
    })

    it('manual path entry: already-registered path shows "Already registered" message', async function () {
      if (!mainApp) return this.skip()


      // taurhaus itself is guaranteed to be registered (we ran the wizard with it)
      await openManageProjects()
      await tryAddProjectPath(TAURHAUS_PROJECT_PATH)

      await browser.waitUntil(
        async () => {
          const msg = await $('[data-testid="manual-error"], [data-testid="validation-message"]')
          if (!(await msg.isExisting())) return false
          const text = await browser.execute((el) => el.textContent, msg)
          return text.toLowerCase().includes('already') || text.toLowerCase().includes('registered')
        },
        { ...WAIT_SHORT, timeoutMsg: '"Already registered" message did not appear' }
      )
    })

    it('closing modal returns to main view', async function () {
      if (!mainApp) return this.skip()

      await openManageProjects()
      await closeModal()

      const modal = await $('[data-testid="manage-projects-modal"]')
      expect(await modal.isExisting()).toBe(false)

      // Main content is visible again
      const overviewTab = await $('[data-testid="tab-overview"]')
      expect(await overviewTab.isExisting()).toBe(true)
    })
  })

  // ─── Sidebar Filtering ────────────────────────────────────────────────────

  describe('sidebar filtering', () => {
    before(async () => {
      // Ensure filter is clear before this suite
      const clearBtn = await $('[data-testid="sidebar-filter-clear"]')
      if (await clearBtn.isExisting()) await clickTestId('sidebar-filter-clear')
    })

    after(async () => {
      // Leave filter cleared after suite
      const clearBtn = await $('[data-testid="sidebar-filter-clear"]')
      if (await clearBtn.isExisting()) await clickTestId('sidebar-filter-clear')
    })

    it('typing in filter narrows project count', async function () {
      if (!mainApp) return this.skip()

      const projectsBefore = await $$('[data-testid="project-item"]')
      const countBefore = projectsBefore.length

      const filter = await $('[data-testid="sidebar-filter"]')
      await filter.waitForExist({ timeout: 5_000 })
      // Use "taurhaus" — we know at least this project exists
      await filter.setValue('taurhaus')

      await browser.waitUntil(
        async () => {
          const projects = await $$('[data-testid="project-item"]')
          return projects.length < countBefore || projects.length === 1
        },
        { ...WAIT_SHORT, timeoutMsg: 'Filter did not narrow project count' }
      )

      const projectsAfter = await $$('[data-testid="project-item"]')
      expect(projectsAfter.length).toBeLessThanOrEqual(countBefore)
    })

    it('clearing filter restores all projects', async function () {
      if (!mainApp) return this.skip()

      const filter = await $('[data-testid="sidebar-filter"]')
      await filter.waitForExist({ timeout: 5_000 })
      await filter.setValue('taurhaus')

      const clearBtn = await $('[data-testid="sidebar-filter-clear"]')
      await clearBtn.waitForExist({ timeout: 5_000 })
      await clickTestId('sidebar-filter-clear')

      await browser.waitUntil(
        async () => {
          const projects = await $$('[data-testid="project-item"]')
          return projects.length > 0
        },
        { ...WAIT_SHORT, timeoutMsg: 'Projects did not return after clearing filter' }
      )

      const filterValue = await filter.getValue()
      expect(filterValue).toBe('')
    })

    it('gibberish filter shows sidebar-no-matches state', async function () {
      if (!mainApp) return this.skip()

      const filter = await $('[data-testid="sidebar-filter"]')
      await filter.waitForExist({ timeout: 5_000 })
      await filter.setValue('zzz_no_match_gibberish_xyz_9999')

      await browser.waitUntil(
        async () => {
          const noMatches = await $('[data-testid="sidebar-no-matches"]')
          return await noMatches.isExisting()
        },
        { ...WAIT_SHORT, timeoutMsg: 'sidebar-no-matches did not appear for gibberish query' }
      )

      // Clear so subsequent tests are not affected
      const clearBtn = await $('[data-testid="sidebar-filter-clear"]')
      if (await clearBtn.isExisting()) await clickTestId('sidebar-filter-clear')
    })
  })

  // ─── Project Switching ────────────────────────────────────────────────────

  describe('project switching', () => {
    it('clicking a different project updates the overview h1', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length < 2) return this.skip()

      const firstProject = projects[0]
      await browser.execute((el) => el.click(), firstProject)
      const firstProjectName = await getCurrentProjectName()

      const secondProject = projects[1]
      await browser.execute((el) => el.click(), secondProject)

      await browser.waitUntil(
        async () => {
          const name = await getCurrentProjectName()
          return name !== firstProjectName
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Project name in h1 did not change after switching' }
      )

      const newName = await getCurrentProjectName()
      expect(newName).not.toBe(firstProjectName)
    })

    it('can switch back to the first project', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      if (projects.length < 2) return this.skip()

      // Go to second
      await browser.execute((el) => el.click(), projects[1])
      await browser.pause(PAUSE_CLICK_SETTLE)

      // Go back to first
      await browser.execute((el) => el.click(), projects[0])

      const firstName = await browser.execute(
        (el) => el.textContent,
        projects[0]
      )

      await browser.waitUntil(
        async () => {
          const name = await getCurrentProjectName()
          return firstName.includes(name) || name.includes(firstName.trim().split('\n')[0].trim())
        },
        { ...WAIT_SHORT, timeoutMsg: 'Did not switch back to first project' }
      )
    })
  })

  // ─── Sidebar State ────────────────────────────────────────────────────────

  describe('sidebar state', () => {
    it('activity group headers are visible (ACTIVE/RECENT/STALE/DORMANT)', async function () {
      if (!mainApp) return this.skip()

      // At least one group header should exist — we look for any of the four group labels
      const sidebarText = await browser.execute(() => document.querySelector('aside')?.textContent || '')
      const hasGroupHeader =
        sidebarText.includes('ACTIVE') ||
        sidebarText.includes('RECENT') ||
        sidebarText.includes('STALE') ||
        sidebarText.includes('DORMANT')

      expect(hasGroupHeader).toBe(true)
    })

    it('sidebar skeleton is gone and projects have loaded', async function () {
      if (!mainApp) return this.skip()

      const skeleton = await $('[data-testid="sidebar-skeleton"]')
      expect(await skeleton.isExisting()).toBe(false)

      const projects = await $$('[data-testid="project-item"]')
      expect(projects.length).toBeGreaterThan(0)
    })

    it('at least one project-item element is present', async function () {
      if (!mainApp) return this.skip()

      const projects = await $$('[data-testid="project-item"]')
      expect(projects.length).toBeGreaterThan(0)
    })
  })
})
