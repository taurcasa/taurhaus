import { BEHAVIORAL_CONTRACT_MODES, normalizeBehavioralContract } from './normalize.js'
import { parseLegacyModel } from '../modelCatalog.js'

function normalizeStringList(value) {
  if (!Array.isArray(value)) return []
  return value.map((entry) => String(entry ?? '').trim()).filter(Boolean)
}

function normalizeOptionalStringList(value) {
  const normalized = normalizeStringList(value)
  return normalized.length > 0 ? normalized : null
}

function normalizeCapabilityPolicy(value) {
  if (!value || typeof value !== 'object') return null
  return withoutAliases({
    ...value,
    modelSelection: value.modelSelection ?? value.model_selection ?? null,
    minimumCapability: value.minimumCapability ?? value.minimum_capability ?? null,
    allowedModels: normalizeStringList(value.allowedModels ?? value.allowed_models),
    effortBand: normalizeStringList(value.effortBand ?? value.effort_band),
  }, ['model_selection', 'minimum_capability', 'allowed_models', 'effort_band'])
}

function optionalTrimmed(value) {
  const normalized = String(value ?? '').trim()
  return normalized.length > 0 ? normalized : null
}

function withoutAliases(value, aliases) {
  for (const alias of aliases) delete value[alias]
  return value
}

/**
 * Splits the canonical model/effort pair out of a response. Stores written
 * before PR 5a still hold the combined form ("gpt-5.4 high"); an explicitly
 * declared effort always wins over the one folded into the model string.
 */
function normalizeModelFields(value) {
  const parsed = parseLegacyModel(value?.model)
  return withoutAliases({
    ...(value && typeof value === 'object' ? value : {}),
    model: value?.model == null ? null : parsed.model,
    reasoningEffort:
      optionalTrimmed(value?.reasoningEffort ?? value?.reasoning_effort) ??
      parsed.reasoningEffort,
  }, ['reasoning_effort'])
}

function normalizeTemplateDefaults(value) {
  if (!value || typeof value !== 'object') return null

  const modelFields = normalizeModelFields(value)
  return withoutAliases({
    ...value,
    cliTool: value.cliTool ?? value.cli_tool ?? null,
    model: modelFields.model,
    reasoningEffort: modelFields.reasoningEffort,
    defaultNamePattern: value.defaultNamePattern ?? value.default_name_pattern ?? null,
  }, ['cli_tool', 'reasoning_effort', 'default_name_pattern'])
}

function normalizeSlotOverrides(value) {
  if (!value || typeof value !== 'object') return null

  const modelFields = normalizeModelFields(value)
  return withoutAliases({
    ...value,
    namePattern: value.namePattern ?? value.name_pattern ?? null,
    model: modelFields.model,
    reasoningEffort: modelFields.reasoningEffort,
    instructionsReplace: value.instructionsReplace ?? value.instructions_replace ?? null,
    instructionsAppend: value.instructionsAppend ?? value.instructions_append ?? null,
    focusArea: value.focusArea ?? value.focus_area ?? null,
    contextSummary: value.contextSummary ?? value.context_summary ?? null,
    behaviorSummary: value.behaviorSummary ?? value.behavior_summary ?? null,
    runtimeCompactSummary: value.runtimeCompactSummary ?? value.runtime_compact_summary ?? null,
    behavioralContractAppend:
      value.behavioralContractAppend ?? value.behavioral_contract_append ?? null,
  }, [
    'name_pattern',
    'reasoning_effort',
    'instructions_replace',
    'instructions_append',
    'focus_area',
    'context_summary',
    'behavior_summary',
    'runtime_compact_summary',
    'behavioral_contract_append',
  ])
}

function normalizeAgentSlot(value) {
  if (!value || typeof value !== 'object') return null

  return withoutAliases({
    ...value,
    roleId: value.roleId ?? value.role_id ?? null,
    count: Math.max(0, Number(value.count ?? 0)),
    projectBinding: value.projectBinding ?? value.project_binding ?? 'lead_project',
    projectId: value.projectId ?? value.project_id ?? null,
    overrides: normalizeSlotOverrides(value.overrides),
  }, ['role_id', 'project_binding', 'project_id'])
}

function normalizeComposeRosterMember(value) {
  if (!value || typeof value !== 'object') return null

  const normalized = {
    ...value,
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

  const modelFields = normalizeModelFields(value)
  if (modelFields.model) normalized.model = modelFields.model
  // `ResolvedMember` carries the effort next to the model (composition.rs); an
  // edited preset detaches this roster, so dropping it here loses the per-role
  // effort on initialize.
  if (modelFields.reasoningEffort) normalized.reasoningEffort = modelFields.reasoningEffort

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

  for (const alias of [
    'role_id',
    'role_kind',
    'cli_tool',
    'reasoning_effort',
    'focus_area',
    'context_summary',
    'behavior_summary',
    'project_binding',
    'project_id',
    'behavioral_contract',
  ]) {
    delete normalized[alias]
  }

  return normalized
}

export function normalizeRoleTemplateResponse(value) {
  if (!value || typeof value !== 'object') return value

  const builtIn =
    String(value.source ?? '').toLowerCase() === 'built_in' ||
    Boolean(value.builtIn ?? value.built_in)
  // The model and the effort are lifted together: consumers that read the role
  // as a flat member (runtime hot-add) must not have to reach into `defaults`.
  const roleModelFields = normalizeModelFields({
    model: value.model ?? value.defaults?.model,
    reasoningEffort:
      value.reasoningEffort ??
      value.reasoning_effort ??
      value.defaults?.reasoningEffort ??
      value.defaults?.reasoning_effort,
  })

  return withoutAliases({
    ...value,
    schema: value.schema ?? null,
    roleId: value.roleId ?? value.role_id ?? '',
    name: value.name ?? '',
    version: value.version ?? null,
    kind: String(value.kind ?? 'agent').toLowerCase(),
    cliTool: value.cliTool ?? value.cli_tool ?? value.defaults?.cliTool ?? value.defaults?.cli_tool ?? null,
    model: roleModelFields.model,
    reasoningEffort: roleModelFields.reasoningEffort,
    capabilityPolicy: normalizeCapabilityPolicy(
      value.capabilityPolicy ?? value.capability_policy
    ),
    focusArea: value.focusArea ?? value.focus_area ?? '',
    contextSummary: value.contextSummary ?? value.context_summary ?? '',
    behaviorSummary: value.behaviorSummary ?? value.behavior_summary ?? '',
    communicationStyle: value.communicationStyle ?? value.communication_style ?? '',
    runtimeCompactSummary: value.runtimeCompactSummary ?? value.runtime_compact_summary ?? null,
    instructions: value.instructions ?? '',
    behavioralContract: normalizeBehavioralContract(
      value.behavioralContract ?? value.behavioral_contract,
      { mode: BEHAVIORAL_CONTRACT_MODES.OPTIONAL_OBJECT }
    ),
    qualityGates: normalizeOptionalStringList(value.qualityGates ?? value.quality_gates),
    handoffExpectations: normalizeOptionalStringList(
      value.handoffExpectations ?? value.handoff_expectations
    ),
    definitionOfDone: normalizeOptionalStringList(value.definitionOfDone ?? value.definition_of_done),
    phaseScope: normalizeOptionalStringList(value.phaseScope ?? value.phase_scope),
    mode: value.mode ?? null,
    inheritsFrom: value.inheritsFrom ?? value.inherits_from ?? null,
    requiredArtifacts: normalizeOptionalStringList(value.requiredArtifacts ?? value.required_artifacts),
    capabilities: Array.isArray(value.capabilities) ? value.capabilities : [],
    defaults: normalizeTemplateDefaults(value.defaults),
    provenance: value.provenance ?? null,
    constraints: value.constraints ?? null,
    source: value.source ?? null,
    builtIn,
    readOnly: Boolean(value.readOnly ?? value.read_only),
  }, [
    'role_id',
    'cli_tool',
    'reasoning_effort',
    'capability_policy',
    'focus_area',
    'context_summary',
    'behavior_summary',
    'communication_style',
    'runtime_compact_summary',
    'behavioral_contract',
    'quality_gates',
    'handoff_expectations',
    'definition_of_done',
    'phase_scope',
    'inherits_from',
    'required_artifacts',
    'built_in',
    'read_only',
  ])
}

export function normalizeTeamPresetResponse(value) {
  if (!value || typeof value !== 'object') return value

  const agentSlots = Array.isArray(value.agentSlots ?? value.agent_slots)
    ? (value.agentSlots ?? value.agent_slots).map((slot) => normalizeAgentSlot(slot)).filter(Boolean)
    : []
  const builtIn =
    String(value.source ?? '').toLowerCase() === 'built_in' ||
    Boolean(value.builtIn ?? value.built_in)

  return withoutAliases({
    ...value,
    schema: value.schema ?? null,
    presetId: value.presetId ?? value.preset_id ?? '',
    name: value.name ?? '',
    description: value.description ?? '',
    version: value.version ?? null,
    leadRoleId: value.leadRoleId ?? value.lead_role_id ?? '',
    leadOverrides: normalizeSlotOverrides(value.leadOverrides ?? value.lead_overrides),
    agentSlots,
    defaults: value.defaults && typeof value.defaults === 'object'
      ? withoutAliases({
          ...value.defaults,
          teamNamePattern:
            value.defaults.teamNamePattern ?? value.defaults.team_name_pattern ?? '{project}-team',
          tmuxLayout: value.defaults.tmuxLayout ?? value.defaults.tmux_layout ?? 'tiled',
        }, ['team_name_pattern', 'tmux_layout'])
      : null,
    tools: Array.isArray(value.tools) ? value.tools : [],
    source: value.source ?? null,
    builtIn,
    readOnly: Boolean(value.readOnly ?? value.read_only),
  }, [
    'preset_id',
    'lead_role_id',
    'lead_overrides',
    'agent_slots',
    'built_in',
    'read_only',
  ])
}

export function normalizeComposeTeamResult(value) {
  if (!value || typeof value !== 'object') return value

  return withoutAliases({
    ...value,
    roster: Array.isArray(value.roster)
      ? value.roster.map((member) => normalizeComposeRosterMember(member)).filter(Boolean)
      : [],
    warnings: normalizeStringList(value.warnings),
    validationErrors: normalizeStringList(value.validationErrors ?? value.validation_errors),
  }, ['validation_errors'])
}
