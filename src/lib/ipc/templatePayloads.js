import { BEHAVIORAL_CONTRACT_MODES, normalizeBehavioralContract } from './normalize.js'
import { parseLegacyModel } from '../modelCatalog.js'

function optionalTrimmedString(value) {
  const normalized = String(value ?? '').trim()
  return normalized.length > 0 ? normalized : null
}

function optionalTrimmedStringList(value) {
  if (!Array.isArray(value)) return null
  const normalized = value.map((entry) => String(entry ?? '').trim()).filter(Boolean)
  return normalized.length > 0 ? normalized : null
}

function normalizeCapabilityPolicy(value) {
  if (!value || typeof value !== 'object') return null
  const normalized = {
    ...value,
    modelSelection: value.modelSelection ?? value.model_selection ?? 'fixed',
    minimumCapability: value.minimumCapability ?? value.minimum_capability ?? null,
    allowedModels: optionalTrimmedStringList(value.allowedModels ?? value.allowed_models) ?? [],
    effortBand: optionalTrimmedStringList(value.effortBand ?? value.effort_band) ?? [],
  }
  delete normalized.model_selection
  delete normalized.minimum_capability
  delete normalized.allowed_models
  delete normalized.effort_band
  return normalized
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
  // Canonical on-disk form is a bare model id plus `reasoning_effort`; legacy
  // combined strings ("gpt-5.4 high") are split on the way out. An unset model
  // stays unset — the backend applies its own catalog default.
  const explicitModel = parseLegacyModel(source.model ?? source.defaults?.model)
  const model = explicitModel.model
  const reasoningEffort =
    optionalTrimmedString(source.reasoningEffort ?? source.reasoning_effort) ??
    optionalTrimmedString(source.defaults?.reasoningEffort ?? source.defaults?.reasoning_effort) ??
    explicitModel.reasoningEffort
  const roleId = String(source.roleId ?? '').trim()
  const capabilities = Array.isArray(source.capabilities)
    ? source.capabilities.map((capability) => String(capability ?? '').trim()).filter(Boolean)
    : []

  const constraints = source.constraints ?? {}
  const minRaw = Number(constraints.minInstances ?? (roleKind === 'lead' ? 1 : 0))
  const maxRaw = Number(constraints.maxInstances ?? (roleKind === 'lead' ? 1 : 8))
  const minInstances = Number.isFinite(minRaw) ? Math.max(0, Math.floor(minRaw)) : (roleKind === 'lead' ? 1 : 0)
  const maxInstances = Number.isFinite(maxRaw) ? Math.max(1, Math.floor(maxRaw)) : (roleKind === 'lead' ? 1 : 8)

  const normalized = {
    ...source,
    schema: {
      ...(source.schema && typeof source.schema === 'object' ? source.schema : {}),
      kind: 'role_template',
      version: Number(source.schema?.version ?? 1) || 1,
    },
    roleId,
    name: String(source.name ?? '').trim(),
    version: String(source.version ?? '1.0.0'),
    kind: roleKind,
    defaults: {
      ...(source.defaults && typeof source.defaults === 'object' ? source.defaults : {}),
      cliTool,
      model,
      reasoning_effort: reasoningEffort,
      defaultNamePattern: String(
        source.defaults?.defaultNamePattern ??
          (roleKind === 'lead' ? 'team-lead' : `${roleId || 'agent'}-{n}`)
      ),
    },
    capabilityPolicy: normalizeCapabilityPolicy(
      source.capabilityPolicy ?? source.capability_policy
    ),
    instructions: String(source.instructions ?? '').trim(),
    focusArea: optionalTrimmedString(source.focusArea ?? source.focus_area),
    contextSummary: optionalTrimmedString(source.contextSummary ?? source.context_summary),
    behaviorSummary: optionalTrimmedString(source.behaviorSummary ?? source.behavior_summary),
    communicationStyle: optionalTrimmedString(source.communicationStyle ?? source.communication_style),
    runtimeCompactSummary: source.runtimeCompactSummary ?? source.runtime_compact_summary ?? null,
    behavioralContract: normalizeBehavioralContract(source.behavioralContract ?? source.behavioral_contract, {
      mode: BEHAVIORAL_CONTRACT_MODES.TEMPLATE_INPUT,
    }),
    qualityGates: optionalTrimmedStringList(source.qualityGates ?? source.quality_gates),
    handoffExpectations: optionalTrimmedStringList(
      source.handoffExpectations ?? source.handoff_expectations
    ),
    definitionOfDone: optionalTrimmedStringList(source.definitionOfDone ?? source.definition_of_done),
    phaseScope: optionalTrimmedStringList(source.phaseScope ?? source.phase_scope),
    mode: optionalTrimmedString(source.mode),
    inheritsFrom: optionalTrimmedString(source.inheritsFrom ?? source.inherits_from),
    requiredArtifacts: optionalTrimmedStringList(source.requiredArtifacts ?? source.required_artifacts),
    capabilities,
    provenance: source.provenance ?? null,
    constraints: {
      ...(constraints && typeof constraints === 'object' ? constraints : {}),
      minInstances: roleKind === 'lead' ? 1 : minInstances,
      maxInstances: roleKind === 'lead' ? 1 : Math.max(maxInstances, minInstances),
      requiresLeadTool: constraints.requiresLeadTool ?? null,
      allowedProjectBinding: constraints.allowedProjectBinding ?? 'lead_project',
    },
  }
  delete normalized.role_id
  delete normalized.reasoning_effort
  delete normalized.capability_policy
  // serde aliases: a payload carrying both spellings is a duplicate field to
  // the backend, so the consumed snake_case spelling must not survive the spread.
  delete normalized.behavioral_contract
  delete normalized.focus_area
  delete normalized.context_summary
  delete normalized.behavior_summary
  delete normalized.communication_style
  delete normalized.runtime_compact_summary
  delete normalized.quality_gates
  delete normalized.handoff_expectations
  delete normalized.definition_of_done
  delete normalized.phase_scope
  delete normalized.inherits_from
  delete normalized.required_artifacts
  delete normalized.defaults.cli_tool
  delete normalized.defaults.reasoningEffort
  delete normalized.defaults.default_name_pattern
  return normalized
}

function normalizeSlotOverridesInput(overrides) {
  if (!overrides || typeof overrides !== 'object') return null

  const { reasoningEffort, reasoning_effort: snakeEffort, ...rest } = overrides
  const parsed = parseLegacyModel(overrides.model)
  const effort =
    optionalTrimmedString(reasoningEffort ?? snakeEffort) ?? parsed.reasoningEffort

  return {
    ...rest,
    model: parsed.model || null,
    reasoning_effort: effort,
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

  const rawSlots = Array.isArray(source.agentSlots ?? source.agent_slots)
    ? (source.agentSlots ?? source.agent_slots)
    : []
  const agentSlots = rawSlots.map((slot) => {
    const normalizedSlot = {
      ...(slot && typeof slot === 'object' ? slot : {}),
      roleId: String(slot?.roleId ?? slot?.role_id ?? '').trim(),
      count: Math.max(1, Number(slot?.count ?? 1) || 1),
      projectBinding: slot?.projectBinding ?? slot?.project_binding ?? 'lead_project',
      projectId: slot?.projectId ?? slot?.project_id ?? null,
      overrides: normalizeSlotOverridesInput(slot?.overrides),
    }
    // AgentSlot declares these as serde aliases; both spellings at once is a
    // duplicate field to the backend.
    delete normalizedSlot.role_id
    delete normalizedSlot.project_binding
    delete normalizedSlot.project_id
    return normalizedSlot
  })

  const normalized = {
    ...source,
    schema: {
      ...(source.schema && typeof source.schema === 'object' ? source.schema : {}),
      kind: 'team_preset',
      version: Number(source.schema?.version ?? 1) || 1,
    },
    presetId: String(source.presetId ?? '').trim(),
    name: String(source.name ?? '').trim(),
    description: String(source.description ?? '').trim(),
    version: String(source.version ?? '1.0.0'),
    leadRoleId: String(source.leadRoleId ?? '').trim(),
    // The preset's own pin for its lead, in the same canonical shape as a slot
    // override (`TeamPreset::lead_overrides`).
    leadOverrides: normalizeSlotOverridesInput(source.leadOverrides ?? source.lead_overrides),
    agentSlots,
    defaults: {
      ...(source.defaults && typeof source.defaults === 'object' ? source.defaults : {}),
      teamNamePattern: String(source.defaults?.teamNamePattern ?? '{project}-team'),
      tmuxLayout: String(source.defaults?.tmuxLayout ?? 'tiled'),
    },
  }
  delete normalized.preset_id
  delete normalized.lead_role_id
  delete normalized.lead_overrides
  delete normalized.agent_slots
  delete normalized.defaults.team_name_pattern
  delete normalized.defaults.tmux_layout
  return normalized
}

export function normalizeComposeTeamRequest(request) {
  const normalizedAgentSlots = (request?.agentSlots ?? []).map((slot) => ({
    ...(slot && typeof slot === 'object' ? slot : {}),
    roleId: slot?.roleId ?? '',
    count: Number(slot?.count ?? 0),
    projectBinding: slot?.projectBinding ?? 'lead_project',
    projectId: slot?.projectId ?? null,
    overrides: slot?.overrides ?? null,
  }))

  const normalized = {
    ...(request && typeof request === 'object' ? request : {}),
    leadRoleId: request?.leadRoleId ?? '',
    agentSlots: normalizedAgentSlots,
    overrides: {
      ...(request?.overrides ?? {}),
      ...(request?.projectName ? { projectName: request.projectName } : {}),
    },
  }
  delete normalized.projectName
  return normalized
}
