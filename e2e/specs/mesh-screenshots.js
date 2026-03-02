import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded, clickTestId } from '../helpers/navigation.js'
import { WAIT_MEDIUM, WAIT_LONG, WAIT_XLONG } from '../helpers/timing.js'

let mainApp = false
let tier2Enabled = false
let uniqueSuffix = `${Date.now()}-${Math.floor(Math.random() * 10_000)}`
let createdTeamName = null

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
      return (
        (await meshTab.isExisting()) &&
        ((await setup.isExisting()) || (await runtime.isExisting()) || (await blocking.isExisting()))
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

  await browser.execute(() => {
    window.confirm = () => true
  })

  const disbandBtn = await $('[data-testid="mesh-disband-button"]')
  if (await disbandBtn.isExisting()) {
    await disbandBtn.click()
    await browser.waitUntil(
      async () => await (await $('[data-testid="mesh-setup-title"]')).isExisting(),
      { ...WAIT_LONG, timeoutMsg: 'Mesh did not return to setup mode after disband' }
    )
  }
}

async function initializeRuntimeForScreenshot() {
  const setupTitle = await $('[data-testid="mesh-setup-title"]')
  if (!(await setupTitle.isExisting())) return false

  const teamName = `mesh-shot-${uniqueSuffix}`
  const firstAgentName = `mesh-shot-agent-a-${uniqueSuffix}`

  const teamNameInput = await $('[data-testid="mesh-team-name-input"]')
  await teamNameInput.waitForExist({ timeout: WAIT_MEDIUM.timeout })
  await teamNameInput.clearValue()
  await teamNameInput.setValue(teamName)

  const firstAgentInput = await $('[data-testid="mesh-agent-name-input-0"]')
  await firstAgentInput.waitForExist({ timeout: WAIT_MEDIUM.timeout })
  await firstAgentInput.clearValue()
  await firstAgentInput.setValue(firstAgentName)

  const selectedSetupProject = await selectFirstNonEmptyOption('[data-testid="mesh-agent-project-select-0"]')
  if (!selectedSetupProject) return false

  const createButton = await $('[data-testid="mesh-create-team-button"]')
  await browser.waitUntil(
    async () => !(await createButton.getProperty('disabled')),
    { ...WAIT_MEDIUM, timeoutMsg: 'Initialize button never became enabled' }
  )
  await createButton.click()

  await browser.waitUntil(
    async () => {
      const runtime = await $('[data-testid="mesh-runtime-title"]')
      const failed = await $('[data-testid="mesh-init-failure"]')
      return (await runtime.isExisting()) || (await failed.isExisting())
    },
    { ...WAIT_XLONG, timeoutMsg: 'Mesh initialization did not resolve to runtime or failure' }
  )

  const failed = await $('[data-testid="mesh-init-failure"]')
  if (await failed.isExisting()) return false

  createdTeamName = teamName
  return true
}

describe('Mesh Screenshot Capture', () => {
  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (!mainApp) return

    await waitForProjectsLoaded()

    const availability = await invokeCoordination('coordination_get_feature_availability')
    if (!availability.ok) {
      tier2Enabled = false
      return
    }
    const report = availability.result || {}
    const canInitialize = report.canInitialize !== false
    const meshAvailable = report.meshAvailable !== false
    const tmuxAvailable = report.tmuxAvailable !== false
    const blockingErrors = Array.isArray(report.blockingErrors) ? report.blockingErrors : []
    tier2Enabled = canInitialize && meshAvailable && tmuxAvailable && blockingErrors.length === 0
  })

  after(async () => {
    await browser.execute(() => {
      window.confirm = () => true
    })
    if (createdTeamName) {
      await invokeCoordination('coordination_disband_team', { teamName: createdTeamName })
    }
  })

  it('captures mesh setup/runtime screenshots', async function () {
    if (!mainApp) return this.skip()

    await browser.execute(() => {
      window.confirm = () => true
    })

    await clickTestId('theme-dark')
    await openMeshTab()
    await ensureSetupMode()
    await browser.saveScreenshot('e2e/screenshots/mesh-setup-dark.png')

    const blocking = await $('[data-testid="mesh-availability-blocking"]')
    if (await blocking.isExisting()) {
      await browser.saveScreenshot('e2e/screenshots/mesh-unavailable-dark.png')
    }

    await clickTestId('theme-light')
    await openMeshTab()
    await ensureSetupMode()
    await browser.saveScreenshot('e2e/screenshots/mesh-setup-light.png')

    await clickTestId('theme-dark')
    await openMeshTab()

    if (!tier2Enabled) return

    const runtimeReady = await initializeRuntimeForScreenshot()
    if (!runtimeReady) return

    const runtimeTitle = await $('[data-testid="mesh-runtime-title"]')
    if (await runtimeTitle.isExisting()) {
      await browser.saveScreenshot('e2e/screenshots/mesh-runtime-dark.png')
    }
  })
})
