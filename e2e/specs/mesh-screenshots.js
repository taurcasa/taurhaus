import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded, clickTestId } from '../helpers/navigation.js'
import { WAIT_MEDIUM, WAIT_LONG, WAIT_XLONG } from '../helpers/timing.js'
import { snapshotTmuxPanes, cleanupNewTmuxPanes } from '../helpers/tmux.js'

let mainApp = false
let tier2Enabled = false
const uniqueSuffix = `${Date.now()}-${Math.floor(Math.random() * 10_000)}`
let createdTeamName = null
const createdTeamNames = new Set()
let tmuxPaneSnapshot = { available: false, paneIds: [], reason: 'snapshot not captured' }

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
        (await hasTestId('mesh-mode-runtime')) ||
        (await hasTestId('mesh-mode-initializing')) ||
        (await hasTestId('mesh-availability-blocking'))
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
  if (!createdTeamNames.has(teamName)) return false

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

  if (await hasTestId('mesh-mode-empty')) {
    await clickTestId('mesh-builder-team-name-display')
  }

  await browser.waitUntil(
    async () => await hasTestId('mesh-mode-setup'),
    { ...WAIT_LONG, timeoutMsg: 'Mesh did not enter setup mode' }
  )

  return true
}

async function initializeRuntimeForScreenshot() {
  if (!(await ensureSetupMode())) return false

  const teamName = `e2e-mesh-shot-${uniqueSuffix}`

  await clickTestId('mesh-action-customize')
  await browser.waitUntil(
    async () => await hasTestId('team-customizer-panel'),
    { ...WAIT_MEDIUM, timeoutMsg: 'Team customizer did not open' }
  )

  const teamNameInput = await $('[data-testid="team-customizer-name-input"]')
  await browser.waitUntil(
    async () => await teamNameInput.isDisplayed(),
    { ...WAIT_MEDIUM, timeoutMsg: 'Team customizer name input did not become visible' }
  )

  const typedViaWebDriver = await teamNameInput.clearValue()
    .then(async () => {
      await teamNameInput.setValue(teamName)
      return true
    })
    .catch(() => false)

  if (!typedViaWebDriver) {
    await browser.execute((el, value) => {
      if (!el) return
      el.focus()
      el.value = ''
      el.dispatchEvent(new Event('input', { bubbles: true }))
      el.value = value
      el.dispatchEvent(new Event('input', { bubbles: true }))
      el.dispatchEvent(new Event('change', { bubbles: true }))
    }, teamNameInput, teamName)
  }

  const savedViaDomClick = await browser.execute(() => {
    const save = document.querySelector('[data-testid="team-customizer-save"]')
    if (!save || save.disabled) return false
    save.click()
    return true
  }).catch(() => false)
  if (!savedViaDomClick) {
    await clickTestId('team-customizer-save')
  }

  await browser.waitUntil(
    async () => !(await hasTestId('team-customizer-panel')),
    { ...WAIT_MEDIUM, timeoutMsg: 'Team customizer did not close' }
  )

  const createButton = await $('[data-testid="mesh-action-initialize"]')
  await browser.waitUntil(
    async () => await createButton.isEnabled(),
    { ...WAIT_MEDIUM, timeoutMsg: 'Initialize button never became enabled' }
  )
  await createButton.click()

  await browser.waitUntil(
    async () => {
      const runtime = await hasTestId('mesh-mode-runtime')
      const failed = await hasTestId('mesh-init-failure')
      return runtime || failed
    },
    { ...WAIT_XLONG, timeoutMsg: 'Mesh initialization did not resolve to runtime or failure' }
  )

  if (await hasTestId('mesh-init-failure')) return false

  createdTeamName = teamName
  createdTeamNames.add(teamName)
  return true
}

describe('Mesh Screenshot Capture', () => {
  before(async () => {
    tmuxPaneSnapshot = snapshotTmuxPanes()

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
    for (const teamName of createdTeamNames) {
      if (!teamName.startsWith('e2e-')) continue
      await invokeCoordinationWithTimeout('coordination_disband_team', { teamName }, 2_500)
    }
    createdTeamNames.clear()
    createdTeamName = null

    const tmuxCleanup = cleanupNewTmuxPanes(tmuxPaneSnapshot)
    if (!tmuxCleanup.attempted) {
      console.log(`[e2e] mesh-screenshots tmux cleanup skipped: ${tmuxCleanup.skippedReason}`)
    } else if (tmuxCleanup.failed.length > 0) {
      console.warn(`[e2e] mesh-screenshots tmux cleanup failures: ${JSON.stringify(tmuxCleanup.failed)}`)
    }
  })

  it('captures mesh setup/runtime screenshots', async function () {
    if (!mainApp) return this.skip()

    await clickTestId('theme-dark')
    await openMeshTab()

    const blocking = await $('[data-testid="mesh-availability-blocking"]')
    if (await blocking.isExisting()) {
      await browser.saveScreenshot('e2e/screenshots/mesh-unavailable-dark.png')
      return
    }

    if (!(await ensureSetupMode())) return this.skip()
    await browser.saveScreenshot('e2e/screenshots/mesh-setup-dark.png')

    await clickTestId('theme-light')
    if (!(await ensureSetupMode())) return this.skip()
    await browser.saveScreenshot('e2e/screenshots/mesh-setup-light.png')

    await clickTestId('theme-dark')
    await openMeshTab()
    if (!tier2Enabled) return

    const runtimeReady = await initializeRuntimeForScreenshot()
    if (!runtimeReady) return

    if (await hasTestId('mesh-runtime-title')) {
      await browser.saveScreenshot('e2e/screenshots/mesh-runtime-dark.png')
    }
  })
})
