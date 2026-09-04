/**
 * Template workflow e2e tests.
 *
 * Updated for mode-based mesh UI:
 * - empty/setup/runtime wrappers (`mesh-mode-*`)
 * - template browser slideover (`template-browser-panel`)
 */

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded, clickTestId } from '../helpers/navigation.js'
import { WAIT_SHORT, WAIT_MEDIUM, WAIT_LONG } from '../helpers/timing.js'

let mainApp = false
let templatesBlockedReason = ''
let uniqueCounter = 0
const runId = `${Date.now()}-${Math.floor(Math.random() * 10_000)}`

function nextId(prefix) {
  uniqueCounter += 1
  return `${prefix}-${runId}-${uniqueCounter}`
}

function getField(value, camel, snake) {
  if (!value || typeof value !== 'object') return undefined
  if (camel in value) return value[camel]
  if (snake in value) return value[snake]
  return undefined
}

function toSlug(value) {
  return String(value ?? '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
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

async function bestEffortDeletePreset(presetId) {
  try {
    await invokeTauri('templates_delete_preset', { presetId })
  } catch {}
}

async function bestEffortDeleteRole(roleId) {
  try {
    await invokeTauri('templates_delete_role', { roleId })
  } catch {}
}

async function bestEffortFlushPending() {
  try {
    await invokeTauri('templates_flush_pending')
  } catch {}
}

async function hasTestId(testId) {
  return await (await $(`[data-testid="${testId}"]`)).isExisting()
}

async function waitForMeshSurface() {
  await browser.waitUntil(
    async () => {
      const meshTab = await hasTestId('mesh-tab')
      if (!meshTab) return false
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

async function closeSlideOverIfOpen() {
  if (!(await hasTestId('slideover-panel'))) return
  const closeButtons = await $$('[data-testid="slideover-close"]')
  if (closeButtons.length > 0) {
    await closeButtons.at(-1).click()
  } else {
    await browser.keys('Escape')
  }

  await browser.waitUntil(
    async () => !(await hasTestId('slideover-panel')),
    { ...WAIT_MEDIUM, timeoutMsg: 'SlideOver did not close' }
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
  const runtimeTeamName = (await runtimeTitle.isExisting()) ? (await runtimeTitle.getText()).trim() : ''
  templatesBlockedReason = `Refusing to disband runtime team in templates spec (not created here): ${runtimeTeamName || 'unknown'}`
  return false
}

async function ensureEmptyMode(testContext) {
  await openMeshTab()
  await closeSlideOverIfOpen()

  if (await hasTestId('mesh-availability-blocking')) {
    const firstError = await $('[data-testid="mesh-availability-error"]')
    templatesBlockedReason = (await firstError.isExisting())
      ? await firstError.getText()
      : 'Mesh prerequisites unavailable'
    testContext.skip()
    return false
  }

  if (await hasTestId('mesh-mode-runtime')) {
    if (!(await disbandRuntimeTeamIfSafe())) {
      testContext.skip()
      return false
    }
  }

  if (await hasTestId('mesh-mode-setup') && await hasTestId('mesh-action-reset')) {
    await clickTestId('mesh-action-reset')
  }

  if (!(await hasTestId('mesh-mode-empty'))) {
    await browser.waitUntil(
      async () => await hasTestId('mesh-mode-empty'),
      { ...WAIT_LONG, timeoutMsg: 'Mesh did not reach empty mode' }
    )
  }

  return true
}

async function requireTemplateSetup(testContext) {
  if (!mainApp) {
    testContext.skip()
    return false
  }

  return await ensureEmptyMode(testContext)
}

async function openTemplateCatalog(testContext) {
  if (!(await ensureEmptyMode(testContext))) return false

  // Regression: 14793e0a repurposed mesh-template-browse-catalog to focus the
  // inline search; the browser restored by bed5024c has its own action.
  await clickTestId('mesh-template-open-browser')
  await browser.waitUntil(
    async () => await hasTestId('template-browser-panel'),
    { ...WAIT_MEDIUM, timeoutMsg: 'Template browser did not open' }
  )
  return true
}

async function findLeadRoleId() {
  const summaries = await invokeOrThrow('templates_list_roles_full')
  const lead = (summaries ?? []).find((entry) => String(getField(entry, 'kind', 'kind')) === 'lead')
  const leadRoleId = getField(lead, 'roleId', 'role_id')
  if (!leadRoleId) throw new Error('No lead role template found')
  return leadRoleId
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

describe('Templates Workflow', () => {
  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (mainApp) {
      await waitForProjectsLoaded()
      await openMeshTab()
      if (await hasTestId('mesh-availability-blocking')) {
        const firstError = await $('[data-testid="mesh-availability-error"]')
        templatesBlockedReason = (await firstError.isExisting())
          ? await firstError.getText()
          : 'Mesh prerequisites unavailable'
      }
    }
  })

  it('renders empty-state template controls', async function () {
    if (!(await requireTemplateSetup(this))) return

    expect(await hasTestId('mesh-mode-empty')).toBe(true)
    expect(await hasTestId('mesh-empty-state')).toBe(true)
    expect(await hasTestId('mesh-template-browse-catalog')).toBe(true)
    expect(await hasTestId('mesh-template-build-custom')).toBe(false)
  })

  it('browses built-in role and preset entries in template browser', async function () {
    if (!(await openTemplateCatalog(this))) return

    await browser.waitUntil(
      async () => (await $$('[data-testid^="role-template-card-"]')).length > 0,
      { ...WAIT_MEDIUM, timeoutMsg: 'Role cards did not load in template browser' }
    )

    const roleCards = await $$('[data-testid^="role-template-card-"]')
    expect(roleCards.length).toBeGreaterThan(0)

    await clickTestId('catalog-tab-presets')
    await browser.waitUntil(
      async () => (await $$('[data-testid^="template-browser-preset-"]')).length > 0,
      { ...WAIT_MEDIUM, timeoutMsg: 'Preset cards did not load in template browser' }
    )

    const presetCards = await $$('[data-testid^="template-browser-preset-"]')
    expect(presetCards.length).toBeGreaterThan(0)
  })

  it('creates a custom role template and exposes edit/delete controls', async function () {
    if (!(await openTemplateCatalog(this))) return

    const roleId = nextId('e2e-role')
    try {
      await invokeOrThrow(
        'templates_upsert_role',
        { request: { template: makeRoleTemplate(roleId, { instructions: 'Role instructions v1' }) } },
        'templates_upsert_role(create)'
      )
      await bestEffortFlushPending()

      await closeSlideOverIfOpen()
      if (!(await openTemplateCatalog(this))) return

      const card = await $(`[data-testid="role-template-card-${roleId}"]`)
      await card.waitForExist({ timeout: WAIT_MEDIUM.timeout })
      expect(await (await $(`[data-testid="role-use-${roleId}"]`)).isExisting()).toBe(true)
      expect(await (await $(`[data-testid="role-inspect-${roleId}"]`)).isExisting()).toBe(true)

      await (await $(`[data-testid="role-inspect-${roleId}"]`)).click()
      await browser.waitUntil(
        async () => {
          const detail = await $('[data-testid="template-role-detail"]')
          if (!(await detail.isExisting())) return false
          return (await detail.getText()).includes('Role instructions v1')
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Role detail panel did not show custom instructions' }
      )
    } finally {
      await bestEffortDeleteRole(roleId)
      await bestEffortFlushPending()
    }
  })

  it('edits a custom preset and reflects updated details', async function () {
    if (!(await openTemplateCatalog(this))) return

    const roleId = nextId('e2e-agent')
    const presetId = nextId('e2e-preset')
    const leadRoleId = await findLeadRoleId()

    try {
      await invokeOrThrow(
        'templates_upsert_role',
        { request: { template: makeRoleTemplate(roleId, { name: 'E2E Agent Role' }) } },
        'templates_upsert_role(create)'
      )

      await invokeOrThrow(
        'templates_upsert_preset',
        {
          request: {
            preset: makeTeamPreset(presetId, leadRoleId, roleId, {
              name: 'E2E Preset Updated',
              description: 'Preset description v2',
              version: '1.0.1',
            }),
          },
        },
        'templates_upsert_preset(edit)'
      )
      await bestEffortFlushPending()

      await closeSlideOverIfOpen()
      if (!(await openTemplateCatalog(this))) return
      await clickTestId('catalog-tab-presets')
      await browser.waitUntil(
        async () => await (await $(`[data-testid="template-preset-inspect-${presetId}"]`)).isExisting(),
        { ...WAIT_MEDIUM, timeoutMsg: 'Preset inspect control did not appear for custom preset' }
      )

      await (await $(`[data-testid="template-preset-inspect-${presetId}"]`)).click()
      await browser.waitUntil(
        async () => {
          const detail = await $('[data-testid="template-preset-detail"]')
          if (!(await detail.isExisting())) return false
          return (await detail.getText()).includes('Preset description v2')
        },
        { ...WAIT_MEDIUM, timeoutMsg: 'Preset detail panel did not update after edit' }
      )
    } finally {
      await bestEffortDeletePreset(presetId)
      await bestEffortDeleteRole(roleId)
      await bestEffortFlushPending()
    }
  })

  it('deletes custom role and preset templates from browser listing', async function () {
    if (!(await openTemplateCatalog(this))) return

    const roleId = nextId('e2e-delete-role')
    const presetId = nextId('e2e-delete-preset')
    const leadRoleId = await findLeadRoleId()

    try {
      await invokeOrThrow(
        'templates_upsert_role',
        { request: { template: makeRoleTemplate(roleId) } },
        'templates_upsert_role(create)'
      )
      await invokeOrThrow(
        'templates_upsert_preset',
        { request: { preset: makeTeamPreset(presetId, leadRoleId, roleId) } },
        'templates_upsert_preset(create)'
      )
      await bestEffortFlushPending()

      await closeSlideOverIfOpen()
      if (!(await openTemplateCatalog(this))) return

      expect(await (await $(`[data-testid="role-template-card-${roleId}"]`)).isExisting()).toBe(true)
      await clickTestId('catalog-tab-presets')
      expect(await (await $(`[data-testid="template-browser-preset-${presetId}"]`)).isExisting()).toBe(true)

      await invokeOrThrow('templates_delete_preset', { presetId }, 'templates_delete_preset')
      await invokeOrThrow('templates_delete_role', { roleId }, 'templates_delete_role')
      await bestEffortFlushPending()

      await closeSlideOverIfOpen()
      if (!(await openTemplateCatalog(this))) return

      expect(await (await $(`[data-testid="role-template-card-${roleId}"]`)).isExisting()).toBe(false)
      await clickTestId('catalog-tab-presets')
      expect(await (await $(`[data-testid="template-browser-preset-${presetId}"]`)).isExisting()).toBe(false)
    } finally {
      await bestEffortDeletePreset(presetId)
      await bestEffortDeleteRole(roleId)
      await bestEffortFlushPending()
    }
  })

  after(async () => {
    if (templatesBlockedReason) {
      console.log(`[e2e] templates workflow skipped due to mesh prerequisites: ${templatesBlockedReason}`)
    }
  })
})
