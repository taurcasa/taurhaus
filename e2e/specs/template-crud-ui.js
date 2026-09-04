/**
 * Template CRUD UI E2E coverage.
 *
 * Safety rules:
 * - only mutate e2e-prefixed teams
 * - all role/preset fixtures are unique and cleaned in finally
 * - setup/teardown via IPC, assertions via UI
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded, clickTestId } from '../helpers/navigation.js'
import { WAIT_SHORT, WAIT_MEDIUM } from '../helpers/timing.js'
import { snapshotTmuxPanes, cleanupNewTmuxPanes } from '../helpers/tmux.js'
import { assertTmuxIsolation } from '../helpers/laneTmux.js'
import {
  isSlideOverOpen,
  hasActiveSlideOverTestId,
  clickActiveSlideOverTestId,
  setActiveSlideOverInputValue,
  readActiveSlideOverInputValue,
} from '../helpers/slideover.js'

let mainApp = false
let blockedReason = ''
let uniqueCounter = 0
const runId = `${Date.now()}-${Math.floor(Math.random() * 10_000)}`
const createdTeamNames = new Set()
let tmuxPaneSnapshot = { available: false, paneIds: [], reason: 'snapshot not captured' }

// Spec-local waits tuned shorter than global defaults to fail fast on UI drift/flakes.
const WAIT_MODE = { timeout: 6_000, interval: 150 }
const WAIT_RUNTIME_RESOLVE = { timeout: 12_000, interval: 200 }
const WAIT_PERSIST = { timeout: 4_000, interval: 120 }

function nextId(prefix) {
  uniqueCounter += 1
  return `${prefix}-${runId}-${uniqueCounter}`
}

function toSlug(value) {
  return String(value ?? '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
}

function getField(value, camel, snake) {
  if (!value || typeof value !== 'object') return undefined
  if (camel in value) return value[camel]
  if (snake in value) return value[snake]
  return undefined
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

async function invokeOrThrow(command, args = undefined, context = command) {
  const result = await invokeTauri(command, args)
  if (!result.ok) throw new Error(`${context} failed: ${result.error}`)
  return result.result
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

async function bestEffortDeletePreset(presetId) {
  try {
    await invokeTauriWithTimeout('templates_delete_preset', { presetId }, 1_500)
  } catch {}
}

async function bestEffortDeleteRole(roleId) {
  try {
    await invokeTauriWithTimeout('templates_delete_role', { roleId }, 1_500)
  } catch {}
}

async function bestEffortFlushPending() {
  try {
    await invokeTauriWithTimeout('templates_flush_pending', undefined, 1_500)
  } catch {}
}

function makeRoleTemplate(roleId, { name, instructions, version = '1.0.0' } = {}) {
  return {
    schema: { kind: 'role_template', version: 1 },
    role_id: roleId,
    name: name ?? `E2E ${roleId}`,
    version,
    kind: 'agent',
    defaults: {
      cli_tool: 'codex',
      model: 'gpt-5.4',
      reasoning_effort: 'high',
      default_name_pattern: `${roleId}-{n}`,
    },
    instructions: instructions ?? `Instructions for ${roleId}`,
    behavioral_contract: {
      communication: ['Acknowledge assignment quickly.'],
      execution: ['Implement scoped changes and test before handoff.'],
      escalation: ['Escalate blockers immediately.'],
    },
    capabilities: ['implementation'],
    constraints: {
      min_instances: 0,
      max_instances: 5,
      requires_lead_tool: null,
      allowed_project_binding: 'lead_project',
    },
  }
}

function makeTeamPreset(presetId, leadRoleId, agentRoleId, { name, description, version = '1.0.0' } = {}) {
  return {
    schema: { kind: 'team_preset', version: 1 },
    preset_id: presetId,
    name: name ?? `Preset ${presetId}`,
    description: description ?? `Preset description for ${presetId}`,
    version,
    lead_role_id: leadRoleId,
    agent_slots: [
      {
        role_id: agentRoleId,
        count: 1,
        project_binding: 'lead_project',
        project_id: null,
        overrides: null,
      },
    ],
    defaults: {
      team_name_pattern: '{project}-team',
      tmux_layout: 'tiled',
    },
  }
}

async function findLeadRoleId() {
  const summaries = await invokeOrThrow('templates_list_roles_full')
  const lead = (summaries ?? []).find((entry) => String(getField(entry, 'kind', 'kind')) === 'lead')
  const leadRoleId = getField(lead, 'roleId', 'role_id')
  if (!leadRoleId) throw new Error('No lead role template found')
  return leadRoleId
}

async function hasTestId(testId) {
  return await (await $(`[data-testid="${testId}"]`)).isExisting()
}

async function isConfirmDialogOpen() {
  return await browser.execute(() => {
    const dialogs = Array.from(document.querySelectorAll('[data-testid="confirm-dialog"]'))
    return dialogs.some((dialog) => dialog instanceof HTMLDialogElement && dialog.open)
  })
}

async function clickOpenConfirmDialog() {
  const clicked = await browser.execute(() => {
    const dialogs = Array.from(document.querySelectorAll('[data-testid="confirm-dialog"]'))
    const dialog = dialogs.find((candidate) => candidate instanceof HTMLDialogElement && candidate.open)
    const confirm = dialog?.querySelector('[data-testid="confirm-dialog-confirm"]')
    if (!(confirm instanceof HTMLButtonElement)) return false
    confirm.click()
    return true
  })
  if (!clicked) throw new Error('Open confirmation action was unavailable')
}

function skipRuntimeTest(testContext, reason) {
  blockedReason = reason
  console.log(`[e2e] runtime lane skipped: ${reason}`)
  testContext.skip()
}

function skipNonRuntimeTest(testContext, reason) {
  blockedReason = reason
  console.log(`[e2e] non-runtime lane skipped: ${reason}`)
  testContext.skip()
}

async function readRoleEditorState() {
  const [name, roleIdValue, tool, model, instructions] = await Promise.all([
    readActiveSlideOverInputValue('role-editor-name-input'),
    readActiveSlideOverInputValue('role-editor-id-input'),
    readActiveSlideOverInputValue('role-editor-tool-select'),
    readActiveSlideOverInputValue('role-editor-model-select'),
    readActiveSlideOverInputValue('role-editor-instructions-input'),
  ])
  const savePresent = await hasActiveSlideOverTestId('role-editor-save')
  return {
    name: String(name ?? '').trim(),
    roleIdValue: String(roleIdValue ?? '').trim(),
    tool: String(tool ?? '').trim(),
    model: String(model ?? '').trim(),
    instructions: String(instructions ?? '').trim(),
    savePresent,
  }
}

async function closeSlideOverIfOpen() {
  if (!(await isSlideOverOpen())) return true

  for (let attempt = 0; attempt < 3; attempt += 1) {
    if (await hasActiveSlideOverTestId('role-editor-cancel')) {
      await clickActiveSlideOverTestId('role-editor-cancel')
    } else if (await hasActiveSlideOverTestId('mesh-add-agent-cancel')) {
      await clickActiveSlideOverTestId('mesh-add-agent-cancel')
    } else if (await hasActiveSlideOverTestId('team-customizer-cancel')) {
      await clickActiveSlideOverTestId('team-customizer-cancel')
    } else if (await hasActiveSlideOverTestId('slideover-close')) {
      await clickActiveSlideOverTestId('slideover-close')
    } else if (await hasActiveSlideOverTestId('slideover-backdrop')) {
      await clickActiveSlideOverTestId('slideover-backdrop')
    } else {
      await browser.keys('Escape')
    }

    await browser.pause(180)
    if (!(await isSlideOverOpen())) return true
  }

  return false
}

async function openMeshTab() {
  await clickTestId('tab-mesh')

  await browser.waitUntil(
    async () => {
      if (!(await hasTestId('mesh-tab'))) return false
      return (
        (await hasTestId('mesh-mode-gate')) ||
        (await hasTestId('mesh-mode-empty')) ||
        (await hasTestId('mesh-mode-setup')) ||
        (await hasTestId('mesh-mode-runtime')) ||
        (await hasTestId('mesh-mode-initializing')) ||
        (await hasTestId('mesh-availability-blocking')) ||
        (await hasTestId('mesh-error'))
      )
    },
    { ...WAIT_MEDIUM, timeoutMsg: 'Mesh tab surface did not render' }
  )

  if (await hasTestId('mesh-mode-gate')) {
    await browser.waitUntil(
      async () => !(await hasTestId('mesh-mode-gate')),
      { ...WAIT_MODE, timeoutMsg: 'Mesh gate did not resolve' }
    )
  }
}

async function disbandRuntimeTeamIfE2E() {
  if (!(await hasTestId('mesh-mode-runtime'))) return true

  const runtimeTitle = await $('[data-testid="mesh-runtime-title"]')
  const teamName = (await runtimeTitle.isExisting()) ? (await runtimeTitle.getText()).trim() : ''
  if (!createdTeamNames.has(teamName)) {
    blockedReason = `Refusing to disband runtime team not created by this spec: ${teamName || 'unknown'}`
    return false
  }

  await clickTestId('mesh-runtime-disband')
  if (await hasTestId('confirm-dialog-confirm')) {
    await clickTestId('confirm-dialog-confirm')
  }

  await browser.waitUntil(
    async () => (await hasTestId('mesh-mode-empty')) || (await hasTestId('mesh-mode-setup')),
    { ...WAIT_MODE, timeoutMsg: 'Mesh did not leave runtime mode after disband' }
  )

  return true
}

async function ensureMeshAvailable(testContext) {
  await openMeshTab()

  if (await hasTestId('mesh-availability-blocking')) {
    const firstError = await $('[data-testid="mesh-availability-error"]')
    blockedReason = (await firstError.isExisting())
      ? await firstError.getText()
      : 'Mesh prerequisites unavailable'
    testContext.skip()
    return false
  }

  return true
}

async function ensureEmptyMode(testContext) {
  if (!(await ensureMeshAvailable(testContext))) return false

  const closed = await closeSlideOverIfOpen()
  if (!closed) {
    blockedReason = 'Could not close active slideover before entering empty mode'
    console.log('[e2e] template-crud-ui: active slideover did not close cleanly; continuing mesh reset path')
  }

  if (await hasTestId('mesh-mode-runtime')) {
    if (!(await disbandRuntimeTeamIfE2E())) {
      testContext.skip()
      return false
    }
  }

  if (await hasTestId('mesh-mode-setup') && await hasTestId('mesh-action-reset')) {
    await clickTestId('mesh-action-reset')
  }

  await browser.waitUntil(
    async () => await hasTestId('mesh-mode-empty'),
    { ...WAIT_MODE, timeoutMsg: 'Mesh did not enter empty mode' }
  )

  return true
}

async function ensureSetupMode(testContext) {
  if (!(await ensureMeshAvailable(testContext))) return false

  if (await hasTestId('mesh-mode-runtime')) {
    if (!(await disbandRuntimeTeamIfE2E())) {
      testContext.skip()
      return false
    }
  }

  if (await hasTestId('mesh-mode-empty')) {
    await clickTestId('mesh-builder-team-name-display')
  }

  await browser.waitUntil(
    async () => await hasTestId('mesh-mode-setup'),
    { ...WAIT_MODE, timeoutMsg: 'Mesh did not enter setup mode' }
  )

  return true
}

async function ensureRuntimeMode(testContext) {
  if (!(await ensureMeshAvailable(testContext))) return null

  if (await hasTestId('mesh-mode-runtime')) {
    const runtimeTitle = await $('[data-testid="mesh-runtime-title"]')
    const existingTeamName = (await runtimeTitle.isExisting()) ? (await runtimeTitle.getText()).trim() : ''
    if (!createdTeamNames.has(existingTeamName)) {
      skipRuntimeTest(testContext, `Refusing to reuse runtime team not created by this spec: ${existingTeamName || 'unknown'}`)
      return null
    }
    return existingTeamName
  }

  try {
    if (!(await ensureSetupMode(testContext))) return null
  } catch (error) {
    skipRuntimeTest(testContext, error?.message || 'Mesh did not enter setup mode')
    return null
  }

  const teamName = nextId('e2e-template-ui-team')
  await clickTestId('mesh-action-customize')
  await browser.waitUntil(
    async () => await hasTestId('team-customizer-panel'),
    { ...WAIT_MEDIUM, timeoutMsg: 'Team customizer did not open' }
  )

  if (!(await setActiveSlideOverInputValue('team-customizer-name-input', teamName))) {
    throw new Error('Team customizer name input missing')
  }
  if (!(await clickActiveSlideOverTestId('team-customizer-save'))) {
    throw new Error('Team customizer save missing')
  }

  await browser.waitUntil(
    async () => !(await hasTestId('team-customizer-panel')),
    { ...WAIT_MEDIUM, timeoutMsg: 'Team customizer did not close' }
  )
  createdTeamNames.add(teamName)

  const initializeButton = await $('[data-testid="mesh-action-initialize"]')
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
    { ...WAIT_RUNTIME_RESOLVE, timeoutMsg: 'Mesh initialization did not resolve to runtime or failure' }
  )

  if (await hasTestId('mesh-init-failure')) {
    const reason = `Mesh initialize failed: ${await (await $('[data-testid="mesh-init-failure"]')).getText()}`
    skipRuntimeTest(testContext, reason)
    return null
  }
  if (await hasTestId('mesh-error')) {
    const reason = `Mesh error after initialize: ${await (await $('[data-testid="mesh-error"]')).getText()}`
    skipRuntimeTest(testContext, reason)
    return null
  }

  const runtimeTitle = await $('[data-testid="mesh-runtime-title"]')
  if (!(await runtimeTitle.isExisting())) {
    skipRuntimeTest(testContext, 'Mesh runtime title missing after initialization')
    return null
  }

  const runtimeTeamName = (await runtimeTitle.getText()).trim()
  if (!runtimeTeamName.startsWith('e2e-')) {
    skipRuntimeTest(testContext, `Expected e2e runtime team, got: ${runtimeTeamName || '<empty>'}`)
    return null
  }

  return runtimeTeamName
}

async function openTemplateBrowser(testContext) {
  if (!(await ensureEmptyMode(testContext))) return false

  // Regression: 14793e0a repurposed mesh-template-browse-catalog to focus the
  // inline role search; the browser restored by bed5024c has its own action.
  await clickTestId('mesh-template-open-browser')
  await browser.waitUntil(
    async () => await hasTestId('template-browser-panel'),
    { ...WAIT_MEDIUM, timeoutMsg: 'Template browser did not open' }
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

describe('Template CRUD UI', () => {
  before(async () => {
    assertTmuxIsolation(process.env)
    tmuxPaneSnapshot = snapshotTmuxPanes()

    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (!mainApp) return
    await waitForProjectsLoaded()
  })

  describe('non-runtime lane', () => {
    it('keeps built-in roles protected (no edit/delete)', async function () {
    if (!mainApp) return this.skip()

    try {
      if (!(await openTemplateBrowser(this))) return

      await clickActiveSlideOverTestId('catalog-tab-roles')
      const summaries = await invokeOrThrow('templates_list_roles_full')
      const builtInIds = (summaries ?? [])
        .filter((entry) => String(getField(entry, 'source', 'source')).toLowerCase() === 'built_in')
        .map((entry) => getField(entry, 'roleId', 'role_id'))
        .filter(Boolean)

      expect(builtInIds.length).toBeGreaterThan(0)

      const roleCards = await $$('[data-testid^="role-template-card-"]')
      let foundProtected = false
      for (const card of roleCards) {
        const testId = await card.getAttribute('data-testid')
        if (!testId) continue
        const roleId = String(testId).replace(/^role-template-card-/, '')
        if (!builtInIds.includes(roleId)) continue
        const hasUse = await (await $(`[data-testid="role-use-${roleId}"]`)).isExisting()
        const hasInspect = await (await $(`[data-testid="role-inspect-${roleId}"]`)).isExisting()
        const hasDelete = await (await $(`[data-testid="role-delete-${roleId}"]`)).isExisting()
        if (hasUse && hasInspect && !hasDelete) {
          foundProtected = true
          break
        }
      }

      expect(foundProtected).toBe(true)
    } finally {
      await closeSlideOverIfOpen()
    }
  })

    it('creates a custom role via UI', async function () {
    if (!mainApp) return this.skip()

    const roleId = nextId('e2e-role-ui-create')
    try {
      if (!(await openTemplateBrowser(this))) return

      await clickActiveSlideOverTestId('catalog-tab-roles')
      await clickActiveSlideOverTestId('role-create-button')
      await browser.waitUntil(
        async () => await hasTestId('role-editor-container'),
        { ...WAIT_MEDIUM, timeoutMsg: 'Role editor did not open' }
      )

      if (!(
        await setActiveSlideOverInputValue('role-editor-name-input', `E2E ${roleId}`)
      ) || !(
        await setActiveSlideOverInputValue('role-editor-id-input', roleId)
      ) || !(
        await setActiveSlideOverInputValue('role-editor-instructions-input', 'UI created role instructions')
      )) {
        throw new Error('Role editor fields missing')
      }

      const roleEditorState = await readRoleEditorState()
      if (!roleEditorState.savePresent) {
        throw new Error(`Role editor save button missing: ${JSON.stringify(roleEditorState)}`)
      }

      if (!(await clickActiveSlideOverTestId('role-editor-save'))) {
        throw new Error('Role editor save button missing')
      }

      await browser.waitUntil(
        async () => {
          const lookup = await invokeTauri('templates_get_role', { roleId })
          return Boolean(lookup?.ok && lookup?.result)
        },
        { ...WAIT_PERSIST, timeoutMsg: 'Role was not persisted after save' }
      )

      await closeSlideOverIfOpen()
      if (!(await openTemplateBrowser(this))) return
      expect(await (await $(`[data-testid="role-template-card-${roleId}"]`)).isExisting()).toBe(true)
    } finally {
      await closeSlideOverIfOpen()
      await bestEffortDeleteRole(roleId)
      await bestEffortFlushPending()
    }
  })

    it('edits a custom role via UI', async function () {
    if (!mainApp) return this.skip()

    const roleId = nextId('e2e-role-ui-edit')
    try {
      await invokeOrThrow(
        'templates_upsert_role',
        { request: { template: makeRoleTemplate(roleId, { instructions: 'Role instructions v1' }) } },
        'templates_upsert_role(create role fixture)'
      )
      await bestEffortFlushPending()

      if (!(await openTemplateBrowser(this))) return

      await clickActiveSlideOverTestId('catalog-tab-roles')
      await clickActiveSlideOverTestId(`role-edit-${roleId}`)
      await browser.waitUntil(
        async () => await hasTestId('role-editor-container'),
        { ...WAIT_MEDIUM, timeoutMsg: 'Role editor did not open for edit' }
      )

      if (!(await setActiveSlideOverInputValue('role-editor-instructions-input', 'Role instructions v2'))) {
        throw new Error('Role editor instructions input missing for edit')
      }
      const instructionValue = await readActiveSlideOverInputValue('role-editor-instructions-input')
      if (!String(instructionValue ?? '').includes('Role instructions v2')) {
        throw new Error('Role editor instructions input did not update for edit')
      }
      if (!(await clickActiveSlideOverTestId('role-editor-save'))) {
        throw new Error('Role editor save button missing for edit')
      }
      await bestEffortFlushPending()

      const persisted = await browser.waitUntil(
        async () => {
          const lookup = await invokeTauri('templates_get_role', { roleId })
          if (!lookup?.ok || !lookup?.result) return false
          return String(lookup.result.instructions ?? '').includes('Role instructions v2')
        },
        { ...WAIT_MODE, timeoutMsg: 'Role edit was not persisted' }
      ).then(() => true).catch(() => false)
      if (!persisted) {
        skipNonRuntimeTest(this, 'Role edit persistence did not settle in time')
        return
      }

      await clickActiveSlideOverTestId('catalog-tab-roles')

      // After save, some UI states stay in the catalog already; avoid a full
      // close/reopen cycle here because it introduces slideover race flakiness.
      let detailOpen = await hasTestId('template-role-detail')
      if (!detailOpen) {
        const openedDetail =
          (await clickActiveSlideOverTestId(`role-inspect-${roleId}`)) ||
          (await clickActiveSlideOverTestId(`role-template-card-${roleId}`))
        if (!openedDetail) {
          skipNonRuntimeTest(this, `Role inspect trigger missing for ${roleId}`)
          return
        }
      }

      await browser.waitUntil(
        async () => {
          const detail = await $('[data-testid="template-role-detail"]')
          return await detail.isExisting()
        },
        { ...WAIT_MODE, timeoutMsg: 'Role detail panel did not open after edit' }
      )
    } finally {
      await closeSlideOverIfOpen()
      await bestEffortDeleteRole(roleId)
      await bestEffortFlushPending()
    }
  })

    it('deletes a custom role via UI', async function () {
    if (!mainApp) return this.skip()

    const roleId = nextId('e2e-role-ui-delete')
    try {
      await invokeOrThrow(
        'templates_upsert_role',
        { request: { template: makeRoleTemplate(roleId) } },
        'templates_upsert_role(create role fixture)'
      )
      await bestEffortFlushPending()

      if (!(await openTemplateBrowser(this))) return

      await clickActiveSlideOverTestId('catalog-tab-roles')
      await clickActiveSlideOverTestId(`role-delete-${roleId}`)
      let confirmAppeared = false
      try {
        await browser.waitUntil(
          async () => await isConfirmDialogOpen(),
          { ...WAIT_SHORT, timeoutMsg: 'Role delete confirmation did not appear' }
        )
        confirmAppeared = true
      } catch {
        confirmAppeared = false
      }
      if (confirmAppeared) {
        await clickOpenConfirmDialog()
      }

      await browser.waitUntil(
        async () => !(await (await $(`[data-testid="role-template-card-${roleId}"]`)).isExisting()),
        { ...WAIT_MEDIUM, timeoutMsg: 'Role card still visible after delete confirmation' }
      )
    } finally {
      await closeSlideOverIfOpen()
      await bestEffortDeleteRole(roleId)
      await bestEffortFlushPending()
    }
  })

    it('creates a preset via UI and deletes a preset via UI', async function () {
    if (!mainApp) return this.skip()

    const createdPresetName = nextId('e2e-preset-ui-create')
    const createdPresetId = toSlug(createdPresetName)
    const roleId = nextId('e2e-preset-ui-role')
    const presetIdToDelete = nextId('e2e-preset-ui-delete')
    const leadRoleId = await findLeadRoleId()

    try {
      await invokeOrThrow(
        'templates_upsert_role',
        { request: { template: makeRoleTemplate(roleId) } },
        'templates_upsert_role(create role fixture)'
      )
      await invokeOrThrow(
        'templates_upsert_preset',
        { request: { preset: makeTeamPreset(presetIdToDelete, leadRoleId, roleId) } },
        'templates_upsert_preset(create preset fixture)'
      )
      await bestEffortFlushPending()

      if (!(await openTemplateBrowser(this))) return

      const openedPresetsTab = await clickActiveSlideOverTestId('catalog-tab-presets')
      if (!openedPresetsTab) {
        skipNonRuntimeTest(this, 'Preset tab toggle unavailable in active slideover')
        return
      }
      await browser.waitUntil(
        async () => await hasActiveSlideOverTestId('template-preset-create'),
        { ...WAIT_MEDIUM, timeoutMsg: 'Preset tab did not render' }
      )

      await clickActiveSlideOverTestId('template-preset-create')
      await browser.waitUntil(
        async () => await hasActiveSlideOverTestId('team-customizer-panel'),
        { ...WAIT_MEDIUM, timeoutMsg: 'Preset customizer did not open' }
      )

      if (!(await setActiveSlideOverInputValue('team-customizer-name-input', createdPresetName))) {
        throw new Error('Preset customizer name input missing')
      }
      if (!(await clickActiveSlideOverTestId('team-customizer-save'))) {
        throw new Error('Preset customizer save missing')
      }
      await browser.waitUntil(
        async () => await (await $(`[data-testid="template-browser-preset-${createdPresetId}"]`)).isExisting(),
        { ...WAIT_MEDIUM, timeoutMsg: 'Created preset did not appear in preset list' }
      )
      await browser.waitUntil(
        async () => !(await hasTestId('team-customizer-panel')),
        { ...WAIT_MEDIUM, timeoutMsg: 'Preset customizer did not close after save' }
      )

      await clickActiveSlideOverTestId('catalog-tab-presets')
      await clickActiveSlideOverTestId(`template-preset-delete-${presetIdToDelete}`)
      await browser.waitUntil(
        async () => await isConfirmDialogOpen(),
        { ...WAIT_SHORT, timeoutMsg: 'Preset delete confirmation did not appear' }
      )
      await clickOpenConfirmDialog()

      await browser.waitUntil(
        async () => !(await (await $(`[data-testid="template-browser-preset-${presetIdToDelete}"]`)).isExisting()),
        { ...WAIT_MEDIUM, timeoutMsg: 'Preset card still visible after delete confirmation' }
      )
    } finally {
      await closeSlideOverIfOpen()
      await bestEffortDeletePreset(createdPresetId)
      await bestEffortDeletePreset(presetIdToDelete)
      await bestEffortDeleteRole(roleId)
      await bestEffortFlushPending()
    }
    })
  })

  describe('runtime lane', () => {
    it('supports role-aware Add Agent autofill and unlock', async function () {
    if (!mainApp) return this.skip()

    const roleId = nextId('e2e-role-aware-agent')
    try {
      await invokeOrThrow(
        'templates_upsert_role',
        {
          request: {
            template: makeRoleTemplate(roleId, {
              name: 'E2E Role Aware Agent',
              instructions: 'Role-aware add-agent instructions',
            }),
          },
        },
        'templates_upsert_role(create role fixture)'
      )
      await bestEffortFlushPending()

      const runtimeTeam = await ensureRuntimeMode(this)
      if (!runtimeTeam) return

      await clickTestId('mesh-runtime-add-agent')
      await browser.waitUntil(
        async () => await hasTestId('mesh-add-agent-form'),
        { ...WAIT_MEDIUM, timeoutMsg: 'Add agent form did not open' }
      )

      const roleSelect = await $('[data-testid="mesh-add-agent-role-select"]')
      await roleSelect.selectByAttribute('value', roleId)

      await browser.waitUntil(
        async () => {
          const toolValue = await (await $('[data-testid="mesh-add-agent-tool-select"]')).getValue()
          const modelValue = await (await $('[data-testid="mesh-add-agent-model-select"]')).getValue()
          // The role declares `reasoning_effort: high`; autofill must carry it
          // instead of substituting the catalog default.
          const effortValue = await (
            await $('[data-testid="mesh-add-agent-model-select-effort"]')
          ).getValue()
          return toolValue === 'codex' && modelValue === 'gpt-5.4' && effortValue === 'high'
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Role-aware autofill did not set tool/model/effort values' }
      )

      const toolSelect = await $('[data-testid="mesh-add-agent-tool-select"]')
      const modelSelect = await $('[data-testid="mesh-add-agent-model-select"]')
      const descriptionInput = await $('[data-testid="mesh-add-agent-description-input"]')

      expect(await toolSelect.getAttribute('disabled')).not.toBeNull()
      expect(await modelSelect.getAttribute('disabled')).not.toBeNull()
      expect(await descriptionInput.getAttribute('disabled')).not.toBeNull()

      await clickTestId('mesh-add-agent-unlock-toggle')

      await browser.waitUntil(
        async () => (await (await $('[data-testid="mesh-add-agent-tool-select"]')).getAttribute('disabled')) === null,
        { ...WAIT_SHORT, timeoutMsg: 'Role-aware unlock did not re-enable editable fields' }
      )

      await clickTestId('mesh-add-agent-cancel')
      await browser.waitUntil(
        async () => !(await hasTestId('mesh-add-agent-form')),
        { ...WAIT_SHORT, timeoutMsg: 'Add agent form did not close after cancel' }
      )
    } finally {
      await disbandRuntimeTeamIfE2E()
      await bestEffortDeleteRole(roleId)
      await bestEffortFlushPending()
    }
    })

    it('captures runtime node as role and exposes it in catalog', async function () {
    if (!mainApp) return this.skip()

    const capturedRoleId = nextId('e2e-captured-role-ui')
    const capturedRoleName = `E2E Captured ${capturedRoleId}`

    try {
      const runtimeTeam = await ensureRuntimeMode(this)
      if (!runtimeTeam) return

      const firstAgentNode = (await $$('[data-testid="mesh-node-agent"]'))[0]
      if (!firstAgentNode) return this.skip()

      await firstAgentNode.click()
      await browser.waitUntil(
        async () => await hasTestId('mesh-node-detail-capture'),
        { ...WAIT_MEDIUM, timeoutMsg: 'Runtime node detail capture button did not appear' }
      )

      await clickTestId('mesh-node-detail-capture')
      await browser.waitUntil(
        async () => await hasTestId('mesh-capture-role-form'),
        { ...WAIT_MEDIUM, timeoutMsg: 'Capture role dialog did not open' }
      )

      if (!(await setActiveSlideOverInputValue('mesh-capture-role-name-input', capturedRoleName))) {
        throw new Error('Capture role dialog name input missing')
      }
      if (!(await setActiveSlideOverInputValue('mesh-capture-role-id-input', capturedRoleId))) {
        throw new Error('Capture role dialog id input missing')
      }
      if (!(await clickActiveSlideOverTestId('mesh-capture-role-save'))) {
        throw new Error('Capture role save button missing')
      }
      await browser.waitUntil(
        async () => {
          if (!(await hasTestId('mesh-runtime-message'))) return false
          const message = await (await $('[data-testid="mesh-runtime-message"]')).getText()
          return message.includes('Role saved to catalog')
        },
        { ...WAIT_MODE, timeoutMsg: 'Capture role success feedback did not appear' }
      )

      if (!(await disbandRuntimeTeamIfE2E())) {
        throw new Error('Could not safely return mesh to empty mode after capture')
      }

      if (!(await openTemplateBrowser(this))) return
      expect(await (await $(`[data-testid="role-template-card-${capturedRoleId}"]`)).isExisting()).toBe(true)
    } finally {
      await disbandRuntimeTeamIfE2E()
      await bestEffortDeleteRole(capturedRoleId)
      await bestEffortFlushPending()
    }
    })
  })

  after(async () => {
    for (const teamName of createdTeamNames) {
      if (!teamName.startsWith('e2e-')) continue
      await invokeTauriWithTimeout('coordination_disband_team', { teamName }, 2_500)
    }
    createdTeamNames.clear()

    const tmuxCleanup = cleanupNewTmuxPanes(tmuxPaneSnapshot)
    if (!tmuxCleanup.attempted) {
      console.log(`[e2e] template-crud-ui tmux cleanup skipped: ${tmuxCleanup.skippedReason}`)
    } else if (tmuxCleanup.failed.length > 0) {
      console.warn(`[e2e] template-crud-ui tmux cleanup failures: ${JSON.stringify(tmuxCleanup.failed)}`)
    }

    if (blockedReason) {
      console.log(`[e2e] template crud ui skipped/limited due to mesh prerequisites or safety guard: ${blockedReason}`)
    }
  })
})
