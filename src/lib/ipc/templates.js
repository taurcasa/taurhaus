import {
  MOCK_ROLE_TEMPLATES,
  MOCK_TEAM_PRESETS,
  MOCK_TEMPLATE_DIFFS,
  MOCK_TEMPLATE_HISTORY,
  MOCK_TEMPLATE_STORAGE_STATUS,
  roleTemplateSummary,
  teamPresetSummary,
} from './mocks/index.js'
import { invokeOrMock } from './client.js'
import {
  normalizeComposeTeamRequest,
  normalizeRoleTemplateInput,
  normalizeTeamPresetInput,
} from './templatePayloads.js'

export async function listRoleTemplates() {
  const templates = await invokeOrMock('templates_list_roles_full', undefined, () =>
    MOCK_ROLE_TEMPLATES.map(roleTemplateSummary)
  )

  return (templates ?? []).map((template) => ({
    ...template,
    roleId: template?.roleId ?? '',
    cliTool: template?.cliTool ?? template?.defaults?.cliTool ?? null,
    model: template?.model ?? template?.defaults?.model ?? null,
    capabilities: Array.isArray(template?.capabilities) ? template.capabilities : [],
    builtIn: String(template?.source ?? '').toLowerCase() === 'built_in',
    readOnly: Boolean(template?.readOnly),
  }))
}

export function getRoleTemplate(id) {
  return invokeOrMock('templates_get_role', { roleId: id }, () => {
    const template = MOCK_ROLE_TEMPLATES.find((entry) => entry.roleId === id)
    return template ? { ...template } : null
  })
}

export function upsertRoleTemplate(roleData) {
  const template = normalizeRoleTemplateInput(roleData)

  return invokeOrMock('templates_upsert_role', { request: { template } }, () => ({
    roleId: template?.roleId ?? null,
    name: template?.name ?? '',
    kind: template?.kind ?? 'agent',
    builtIn: false,
    readOnly: false,
  }))
}

export function deleteRoleTemplate(roleId) {
  return invokeOrMock('templates_delete_role', { roleId }, () => ({
    roleId,
    deleted: true,
  }))
}

export async function listTeamPresets() {
  const presets = await invokeOrMock('templates_list_presets_full', undefined, () =>
    MOCK_TEAM_PRESETS.map(teamPresetSummary)
  )

  return (presets ?? []).map((preset) => {
    const leadRoleId = preset?.leadRoleId ?? ''
    const agentSlots = Array.isArray(preset?.agentSlots) ? preset.agentSlots : []

    return {
      ...preset,
      leadRoleId,
      roleCount: agentSlots.length,
      agentCount: agentSlots.reduce((total, slot) => total + (slot?.count ?? 0), 0),
      tools: Array.isArray(preset?.tools) ? preset.tools : [],
      capabilities: Array.isArray(preset?.capabilities) ? preset.capabilities : [],
      builtIn: String(preset?.source ?? '').toLowerCase() === 'built_in',
      readOnly: Boolean(preset?.readOnly),
    }
  })
}

export function getTeamPreset(id) {
  return invokeOrMock('templates_get_preset', { presetId: id }, () => {
    const preset = MOCK_TEAM_PRESETS.find((entry) => entry.presetId === id)
    return preset ? { ...preset } : null
  })
}

export function upsertTeamPreset(presetData) {
  const preset = normalizeTeamPresetInput(presetData)

  return invokeOrMock('templates_upsert_preset', { request: { preset } }, () => ({
    presetId: preset?.presetId ?? null,
    name: preset?.name ?? '',
    leadRoleId: preset?.leadRoleId ?? '',
    agentSlots: Array.isArray(preset?.agentSlots) ? preset.agentSlots : [],
    builtIn: false,
    readOnly: false,
  }))
}

export function deleteTeamPreset(presetId) {
  return invokeOrMock('templates_delete_preset', { presetId }, () => ({
    presetId,
    deleted: true,
  }))
}

export function composeTeam(request) {
  const normalizedRequest = normalizeComposeTeamRequest(request)

  return invokeOrMock('templates_compose_team', { request: normalizedRequest }, () => {
    const leadName = request?.projectName ? `lead-${request.projectName}` : 'lead-project'

    return {
      roster: [
        {
          name: leadName,
          roleId: 'claude-orchestrator',
          roleKind: 'lead',
          cliTool: 'claude',
          model: 'claude-opus-4-6',
          instructions: 'Coordinate execution and unblock the team.',
          behavioralContract: {
            communication: ['Acknowledge assignments quickly.'],
            execution: ['Delegate scoped tasks and verify completion evidence.'],
            escalation: ['Escalate blockers immediately.'],
          },
          capabilities: ['planning', 'coordination'],
          projectBinding: 'lead_project',
          projectId: null,
        },
      ],
      warnings: normalizedRequest.agentSlots.length ? [] : ['No agent slots selected; roster includes lead only.'],
      validationErrors: [],
    }
  }).catch((error) => {
    const message =
      error?.message ||
      (typeof error === 'string' ? error : '') ||
      (error ? JSON.stringify(error) : '') ||
      'templates_compose_team failed'

    throw new Error(message)
  })
}

export function getTemplateStorageStatus() {
  return invokeOrMock('templates_get_storage_status', undefined, () => ({
    ...MOCK_TEMPLATE_STORAGE_STATUS,
  }))
}

export function getTemplateHistory(limit = 50, cursor = null) {
  return invokeOrMock('templates_get_history', { limit, cursor }, () => {
    const page = (MOCK_TEMPLATE_HISTORY ?? []).slice(0, Math.max(1, Math.min(200, limit || 50)))
    return { commits: page, nextCursor: null }
  })
}

export function getTemplateDiff(commitId) {
  return invokeOrMock('templates_get_diff', { commitId }, () => {
    return (
      MOCK_TEMPLATE_DIFFS[commitId] ?? {
        commitId,
        files: [],
        stats: { filesChanged: 0, insertions: 0, deletions: 0 },
      }
    )
  })
}

export function revertTemplateVersion(id, commitHash) {
  return invokeOrMock('templates_revert', { request: { id, commitHash } }, () => undefined)
}
