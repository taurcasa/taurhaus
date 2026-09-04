import { resolveRoleModel, resolveRoleReasoningEffort } from '../meshDefaults.js'
import { parseLegacyModel } from '../modelCatalog.js'

function normalizeBehavioralContractForEditor(contract) {
  if (Array.isArray(contract)) {
    return contract
      .map((entry) => {
        if (typeof entry === 'string') {
          const rule = entry.trim()
          return rule ? { rule, enabled: true } : null
        }
        if (entry && typeof entry === 'object') {
          const rule = String(entry.rule ?? '').trim()
          if (!rule) return null
          return {
            rule,
            enabled: entry.enabled !== false,
          }
        }
        return null
      })
      .filter(Boolean)
  }

  if (contract && typeof contract === 'object') {
    return ['communication', 'execution', 'escalation']
      .flatMap((section) => (Array.isArray(contract[section]) ? contract[section] : []))
      .map((entry) => String(entry ?? '').trim())
      .filter(Boolean)
      .map((rule) => ({ rule, enabled: true }))
  }

  return []
}

function normalizeRoleProvenance(value) {
  const provenance = value?.provenance
  if (!provenance || typeof provenance !== 'object') {
    return null
  }

  return {
    sourceFormat: provenance?.sourceFormat ?? provenance?.source_format ?? '',
    sourceVersion: provenance?.sourceVersion ?? provenance?.source_version ?? null,
    sourcePath: provenance?.sourcePath ?? provenance?.source_path ?? null,
    importedAt: provenance?.importedAt ?? provenance?.imported_at ?? null,
    nonRoundtrippableFields: Array.isArray(
      provenance?.nonRoundtrippableFields ?? provenance?.non_roundtrippable_fields
    )
      ? (provenance?.nonRoundtrippableFields ?? provenance?.non_roundtrippable_fields)
      : [],
  }
}

export function normalizeRoleTemplate(value) {
  return {
    roleId: value?.roleId ?? value?.role_id ?? '',
    name: value?.name ?? '',
    kind: String(value?.kind ?? 'agent').toLowerCase(),
    cliTool: String(value?.cliTool ?? value?.cli_tool ?? 'claude').toLowerCase(),
    model: value?.model ?? value?.defaults?.model ?? '',
    reasoningEffort: resolveRoleReasoningEffort(value),
    capabilityPolicy: value?.capabilityPolicy ?? value?.capability_policy ?? null,
    focusArea: value?.focusArea ?? value?.focus_area ?? '',
    contextSummary: value?.contextSummary ?? value?.context_summary ?? '',
    behaviorSummary: value?.behaviorSummary ?? value?.behavior_summary ?? '',
    instructions: value?.instructions ?? '',
    behavioralContract: normalizeBehavioralContractForEditor(
      value?.behavioralContract ?? value?.behavioral_contract
    ),
    provenance: normalizeRoleProvenance(value),
    builtIn: Boolean(value?.builtIn ?? value?.built_in),
    readOnly: Boolean(value?.readOnly ?? value?.read_only),
  }
}

export function normalizeTeamPreset(value) {
  return {
    presetId: value?.presetId ?? value?.preset_id ?? '',
    name: value?.name ?? '',
    description: value?.description ?? '',
    leadRoleId: value?.leadRoleId ?? value?.lead_role_id ?? '',
    roleCount: value?.roleCount ?? value?.role_count ?? 0,
    agentCount: value?.agentCount ?? value?.agent_count ?? 0,
    tools: Array.isArray(value?.tools) ? value.tools : [],
    builtIn: Boolean(value?.builtIn ?? value?.built_in),
    readOnly: Boolean(value?.readOnly ?? value?.read_only),
  }
}

export function isCustomRole(role) {
  return !Boolean(role?.builtIn)
}

export function isCustomPreset(preset) {
  return !Boolean(preset?.builtIn || preset?.readOnly)
}

function toSlug(value) {
  return String(value ?? '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
}

export function ensureUniquePresetId(baseId, teamPresets = [], currentId = '') {
  const normalizedBase = toSlug(baseId) || 'custom-preset'
  const existing = new Set((teamPresets ?? []).map((preset) => preset?.presetId).filter(Boolean))
  existing.delete(currentId)

  if (!existing.has(normalizedBase)) {
    return normalizedBase
  }

  let idx = 2
  while (existing.has(`${normalizedBase}-${idx}`)) {
    idx += 1
  }
  return `${normalizedBase}-${idx}`
}

export function defaultLeadRoleId(roleTemplates = []) {
  return (
    roleTemplates.find((role) => role.kind === 'lead')?.roleId ??
    roleTemplates[0]?.roleId ??
    ''
  )
}

export function defaultAgentRoleId(roleTemplates = [], leadRoleId = defaultLeadRoleId(roleTemplates)) {
  return (
    roleTemplates.find((role) => role.kind === 'agent')?.roleId ??
    roleTemplates.find((role) => role.roleId !== leadRoleId)?.roleId ??
    roleTemplates[0]?.roleId ??
    'codex-developer'
  )
}

/**
 * A preset slot can pin its own model/effort on top of the role defaults. Losing
 * those on the way into the editor silently rewrites the preset on the next save,
 * so they are normalized (camelCase, legacy combined models split) and kept
 * alongside every other override field the slot carries.
 */
export function normalizeSlotOverridesForDraft(value) {
  if (!value || typeof value !== 'object') return null

  const { reasoningEffort, reasoning_effort: snakeEffort, ...rest } = value
  const parsed = parseLegacyModel(value.model)
  const model = parsed.model || null
  const effort = String(reasoningEffort ?? snakeEffort ?? '').trim() || parsed.reasoningEffort || null

  const normalized = { ...rest, model, reasoningEffort: effort }
  const pinsSomething = Object.values(normalized).some(
    (entry) => entry !== null && entry !== undefined && entry !== ''
  )
  return pinsSomething ? normalized : null
}

/**
 * What a role template contributes to a member that pins nothing: its model and
 * its effort, with the pre-PR-5a combined spelling ("gpt-5.4 high") split the
 * way the Rust `ModelSpec::parse_legacy` splits it. This is what an unpinned row
 * renders from; what a save writes back comes from the row's own overrides, never
 * from comparing the rendered value with these defaults.
 */
function roleModelDefaults(role) {
  const parsed = parseLegacyModel(resolveRoleModel(role))
  return {
    model: parsed.model,
    reasoningEffort: resolveRoleReasoningEffort(role) ?? parsed.reasoningEffort,
  }
}

export function normalizePresetDraft(source = {}, roleTemplates = [], teamPresets = []) {
  const leadRoleId = source?.leadRoleId ?? source?.lead_role_id ?? defaultLeadRoleId(roleTemplates)
  // The lead pins its own model/effort the same way a slot does; the editor renders
  // and saves it, and composition applies it as `CompositionOverrides::lead`.
  const leadOverrides = normalizeSlotOverridesForDraft(
    source?.leadOverrides ?? source?.lead_overrides
  )
  const slots = Array.isArray(source?.agentSlots ?? source?.agent_slots)
    ? (source?.agentSlots ?? source?.agent_slots)
    : []
  const fallbackAgentRoleId = defaultAgentRoleId(roleTemplates, leadRoleId)
  const agentSlots = slots.length > 0
    ? slots.map((slot) => ({
      roleId: slot?.roleId ?? slot?.role_id ?? fallbackAgentRoleId,
      count: Math.max(1, Number(slot?.count ?? 1)),
      projectBinding: slot?.projectBinding ?? slot?.project_binding ?? 'lead_project',
      projectId: slot?.projectId ?? slot?.project_id ?? null,
      overrides: normalizeSlotOverridesForDraft(slot?.overrides),
    }))
    : [{
      roleId: fallbackAgentRoleId,
      count: 1,
      projectBinding: 'lead_project',
      projectId: null,
      overrides: null,
    }]

  return {
    presetId: source?.presetId ?? source?.preset_id ?? ensureUniquePresetId('custom-preset', teamPresets),
    name: source?.name ?? 'New Preset',
    description: source?.description ?? 'Custom team preset',
    version: source?.version ?? '1.0.0',
    leadRoleId,
    leadOverrides,
    agentSlots,
    defaults: {
      teamNamePattern: source?.defaults?.teamNamePattern ?? source?.defaults?.team_name_pattern ?? '{project}-team',
      tmuxLayout: source?.defaults?.tmuxLayout ?? source?.defaults?.tmux_layout ?? 'tiled',
    },
  }
}

export function presetDraftToTeamConfig(presetDraft, roleTemplates = []) {
  const draft = normalizePresetDraft(presetDraft, roleTemplates)
  const leadRole = roleTemplates.find((role) => role.roleId === draft.leadRoleId) ?? null
  const agentRoleCounts = new Map()
  const agents = []
  let nextAgent = 1

  for (const [slotIndex, slot] of draft.agentSlots.entries()) {
    const role = roleTemplates.find((entry) => entry.roleId === slot.roleId) ?? null
    const roleDefaults = roleModelDefaults(role)
    const overrideModel = String(slot.overrides?.model ?? '').trim()
    const overrideEffort = String(slot.overrides?.reasoningEffort ?? '').trim()
    for (let idx = 0; idx < slot.count; idx += 1) {
      const previous = agentRoleCounts.get(slot.roleId) ?? 0
      agentRoleCounts.set(slot.roleId, previous + 1)
      const roleSequence = agentRoleCounts.get(slot.roleId)
      const roleName = role?.name || 'agent'
      const tool = String(role?.cliTool ?? '').trim().toLowerCase()
      agents.push({
        id: `agent-${nextAgent}`,
        name: slot.count > 1 ? `${roleName}-${roleSequence}` : roleName,
        tool,
        model: overrideModel || roleDefaults.model,
        reasoningEffort: overrideEffort || (overrideModel ? null : roleDefaults.reasoningEffort),
        projectId: '',
        description: slot.roleId || '',
        roleId: slot.roleId ?? null,
        roleName: role?.name ?? null,
        slotIndex,
      })
      nextAgent += 1
    }
  }

  const leadTool = String(leadRole?.cliTool ?? '').trim().toLowerCase()
  const leadDefaults = roleModelDefaults(leadRole)
  const leadOverrideModel = String(draft.leadOverrides?.model ?? '').trim()
  const leadOverrideEffort = String(draft.leadOverrides?.reasoningEffort ?? '').trim()

  return {
    teamName: draft.name,
    description: draft.description,
    presetId: draft.presetId,
    lead: {
      id: 'lead',
      name: leadRole?.name || 'team-lead',
      tool: leadTool,
      model: leadOverrideModel || leadDefaults.model,
      reasoningEffort:
        leadOverrideEffort || (leadOverrideModel ? null : leadDefaults.reasoningEffort),
      projectId: '',
      description: draft.leadRoleId || 'Team lead',
      roleId: draft.leadRoleId || null,
      roleName: leadRole?.name ?? null,
    },
    agents,
  }
}

export function roleKindBadgeTone(kind, dark) {
  if (kind === 'lead') {
    return dark
      ? 'border border-brand-500/40 bg-brand-500/10 text-brand-300'
      : 'border border-brand-300 bg-brand-100 text-brand-700'
  }
  return dark
    ? 'border border-zinc-600 bg-zinc-800 text-zinc-300'
    : 'border border-zinc-300 bg-zinc-100 text-zinc-700'
}
