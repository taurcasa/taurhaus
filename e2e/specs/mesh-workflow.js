/**
 * Mesh Workflow e2e tests.
 *
 * Tier 1 (always runs): tab rendering, availability gate surface, tab switching.
 * Tier 2 (skips if mesh/tmux unavailable): setup form -> initialize -> hot-add -> disband.
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded, clickTestId, switchToTab } from '../helpers/navigation.js'
import { WAIT_SHORT, WAIT_MEDIUM, WAIT_LONG, WAIT_XLONG } from '../helpers/timing.js'

let mainApp = false
let tier2Enabled = false
let tier2SkipReason = 'Mesh prerequisites unavailable'
let createdTeamName = null
let uniqueSuffix = `${Date.now()}-${Math.floor(Math.random() * 10_000)}`

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

async function waitForMeshSurface() {
  await browser.waitUntil(
    async () => {
      const meshTab = await $('[data-testid="mesh-tab"]')
      const setup = await $('[data-testid="mesh-setup-form"]')
      const runtime = await $('[data-testid="mesh-team-roster"]')
      const blocking = await $('[data-testid="mesh-availability-blocking"]')
      const loading = await $('[data-testid="mesh-loading"]')
      const error = await $('[data-testid="mesh-error"]')

      return (
        (await meshTab.isExisting()) &&
        ((await setup.isExisting()) ||
          (await runtime.isExisting()) ||
          (await blocking.isExisting()) ||
          (await loading.isExisting()) ||
          (await error.isExisting()))
      )
    },
    { ...WAIT_MEDIUM, timeoutMsg: 'Mesh tab surface did not render' }
  )
}

async function openMeshTab() {
  await clickTestId('tab-mesh')
  await waitForMeshSurface()
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

async function ensureSetupMode() {
  const runtimeTitle = await $('[data-testid="mesh-runtime-title"]')
  if (!(await runtimeTitle.isExisting())) return

  // Confirm disband in-browser to avoid modal blocking.
  await browser.execute(() => {
    window.confirm = () => true
  })

  const disbandBtn = await $('[data-testid="mesh-disband-button"]')
  if (await disbandBtn.isExisting()) {
    await disbandBtn.click()
    await browser.waitUntil(
      async () => {
        const setupTitle = await $('[data-testid="mesh-setup-title"]')
        return await setupTitle.isExisting()
      },
      { ...WAIT_LONG, timeoutMsg: 'Mesh did not return to setup mode after disband' }
    )
  }
}

describe('Mesh Workflow', () => {
  before(async () => {
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
    // Best-effort cleanup: disband team created by this spec if still present.
    if (!createdTeamName) return
    await invokeCoordination('coordination_disband_team', { teamName: createdTeamName })
    createdTeamName = null
  })

  describe('tier 1', () => {
    it('renders Mesh tab and availability gate surface', async function () {
      if (!mainApp) return this.skip()

      await openMeshTab()

      const meshTab = await $('[data-testid="mesh-tab"]')
      expect(await meshTab.isExisting()).toBe(true)

      const blocking = await $('[data-testid="mesh-availability-blocking"]')
      const setup = await $('[data-testid="mesh-setup-form"]')
      const runtime = await $('[data-testid="mesh-team-roster"]')

      const hasAvailabilitySurface =
        (await blocking.isExisting()) ||
        (await setup.isExisting()) ||
        (await runtime.isExisting())

      expect(hasAvailabilitySurface).toBe(true)
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
      const meshTab = await $('[data-testid="mesh-tab"]')
      expect(await meshTab.isExisting()).toBe(true)
    })
  })

  describe('tier 2', () => {
    it('shows setup form team creation controls', async function () {
      if (!mainApp) return this.skip()
      if (!tier2Enabled) return this.skip()

      await openMeshTab()
      await ensureSetupMode()

      const setupTitle = await $('[data-testid="mesh-setup-title"]')
      const teamNameInput = await $('[data-testid="mesh-team-name-input"]')
      const createButton = await $('[data-testid="mesh-create-team-button"]')

      expect(await setupTitle.isExisting()).toBe(true)
      expect(await teamNameInput.isExisting()).toBe(true)
      expect(await createButton.isExisting()).toBe(true)
    })

    it('initializes a team, hot-adds an agent, then disbands', async function () {
      if (!mainApp) return this.skip()
      if (!tier2Enabled) return this.skip()

      await openMeshTab()
      await ensureSetupMode()

      const setupTitle = await $('[data-testid="mesh-setup-title"]')
      if (!(await setupTitle.isExisting())) throw new Error('Mesh setup form not available in Tier 2 run')

      const teamName = `mesh-e2e-${uniqueSuffix}`
      const firstAgentName = `mesh-e2e-agent-a-${uniqueSuffix}`
      const secondAgentName = `mesh-e2e-agent-b-${uniqueSuffix}`

      const teamNameInput = await $('[data-testid="mesh-team-name-input"]')
      await teamNameInput.waitForExist({ timeout: WAIT_SHORT.timeout })
      await teamNameInput.clearValue()
      await teamNameInput.setValue(teamName)

      const firstAgentInput = await $('[data-testid="mesh-agent-name-input-0"]')
      await firstAgentInput.waitForExist({ timeout: WAIT_SHORT.timeout })
      await firstAgentInput.clearValue()
      await firstAgentInput.setValue(firstAgentName)

      const selectedSetupProject = await selectFirstNonEmptyOption('[data-testid="mesh-agent-project-select-0"]')
      if (!selectedSetupProject) return this.skip()

      const createButton = await $('[data-testid="mesh-create-team-button"]')
      await browser.waitUntil(
        async () => !(await createButton.getProperty('disabled')),
        { ...WAIT_MEDIUM, timeoutMsg: 'Initialize button never became enabled' }
      )
      await createButton.click()

      await browser.waitUntil(
        async () => {
          const progress = await $('[data-testid="mesh-init-progress"]')
          const runtime = await $('[data-testid="mesh-runtime-title"]')
          const failed = await $('[data-testid="mesh-init-failure"]')
          return (await progress.isExisting()) || (await runtime.isExisting()) || (await failed.isExisting())
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Mesh initialization UI did not transition' }
      )

      await browser.waitUntil(
        async () => {
          const runtime = await $('[data-testid="mesh-runtime-title"]')
          const failed = await $('[data-testid="mesh-init-failure"]')
          return (await runtime.isExisting()) || (await failed.isExisting())
        },
        { ...WAIT_XLONG, timeoutMsg: 'Mesh initialization did not resolve to runtime or failure' }
      )

      const failed = await $('[data-testid="mesh-init-failure"]')
      if (await failed.isExisting()) {
        const text = await failed.getText()
        throw new Error(`Mesh initialize failed in Tier 2 path: ${text}`)
      }

      const runtimeTitle = await $('[data-testid="mesh-runtime-title"]')
      expect(await runtimeTitle.isExisting()).toBe(true)
      expect(await runtimeTitle.getText()).toContain(teamName)
      createdTeamName = teamName

      await clickTestId('mesh-add-agent-button')
      await browser.waitUntil(
        async () => await (await $('[data-testid="mesh-add-agent-form"]')).isExisting(),
        { ...WAIT_SHORT, timeoutMsg: 'Hot-add form did not appear' }
      )

      const addAgentNameInput = await $('[data-testid="mesh-add-agent-name-input"]')
      await addAgentNameInput.setValue(secondAgentName)

      const selectedAddProject = await selectFirstNonEmptyOption('[data-testid="mesh-add-agent-project-select"]')
      if (!selectedAddProject) return this.skip()

      await clickTestId('mesh-add-agent-submit')

      await browser.waitUntil(
        async () => {
          const rosterCard = await $(`[data-testid="mesh-roster-card-${secondAgentName}"]`)
          const addError = await $('[data-testid="mesh-add-agent-error"]')
          const runtimeMessage = await $('[data-testid="mesh-runtime-message"]')

          return (
            (await rosterCard.isExisting()) ||
            (await addError.isExisting()) ||
            (await runtimeMessage.isExisting())
          )
        },
        { ...WAIT_LONG, timeoutMsg: 'Hot-add did not update UI state' }
      )

      const addError = await $('[data-testid="mesh-add-agent-error"]')
      if (await addError.isExisting()) {
        throw new Error(`Hot-add failed in Tier 2 path: ${await addError.getText()}`)
      }

      // Confirm disband in-browser to avoid modal blocking.
      await browser.execute(() => {
        window.confirm = () => true
      })

      await clickTestId('mesh-disband-button')

      await browser.waitUntil(
        async () => await (await $('[data-testid="mesh-setup-title"]')).isExisting(),
        { ...WAIT_LONG, timeoutMsg: 'Disband did not return Mesh tab to setup mode' }
      )

      createdTeamName = null
    })

    it('skips Tier 2 when mesh prerequisites are unavailable', async function () {
      if (!mainApp) return this.skip()
      if (tier2Enabled) return this.skip()
      expect(typeof tier2SkipReason).toBe('string')
      expect(tier2SkipReason.length).toBeGreaterThan(0)
    })
  })
})
