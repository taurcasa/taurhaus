/**
 * Role detail screenshot verification in the actual app.
 *
 * Run with:
 *   just test-e2e-spec role-detail-screenshots
 */

import { mkdirSync, rmSync } from 'node:fs'
import { resolve } from 'node:path'

import { expect } from 'expect-webdriverio'

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { clickTestId, fastClick, waitForProjectsLoaded } from '../helpers/navigation.js'
import { WAIT_MEDIUM, WAIT_LONG, WAIT_XLONG } from '../helpers/timing.js'

const screenshotDir = resolve(import.meta.dirname, '..', 'screenshots', 'role-detail')
let mainApp = false

function selector(testId) {
  return `[data-testid="${testId}"]`
}

async function hasTestId(testId) {
  return await (await $(selector(testId))).isExisting()
}

async function shot(name) {
  await browser.pause(180)
  await browser.saveScreenshot(resolve(screenshotDir, `${name}.png`))
}

async function openMeshTab() {
  await clickTestId('tab-mesh')
  await browser.waitUntil(
    async () =>
      (await hasTestId('mesh-mode-gate')) ||
      (await hasTestId('mesh-mode-empty')) ||
      (await hasTestId('mesh-mode-setup')) ||
      (await hasTestId('mesh-mode-runtime')) ||
      (await hasTestId('mesh-availability-blocking')),
    { ...WAIT_MEDIUM, timeoutMsg: 'Mesh surface did not render' }
  )

  if (await hasTestId('mesh-mode-gate')) {
    await browser.waitUntil(
      async () => !(await hasTestId('mesh-mode-gate')),
      { ...WAIT_XLONG, timeoutMsg: 'Mesh gate did not resolve' }
    )
  }
}

async function setTheme(theme) {
  const testId = theme === 'light' ? 'theme-light' : 'theme-dark'
  const domClicked = await browser.execute((targetTestId) => {
    const button = document.querySelector(`[data-testid="${targetTestId}"]`)
    if (!(button instanceof HTMLElement)) return false
    button.click()
    return true
  }, testId).catch(() => false)

  if (!domClicked) {
    await clickTestId(testId)
  }

  await browser.pause(220)
}

async function ensureSetupMode(testContext) {
  await openMeshTab()

  if (await hasTestId('mesh-availability-blocking')) {
    testContext.skip()
    return false
  }

  if (await hasTestId('mesh-mode-empty')) {
    await clickTestId('mesh-builder-team-name-display')
  }

  if (await hasTestId('mesh-mode-setup') || await hasTestId('mesh-builder-shell')) {
    return true
  }

  if (await hasTestId('mesh-mode-runtime')) {
    testContext.skip()
    return false
  }

  await browser.waitUntil(
    async () => (await hasTestId('mesh-mode-setup')) || (await hasTestId('mesh-builder-shell')),
    { ...WAIT_LONG, timeoutMsg: 'Mesh did not enter setup mode' }
  )

  return true
}

async function openCatalogRole(roleId = 'agent-codex') {
  const explicitInfoButton = selector(`mesh-builder-role-info-${roleId}`)
  if (await hasTestId(`mesh-builder-role-info-${roleId}`)) {
    const clicked = await fastClick(explicitInfoButton)
    if (!clicked) {
      await (await $(explicitInfoButton)).click()
    }
  } else {
    const buttons = await $$('[data-testid^="mesh-builder-role-info-"]')
    const firstVisible = []
    for (const button of buttons) {
      if (await button.isDisplayed()) {
        firstVisible.push(button)
      }
    }
    if (!firstVisible[0]) {
      throw new Error('No visible role info button found in the roster builder catalog')
    }
    await firstVisible[0].click()
  }

  await browser.waitUntil(
    async () => await hasTestId('mesh-node-detail'),
    { ...WAIT_MEDIUM, timeoutMsg: 'Role detail overlay did not open' }
  )
}

async function clickInsideOverlay(testId) {
  const domClicked = await browser.execute((targetTestId) => {
    const button = document.querySelector(`[data-testid="${targetTestId}"]`)
    if (!(button instanceof HTMLElement)) return false
    button.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, cancelable: true }))
    button.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true }))
    button.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, cancelable: true }))
    button.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
    return true
  }, testId).catch(() => false)

  if (!domClicked) {
    await clickTestId(testId)
  }
}

async function closeRoleDetail() {
  await clickInsideOverlay('mesh-node-detail-close')
  await browser.waitUntil(
    async () => !(await hasTestId('mesh-node-detail')),
    { ...WAIT_MEDIUM, timeoutMsg: 'Role detail overlay did not close' }
  )
}

async function openInlineEditMode() {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const clicked = await fastClick(selector('mesh-node-detail-edit'))
    if (!clicked) {
      await clickInsideOverlay('mesh-node-detail-edit')
    }

    const opened = await browser.waitUntil(
      async () => await hasTestId('mesh-node-detail-name-input'),
      { ...WAIT_LONG, timeoutMsg: 'Inline role editing did not open' }
    ).catch(() => false)

    if (opened) {
      return true
    }

    await browser.pause(180)
  }

  return false
}

describe('Role detail screenshot verification', () => {
  before(async () => {
    rmSync(screenshotDir, { recursive: true, force: true })
    mkdirSync(screenshotDir, { recursive: true })

    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (!mainApp) return

    await waitForProjectsLoaded()
  })

  it('captures dark read, light read, and dark edit role detail states', async function () {
    if (!mainApp) return this.skip()

    if (!(await ensureSetupMode(this))) return

    await setTheme('dark')
    await openCatalogRole('agent-codex')
    expect(await (await $(selector('mesh-node-detail'))).isExisting()).toBe(true)
    await shot('01-role-detail-read-dark')
    await closeRoleDetail()

    await setTheme('light')
    await openCatalogRole('agent-codex')
    await shot('02-role-detail-read-light')
    await closeRoleDetail()

    await setTheme('dark')
    await openCatalogRole('agent-codex')
    expect(await openInlineEditMode()).toBe(true)
    await shot('03-role-detail-edit-dark')
  })
})
