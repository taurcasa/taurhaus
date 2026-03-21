import { BEHAVIORAL_CONTRACT_MODES, normalizeBehavioralContract } from './normalize.js'
import { defaultModelForTool } from '../meshDefaults.js'

function optionalTrimmedString(value) {
  const normalized = String(value ?? '').trim()
  return normalized.length > 0 ? normalized : null
}

function optionalTrimmedStringList(value) {
  if (!Array.isArray(value)) return null
  const normalized = value.map((entry) => String(entry ?? '').trim()).filter(Boolean)
  return normalized.length > 0 ? normalized : null
}

export function normalizeRoleTemplateInput(roleData) {
  const source =
    roleData && typeof roleData === 'object' && roleData.template
      ? roleData.template
      : roleData

  if (!source || typeof source !== 'object') {
    return source
  }

  const roleKind = String(source.kind ?? 'agent').toLowerCase() === 'lead' ? 'lead' : 'agent'
  const explicitCliTool = source.tool ?? source.cliTool ?? source.defaults?.cliTool ?? source.defaults?.cli_tool
  const cliTool = String(
    explicitCliTool ??
      (roleKind === 'lead' ? '' : 'codex')
  ).toLowerCase()
  const explicitModel = source.model ?? source.defaults?.model
  const model = String(
    explicitModel ??
      (roleKind === 'lead'
        ? ''
        : defaultModelForTool(cliTool || 'codex'))
  )
  const roleId = String(source.roleId ?? '').trim()
  const capabilities = Array.isArray(source.capabilities)
    ? source.capabilities.map((capability) => String(capability ?? '').trim()).filter(Boolean)
    : []

  const constraints = source.constraints ?? {}
  const minRaw = Number(constraints.minInstances ?? (roleKind === 'lead' ? 1 : 0))
  const maxRaw = Number(constraints.maxInstances ?? (roleKind === 'lead' ? 1 : 8))
  const minInstances = Number.isFinite(minRaw) ? Math.max(0, Math.floor(minRaw)) : (roleKind === 'lead' ? 1 : 0)
  const maxInstances = Number.isFinite(maxRaw) ? Math.max(1, Math.floor(maxRaw)) : (roleKind === 'lead' ? 1 : 8)

  return {
    schema: {
      kind: 'role_template',
      version: Number(source.schema?.version ?? 1) || 1,
    },
    roleId,
    name: String(source.name ?? '').trim(),
    version: String(source.version ?? '1.0.0'),
    kind: roleKind,
    defaults: {
      cliTool,
      model,
      defaultNamePattern: String(
        source.defaults?.defaultNamePattern ??
          (roleKind === 'lead' ? 'team-lead' : `${roleId || 'agent'}-{n}`)
      ),
    },
    instructions: String(source.instructions ?? '').trim(),
    focusArea: optionalTrimmedString(source.focusArea ?? source.focus_area),
    contextSummary: optionalTrimmedString(source.contextSummary ?? source.context_summary),
    behaviorSummary: optionalTrimmedString(source.behaviorSummary ?? source.behavior_summary),
    communicationStyle: optionalTrimmedString(source.communicationStyle ?? source.communication_style),
    runtimeCompactSummary: source.runtimeCompactSummary ?? source.runtime_compact_summary ?? null,
    behavioralContract: normalizeBehavioralContract(source.behavioralContract, {
      mode: BEHAVIORAL_CONTRACT_MODES.TEMPLATE_INPUT,
    }),
    qualityGates: optionalTrimmedStringList(source.qualityGates ?? source.quality_gates),
    definitionOfDone: optionalTrimmedStringList(source.definitionOfDone ?? source.definition_of_done),
    phaseScope: optionalTrimmedStringList(source.phaseScope ?? source.phase_scope),
    mode: optionalTrimmedString(source.mode),
    inheritsFrom: optionalTrimmedString(source.inheritsFrom ?? source.inherits_from),
    requiredArtifacts: optionalTrimmedStringList(source.requiredArtifacts ?? source.required_artifacts),
    capabilities,
    provenance: source.provenance ?? null,
    constraints: {
      minInstances: roleKind === 'lead' ? 1 : minInstances,
      maxInstances: roleKind === 'lead' ? 1 : Math.max(maxInstances, minInstances),
      requiresLeadTool: constraints.requiresLeadTool ?? null,
      allowedProjectBinding: constraints.allowedProjectBinding ?? 'lead_project',
    },
  }
}

export function normalizeTeamPresetInput(presetData) {
  const source =
    presetData && typeof presetData === 'object' && presetData.preset
      ? presetData.preset
      : presetData

  if (!source || typeof source !== 'object') {
    return source
  }

  const rawSlots = Array.isArray(source.agentSlots) ? source.agentSlots : []
  const agentSlots = rawSlots.map((slot) => ({
    roleId: String(slot?.roleId ?? '').trim(),
    count: Math.max(1, Number(slot?.count ?? 1) || 1),
    projectBinding: slot?.projectBinding ?? 'lead_project',
    projectId: slot?.projectId ?? null,
    overrides: slot?.overrides ?? null,
  }))

  return {
    schema: {
      kind: 'team_preset',
      version: Number(source.schema?.version ?? 1) || 1,
    },
    presetId: String(source.presetId ?? '').trim(),
    name: String(source.name ?? '').trim(),
    description: String(source.description ?? '').trim(),
    version: String(source.version ?? '1.0.0'),
    leadRoleId: String(source.leadRoleId ?? '').trim(),
    agentSlots,
    defaults: {
      teamNamePattern: String(source.defaults?.teamNamePattern ?? '{project}-team'),
      tmuxLayout: String(source.defaults?.tmuxLayout ?? 'tiled'),
    },
  }
}

export function normalizeComposeTeamRequest(request) {
  const normalizedAgentSlots = (request?.agentSlots ?? []).map((slot) => ({
    roleId: slot?.roleId ?? '',
    count: Number(slot?.count ?? 0),
    projectBinding: slot?.projectBinding ?? 'lead_project',
    projectId: slot?.projectId ?? null,
    overrides: slot?.overrides ?? null,
  }))

  return {
    leadRoleId: request?.leadRoleId ?? '',
    agentSlots: normalizedAgentSlots,
    overrides: {
      ...(request?.overrides ?? {}),
      ...(request?.projectName ? { projectName: request.projectName } : {}),
    },
  }
}
