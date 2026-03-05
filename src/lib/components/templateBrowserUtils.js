export function normalizeRoleTemplate(value) {
  return {
    roleId: value?.roleId ?? value?.role_id ?? '',
    name: value?.name ?? '',
    kind: String(value?.kind ?? 'agent').toLowerCase(),
    cliTool: String(value?.cliTool ?? value?.cli_tool ?? 'claude').toLowerCase(),
    model: value?.model ?? '',
    capabilities: Array.isArray(value?.capabilities) ? value.capabilities : [],
    instructions: value?.instructions ?? '',
    behavioralContract: value?.behavioralContract ?? value?.behavioral_contract ?? [],
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
    capabilities: Array.isArray(value?.capabilities) ? value.capabilities : [],
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

export function toSlug(value) {
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
    'claude-orchestrator'
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

export function normalizePresetDraft(source = {}, roleTemplates = [], teamPresets = []) {
  const leadRoleId = source?.leadRoleId ?? source?.lead_role_id ?? defaultLeadRoleId(roleTemplates)
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
    }))
    : [{
      roleId: fallbackAgentRoleId,
      count: 1,
      projectBinding: 'lead_project',
      projectId: null,
    }]

  return {
    presetId: source?.presetId ?? source?.preset_id ?? ensureUniquePresetId('custom-preset', teamPresets),
    name: source?.name ?? 'New Preset',
    description: source?.description ?? 'Custom team preset',
    version: source?.version ?? '1.0.0',
    leadRoleId,
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

  for (const slot of draft.agentSlots) {
    const role = roleTemplates.find((entry) => entry.roleId === slot.roleId) ?? null
    for (let idx = 0; idx < slot.count; idx += 1) {
      const previous = agentRoleCounts.get(slot.roleId) ?? 0
      agentRoleCounts.set(slot.roleId, previous + 1)
      const roleSequence = agentRoleCounts.get(slot.roleId)
      const roleName = role?.name || 'agent'
      agents.push({
        id: `agent-${nextAgent}`,
        name: slot.count > 1 ? `${roleName}-${roleSequence}` : roleName,
        tool: role?.cliTool ?? 'codex',
        model: role?.model ?? 'gpt-5.3-codex',
        projectId: '',
        description: slot.roleId,
      })
      nextAgent += 1
    }
  }

  return {
    teamName: draft.name,
    description: draft.description,
    presetId: draft.presetId,
    lead: {
      id: 'lead',
      name: leadRole?.name || 'team-lead',
      tool: leadRole?.cliTool ?? 'claude',
      model: leadRole?.model ?? 'claude-opus-4-6',
      projectId: '',
      description: draft.leadRoleId,
    },
    agents,
  }
}

export function capabilityTestId(roleId, capability) {
  const normalized = String(capability ?? '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return `role-capability-${roleId}-${normalized}`
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

export function capabilityChipTone(dark) {
  return dark
    ? 'border border-zinc-700 text-zinc-300 bg-zinc-900/80'
    : 'border border-zinc-200 text-zinc-700 bg-zinc-50'
}
