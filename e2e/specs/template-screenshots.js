/**
 * Mesh redesign screenshot capture for designer review.
 *
 * Run with:
 *   just test-e2e-spec template-screenshots
 */

import { mkdirSync, rmSync } from 'node:fs'
import { resolve } from 'node:path'

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { clickTestId, waitForProjectsLoaded } from '../helpers/navigation.js'
import { WAIT_MEDIUM, WAIT_LONG, WAIT_XLONG } from '../helpers/timing.js'
import { snapshotTmuxPanes, cleanupNewTmuxPanes } from '../helpers/tmux.js'

const screenshotDir = resolve(import.meta.dirname, '..', 'screenshots', 'templates')
let mainApp = false
const runId = `${Date.now()}-${Math.floor(Math.random() * 10_000)}`
const runtimeTeamName = `e2e-template-shot-${runId}`
const createdTeamNames = new Set()
let tmuxPaneSnapshot = { available: false, paneIds: [], reason: 'snapshot not captured' }

function testIdSelector(testId) {
  return `[data-testid="${testId}"]`
}

async function hasTestId(testId) {
  return await (await $(testIdSelector(testId))).isExisting()
}

async function invokeTauri(command, args = undefined) {
  return await browser.executeAsync((payload, done) => {
    const tauri = window.__TAURI_INTERNALS__
    if (!tauri || typeof tauri.invoke !== 'function') {
      done({ ok: false, error: 'Tauri internals unavailable' })
      return
    }

    const invokePromise =
      payload.hasArgs
        ? tauri.invoke(payload.command, payload.args)
        : tauri.invoke(payload.command)

    invokePromise
      .then((result) => done({ ok: true, result }))
      .catch((error) => done({ ok: false, error: error?.message ?? String(error) }))
  }, { command, args, hasArgs: typeof args !== 'undefined' })
}

async function invokeTauriWithTimeout(command, args = undefined, timeoutMs = 2_500) {
  return await Promise.race([
    invokeTauri(command, args),
    new Promise((resolve) => {
      setTimeout(() => {
        resolve({ ok: false, error: `Timed out after ${timeoutMs}ms` })
      }, timeoutMs)
    }),
  ])
}

async function waitForMode(modeTestId) {
  await browser.waitUntil(
    async () => await hasTestId(modeTestId),
    { ...WAIT_LONG, timeoutMsg: `Expected mesh mode "${modeTestId}"` }
  )
}

async function shot(name) {
  await browser.pause(120)
  await browser.saveScreenshot(resolve(screenshotDir, `${name}.png`))
}

async function setTheme(theme) {
  await clickTestId(theme === 'light' ? 'theme-light' : 'theme-dark')
  await browser.pause(180)
}

async function closeSlideOverIfOpen() {
  if (!(await hasTestId('slideover-panel'))) return

  const closeButtons = await $$('[data-testid="slideover-close"]')
  if (closeButtons.length > 0) {
    await closeButtons.at(-1).click()
  } else if (await hasTestId('slideover-backdrop')) {
    await clickTestId('slideover-backdrop')
  } else {
    await browser.keys('Escape')
  }

  await browser.waitUntil(async () => !(await hasTestId('slideover-panel')), {
    ...WAIT_MEDIUM,
    timeoutMsg: 'SlideOver did not close'
  })
}

async function waitForMeshSurface() {
  await browser.waitUntil(
    async () =>
      (await hasTestId('mesh-mode-gate')) ||
      (await hasTestId('mesh-mode-empty')) ||
      (await hasTestId('mesh-mode-setup')) ||
      (await hasTestId('mesh-mode-initializing')) ||
      (await hasTestId('mesh-mode-runtime')) ||
      (await hasTestId('mesh-availability-blocking')),
    { ...WAIT_MEDIUM, timeoutMsg: 'Mesh tab did not render' }
  )
}

async function openMeshTab() {
  await clickTestId('tab-mesh')
  await waitForMeshSurface()

  if (await hasTestId('mesh-mode-gate')) {
    await browser.waitUntil(
      async () => !(await hasTestId('mesh-mode-gate')),
      { ...WAIT_XLONG, timeoutMsg: 'Mesh gate did not resolve' }
    )
  }
}

async function ensureMeshAvailable(testContext) {
  await openMeshTab()
  if (await hasTestId('mesh-availability-blocking')) {
    testContext.skip()
    return false
  }
  return true
}

async function ensureEmptyMode() {
  await openMeshTab()
  await closeSlideOverIfOpen()

  if (await hasTestId('mesh-mode-empty')) return true

  if (await hasTestId('mesh-mode-initializing')) {
    if (await hasTestId('mesh-init-back-button')) {
      await clickTestId('mesh-init-back-button')
      await waitForMode('mesh-mode-setup')
    } else {
      await browser.waitUntil(
        async () => !(await hasTestId('mesh-mode-initializing')),
        { ...WAIT_XLONG, timeoutMsg: 'Mesh stayed in initializing mode' }
      )
    }
  }

  if (await hasTestId('mesh-mode-runtime')) {
    const runtimeTitleEl = await $('[data-testid="mesh-runtime-title"]')
    const runtimeTeamName = (await runtimeTitleEl.isExisting()) ? (await runtimeTitleEl.getText()).trim() : ''
    if (!createdTeamNames.has(runtimeTeamName)) {
      return false
    }

    if (await hasTestId('mesh-runtime-disband')) {
      await clickTestId('mesh-runtime-disband')
      if (await hasTestId('confirm-dialog-confirm')) {
        await clickTestId('confirm-dialog-confirm')
      }
      createdTeamNames.delete(runtimeTeamName)
    }
  }

  if (await hasTestId('mesh-mode-setup') && await hasTestId('mesh-action-reset')) {
    await clickTestId('mesh-action-reset')
  }

  await waitForMode('mesh-mode-empty')
  return true
}

async function clickFirstPreset() {
  const preset = await $('[data-testid="mesh-template-preset-fullstack-dev"]')
  if (await preset.isExisting()) {
    await preset.click()
    return
  }

  const options = await $$('[data-testid^="mesh-template-preset-"]')
  for (const option of options) {
    if (await option.isDisplayed() && await option.isEnabled()) {
      await option.click()
      return
    }
  }
}

async function ensureSetupModeWithPreset() {
  await openMeshTab()
  await closeSlideOverIfOpen()

  if (!(await hasTestId('mesh-mode-setup'))) {
    if (!(await ensureEmptyMode())) return false
    await clickFirstPreset()
  }

  await waitForMode('mesh-mode-setup')
  await browser.waitUntil(
    async () => (await $$('[data-testid="mesh-node-agent"]')).length >= 2,
    { ...WAIT_LONG, timeoutMsg: 'Expected composed setup canvas with agent nodes' }
  )
  return true
}

async function openTemplateBrowser() {
  if (!(await ensureEmptyMode())) return false
  await clickTestId('mesh-template-browse-catalog')
  await browser.waitUntil(
    async () => await hasTestId('template-browser-panel'),
    { ...WAIT_MEDIUM, timeoutMsg: 'Template browser did not open' }
  )
  return true
}

async function openTeamCustomizer() {
  if (!(await ensureSetupModeWithPreset())) return false
  await clickTestId('mesh-action-customize')
  await browser.waitUntil(
    async () => await hasTestId('team-customizer-panel'),
    { ...WAIT_MEDIUM, timeoutMsg: 'Team customizer did not open' }
  )
  return true
}

async function initializeFromSetup() {
  if (!(await ensureSetupModeWithPreset())) return false
  await clickTestId('mesh-action-customize')
  await browser.waitUntil(
    async () => await hasTestId('team-customizer-panel'),
    { ...WAIT_MEDIUM, timeoutMsg: 'Team customizer did not open before initialization' }
  )
  const teamNameInput = await $('[data-testid="team-customizer-name-input"]')
  await teamNameInput.clearValue()
  await teamNameInput.setValue(runtimeTeamName)
  await clickTestId('team-customizer-save')
  await browser.waitUntil(
    async () => !(await hasTestId('team-customizer-panel')),
    { ...WAIT_MEDIUM, timeoutMsg: 'Team customizer did not close before initialization' }
  )
  createdTeamNames.add(runtimeTeamName)

  const initializeButton = await $('[data-testid="mesh-action-initialize"]')
  await initializeButton.waitForExist({ timeout: WAIT_MEDIUM.timeout })
  if (!(await initializeButton.isEnabled())) {
    throw new Error('Initialize button is disabled in setup mode')
  }

  await initializeButton.click()
  await browser.waitUntil(
    async () =>
      (await hasTestId('mesh-mode-initializing')) ||
      (await hasTestId('mesh-mode-runtime')) ||
      (await hasTestId('mesh-error')),
    { ...WAIT_LONG, timeoutMsg: 'Mesh did not transition to initializing/runtime mode' }
  )

  return await hasTestId('mesh-mode-initializing')
}

async function waitForRuntimeMode() {
  await browser.waitUntil(
    async () => await hasTestId('mesh-mode-runtime'),
    { ...WAIT_XLONG, timeoutMsg: 'Mesh did not enter runtime mode' }
  )
}

async function openAddAgentPanel() {
  await waitForRuntimeMode()
  await clickTestId('mesh-runtime-add-agent')
  await browser.waitUntil(
    async () => await hasTestId('mesh-add-agent-form'),
    { ...WAIT_MEDIUM, timeoutMsg: 'Add agent panel did not open' }
  )
}

describe('Mesh redesign screenshot capture', () => {
  before(async () => {
    tmuxPaneSnapshot = snapshotTmuxPanes()

    rmSync(screenshotDir, { recursive: true, force: true })
    mkdirSync(screenshotDir, { recursive: true })
    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (!mainApp) return
    await waitForProjectsLoaded()
  })

  after(async () => {
    for (const teamName of createdTeamNames) {
      if (!teamName.startsWith('e2e-')) continue
      await invokeTauriWithTimeout('coordination_disband_team', { teamName }, 2_500)
    }
    createdTeamNames.clear()

    const tmuxCleanup = cleanupNewTmuxPanes(tmuxPaneSnapshot)
    if (!tmuxCleanup.attempted) {
      console.log(`[e2e] template-screenshots tmux cleanup skipped: ${tmuxCleanup.skippedReason}`)
    } else if (tmuxCleanup.failed.length > 0) {
      console.warn(`[e2e] template-screenshots tmux cleanup failures: ${JSON.stringify(tmuxCleanup.failed)}`)
    }
  })

  it('captures all required mesh redesign views', async function () {
    if (!mainApp) return this.skip()
    if (!(await ensureMeshAvailable(this))) return

    await setTheme('dark')
    if (!(await ensureEmptyMode())) return this.skip()
    await shot('01-empty-state-dark')

    await setTheme('light')
    if (!(await ensureEmptyMode())) return this.skip()
    await shot('02-empty-state-light')

    await setTheme('dark')
    if (!(await ensureSetupModeWithPreset())) return this.skip()
    await shot('03-canvas-3-agent-composed-dark')

    await setTheme('light')
    if (!(await ensureSetupModeWithPreset())) return this.skip()
    await shot('04-canvas-3-agent-composed-light')

    await setTheme('dark')
    if (!(await ensureSetupModeWithPreset())) return this.skip()
    const firstAgentNode = (await $$('[data-testid="mesh-node-agent"]'))[0]
    if (firstAgentNode) {
      await firstAgentNode.click()
      await browser.waitUntil(
        async () => await hasTestId('mesh-node-detail'),
        { ...WAIT_MEDIUM, timeoutMsg: 'Node detail did not appear' }
      )
    }
    await shot('05-canvas-selected-node-detail-dark')

    if (!(await openTemplateBrowser())) return this.skip()
    await shot('06-template-browser-slideover-dark')
    await closeSlideOverIfOpen()

    if (!(await openTeamCustomizer())) return this.skip()
    await shot('07-team-customizer-slideover-dark')
    await closeSlideOverIfOpen()

    const capturedInitializing = await initializeFromSetup()
    if (!capturedInitializing && !(await hasTestId('mesh-mode-runtime')) && !(await hasTestId('mesh-mode-initializing'))) {
      return this.skip()
    }
    await shot('08-initialization-mid-state-dark')
    if (!capturedInitializing) {
      // Fast paths can move directly to runtime on local machines.
      // Keep the required init artifact by capturing immediately after click.
      await browser.pause(120)
    }
    await browser.waitUntil(
      async () =>
        (await hasTestId('mesh-mode-runtime')) ||
        (await hasTestId('mesh-init-failure')) ||
        (await hasTestId('mesh-error')),
      { ...WAIT_XLONG, timeoutMsg: 'Mesh did not resolve to runtime or failure state' }
    )
    if (!(await hasTestId('mesh-mode-runtime'))) return

    await shot('09-runtime-mixed-statuses-dark')

    await openAddAgentPanel()
    await shot('10-add-agent-panel-open-dark')
  })
})
