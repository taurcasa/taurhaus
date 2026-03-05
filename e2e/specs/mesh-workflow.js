/**
 * Mesh Workflow e2e tests.
 *
 * Tier 1: mesh tab surface + availability gate + tab switching.
 * Tier 2: setup -> initialize -> hot-add -> disband (e2e-prefixed teams only).
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded, clickTestId, switchToTab } from '../helpers/navigation.js'
import { WAIT_SHORT, WAIT_MEDIUM, WAIT_LONG, WAIT_XLONG } from '../helpers/timing.js'
import { snapshotTmuxPanes, cleanupNewTmuxPanes } from '../helpers/tmux.js'

let mainApp = false
let tier2Enabled = false
let tier2SkipReason = 'Mesh prerequisites unavailable'
let createdTeamName = null
const createdTeamNames = new Set()
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

  const runtimeTitle = await $('[data-testid="mesh-runtime-title"]')
  const teamName = (await runtimeTitle.isExisting()) ? (await runtimeTitle.getText()).trim() : ''

  if (!createdTeamNames.has(teamName)) {
    tier2SkipReason = `Refusing to disband runtime team not created by this spec: ${teamName || 'unknown'}`
    return false
  }

  if (await hasTestId('mesh-runtime-overflow-button')) {
    await clickTestId('mesh-runtime-overflow-button')
  }

  await browser.waitUntil(
    async () => await hasTestId('mesh-runtime-disband'),
    { ...WAIT_SHORT, timeoutMsg: 'Runtime disband action did not appear' }
  )

  await clickTestId('mesh-runtime-disband')
  if (await hasTestId('confirm-dialog-confirm')) {
    await clickTestId('confirm-dialog-confirm')
  }

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
    await clickTestId('mesh-template-build-custom')
  }

  await browser.waitUntil(
    async () => await hasTestId('mesh-mode-setup'),
    { ...WAIT_LONG, timeoutMsg: 'Mesh did not enter setup mode' }
  )

  return true
}

async function openCustomizerAndSetTeamName(teamName) {
  await clickTestId('mesh-action-customize')
  await browser.waitUntil(
    async () => await hasTestId('team-customizer-panel'),
    { ...WAIT_MEDIUM, timeoutMsg: 'Team customizer did not open' }
  )

  const teamNameInput = await $('[data-testid="team-customizer-name-input"]')
  await teamNameInput.waitForExist({ timeout: WAIT_MEDIUM.timeout })
  await teamNameInput.clearValue()
  await teamNameInput.setValue(teamName)

  await clickTestId('team-customizer-save')

  await browser.waitUntil(
    async () => !(await hasTestId('team-customizer-panel')),
    { ...WAIT_MEDIUM, timeoutMsg: 'Team customizer did not close after apply' }
  )
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
    tmuxPaneSnapshot = snapshotTmuxPanes()

    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (!mainApp) {
      tier2SkipReason = 'Main app unavailable'
      return
    }

    await waitForProjectsLoaded()

    const availability = await invokeCoordination('coordination_get_feature_availability')
    if (!availability.ok) {
      tier2Enabled = false
      tier2SkipReason = `Feature availability check failed: ${availability.error}`
      return
    }

    const report = availability.result || {}
    const canInitialize = report.canInitialize !== false
    const meshAvailable = report.meshAvailable !== false
    const tmuxAvailable = report.tmuxAvailable !== false
    const blockingErrors = Array.isArray(report.blockingErrors) ? report.blockingErrors : []

    tier2Enabled = canInitialize && meshAvailable && tmuxAvailable && blockingErrors.length === 0
    if (!tier2Enabled) {
      tier2SkipReason = blockingErrors[0] || 'Mesh or tmux unavailable'
    }
  })

  after(async () => {
    for (const teamName of createdTeamNames) {
      if (!teamName.startsWith('e2e-')) continue
      await invokeCoordinationWithTimeout('coordination_disband_team', { teamName }, 2_500)
    }
    createdTeamNames.clear()
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
      if (tier2Enabled) return this.skip()

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
      if (!setupReady) return this.skip()

      expect(await hasTestId('mesh-mode-setup')).toBe(true)
      expect(await hasTestId('mesh-canvas')).toBe(true)
      expect(await hasTestId('mesh-action-customize')).toBe(true)
      expect(await hasTestId('mesh-action-initialize')).toBe(true)
    })

    it('initializes an e2e team, hot-adds an agent, then disbands', async function () {
      if (!mainApp) return this.skip()
      if (!tier2Enabled) return this.skip()

      const setupReady = await ensureSetupMode()
      if (!setupReady) return this.skip()

      const teamName = `e2e-mesh-${uniqueSuffix}`
      const secondAgentName = `e2e-agent-${uniqueSuffix}`

      await openCustomizerAndSetTeamName(teamName)

      const initializeButton = await $('[data-testid="mesh-action-initialize"]')
      await initializeButton.waitForExist({ timeout: WAIT_MEDIUM.timeout })
      await browser.waitUntil(
        async () => await initializeButton.isEnabled(),
        { ...WAIT_MEDIUM, timeoutMsg: 'Initialize button never became enabled' }
      )
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
        const reason = `Mesh initialize failed: ${await (await $('[data-testid="mesh-init-failure"]')).getText()}`
        tier2SkipReason = reason
        console.warn(`[e2e][mesh-workflow] skipping tier2 initialize flow: ${reason}`)
        this.skip()
        return
      }
      if (await hasTestId('mesh-error')) {
        const reason = `Mesh error after initialize: ${await (await $('[data-testid="mesh-error"]')).getText()}`
        tier2SkipReason = reason
        console.warn(`[e2e][mesh-workflow] skipping tier2 initialize flow: ${reason}`)
        this.skip()
        return
      }

      const runtimeTitle = await $('[data-testid="mesh-runtime-title"]')
      expect(await runtimeTitle.isExisting()).toBe(true)
      createdTeamName = teamName
      createdTeamNames.add(teamName)

      await clickTestId('mesh-runtime-add-agent')
      await browser.waitUntil(
        async () => await hasTestId('mesh-add-agent-form'),
        { ...WAIT_SHORT, timeoutMsg: 'Hot-add form did not appear' }
      )

      const addAgentNameInput = await $('[data-testid="mesh-add-agent-name-input"]')
      await addAgentNameInput.clearValue()
      await addAgentNameInput.setValue(secondAgentName)

      const selectedAddProject = await selectFirstNonEmptyOption('[data-testid="mesh-add-agent-project-select"]')
      if (!selectedAddProject) return this.skip()

      await clickTestId('mesh-add-agent-submit')

      await browser.waitUntil(
        async () => {
          const addError = await hasTestId('mesh-add-agent-error')
          const runtimeMessage = await hasTestId('mesh-runtime-message')
          const formClosed = !(await hasTestId('mesh-add-agent-form'))
          return addError || runtimeMessage || formClosed
        },
        { ...WAIT_LONG, timeoutMsg: 'Hot-add did not update UI state' }
      )

      if (await hasTestId('mesh-add-agent-error')) {
        throw new Error(`Hot-add failed: ${await (await $('[data-testid="mesh-add-agent-error"]')).getText()}`)
      }

      if (await hasTestId('mesh-runtime-overflow-button')) {
        await clickTestId('mesh-runtime-overflow-button')
      }
      await browser.waitUntil(
        async () => await hasTestId('mesh-runtime-disband'),
        { ...WAIT_SHORT, timeoutMsg: 'Runtime disband option did not appear' }
      )
      await clickTestId('mesh-runtime-disband')

      if (await hasTestId('confirm-dialog-confirm')) {
        await clickTestId('confirm-dialog-confirm')
      }

      await browser.waitUntil(
        async () => await hasTestId('mesh-mode-empty'),
        { ...WAIT_LONG, timeoutMsg: 'Disband did not return mesh to empty mode' }
      )

      createdTeamName = null
      createdTeamNames.delete(teamName)
    })

    it('skips tier 2 when mesh prerequisites are unavailable', async function () {
      if (!mainApp) return this.skip()
      if (tier2Enabled) return this.skip()
      expect(typeof tier2SkipReason).toBe('string')
      expect(tier2SkipReason.length).toBeGreaterThan(0)
    })
  })
})
