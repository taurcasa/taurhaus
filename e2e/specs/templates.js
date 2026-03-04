/**
 * Template workflow e2e tests.
 *
 * Covers:
 * - Mesh setup template picker + quick presets + blank-slate fallback
 * - Template catalog browse + composition apply back into setup
 * - Team composer validation errors
 * - Custom template CRUD via backend IPC with UI verification
 */

import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { waitForAppReady, ensureMainApp } from '../helpers.js'
import { waitForProjectsLoaded, clickTestId } from '../helpers/navigation.js'
import { WAIT_SHORT, WAIT_MEDIUM, WAIT_LONG, WAIT_XLONG } from '../helpers/timing.js'

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

async function waitForMeshSurface() {
  await browser.waitUntil(
    async () => {
      const setup = await $('[data-testid="mesh-setup-form"]')
      const runtime = await $('[data-testid="mesh-team-roster"]')
      const blocking = await $('[data-testid="mesh-availability-blocking"]')
      const loading = await $('[data-testid="mesh-loading"]')
      const error = await $('[data-testid="mesh-error"]')
      return (
        (await setup.isExisting()) ||
        (await runtime.isExisting()) ||
        (await blocking.isExisting()) ||
        (await loading.isExisting()) ||
        (await error.isExisting())
      )
    },
    { ...WAIT_MEDIUM, timeoutMsg: 'Mesh tab surface did not render' }
  )
}

async function openMeshTab() {
  await clickTestId('tab-mesh')
  await waitForMeshSurface()

  const loading = await $('[data-testid="mesh-loading"]')
  if (await loading.isExisting()) {
    await browser.waitUntil(
      async () => !(await (await $('[data-testid="mesh-loading"]')).isExisting()),
      { ...WAIT_LONG, timeoutMsg: 'Mesh tab did not finish loading' }
    )
  }
}

async function ensureSetupMode() {
  const setupTitle = await $('[data-testid="mesh-setup-title"]')
  if (await setupTitle.isExisting()) return

  const runtimeTitle = await $('[data-testid="mesh-runtime-title"]')
  if (!(await runtimeTitle.isExisting())) return

  const overflow = await $('[data-testid="mesh-overflow-menu-button"]')
  if (!(await overflow.isExisting())) return
  await overflow.click()

  const disband = await $('[data-testid="mesh-disband-button"]')
  await disband.waitForExist({ timeout: WAIT_MEDIUM.timeout })
  await disband.click()

  const confirm = await $('[data-testid="confirm-dialog-confirm"]')
  if (await confirm.isExisting()) {
    await confirm.click()
  }

  await browser.waitUntil(
    async () => await (await $('[data-testid="mesh-setup-title"]')).isExisting(),
    { ...WAIT_LONG, timeoutMsg: 'Mesh did not return to setup mode after disband' }
  )
}

async function requireTemplateSetup(testContext) {
  if (!mainApp) {
    testContext.skip()
    return false
  }

  await openMeshTab()

  const blocking = await $('[data-testid="mesh-availability-blocking"]')
  if (await blocking.isExisting()) {
    const firstError = await $('[data-testid="mesh-availability-error"]')
    templatesBlockedReason = (await firstError.isExisting())
      ? await firstError.getText()
      : 'Mesh prerequisites unavailable'
    testContext.skip()
    return false
  }

  await ensureSetupMode()

  const setupTitle = await $('[data-testid="mesh-setup-title"]')
  if (!(await setupTitle.isExisting())) {
    templatesBlockedReason = 'Mesh setup form unavailable'
    testContext.skip()
    return false
  }

  return true
}

async function openTemplateCatalog(refresh = true) {
  if (refresh) {
    await clickTestId('mesh-template-blank-slate')
  }
  await clickTestId('mesh-template-browse-catalog')
  await browser.waitUntil(
    async () => {
      const catalog = await $('[data-testid="template-catalog"]')
      if (!(await catalog.isExisting())) return false
      const loading = await $('[data-testid="template-catalog-loading"]')
      return !(await loading.isExisting())
    },
    { ...WAIT_MEDIUM, timeoutMsg: 'Template catalog did not load' }
  )
}

async function pickFirstQuickPreset() {
  const quickPresetButtons = await $$('[data-testid^="mesh-template-preset-"]')
  for (const button of quickPresetButtons) {
    if (!(await button.isExisting())) continue
    if (!(await button.isEnabled())) continue
    await button.click()
    return await button.getText()
  }
  return null
}

async function findLeadRoleId() {
  const summaries = await invokeOrThrow('templates_list_roles')
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
      model: 'gpt-5.3-codex',
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

function commitMessage(commit) {
  return String(getField(commit, 'message', 'message') ?? '')
}

function commitShortId(commit) {
  return String(getField(commit, 'shortId', 'short_id') ?? '')
}

function commitChangedPaths(commit) {
  const paths = getField(commit, 'changedPaths', 'changed_paths')
  return Array.isArray(paths) ? paths : []
}

function commitTouchesPath(commit, path) {
  return commitChangedPaths(commit).includes(path)
}

async function listTemplateHistory(limit = 100) {
  const page = await invokeOrThrow(
    'templates_get_history',
    { limit, cursor: null },
    'templates_get_history'
  )
  return Array.isArray(page?.commits) ? page.commits : []
}

async function waitForHistoryCommit(predicate, timeoutMsg) {
  let matched = null
  await browser.waitUntil(
    async () => {
      try {
        await bestEffortFlushPending()
        const commits = await listTemplateHistory(200)
        matched = commits.find((commit) => predicate(commit)) ?? null
        return Boolean(matched)
      } catch {
        return false
      }
    },
    { ...WAIT_LONG, timeoutMsg }
  )
  return matched
}

async function withTempTemplateFile(filename, payload, fn) {
  const directory = await mkdtemp(join(tmpdir(), 'taurhaus-templates-e2e-'))
  const templatePath = join(directory, filename)
  await writeFile(templatePath, payload, 'utf8')
  try {
    return await fn(templatePath)
  } finally {
    await rm(directory, { recursive: true, force: true })
  }
}

describe('Templates Workflow', () => {
  before(async () => {
    await waitForAppReady()
    mainApp = await ensureMainApp()
    if (mainApp) {
      await waitForProjectsLoaded()
      await openMeshTab()
      const blocking = await $('[data-testid="mesh-availability-blocking"]')
      if (await blocking.isExisting()) {
        const firstError = await $('[data-testid="mesh-availability-error"]')
        templatesBlockedReason = (await firstError.isExisting())
          ? await firstError.getText()
          : 'Mesh prerequisites unavailable'
      }
    }
  })

  it('renders setup template picker controls', async function () {
    if (!(await requireTemplateSetup(this))) return

    expect(await (await $('[data-testid="mesh-template-picker"]')).isExisting()).toBe(true)
    expect(await (await $('[data-testid="mesh-template-blank-slate"]')).isExisting()).toBe(true)
    expect(await (await $('[data-testid="mesh-template-browse-catalog"]')).isExisting()).toBe(true)
    expect(await (await $('[data-testid="mesh-template-build-custom"]')).isExisting()).toBe(true)
  })

  it('browses built-in entries in template catalog', async function () {
    if (!(await requireTemplateSetup(this))) return

    await openTemplateCatalog(true)

    const roleCards = await $$('[data-testid^="role-template-card-"]')
    const presetCards = await $$('[data-testid^="team-preset-card-"]')
    expect(roleCards.length).toBeGreaterThan(0)
    expect(presetCards.length).toBeGreaterThan(0)

    const readonlyRoles = await $$('[data-testid^="role-readonly-"]')
    const readonlyPresets = await $$('[data-testid^="preset-readonly-"]')
    expect(readonlyRoles.length).toBeGreaterThan(0)
    expect(readonlyPresets.length).toBeGreaterThan(0)
  })

  it('applies a quick preset to setup roster', async function () {
    if (!(await requireTemplateSetup(this))) return

    const presetLabel = await pickFirstQuickPreset()
    if (!presetLabel) return this.skip()

    await browser.waitUntil(
      async () => {
        const notice = await $('[data-testid="mesh-template-notice"]')
        if ((await notice.isExisting()) && (await notice.getText()).includes('Applied preset')) return true
        const error = await $('[data-testid="mesh-template-error"]')
        return await error.isExisting()
      },
      { ...WAIT_XLONG, timeoutMsg: 'Quick preset application notice did not appear' }
    )

    const templateError = await $('[data-testid="mesh-template-error"]')
    if (await templateError.isExisting()) {
      throw new Error(`Preset application failed: ${await templateError.getText()}`)
    }

    const notice = await $('[data-testid="mesh-template-notice"]')
    expect(await notice.getText()).toContain('Applied preset')
    expect(await notice.getText()).toContain(presetLabel)

    const agentCards = await $$('[data-testid="mesh-agent-card"]')
    expect(agentCards.length).toBeGreaterThan(0)
  })

  it('blank-slate fallback resets roster to default shape', async function () {
    if (!(await requireTemplateSetup(this))) return

    const presetLabel = await pickFirstQuickPreset()
    if (!presetLabel) return this.skip()

    await browser.waitUntil(
      async () => {
        const notice = await $('[data-testid="mesh-template-notice"]')
        if ((await notice.isExisting()) && (await notice.getText()).includes('Applied preset')) return true
        const error = await $('[data-testid="mesh-template-error"]')
        return await error.isExisting()
      },
      { ...WAIT_XLONG, timeoutMsg: 'Preset notice did not appear before blank-slate reset' }
    )

    const templateError = await $('[data-testid="mesh-template-error"]')
    if (await templateError.isExisting()) {
      throw new Error(`Preset application failed before blank-slate reset: ${await templateError.getText()}`)
    }

    await clickTestId('mesh-add-agent-button')
    const expandedCards = await $$('[data-testid="mesh-agent-card"]')
    expect(expandedCards.length).toBeGreaterThan(1)

    await clickTestId('mesh-template-blank-slate')
    await browser.waitUntil(
      async () => (await $$('[data-testid="mesh-agent-card"]')).length === 1,
      { ...WAIT_SHORT, timeoutMsg: 'Blank slate did not reset roster to one agent row' }
    )
  })

  it('applies catalog composition back into mesh setup', async function () {
    if (!(await requireTemplateSetup(this))) return

    await openTemplateCatalog(true)

    const previewButtons = await $$('[data-testid^="preset-preview-"]')
    if (previewButtons.length === 0) return this.skip()

    await previewButtons[0].click()

    await browser.waitUntil(
      async () => await (await $('[data-testid="team-composer"]')).isExisting(),
      { ...WAIT_MEDIUM, timeoutMsg: 'Team composer did not open from preset preview' }
    )

    await browser.waitUntil(
      async () => {
        const apply = await $('[data-testid="composer-apply"]')
        if (await apply.isEnabled()) return true
        const errors = await $('[data-testid="composer-validation-errors"]')
        return await errors.isExisting()
      },
      { ...WAIT_XLONG, timeoutMsg: 'Composer apply button did not become enabled' }
    )

    const composerErrors = await $('[data-testid="composer-validation-errors"]')
    if (await composerErrors.isExisting()) {
      throw new Error(`Composer validation blocked apply: ${await composerErrors.getText()}`)
    }

    await clickTestId('composer-apply')

    await browser.waitUntil(
      async () => {
        const composer = await $('[data-testid="team-composer"]')
        const notice = await $('[data-testid="mesh-template-notice"]')
        return (
          !(await composer.isExisting()) &&
          (await notice.isExisting()) &&
          (await notice.getText()).includes('Applied catalog composition')
        )
      },
      { ...WAIT_MEDIUM, timeoutMsg: 'Catalog composition did not apply back to setup' }
    )

    const agentCards = await $$('[data-testid="mesh-agent-card"]')
    expect(agentCards.length).toBeGreaterThan(0)
  })

  it('shows validation errors for name collisions in team composer', async function () {
    if (!(await requireTemplateSetup(this))) return

    await clickTestId('mesh-template-build-custom')
    await browser.waitUntil(
      async () => await (await $('[data-testid="team-composer"]')).isExisting(),
      { ...WAIT_MEDIUM, timeoutMsg: 'Team composer did not open from custom flow' }
    )

    const increaseButtons = await $$('[data-testid^="agent-increase-"]')
    if (increaseButtons.length === 0) return this.skip()
    await increaseButtons[0].click()

    await browser.waitUntil(
      async () => {
        const secondCard = await $('[data-testid="composer-roster-card-1"]')
        if (await secondCard.isExisting()) return true
        const errors = await $('[data-testid="composer-validation-errors"]')
        return await errors.isExisting()
      },
      { ...WAIT_XLONG, timeoutMsg: 'Second roster card did not appear after increasing agent count' }
    )

    const composerErrors = await $('[data-testid="composer-validation-errors"]')
    if (await composerErrors.isExisting()) {
      throw new Error(`Composer stayed invalid after adding agent role: ${await composerErrors.getText()}`)
    }

    const name0 = await $('[data-testid="composer-roster-name-0"]')
    const name1 = await $('[data-testid="composer-roster-name-1"]')
    await name0.clearValue()
    await name0.setValue('collision-name')
    await name1.clearValue()
    await name1.setValue('collision-name')

    await browser.waitUntil(
      async () => {
        const errors = await $('[data-testid="composer-validation-errors"]')
        return (await errors.isExisting()) && (await errors.getText()).includes('Name collisions')
      },
      { ...WAIT_MEDIUM, timeoutMsg: 'Name collision validation error did not appear' }
    )

    const apply = await $('[data-testid="composer-apply"]')
    expect(await apply.isEnabled()).toBe(false)
  })

  it('creates a custom role template and exposes edit/delete controls', async function () {
    if (!(await requireTemplateSetup(this))) return

    const roleId = nextId('e2e-role')
    try {
      await invokeOrThrow(
        'templates_upsert_role',
        { request: { template: makeRoleTemplate(roleId, { instructions: 'Role instructions v1' }) } },
        'templates_upsert_role(create)'
      )
      await bestEffortFlushPending()

      await openTemplateCatalog(true)

      const card = await $(`[data-testid="role-template-card-${roleId}"]`)
      await card.waitForExist({ timeout: WAIT_MEDIUM.timeout })
      expect(await (await $(`[data-testid="role-edit-${roleId}"]`)).isExisting()).toBe(true)
      expect(await (await $(`[data-testid="role-delete-${roleId}"]`)).isExisting()).toBe(true)

      await (await $(`[data-testid="role-inspect-${roleId}"]`)).click()
      await browser.waitUntil(
        async () => (await (await $('[data-testid="template-detail-panel"]')).getText()).includes('Role instructions v1'),
        { ...WAIT_MEDIUM, timeoutMsg: 'Role details panel did not show custom instructions' }
      )
    } finally {
      await bestEffortDeleteRole(roleId)
      await bestEffortFlushPending()
    }
  })

  it('edits a custom preset and reflects updated details', async function () {
    if (!(await requireTemplateSetup(this))) return

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
              name: 'E2E Preset',
              description: 'Preset description v1',
              version: '1.0.0',
            }),
          },
        },
        'templates_upsert_preset(create)'
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

      await openTemplateCatalog(true)
      await (await $(`[data-testid="preset-inspect-${presetId}"]`)).click()
      await browser.waitUntil(
        async () => (await (await $('[data-testid="template-detail-panel"]')).getText()).includes('Preset description v2'),
        { ...WAIT_MEDIUM, timeoutMsg: 'Preset detail panel did not update after edit' }
      )

      await (await $(`[data-testid="preset-preview-${presetId}"]`)).click()
      await browser.waitUntil(
        async () => await (await $('[data-testid="team-composer"]')).isExisting(),
        { ...WAIT_MEDIUM, timeoutMsg: 'Preset preview did not open team composer' }
      )
      expect(await (await $('[data-testid="composer-lead-select"]')).getValue()).toBe(leadRoleId)
    } finally {
      await bestEffortDeletePreset(presetId)
      await bestEffortDeleteRole(roleId)
      await bestEffortFlushPending()
    }
  })

  it('deletes custom role and preset templates from catalog', async function () {
    if (!(await requireTemplateSetup(this))) return

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

      await openTemplateCatalog(true)
      expect(await (await $(`[data-testid="role-template-card-${roleId}"]`)).isExisting()).toBe(true)
      expect(await (await $(`[data-testid="team-preset-card-${presetId}"]`)).isExisting()).toBe(true)

      await invokeOrThrow('templates_delete_preset', { presetId }, 'templates_delete_preset')
      await invokeOrThrow('templates_delete_role', { roleId }, 'templates_delete_role')
      await bestEffortFlushPending()

      await openTemplateCatalog(true)
      expect(await (await $(`[data-testid="role-template-card-${roleId}"]`)).isExisting()).toBe(false)
      expect(await (await $(`[data-testid="team-preset-card-${presetId}"]`)).isExisting()).toBe(false)
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
