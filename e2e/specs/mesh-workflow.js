/**
 * Mesh Workflow e2e tests.
 *
 * Tier 1: mesh tab surface + availability gate + tab switching.
 * Tier 2: setup -> initialize -> hot-add -> disband (e2e-prefixed teams only).
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { clickUntil } from '../helpers/clickUntil.js'
import { clickRuntimeAddAgent } from '../helpers/meshRuntime.js'
import { isConfirmDialogOpen, clickOpenConfirmDialog } from '../helpers/confirmDialog.js'
import { waitForProjectsLoaded, clickTestId, switchToTab } from '../helpers/navigation.js'
import { setInlineBuilderTeamName } from '../helpers/meshBuilder.js'
import { clickActiveSlideOverTestId } from '../helpers/slideover.js'
import { WAIT_SHORT, WAIT_MEDIUM, WAIT_LONG, WAIT_XLONG } from '../helpers/timing.js'
import { snapshotTmuxPanes, cleanupNewTmuxPanes } from '../helpers/tmux.js'
import { assertTmuxIsolation } from '../helpers/laneTmux.js'
import { assertWorkerMeshAvailable } from '../helpers/workerEnv.js'
import { registerCreatedTeam, forgetCreatedTeam, isOwnedTeam, ownedTeams, clearOwnedTeams } from '../helpers/teamRegistry.js'

let mainApp = false
let tier2Enabled = false
let tier2SkipReason = 'Mesh prerequisites unavailable'
let createdTeamName = null
let tmuxPaneSnapshot = { available: false, paneIds: [], reason: 'snapshot not captured' }
const uniqueSuffix = `${Date.now()}-${Math.floor(Math.random() * 10_000)}`

async function invokeCoordination(command, args = {}) {
  return await browser.executeAsync((payload, done) => {
    const tauri = window.__TAURI_INTERNALS__
    if (!tauri || typeof tauri.invoke !== 'function') {
      done({ ok: false, error: 'Tauri internals unavailable' })
      return
    }

    tauri
      .invoke(payload.command, payload.args)
      .then((result) => done({ ok: true, result }))
      .catch((error) => done({ ok: false, error: error?.message ?? String(error) }))
  }, { command, args })
}

async function invokeCoordinationWithTimeout(command, args = {}, timeoutMs = 2_500) {
  return await Promise.race([
    invokeCoordination(command, args),
    new Promise((resolve) => {
      setTimeout(() => {
        resolve({ ok: false, error: `Timed out after ${timeoutMs}ms` })
      }, timeoutMs)
    }),
  ])
}

async function hasTestId(testId) {
  return await (await $(`[data-testid="${testId}"]`)).isExisting()
}

async function waitForMeshSurface() {
  await browser.waitUntil(
    async () => {
      if (!(await hasTestId('mesh-tab'))) return false
      return (
        (await hasTestId('mesh-mode-gate')) ||
        (await hasTestId('mesh-mode-empty')) ||
        (await hasTestId('mesh-mode-setup')) ||
        (await hasTestId('mesh-mode-initializing')) ||
        (await hasTestId('mesh-mode-runtime')) ||
        (await hasTestId('mesh-availability-blocking')) ||
        (await hasTestId('mesh-error'))
      )
    },
    { ...WAIT_MEDIUM, timeoutMsg: 'Mesh tab surface did not render' }
  )
}

async function openMeshTab() {
  await clickTestId('tab-mesh')
  await waitForMeshSurface()

  if (await hasTestId('mesh-mode-gate')) {
    await browser.waitUntil(
      async () => !(await hasTestId('mesh-mode-gate')),
      { ...WAIT_LONG, timeoutMsg: 'Mesh gate did not resolve' }
    )
  }
}

async function disbandRuntimeTeamIfSafe() {
  if (!(await hasTestId('mesh-mode-runtime'))) return true

  // Regression: 3b65c760 only trusted teams created inside this spec, so the
  // sealed mesh group could not clean up mesh-recovery's team. Ownership now
  // lives in the shared group registry instead of a naming-convention match.
  const isSafeRuntimeTeam = (teamName) => isOwnedTeam(teamName)
  const safeTitleResolved = await browser.waitUntil(
    async () => {
      const teamName = await browser.execute(() => {
        return document.querySelector('[data-testid="mesh-runtime-title"]')?.textContent?.trim() ?? ''
      })
      return isSafeRuntimeTeam(teamName)
    },
    { ...WAIT_MEDIUM, timeoutMsg: 'Runtime team title did not resolve to a safe e2e team' }
  ).then(() => true).catch(() => false)
  const teamName = await browser.execute(() => {
    return document.querySelector('[data-testid="mesh-runtime-title"]')?.textContent?.trim() ?? ''
  })

  if (!safeTitleResolved || !isSafeRuntimeTeam(teamName)) {
    tier2SkipReason = `Refusing to disband runtime team outside the sealed e2e group: ${teamName || 'unknown'}`
    return false
  }

  await clickUntil('mesh-runtime-more-toggle', 'mesh-runtime-disband',
    { ...WAIT_SHORT, timeoutMsg: 'Disband action did not appear' }
  )
  await clickUntil('mesh-runtime-disband',
    isConfirmDialogOpen,
    { ...WAIT_SHORT, timeoutMsg: 'Disband confirmation dialog did not appear' }
  )
  await clickOpenConfirmDialog()

  await browser.waitUntil(
    async () => (await hasTestId('mesh-mode-empty')) || (await hasTestId('mesh-mode-setup')),
    { ...WAIT_LONG, timeoutMsg: 'Mesh did not leave runtime mode after disband' }
  )

  return true
}

async function ensureSetupMode() {
  await openMeshTab()

  if (await hasTestId('mesh-availability-blocking')) return false

  if (await hasTestId('mesh-mode-runtime')) {
    const disbanded = await disbandRuntimeTeamIfSafe()
    if (!disbanded) return false
  }

  if (await hasTestId('mesh-mode-setup')) return true

  if (await hasTestId('mesh-mode-empty')) {
    // Regression: 17e0f9d1 made setup contingent on the inline input event;
    // clicking the display alone leaves the builder in empty mode.
    await setInlineBuilderTeamName()
  }

  await browser.waitUntil(
    async () => await hasTestId('mesh-mode-setup'),
    { ...WAIT_LONG, timeoutMsg: 'Mesh did not enter setup mode' }
  )

  return true
}

async function selectFirstNonEmptyOption(selector) {
  const select = await $(selector)
  if (!(await select.isExisting())) return false

  const value = await browser.execute((sel) => {
    const el = document.querySelector(sel)
    if (!el) return null
    const options = Array.from(el.options || [])
    const found = options.find((opt) => String(opt.value || '').trim().length > 0)
    return found?.value ?? null
  }, selector)

  if (!value) return false
  await select.selectByAttribute('value', value)
  return true
}

describe('Mesh Workflow', () => {
  before(async () => {
    assertTmuxIsolation(process.env)
    tmuxPaneSnapshot = snapshotTmuxPanes()

    // Regression: 960e61ec clears shared ownership after recovery's IPC
    // disband, but this WebView can retain that deleted team's cached runtime.
    // Establish a fresh project snapshot before probing tier-2 eligibility.
    await browser.refresh()
    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (!mainApp) {
      throw new Error('Main app unavailable: Mesh workflow requires the main app')
    }

    await waitForProjectsLoaded()

    const availability = await invokeCoordination('coordination_get_feature_availability')
    // Regression: 5cebfef81 silently disabled both tier-2 cases on a transient
    // probe failure. This sealed worker requires mesh/tmux: establish a hard
    // precondition here so missing readiness fails the run, never its skip set.
    if (!availability.ok) {
      throw new Error(`Mesh prerequisite check failed: ${availability.error}`)
    }

    const report = availability.result || {}
    assertWorkerMeshAvailable(report)
    const canInitialize = report.canInitialize === true
    const meshAvailable = report.meshAvailable === true
    const tmuxAvailable = report.tmuxAvailable === true
    const blockingErrors = Array.isArray(report.blockingErrors) ? report.blockingErrors : []

    tier2Enabled = canInitialize && meshAvailable && tmuxAvailable && blockingErrors.length === 0
    if (!tier2Enabled) {
      throw new Error(`Mesh prerequisites unavailable: ${blockingErrors[0] || 'mesh/tmux readiness not confirmed'}`)
    }
  })

  after(async () => {
    for (const teamName of ownedTeams()) {
      if (!teamName.startsWith('e2e-')) continue
      await invokeCoordinationWithTimeout('coordination_disband_team', { teamName }, 2_500)
    }
    clearOwnedTeams()
    createdTeamName = null

    const tmuxCleanup = cleanupNewTmuxPanes(tmuxPaneSnapshot)
    if (!tmuxCleanup.attempted) {
      console.log(`[e2e] mesh-workflow tmux cleanup skipped: ${tmuxCleanup.skippedReason}`)
    } else if (tmuxCleanup.failed.length > 0) {
      console.warn(`[e2e] mesh-workflow tmux cleanup failures: ${JSON.stringify(tmuxCleanup.failed)}`)
    }
  })

  describe('tier 1', () => {
    it('renders Mesh tab and availability surface', async function () {
      if (!mainApp) return this.skip()

      await openMeshTab()

      expect(await hasTestId('mesh-tab')).toBe(true)
      expect(
        (await hasTestId('mesh-availability-blocking')) ||
        (await hasTestId('mesh-mode-empty')) ||
        (await hasTestId('mesh-mode-setup')) ||
        (await hasTestId('mesh-mode-runtime'))
      ).toBe(true)
    })

    it('shows blocking availability messaging when mesh is unavailable', async function () {
      if (!mainApp) return this.skip()
      if (tier2Enabled) {
        console.log('[e2e][mesh-workflow] skipped unavailable-mesh messaging: worker has mesh and tmux installed')
        return this.skip()
      }

      await openMeshTab()

      const blockingTitle = await $('[data-testid="mesh-availability-title"]')
      const blockingError = await $('[data-testid="mesh-availability-error"]')

      expect(await blockingTitle.isExisting()).toBe(true)
      expect(await blockingError.isExisting()).toBe(true)
    })

    it('supports tab switching mesh -> overview -> mesh', async function () {
      if (!mainApp) return this.skip()

      await openMeshTab()
      await switchToTab('overview')
      await browser.waitUntil(
        async () => await (await $('[data-testid="tab-overview"]')).isExisting(),
        { ...WAIT_SHORT, timeoutMsg: 'Overview tab did not render after switching from Mesh' }
      )

      await openMeshTab()
      expect(await hasTestId('mesh-tab')).toBe(true)
    })
  })

  describe('tier 2', () => {
    it('shows setup controls in setup mode', async function () {
      if (!mainApp) return this.skip()
      if (!tier2Enabled) return this.skip()

      const setupReady = await ensureSetupMode()
      if (!setupReady) throw new Error(`Mesh setup precondition failed: ${tier2SkipReason}`)

      expect(await hasTestId('mesh-mode-setup')).toBe(true)
      // Regression: d35063e4 replaced the setup canvas with the roster builder
      // shell while this workflow kept asserting the runtime-only canvas.
      expect(await hasTestId('mesh-builder-shell')).toBe(true)
      expect(await hasTestId('mesh-builder-team-name-display')).toBe(true)
      expect(await hasTestId('mesh-action-initialize')).toBe(true)
    })

    it('initializes an e2e team, hot-adds an agent, then disbands', async function () {
      if (!mainApp) return this.skip()
      if (!tier2Enabled) return this.skip()

      const setupReady = await ensureSetupMode()
      if (!setupReady) throw new Error(`Mesh setup precondition failed: ${tier2SkipReason}`)

      const teamName = `e2e-mesh-${uniqueSuffix}`
      const secondAgentName = `e2e-agent-${uniqueSuffix}`

      // Regression: 14793e0a moved team-name editing from the setup action bar
      // into the roster builder's inline editor.
      await setInlineBuilderTeamName(teamName)

      // Regression: 14793e0a made the roster catalog authoritative; a lead
      // must be selected explicitly before initialization can be enabled.
      if (!(await hasTestId('mesh-builder-lead-card'))) {
        const firstLead = await $(
          '[data-testid="mesh-builder-role-section-leads"] button[data-testid^="mesh-builder-role-"]'
        )
        await firstLead.waitForExist({ timeout: WAIT_MEDIUM.timeout })
        await firstLead.click()
        await browser.waitUntil(
          async () => await hasTestId('mesh-builder-lead-card'),
          { ...WAIT_MEDIUM, timeoutMsg: 'Selected lead role did not populate the roster' }
        )
      }

      const initializeButton = await $('[data-testid="mesh-action-initialize"]')
      await initializeButton.waitForExist({ timeout: WAIT_MEDIUM.timeout })
      await browser.waitUntil(
        async () => await initializeButton.isEnabled(),
        { ...WAIT_MEDIUM, timeoutMsg: 'Initialize button never became enabled' }
      )
      // Own the attempted fixture too, so partial initialization is cleaned up
      // when a required runtime assertion fails below.
      registerCreatedTeam(teamName)
      await initializeButton.click()

      await browser.waitUntil(
        async () => {
          return (
            (await hasTestId('mesh-mode-runtime')) ||
            (await hasTestId('mesh-init-failure')) ||
            (await hasTestId('mesh-error'))
          )
        },
        { ...WAIT_XLONG, timeoutMsg: 'Mesh initialization did not resolve to runtime or failure' }
      )

      if (await hasTestId('mesh-init-failure')) {
        throw new Error(`Mesh initialize failed: ${await (await $('[data-testid="mesh-init-failure"]')).getText()}`)
      }
      if (await hasTestId('mesh-error')) {
        throw new Error(`Mesh error after initialize: ${await (await $('[data-testid="mesh-error"]')).getText()}`)
      }

      const runtimeTitle = await $('[data-testid="mesh-runtime-title"]')
      expect(await runtimeTitle.isExisting()).toBe(true)
      createdTeamName = teamName

      // Regression: 430e09ee removed the duplicate Add Agent button; runtime
      // additions now begin from the primary action.
      await clickRuntimeAddAgent(
        { ...WAIT_SHORT, timeoutMsg: 'Hot-add form did not appear' }
      )

      // Regression: 372511aa replaced the runtime role select with role cards;
      // hot-add remains disabled until the user chooses one.
      const roleCardTestId = await browser.execute(() => {
        const cards = Array.from(document.querySelectorAll('[data-testid^="mesh-add-agent-role-card-"]'))
        const enabled = cards.find((card) => !card.disabled && card.getAttribute('aria-disabled') !== 'true')
        return enabled?.getAttribute('data-testid') ?? null
      })
      if (!roleCardTestId) throw new Error('No enabled add-agent role card was available')
      await clickActiveSlideOverTestId(roleCardTestId)

      const addAgentNameInput = await $('[data-testid="mesh-add-agent-name-input"]')
      await addAgentNameInput.clearValue()
      await addAgentNameInput.setValue(secondAgentName)

      const selectedAddProject = await selectFirstNonEmptyOption('[data-testid="mesh-add-agent-project-select"]')
      if (!selectedAddProject) throw new Error('No add-agent project option was available')

      await browser.waitUntil(
        async () => await (await $('[data-testid="mesh-add-agent-submit"]')).isEnabled(),
        { ...WAIT_MEDIUM, timeoutMsg: 'Add Agent button never became enabled' }
      )
      await clickTestId('mesh-add-agent-submit')

      // Regression: 372511aa left the initialization success message visible
      // behind the add-agent form, so it cannot signal hot-add completion.
      await browser.waitUntil(
        async () => {
          const addError = await hasTestId('mesh-add-agent-error')
          const formClosed = !(await hasTestId('mesh-add-agent-form'))
          return addError || formClosed
        },
        { ...WAIT_LONG, timeoutMsg: 'Hot-add did not update UI state' }
      )

      if (await hasTestId('mesh-add-agent-error')) {
        throw new Error(`Hot-add failed: ${await (await $('[data-testid="mesh-add-agent-error"]')).getText()}`)
      }

      // Regression: 430e09ee moved disband into the runtime overflow menu;
      // reuse the safety-checked user flow for teardown.
      if (!(await disbandRuntimeTeamIfSafe())) {
        throw new Error(tier2SkipReason)
      }

      if (await hasTestId('mesh-mode-setup') && await hasTestId('mesh-action-reset')) {
        await clickTestId('mesh-action-reset')
      }
      await browser.waitUntil(
        async () => await hasTestId('mesh-mode-empty'),
        { ...WAIT_LONG, timeoutMsg: 'Disband did not return mesh to empty mode' }
      )

      createdTeamName = null
      forgetCreatedTeam(teamName)
    })

    it('skips tier 2 when mesh prerequisites are unavailable', async function () {
      if (!mainApp) return this.skip()
      if (tier2Enabled) {
        console.log('[e2e][mesh-workflow] skipped unavailable-prerequisite case: worker has mesh and tmux installed')
        return this.skip()
      }
      expect(typeof tier2SkipReason).toBe('string')
      expect(tier2SkipReason.length).toBeGreaterThan(0)
    })
  })
})
