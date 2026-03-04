/**
 * Template & Mesh UX screenshot capture.
 *
 * Captures current state of all template/team management UI surfaces
 * for designer review. Run with:
 *   just test-e2e-spec template-screenshots
 */

import { resolve } from 'node:path'
import { mkdirSync } from 'node:fs'

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded, clickTestId } from '../helpers/navigation.js'
import { WAIT_SHORT, WAIT_MEDIUM, WAIT_LONG } from '../helpers/timing.js'

const screenshotDir = resolve(import.meta.dirname, '..', 'screenshots', 'templates')
let mainApp = false

async function invokeTauri(command, args = undefined) {
  return await browser.executeAsync((payload, done) => {
    const tauri = window.__TAURI_INTERNALS__
    if (!tauri || typeof tauri.invoke !== 'function') {
      done({ ok: false, error: 'Tauri internals unavailable' })
      return
    }
    const invokePromise = payload.hasArgs
      ? tauri.invoke(payload.command, payload.args)
      : tauri.invoke(payload.command)
    invokePromise
      .then((result) => done({ ok: true, result }))
      .catch((error) => done({ ok: false, error: error?.message ?? String(error) }))
  }, { command, args, hasArgs: typeof args !== 'undefined' })
}

function getField(value, camel, snake) {
  if (!value || typeof value !== 'object') return undefined
  if (camel in value) return value[camel]
  if (snake in value) return value[snake]
  return undefined
}

async function openMeshTab() {
  await clickTestId('tab-mesh')
  await browser.waitUntil(
    async () => {
      const empty = await $('[data-testid="mesh-mode-empty"]')
      const setup = await $('[data-testid="mesh-mode-setup"]')
      const runtime = await $('[data-testid="mesh-mode-runtime"]')
      const blocking = await $('[data-testid="mesh-availability-blocking"]')
      return (
        (await empty.isExisting()) ||
        (await setup.isExisting()) ||
        (await runtime.isExisting()) ||
        (await blocking.isExisting())
      )
    },
    { ...WAIT_MEDIUM, timeoutMsg: 'Mesh tab surface did not render' }
  )
}

async function ensureEmptyMode() {
  const empty = await $('[data-testid="mesh-mode-empty"]')
  if (await empty.isExisting()) return

  const runtime = await $('[data-testid="mesh-mode-runtime"]')
  if (await runtime.isExisting()) {
    const disband = await $('[data-testid="mesh-runtime-disband"]')
    if (await disband.isExisting()) {
      await disband.click()
      const confirm = await $('[data-testid="confirm-dialog-confirm"]')
      if (await confirm.isExisting()) await confirm.click()
    }
  }

  const setup = await $('[data-testid="mesh-mode-setup"]')
  if (await setup.isExisting()) {
    const reset = await $('[data-testid="mesh-action-reset"]')
    if (await reset.isExisting()) {
      await reset.click()
    }
  }

  await browser.waitUntil(
    async () => await (await $('[data-testid="mesh-mode-empty"]')).isExisting(),
    { ...WAIT_LONG, timeoutMsg: 'Mesh did not return to empty mode' }
  )
}

async function ensureSetupMode() {
  const setup = await $('[data-testid="mesh-mode-setup"]')
  if (await setup.isExisting()) return

  await ensureEmptyMode()
  await clickTestId('mesh-template-build-custom')

  await browser.waitUntil(
    async () => await (await $('[data-testid="mesh-mode-setup"]')).isExisting(),
    { ...WAIT_LONG, timeoutMsg: 'Mesh did not enter setup mode' }
  )
}

async function shot(name) {
  await browser.saveScreenshot(resolve(screenshotDir, `${name}.png`))
}

describe('Template & Mesh UX Screenshots', () => {
  before(async () => {
    mkdirSync(screenshotDir, { recursive: true })
    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (!mainApp) return
    await waitForProjectsLoaded()
  })

  it('captures mesh setup — template picker (dark)', async function () {
    if (!mainApp) return this.skip()
    await clickTestId('theme-dark')
    await openMeshTab()
    await ensureEmptyMode()
    await shot('01-mesh-setup-template-picker-dark')
  })

  it('captures mesh setup — template picker (light)', async function () {
    if (!mainApp) return this.skip()
    await clickTestId('theme-light')
    await openMeshTab()
    await ensureEmptyMode()
    await shot('02-mesh-setup-template-picker-light')
  })

  it('captures template catalog — role list (dark)', async function () {
    if (!mainApp) return this.skip()
    await clickTestId('theme-dark')
    await openMeshTab()
    await ensureEmptyMode()

    // Open catalog via browse button
    await clickTestId('mesh-template-browse-catalog')
    await browser.waitUntil(
      async () => {
        const catalog = await $('[data-testid="template-browser-panel"]')
        return await catalog.isExisting()
      },
      { ...WAIT_MEDIUM, timeoutMsg: 'Template browser panel did not load' }
    )
    await shot('03-template-catalog-roles-dark')
  })

  it('captures template catalog — role detail panel', async function () {
    if (!mainApp) return this.skip()

    // Click first role card to show detail panel
    const roleCards = await $$('[data-testid^="role-template-card-"]')
    if (roleCards.length > 0) {
      const inspect = await $('[data-testid^="role-inspect-"]')
      if (await inspect.isExisting()) {
        await inspect.click()
      }
      await browser.waitUntil(
        async () => await (await $('[data-testid="template-role-detail"]')).isExisting(),
        { ...WAIT_MEDIUM, timeoutMsg: 'Detail panel did not appear' }
      )
      await shot('04-template-catalog-role-detail-dark')
    }
  })

  it('captures template catalog — preset list', async function () {
    if (!mainApp) return this.skip()

    // Switch to presets tab if available
    const presetsTab = await $('[data-testid="catalog-tab-presets"]')
    if (await presetsTab.isExisting()) {
      await presetsTab.click()
      await browser.pause(500)
      await shot('05-template-catalog-presets-dark')

      // Click first preset to show detail
      const presetCards = await $$('[data-testid^="template-browser-preset-"]')
      if (presetCards.length > 0) {
        await presetCards[0].click()
        await browser.waitUntil(
          async () => await (await $('[data-testid="template-preset-detail"]')).isExisting(),
          { ...WAIT_MEDIUM, timeoutMsg: 'Preset detail panel did not appear' }
        )
        await shot('06-template-catalog-preset-detail-dark')
      }
    }
  })

  it('captures team customizer — composition flow', async function () {
    if (!mainApp) return this.skip()

    await openMeshTab()
    await ensureSetupMode()
    await clickTestId('mesh-action-customize')
    await browser.waitUntil(
      async () => await (await $('[data-testid="team-customizer-panel"]')).isExisting(),
      { ...WAIT_MEDIUM, timeoutMsg: 'Team customizer did not open' }
    )

    const customizer = await $('[data-testid="team-customizer-panel"]')
    if (await customizer.isExisting()) {
      await shot('07-team-customizer-from-setup-dark')
    }
  })

  it('captures team customizer — custom build flow', async function () {
    if (!mainApp) return this.skip()

    await openMeshTab()
    await ensureSetupMode()
    await clickTestId('mesh-action-customize')
    await browser.waitUntil(
      async () => await (await $('[data-testid="team-customizer-panel"]')).isExisting(),
      { ...WAIT_MEDIUM, timeoutMsg: 'Team customizer did not open from custom flow' }
    ).catch(() => {})

    const customizer = await $('[data-testid="team-customizer-panel"]')
    if (await customizer.isExisting()) {
      await shot('08-team-customizer-custom-build-dark')
    }
  })

  it('captures template history panel', async function () {
    if (!mainApp) return this.skip()

    await openMeshTab()
    await ensureEmptyMode()
    await clickTestId('mesh-template-browse-catalog')
    await browser.waitUntil(
      async () => {
        const catalog = await $('[data-testid="template-browser-panel"]')
        return await catalog.isExisting()
      },
      { ...WAIT_MEDIUM, timeoutMsg: 'Template browser panel did not load' }
    )

    // Look for history tab/toggle in catalog
    const historyTab = await $('[data-testid="catalog-tab-history"]')
    if (await historyTab.isExisting()) {
      await historyTab.click()
      await browser.pause(1000) // Wait for git log to load
      await shot('09-template-history-panel-dark')
    }
  })

  it('captures mesh setup — customizer opened', async function () {
    if (!mainApp) return this.skip()

    await openMeshTab()
    await ensureSetupMode()

    await clickTestId('mesh-action-customize')
    await browser.waitUntil(
      async () => await (await $('[data-testid="team-customizer-panel"]')).isExisting(),
      { ...WAIT_MEDIUM, timeoutMsg: 'Team customizer did not open' }
    )
    await shot('10-mesh-setup-advanced-expanded-dark')
  })

  it('captures mesh setup — light theme with preset applied', async function () {
    if (!mainApp) return this.skip()
    await clickTestId('theme-light')
    await openMeshTab()
    await ensureEmptyMode()

    const quickPresetButtons = await $$('[data-testid^="mesh-template-preset-"]')
    for (const button of quickPresetButtons) {
      if (await button.isExisting() && await button.isEnabled()) {
        await button.click()
        await browser.pause(500)
        break
      }
    }
    await shot('11-mesh-setup-preset-applied-light')
  })
})
