/**
 * Roster Builder screenshot capture for design/product review.
 *
 * Run with:
 *   just test-e2e-spec template-screenshots
 */

import { mkdirSync, rmSync } from 'node:fs'
import { resolve } from 'node:path'

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { clickUntil } from '../helpers/clickUntil.js'
import { clickTestId, fastClick, waitForProjectsLoaded } from '../helpers/navigation.js'
import { WAIT_SHORT, WAIT_MEDIUM, WAIT_LONG, WAIT_XLONG } from '../helpers/timing.js'
import { snapshotTmuxPanes, cleanupNewTmuxPanes } from '../helpers/tmux.js'
import { assertTmuxIsolation } from '../helpers/laneTmux.js'

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

    await clickUntil('mesh-runtime-more-toggle', 'mesh-runtime-disband',
      { ...WAIT_SHORT, timeoutMsg: 'Disband action did not appear' }
    )
    await clickUntil('mesh-runtime-disband',
      async () => await browser.execute(() =>
        document.querySelector('dialog[open][data-testid="confirm-dialog"]') !== null),
      { ...WAIT_SHORT, timeoutMsg: 'Disband confirmation dialog did not appear' }
    )
    await (await $('dialog[open][data-testid="confirm-dialog"] [data-testid="confirm-dialog-confirm"]')).click()
    createdTeamNames.delete(runtimeTeamName)
  }

  if (await hasTestId('mesh-mode-setup') && await hasTestId('mesh-action-reset')) {
    await clickTestId('mesh-action-reset')
  }

  await waitForMode('mesh-mode-empty')
  return true
}

async function clickPreset(presetId) {
  const selector = `[data-testid="mesh-template-preset-${presetId}"]`
  if (await hasTestId(`mesh-template-preset-${presetId}`)) {
    const clicked = await fastClick(selector)
    if (clicked) return
  }

  const options = await $$('[data-testid^="mesh-template-preset-"]')
  for (const option of options) {
    if (await option.isDisplayed() && await option.isEnabled()) {
      await option.scrollIntoView().catch(() => {})
      const clicked = await browser.execute((el) => {
        if (!el) return false
        el.click()
        return true
      }, option).catch(() => false)
      if (clicked) return
    }
  }
}

async function ensureSetupModeFromPreset(presetId) {
  await openMeshTab()
  await closeSlideOverIfOpen()

  if (!(await hasTestId('mesh-mode-setup'))) {
    if (!(await ensureEmptyMode())) return false
    await clickPreset(presetId)
  }

  await waitForMode('mesh-mode-setup')
  return true
}

async function ensureSetupModeFromCustom() {
  await openMeshTab()
  await closeSlideOverIfOpen()

  if (!(await hasTestId('mesh-mode-setup'))) {
    if (!(await ensureEmptyMode())) return false
    await clickTestId('mesh-builder-team-name-display')
  }

  await waitForMode('mesh-mode-setup')
  return true
}

async function ensureCatalogExpanded() {
  return
}

async function ensureCatalogCollapsed() {
  return
}

async function clickFirstSectionAction(sectionTestId, prefix) {
  const buttons = await $$(`[data-testid="${sectionTestId}"] [data-testid^="${prefix}"]`)
  for (const button of buttons) {
    if (await button.isDisplayed() && await button.isEnabled()) {
      await button.scrollIntoView().catch(() => {})
      const testId = await button.getAttribute('data-testid')
      if (testId) {
        const clicked = await fastClick(`[data-testid="${testId}"]`)
        if (clicked) return true
      }

      const clicked = await browser.execute((el) => {
        if (!el) return false
        el.click()
        return true
      }, button).catch(() => false)
      if (clicked) return true
    }
  }
  return false
}

async function clickNthSectionAction(sectionTestId, prefix, index) {
  const buttons = await $$(`[data-testid="${sectionTestId}"] [data-testid^="${prefix}"]`)
  if (buttons[index] && await buttons[index].isDisplayed() && await buttons[index].isEnabled()) {
    const button = buttons[index]
    await button.scrollIntoView().catch(() => {})
    const testId = await button.getAttribute('data-testid')
    if (testId) {
      const clicked = await fastClick(`[data-testid="${testId}"]`)
      if (clicked) return true
    }

    return await browser.execute((el) => {
      if (!el) return false
      el.click()
      return true
    }, button).catch(() => false)
  }
  return false
}

async function buildPartialRoster() {
  if (!(await ensureSetupModeFromCustom())) return false

  await clickFirstSectionAction('mesh-builder-role-section-leads', 'mesh-builder-add-')
  await browser.waitUntil(
    async () => await hasTestId('mesh-builder-lead-card'),
    { ...WAIT_MEDIUM, timeoutMsg: 'Lead card did not appear after selecting a lead' }
  )

  await clickNthSectionAction('mesh-builder-role-section-agents', 'mesh-builder-add-', 0)
  await clickNthSectionAction('mesh-builder-role-section-agents', 'mesh-builder-add-', 1)
  await browser.waitUntil(
    async () => (await $$('[data-testid^="mesh-builder-agent-card-"]')).length >= 2,
    { ...WAIT_MEDIUM, timeoutMsg: 'Expected at least two agent cards in the roster' }
  )

  return true
}

async function pinFavorites() {
  if (!(await ensureSetupModeFromCustom())) return false

  await clickNthSectionAction('mesh-builder-role-section-leads', 'mesh-builder-pin-', 0)
  await clickNthSectionAction('mesh-builder-role-section-agents', 'mesh-builder-pin-', 0)
  await clickNthSectionAction('mesh-builder-role-section-agents', 'mesh-builder-pin-', 1)

  await browser.waitUntil(
    async () => await hasTestId('mesh-builder-pinned-strip'),
    { ...WAIT_MEDIUM, timeoutMsg: 'Pinned favorites strip did not appear' }
  )

  return true
}

async function buildFullTeam() {
  if (!(await ensureSetupModeFromPreset('full-team'))) return false
  await clickNthSectionAction('mesh-builder-role-section-agents', 'mesh-builder-add-', 0)
  await browser.waitUntil(
    async () => (await $$('[data-testid^="mesh-builder-agent-card-"]')).length >= 4,
    { ...WAIT_MEDIUM, timeoutMsg: 'Expected full team roster with four agents' }
  )

  return true
}

describe('Roster Builder screenshot capture', () => {
  before(async () => {
    assertTmuxIsolation(process.env)
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

  it('captures all required roster builder views', async function () {
    if (!mainApp) return this.skip()
    if (!(await ensureMeshAvailable(this))) return

    await setTheme('dark')
    if (!(await ensureEmptyMode())) return this.skip()
    await shot('01-roster-builder-empty-state-dark')

    await setTheme('dark')
    if (!(await ensureSetupModeFromPreset('dev-team'))) return this.skip()
    await shot('02-roster-builder-preset-applied-dark')

    await setTheme('light')
    if (!(await buildPartialRoster())) return this.skip()
    await shot('03-roster-builder-partially-built-light')

    await setTheme('dark')
    if (!(await buildFullTeam())) return this.skip()
    await shot('04-roster-builder-full-team-ready-dark')
  })
})
