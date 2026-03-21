/**
 * Role detail screenshot verification in the actual app.
 *
 * Run with:
 *   just test-e2e-spec role-detail-screenshots
 */

import { mkdirSync, rmSync } from 'node:fs'
import { resolve } from 'node:path'

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { clickTestId, fastClick, waitForProjectsLoaded } from '../helpers/navigation.js'
import { WAIT_MEDIUM, WAIT_LONG, WAIT_XLONG } from '../helpers/timing.js'

const screenshotDir = resolve(import.meta.dirname, '..', 'screenshots', 'role-detail')
let mainApp = false
let uniqueCounter = 0
const runId = `${Date.now()}-${Math.floor(Math.random() * 10_000)}`

function selector(testId) {
  return `[data-testid="${testId}"]`
}

function nextId(prefix) {
  uniqueCounter += 1
  return `${prefix}-${runId}-${uniqueCounter}`
}

function slugify(value) {
  return String(value ?? '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
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

async function bestEffortDeleteRole(roleId) {
  if (!roleId) return
  try {
    await invokeTauriWithTimeout('templates_delete_role', { roleId }, 1_500)
  } catch {}
}

async function bestEffortFlushPending() {
  try {
    await invokeTauriWithTimeout('templates_flush_pending', undefined, 1_500)
  } catch {}
}

async function setInputValue(testId, value) {
  const input = await $(selector(testId))
  await input.waitForExist(WAIT_MEDIUM)
  await input.click()
  await input.clearValue()
  await input.setValue(value)
}

async function setCatalogSearch(value) {
  await setInputValue('mesh-builder-role-search', value)
  await browser.pause(180)
}

async function openCatalogRole(roleId) {
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

async function cancelInlineEditMode() {
  await clickInsideOverlay('mesh-node-detail-cancel')
  await browser.waitUntil(
    async () => !(await hasTestId('mesh-node-detail-name-input')),
    { ...WAIT_LONG, timeoutMsg: 'Inline role editing did not close' }
  )
}

async function closeRoleEditorIfOpen() {
  if (!(await hasTestId('mesh-role-editor'))) return

  await clickTestId('mesh-role-editor-close')
  await browser.waitUntil(
    async () => !(await hasTestId('mesh-role-editor')),
    { ...WAIT_MEDIUM, timeoutMsg: 'Role editor dialog did not close' }
  )
}

async function createEditableCatalogRole() {
  const roleName = `E2E ${nextId('role-detail-editor')}`
  const roleId = slugify(roleName)

  await clickTestId('mesh-builder-create-role')
  await browser.waitUntil(
    async () => await hasTestId('mesh-role-editor'),
    { ...WAIT_MEDIUM, timeoutMsg: 'Role editor dialog did not open' }
  )

  await setInputValue('mesh-role-editor-name-input', roleName)
  await setInputValue(
    'mesh-role-editor-context-summary-input',
    'Owns stacked-section editor verification in the live app.'
  )
  await setInputValue(
    'mesh-role-editor-instructions-input',
    'Keep the role detail editor readable, calm, and easy to scan.'
  )
  await clickTestId('mesh-role-editor-save')

  await browser.waitUntil(
    async () => !(await hasTestId('mesh-role-editor')),
    { ...WAIT_LONG, timeoutMsg: 'Role editor dialog did not close after save' }
  )

  await browser.waitUntil(
    async () =>
      (await hasTestId(`mesh-builder-role-info-${roleId}`)) || (await hasTestId('mesh-node-detail')),
    { ...WAIT_LONG, timeoutMsg: `Custom role ${roleId} did not appear in the builder` }
  )

  if (await hasTestId('mesh-node-detail')) {
    await closeRoleDetail()
  }

  await setCatalogSearch(roleName)
  await browser.waitUntil(
    async () => await hasTestId(`mesh-builder-role-info-${roleId}`),
    { ...WAIT_LONG, timeoutMsg: `Custom role ${roleId} did not become searchable` }
  )

  return { roleId }
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

  it('captures stacked role detail states in both themes', async function () {
    if (!mainApp) return this.skip()

    if (!(await ensureSetupMode(this))) return

    let createdRoleId = ''

    try {
      const createdRole = await createEditableCatalogRole()
      createdRoleId = createdRole.roleId

      await setTheme('dark')
      await openCatalogRole(createdRoleId)
      expect(await (await $(selector('mesh-node-detail'))).isExisting()).toBe(true)
      await shot('01-role-detail-read-dark')
      await closeRoleDetail()

      await setTheme('light')
      await openCatalogRole(createdRoleId)
      await shot('02-role-detail-read-light')
      await closeRoleDetail()

      await setTheme('dark')
      await openCatalogRole(createdRoleId)
      expect(await openInlineEditMode()).toBe(true)
      await shot('03-role-detail-edit-dark')
      await cancelInlineEditMode()
      await closeRoleDetail()

      await setTheme('light')
      await openCatalogRole(createdRoleId)
      expect(await openInlineEditMode()).toBe(true)
      await shot('04-role-detail-edit-light')
      await cancelInlineEditMode()
      await closeRoleDetail()
    } finally {
      await closeRoleEditorIfOpen().catch(() => {})
      if (await hasTestId('mesh-node-detail-name-input')) {
        await cancelInlineEditMode().catch(() => {})
      }
      if (await hasTestId('mesh-node-detail')) {
        await closeRoleDetail().catch(() => {})
      }
      await setCatalogSearch('').catch(() => {})
      await bestEffortDeleteRole(createdRoleId)
      await bestEffortFlushPending()
    }
  })
})
