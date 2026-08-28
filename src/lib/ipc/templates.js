import {
  MOCK_MODEL_CATALOG,
  MOCK_ROLE_TEMPLATES,
  MOCK_TEAM_PRESETS,
  MOCK_TEMPLATE_DIFFS,
  MOCK_TEMPLATE_HISTORY,
  MOCK_TEMPLATE_STORAGE_STATUS,
  mockRoleExportResult,
  roleTemplateSummary,
  teamPresetSummary,
} from './mocks/index.js'
import { invokeOrMock } from './client.js'
import { defaultModelFor } from '../modelCatalog.js'
import {
  normalizeComposeTeamRequest,
  normalizeRoleTemplateInput,
  normalizeTeamPresetInput,
} from './templatePayloads.js'
import {
  normalizeComposeTeamResult,
  normalizeRoleTemplateResponse,
  normalizeTeamPresetResponse,
} from './templateResponses.js'
import { formatUserFacingError } from '../format.js'

export async function listRoleTemplates() {
  const templates = await invokeOrMock('templates_list_roles_full', undefined, () =>
    MOCK_ROLE_TEMPLATES.map(roleTemplateSummary)
  )

  return (templates ?? []).map((template) => normalizeRoleTemplateResponse(template))
}

export function getRoleTemplate(id) {
  return invokeOrMock('templates_get_role', { roleId: id }, () => {
    const template = MOCK_ROLE_TEMPLATES.find((entry) => entry.roleId === id)
    return template ? { ...template } : null
  }).then((template) => normalizeRoleTemplateResponse(template))
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

export function importRoleFromFile(filePath) {
  return invokeOrMock('import_role_from_file', { request: { filePath } }, () => ({
    success: true,
    role: {
      roleId: 'imported-role',
      name: 'Imported Role',
      version: '1.0.0',
      kind: 'agent',
      defaults: {
        cliTool: 'claude',
        // Browser mode has no backend catalog; the mock one stands in for it so
        // the fallback never pins a model the catalog does not offer.
        model: defaultModelFor(MOCK_MODEL_CATALOG, 'claude'),
        defaultNamePattern: 'imported-role-{n}',
      },
      instructions: 'Imported role instructions.',
      focusArea: 'Imported work',
      contextSummary: 'Imported into the local template catalog.',
      behaviorSummary: 'Preserves imported prompt semantics until edited.',
      behavioralContract: {
        communication: ['Acknowledge imports clearly.'],
        execution: ['Preserve prompt intent when importing.'],
        escalation: ['Escalate malformed role imports.'],
      },
      capabilities: [],
      provenance: {
        sourceFormat: 'claude_agent',
        sourceVersion: null,
        sourcePath: filePath,
        importedAt: '2026-03-08T00:00:00Z',
        nonRoundtrippableFields: [],
      },
      constraints: {
        minInstances: 0,
        maxInstances: 8,
        requiresLeadTool: null,
        allowedProjectBinding: 'any',
      },
    },
    conflict: null,
  }))
}

export async function exportRoleToFile(roleId, targetFormat) {
  const exported = await invokeOrMock(
    'export_role_to_file',
    { request: { roleId, targetFormat } },
    () => mockRoleExportResult(roleId, targetFormat)
  )

  return {
    targetFormat: exported?.targetFormat ?? exported?.target_format ?? targetFormat,
    fileContent: exported?.fileContent ?? exported?.file_content ?? '',
    lossyFields: Array.isArray(exported?.lossyFields ?? exported?.lossy_fields)
      ? (exported?.lossyFields ?? exported?.lossy_fields)
      : [],
  }
}

/**
 * Write the Claude role templates into the project's `.claude/agents`
 * directory, where Claude Code resolves a subagent by name.
 */
export async function exportAgentDefinitions(projectId) {
  const result = await invokeOrMock('export_agent_definitions', { projectId }, () => ({
    // Browser mode has no project on disk, so nothing is written there.
    written: [],
    skipped: [],
  }))

  return {
    written: Array.isArray(result?.written) ? result.written : [],
    skipped: Array.isArray(result?.skipped) ? result.skipped : [],
  }
}

export async function listTeamPresets() {
  const presets = await invokeOrMock('templates_list_presets_full', undefined, () =>
    MOCK_TEAM_PRESETS.map(teamPresetSummary)
  )

  return (presets ?? []).map((preset) => {
    const normalized = normalizeTeamPresetResponse(preset)
    const agentSlots = Array.isArray(normalized?.agentSlots) ? normalized.agentSlots : []

    return {
      ...normalized,
      roleCount: agentSlots.length,
      agentCount: agentSlots.reduce((total, slot) => total + (slot?.count ?? 0), 0),
    }
  })
}

export function getTeamPreset(id) {
  return invokeOrMock('templates_get_preset', { presetId: id }, () => {
    const preset = MOCK_TEAM_PRESETS.find((entry) => entry.presetId === id)
    return preset ? { ...preset } : null
  }).then((preset) => normalizeTeamPresetResponse(preset))
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
    const leadRole = MOCK_ROLE_TEMPLATES.find((entry) => entry.roleId === normalizedRequest.leadRoleId) ?? null

    return {
      roster: [
        {
          name: leadName,
          roleId: normalizedRequest.leadRoleId || null,
          roleKind: 'lead',
          cliTool: leadRole?.cliTool ?? '',
          model: leadRole?.model ?? '',
          reasoningEffort: leadRole?.reasoningEffort ?? null,
          focusArea: leadRole?.focusArea ?? '',
          contextSummary: leadRole?.contextSummary ?? '',
          behaviorSummary: leadRole?.behaviorSummary ?? '',
          instructions: leadRole?.instructions ?? '',
          behavioralContract: leadRole?.behavioralContract ?? {
            communication: ['Acknowledge assignments quickly.'],
            execution: ['Delegate scoped tasks and verify completion evidence.'],
            escalation: ['Escalate blockers immediately.'],
          },
          projectBinding: 'lead_project',
          projectId: null,
        },
      ],
      warnings: normalizedRequest.agentSlots.length ? [] : ['No agent slots selected; roster includes lead only.'],
      validationErrors: [],
    }
  })
    .then((response) => normalizeComposeTeamResult(response))
    .catch((error) => {
    throw new Error(formatUserFacingError(error, "Couldn't prepare a team from these role selections"))
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
