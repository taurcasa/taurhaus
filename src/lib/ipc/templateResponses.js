import { BEHAVIORAL_CONTRACT_MODES, normalizeBehavioralContract } from './normalize.js'

function normalizeStringList(value) {
  if (!Array.isArray(value)) return []
  return value.map((entry) => String(entry ?? '').trim()).filter(Boolean)
}

function normalizeTemplateDefaults(value) {
  if (!value || typeof value !== 'object') return null

  return {
    cliTool: value.cliTool ?? value.cli_tool ?? null,
    model: value.model ?? null,
    defaultNamePattern: value.defaultNamePattern ?? value.default_name_pattern ?? null,
  }
}

function normalizeAgentSlot(value) {
  if (!value || typeof value !== 'object') return null

  return {
    roleId: value.roleId ?? value.role_id ?? null,
    count: Math.max(0, Number(value.count ?? 0)),
    projectBinding: value.projectBinding ?? value.project_binding ?? 'lead_project',
    projectId: value.projectId ?? value.project_id ?? null,
    overrides: value.overrides ?? null,
  }
}

function normalizeComposeRosterMember(value) {
  if (!value || typeof value !== 'object') return null

  const normalized = {
    name: String(value.name ?? '').trim(),
    roleId: value.roleId ?? value.role_id ?? null,
    roleKind: value.roleKind ?? value.role_kind ?? 'agent',
    cliTool: value.cliTool ?? value.cli_tool ?? '',
    focusArea: value.focusArea ?? value.focus_area ?? '',
    contextSummary: value.contextSummary ?? value.context_summary ?? '',
    behaviorSummary: value.behaviorSummary ?? value.behavior_summary ?? '',
    projectBinding: value.projectBinding ?? value.project_binding ?? 'lead_project',
    projectId: value.projectId ?? value.project_id ?? null,
  }

  const model = String(value.model ?? '').trim()
  if (model) normalized.model = model

  const instructions = value.instructions ?? ''
  if (instructions) normalized.instructions = instructions

  const behavioralContract = normalizeBehavioralContract(
    value.behavioralContract ?? value.behavioral_contract,
    { mode: BEHAVIORAL_CONTRACT_MODES.OPTIONAL_OBJECT }
  )
  if (behavioralContract) normalized.behavioralContract = behavioralContract

  if (Array.isArray(value.capabilities)) {
    normalized.capabilities = value.capabilities
  }

  return normalized
}

export function normalizeRoleTemplateResponse(value) {
  if (!value || typeof value !== 'object') return value

  const builtIn =
    String(value.source ?? '').toLowerCase() === 'built_in' ||
    Boolean(value.builtIn ?? value.built_in)

  return {
    schema: value.schema ?? null,
    roleId: value.roleId ?? value.role_id ?? '',
    name: value.name ?? '',
    version: value.version ?? null,
    kind: String(value.kind ?? 'agent').toLowerCase(),
    cliTool: value.cliTool ?? value.cli_tool ?? value.defaults?.cliTool ?? value.defaults?.cli_tool ?? null,
    model: value.model ?? value.defaults?.model ?? null,
    focusArea: value.focusArea ?? value.focus_area ?? '',
    contextSummary: value.contextSummary ?? value.context_summary ?? '',
    behaviorSummary: value.behaviorSummary ?? value.behavior_summary ?? '',
    instructions: value.instructions ?? '',
    behavioralContract: normalizeBehavioralContract(
      value.behavioralContract ?? value.behavioral_contract,
      { mode: BEHAVIORAL_CONTRACT_MODES.OPTIONAL_OBJECT }
    ),
    capabilities: Array.isArray(value.capabilities) ? value.capabilities : [],
    defaults: normalizeTemplateDefaults(value.defaults),
    provenance: value.provenance ?? null,
    constraints: value.constraints ?? null,
    source: value.source ?? null,
    builtIn,
    readOnly: Boolean(value.readOnly ?? value.read_only),
  }
}

export function normalizeTeamPresetResponse(value) {
  if (!value || typeof value !== 'object') return value

  const agentSlots = Array.isArray(value.agentSlots ?? value.agent_slots)
    ? (value.agentSlots ?? value.agent_slots).map((slot) => normalizeAgentSlot(slot)).filter(Boolean)
    : []
  const builtIn =
    String(value.source ?? '').toLowerCase() === 'built_in' ||
    Boolean(value.builtIn ?? value.built_in)

  return {
    schema: value.schema ?? null,
    presetId: value.presetId ?? value.preset_id ?? '',
    name: value.name ?? '',
    description: value.description ?? '',
    version: value.version ?? null,
    leadRoleId: value.leadRoleId ?? value.lead_role_id ?? '',
    agentSlots,
    defaults: value.defaults && typeof value.defaults === 'object'
      ? {
          teamNamePattern:
            value.defaults.teamNamePattern ?? value.defaults.team_name_pattern ?? '{project}-team',
          tmuxLayout: value.defaults.tmuxLayout ?? value.defaults.tmux_layout ?? 'tiled',
        }
      : null,
    tools: Array.isArray(value.tools) ? value.tools : [],
    source: value.source ?? null,
    builtIn,
    readOnly: Boolean(value.readOnly ?? value.read_only),
  }
}

export function normalizeComposeTeamResult(value) {
  if (!value || typeof value !== 'object') return value

  return {
    roster: Array.isArray(value.roster)
      ? value.roster.map((member) => normalizeComposeRosterMember(member)).filter(Boolean)
      : [],
    warnings: normalizeStringList(value.warnings),
    validationErrors: normalizeStringList(value.validationErrors ?? value.validation_errors),
  }
}
