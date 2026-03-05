function normalizeBehavioralContractInput(value) {
  if (Array.isArray(value)) {
    const execution = value
      .map((entry) => {
        if (typeof entry === 'string') return entry.trim()
        if (!entry || typeof entry !== 'object') return ''
        if (entry.enabled === false) return ''
        return String(entry.rule ?? entry.text ?? '').trim()
      })
      .filter(Boolean)

    if (execution.length === 0) {
      return {
        communication: [],
        execution: ['Execute assigned tasks and report status clearly.'],
        escalation: [],
      }
    }

    return {
      communication: [],
      execution,
      escalation: [],
    }
  }

  const communication = Array.isArray(value?.communication)
    ? value.communication.map((line) => String(line ?? '').trim()).filter(Boolean)
    : []
  const execution = Array.isArray(value?.execution)
    ? value.execution.map((line) => String(line ?? '').trim()).filter(Boolean)
    : []
  const escalation = Array.isArray(value?.escalation)
    ? value.escalation.map((line) => String(line ?? '').trim()).filter(Boolean)
    : []

  if (communication.length || execution.length || escalation.length) {
    return { communication, execution, escalation }
  }

  return {
    communication: [],
    execution: ['Execute assigned tasks and report status clearly.'],
    escalation: [],
  }
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
  const cliTool = String(
    source.tool ??
      source.cliTool ??
      source.defaults?.cliTool ??
      (roleKind === 'lead' ? 'claude' : 'codex')
  ).toLowerCase()
  const model = String(
    source.model ??
      source.defaults?.model ??
      (cliTool === 'claude'
        ? 'claude-opus-4-6'
        : (cliTool === 'gemini' ? 'gemini-3.1-pro' : 'gpt-5.3-codex'))
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
    behavioralContract: normalizeBehavioralContractInput(source.behavioralContract),
    capabilities: capabilities.length > 0 ? capabilities : [roleKind === 'lead' ? 'orchestration' : 'implementation'],
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
