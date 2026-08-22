import {
  TOOL_OPTIONS,
  applyNamePattern,
  normalizeTool,
  resolveRoleReasoningEffort,
  resolveRoleTool,
  resolveSlotNamePattern,
  uniquifyMemberName,
} from '../meshDefaults.js'
import { EMPTY_MODEL_CATALOG, resolveMemberModel } from '../modelCatalog.js'
import { normalizeProjectPath as normalizeSharedProjectPath } from '../pathUtils.js'

function optionalEffort(value) {
  return String(value ?? '').trim() || null
}

function normalizeStatus(status) {
  const value = String(status || '').trim().toLowerCase()
  if (value === 'active' || value === 'idle') return value
  return 'offline'
}

export function inferTeamName(path) {
  const project = projectNameFromPath(path)
  return `${project}-team`
    .toLowerCase()
    .replace(/[^a-z0-9-]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
}

function normalizeProjectPath(path) {
  return normalizeSharedProjectPath(path)
}

function normalizeOptionalTool(tool) {
  const value = String(tool ?? '').trim().toLowerCase()
  return TOOL_OPTIONS.includes(value) ? value : ''
}

function normalizeRoleTemplateMetadata(role) {
  return {
    roleId: role?.roleId ?? role?.role_id ?? null,
    roleName: role?.name ?? role?.roleName ?? role?.role_name ?? null,
    focusArea: role?.focusArea ?? role?.focus_area ?? null,
    contextSummary: role?.contextSummary ?? role?.context_summary ?? null,
    behaviorSummary: role?.behaviorSummary ?? role?.behavior_summary ?? null,
    instructions: role?.instructions ?? null,
  }
}

function normalizeLeadPath(team) {
  return team?.leadProjectPath ?? team?.lead_project_path ?? null
}

export function projectNameFromPath(path) {
  const normalized = normalizeProjectPath(path)
  const segments = normalized.split('/').filter(Boolean)
  return segments.at(-1) || 'project'
}

function normalizeProjectLabel(path) {
  const normalized = normalizeProjectPath(path)
  const segments = normalized.split('/').filter(Boolean)
  return segments.at(-1) || ''
}

function isSameProjectPath(left, right) {
  const leftNormalized = normalizeProjectPath(left)
  const rightNormalized = normalizeProjectPath(right)
  if (!leftNormalized || !rightNormalized) return false
  return leftNormalized === rightNormalized
}

function normalizeAgentSlots(preset) {
  return Array.isArray(preset?.agentSlots ?? preset?.agent_slots)
    ? (preset?.agentSlots ?? preset?.agent_slots)
    : []
}

function coerceTeams(response) {
  if (Array.isArray(response)) return response
  return Array.isArray(response?.teams) ? response.teams : []
}

function normalizeTeamName(team) {
  return team?.teamName ?? team?.team_name ?? ''
}

function teamMatchesProject(team, currentProjectPath) {
  return isSameProjectPath(normalizeLeadPath(team), currentProjectPath)
}

export function createLead(overrides = {}, projectPath = '') {
  const normalizedTool = normalizeOptionalTool(overrides.tool ?? overrides.cliTool)
  return {
    id: String(overrides.id ?? 'lead'),
    name: String(overrides.name ?? 'team-lead'),
    tool: normalizedTool,
    model: String(overrides.model ?? ''),
    reasoningEffort: optionalEffort(overrides.reasoningEffort ?? overrides.reasoning_effort),
    status: normalizeStatus(overrides.status),
    projectId: String(overrides.projectId ?? projectPath ?? ''),
    isCrossProject: false,
    projectLabel: '',
    description: overrides.description ?? 'Team lead',
    paneId: overrides.paneId ?? null,
    roleId: overrides.roleId ?? null,
    roleName: overrides.roleName ?? overrides.role_name ?? null,
    focusArea: overrides.focusArea ?? overrides.focus_area ?? null,
    contextSummary: overrides.contextSummary ?? overrides.context_summary ?? null,
    behaviorSummary: overrides.behaviorSummary ?? overrides.behavior_summary ?? null,
    instructions: overrides.instructions ?? null,
    behavioralContract: overrides.behavioralContract ?? null,
    capabilities: Array.isArray(overrides.capabilities) ? overrides.capabilities : null,
  }
}

export function createAgent(index, overrides = {}, projectPath = '') {
  const normalizedTool = normalizeTool(overrides.tool ?? overrides.cliTool ?? 'codex')
  return {
    id: String(overrides.id ?? `agent-${index + 1}`),
    name: String(overrides.name ?? `agent-${index + 1}`),
    tool: normalizedTool,
    model: String(overrides.model ?? ''),
    reasoningEffort: optionalEffort(overrides.reasoningEffort ?? overrides.reasoning_effort),
    status: normalizeStatus(overrides.status),
    projectId: String(overrides.projectId ?? projectPath ?? ''),
    isCrossProject: Boolean(overrides.isCrossProject ?? overrides.is_cross_project),
    projectLabel: String(overrides.projectLabel ?? overrides.project_label ?? ''),
    description: overrides.description ?? null,
    paneId: overrides.paneId ?? null,
    roleId: overrides.roleId ?? null,
    roleName: overrides.roleName ?? overrides.role_name ?? null,
    focusArea: overrides.focusArea ?? overrides.focus_area ?? null,
    contextSummary: overrides.contextSummary ?? overrides.context_summary ?? null,
    behaviorSummary: overrides.behaviorSummary ?? overrides.behavior_summary ?? null,
    instructions: overrides.instructions ?? null,
    behavioralContract: overrides.behavioralContract ?? null,
    capabilities: Array.isArray(overrides.capabilities) ? overrides.capabilities : null,
  }
}

export function deriveCrossProjectMeta(member, leadProjectId = '') {
  const explicitFlag = member?.isCrossProject ?? member?.is_cross_project
  const memberProjectId = String(member?.projectId ?? member?.project_id ?? '')
  const normalizedMemberProject = canonicalProjectIdentity(memberProjectId)
  const normalizedLeadProject = canonicalProjectIdentity(leadProjectId)
  const fallbackIsCrossProject = Boolean(
    normalizedMemberProject
    && normalizedLeadProject
    && normalizedMemberProject !== normalizedLeadProject
  )
  const isCrossProject = typeof explicitFlag === 'boolean'
    ? explicitFlag
    : fallbackIsCrossProject
  const explicitLabel = String(member?.projectLabel ?? member?.project_label ?? '').trim()
  const projectLabel = isCrossProject
    ? (explicitLabel || normalizeProjectLabel(memberProjectId))
    : ''

  return {
    isCrossProject,
    projectLabel,
  }
}

function canonicalProjectIdentity(path) {
  const normalized = normalizeProjectPath(path)
  return isWindowsMountPath(normalized) ? normalized.toLowerCase() : normalized
}

function isWindowsMountPath(path) {
  return /^\/mnt\/[a-z](?:\/|$)/i.test(path)
}

function resolvePresetLeadTool(preset) {
  const explicitTool = normalizeOptionalTool(
    preset?.lead?.tool ??
    preset?.lead?.cliTool ??
    preset?.leadTool ??
    preset?.lead_tool
  )
  if (explicitTool) return explicitTool

  const tools = Array.isArray(preset?.tools) ? preset.tools.map((entry) => normalizeOptionalTool(entry)).filter(Boolean) : []
  return tools.length === 1 ? tools[0] : ''
}

export function buildTeamConfigFromPreset(preset, compositionResult = null, projectPath = '') {
  const tools = Array.isArray(preset?.tools) && preset.tools.length > 0
    ? preset.tools.map((entry) => normalizeTool(entry))
    : TOOL_OPTIONS

  const leadRoleId = preset?.leadRoleId ?? preset?.lead_role_id ?? ''
  const seenNames = new Map()
  const leadName = uniquifyMemberName('team-lead', seenNames) || 'team-lead'
  const agentSlots = normalizeAgentSlots(preset)
  const roster = Array.isArray(compositionResult?.roster) ? compositionResult.roster : []
  const resolvedLead = roster[0] ?? null
  const resolvedAgents = roster.slice(1)

  const leadTool = normalizeOptionalTool(
    resolvedLead?.cliTool ??
    resolvedLead?.cli_tool ??
    resolvePresetLeadTool(preset)
  )
  const leadModel = String(resolvedLead?.model ?? '')

  const lead = createLead(
    {
      id: 'lead',
      name: String(resolvedLead?.name ?? leadName),
      tool: leadTool,
      model: String(leadModel),
      reasoningEffort: resolveRoleReasoningEffort(resolvedLead),
      status: 'offline',
      projectId: projectPath,
      roleId: (resolvedLead?.roleId ?? resolvedLead?.role_id ?? leadRoleId) || null,
      roleName: resolvedLead?.roleName ?? resolvedLead?.role_name ?? null,
      focusArea: resolvedLead?.focusArea ?? resolvedLead?.focus_area ?? null,
      contextSummary: resolvedLead?.contextSummary ?? resolvedLead?.context_summary ?? null,
      behaviorSummary: resolvedLead?.behaviorSummary ?? resolvedLead?.behavior_summary ?? null,
      instructions: resolvedLead?.instructions ?? null,
      behavioralContract: resolvedLead?.behavioralContract ?? resolvedLead?.behavioral_contract ?? null,
      capabilities: Array.isArray(resolvedLead?.capabilities) ? resolvedLead.capabilities : null,
      description: resolvedLead?.instructions ?? (leadRoleId || 'Team lead'),
    },
    projectPath
  )

  let agents = []

  if (resolvedAgents.length > 0) {
    agents = resolvedAgents.map((member, index) =>
      createAgent(
        index,
        {
          id: member?.name ?? `agent-${index + 1}`,
          name: member?.name ?? `agent-${index + 1}`,
          tool: member?.cliTool ?? member?.cli_tool ?? tools[(index + 1) % tools.length] ?? 'codex',
          model: member?.model ?? '',
          reasoningEffort: resolveRoleReasoningEffort(member),
          status: 'offline',
          projectId: member?.projectId ?? member?.project_id ?? projectPath,
          roleId: member?.roleId ?? member?.role_id ?? null,
          roleName: member?.roleName ?? member?.role_name ?? null,
          focusArea: member?.focusArea ?? member?.focus_area ?? null,
          contextSummary: member?.contextSummary ?? member?.context_summary ?? null,
          behaviorSummary: member?.behaviorSummary ?? member?.behavior_summary ?? null,
          instructions: member?.instructions ?? null,
          behavioralContract: member?.behavioralContract ?? member?.behavioral_contract ?? null,
          capabilities: Array.isArray(member?.capabilities) ? member.capabilities : null,
          description: member?.instructions ?? null,
        },
        projectPath
      )
    )
  } else if (agentSlots.length > 0) {
    const projectName = projectNameFromPath(projectPath)
    let agentIndex = 0
    agents = agentSlots.flatMap((slot) => {
      const count = Math.max(0, Number(slot?.count ?? 0))
      const roleId = slot?.roleId ?? slot?.role_id ?? null
      const pattern = resolveSlotNamePattern(slot, null)

      const members = Array.from({ length: count }, (_, offset) => {
        const memberIndex = offset + 1
        const fallbackName = `agent-${agentIndex + 1}`
        const resolvedName = applyNamePattern(pattern, memberIndex, projectName)
        const name = uniquifyMemberName(resolvedName || fallbackName, seenNames) || fallbackName
        const member = createAgent(
          agentIndex,
          {
            id: name,
            name,
            tool: tools[(agentIndex + 1) % tools.length] ?? 'codex',
            status: 'offline',
            projectId: projectPath,
            roleId,
          },
          projectPath
        )
        agentIndex += 1
        return member
      })

      return members
    })
  }

  if (agents.length === 0) {
    const agentCount = Math.max(
      1,
      Number(preset?.agentCount ?? Math.max(0, Number(preset?.roleCount ?? 1) - 1) ?? 1)
    )

    agents = Array.from({ length: agentCount }, (_, index) => {
      const tool = tools[(index + 1) % tools.length] ?? 'codex'
      const fallbackName = `agent-${index + 1}`
      const name = uniquifyMemberName(fallbackName, seenNames) || fallbackName
      return createAgent(
        index,
        {
          id: name,
          name,
          tool,
          status: 'offline',
          projectId: projectPath,
        },
        projectPath
      )
    })
  }

  return {
    description: String(preset?.description ?? ''),
    lead,
    agents,
    presetId: preset?.presetId ?? '',
    presetName: preset?.name ?? '',
    initializationMode: 'preset',
    composition: {
      presetId: preset?.presetId ?? '',
      name: preset?.name ?? '',
      leadRoleId,
      agentSlots,
    },
  }
}

export function buildTeamConfigFromRuntimeStatus(status, projectPath = '') {
  const runtimeSnapshotFreshness = (() => {
    const normalized = String(
      status?.runtimeSnapshotFreshness ?? status?.runtime_snapshot_freshness ?? ''
    )
      .trim()
      .toLowerCase()
    if (normalized === 'fresh') return 'fresh'
    if (normalized === 'cached') return 'cached'
    if (normalized === 'attachmentsonly' || normalized === 'attachments_only') {
      return 'attachments_only'
    }
    return null
  })()
  const members = Array.isArray(status?.members) ? status.members : []
  const normalizedMembers = members.map((member, index) => ({
    ...member,
    name: String(member?.name ?? `member-${index + 1}`),
    role: String(member?.role ?? '').toLowerCase(),
    tool: normalizeTool(member?.cliTool),
    model: String(member?.model || ''),
    reasoningEffort: optionalEffort(member?.reasoningEffort ?? member?.reasoning_effort),
    status: normalizeStatus(member?.sessionStatus),
    projectId: String(member?.projectId ?? member?.project_id ?? projectPath ?? ''),
    description: member?.description ?? null,
    paneId: member?.paneId ?? null,
    roleId: member?.roleId ?? null,
    roleName: member?.roleName ?? member?.role_name ?? null,
    focusArea: member?.focusArea ?? member?.focus_area ?? null,
    contextSummary: member?.contextSummary ?? member?.context_summary ?? null,
    behaviorSummary: member?.behaviorSummary ?? member?.behavior_summary ?? null,
    instructions: member?.instructions ?? null,
    behavioralContract: member?.behavioralContract ?? null,
    capabilities: Array.isArray(member?.capabilities) ? member.capabilities : null,
  }))

  const leadMember = normalizedMembers.find((member) => member.role === 'lead')
  const fallbackLeadName = status?.leadName ?? 'team-lead'
  const leadProjectId = String(leadMember?.projectId ?? projectPath ?? '')
  const membersWithCrossProjectMeta = normalizedMembers.map((member) => ({
    ...member,
    ...deriveCrossProjectMeta(member, leadProjectId),
  }))
  const normalizedLeadMember = membersWithCrossProjectMeta.find((member) => member.role === 'lead')

  const lead = createLead(
    {
      id: String(normalizedLeadMember?.name ?? 'lead'),
      name: normalizedLeadMember?.name ?? fallbackLeadName,
      tool: normalizedLeadMember?.tool ?? 'claude',
      model: normalizedLeadMember?.model ?? '',
      reasoningEffort: normalizedLeadMember?.reasoningEffort ?? null,
      status: normalizedLeadMember?.status ?? 'active',
      projectId: normalizedLeadMember?.projectId ?? projectPath,
      description: normalizedLeadMember?.description ?? 'Team lead',
      paneId: normalizedLeadMember?.paneId ?? null,
      roleId: normalizedLeadMember?.roleId ?? null,
      roleName: normalizedLeadMember?.roleName ?? null,
      focusArea: normalizedLeadMember?.focusArea ?? null,
      contextSummary: normalizedLeadMember?.contextSummary ?? null,
      behaviorSummary: normalizedLeadMember?.behaviorSummary ?? null,
      instructions: normalizedLeadMember?.instructions ?? null,
      behavioralContract: normalizedLeadMember?.behavioralContract ?? null,
      capabilities: normalizedLeadMember?.capabilities ?? null,
    },
    projectPath
  )

  const agents = membersWithCrossProjectMeta
    .filter((member) => member.role !== 'lead')
    .map((member, index) =>
      createAgent(
        index,
        {
          id: member.name,
          name: member.name,
          tool: member.tool,
          model: member.model,
          reasoningEffort: member.reasoningEffort,
          status: member.status,
          projectId: member.projectId,
          isCrossProject: member.isCrossProject,
          projectLabel: member.projectLabel,
          description: member.description,
          paneId: member.paneId,
          roleId: member.roleId,
          roleName: member.roleName,
          focusArea: member.focusArea,
          contextSummary: member.contextSummary,
          behaviorSummary: member.behaviorSummary,
          instructions: member.instructions,
          behavioralContract: member.behavioralContract,
          capabilities: member.capabilities,
        },
        projectPath
      )
    )

  return {
    description: String(status?.description ?? ''),
    runtimeSnapshotFreshness,
    lead,
    agents,
    presetId: '',
    presetName: '',
    initializationMode: 'runtime',
    composition: null,
  }
}

export function buildInitializationRequest(
  config,
  teamName,
  projectPath = '',
  catalog = EMPTY_MODEL_CATALOG
) {
  const lead = config?.lead
  const agents = Array.isArray(config?.agents) ? config.agents : []
  const isPresetInitialization = config?.initializationMode === 'preset' && String(config?.presetId ?? '').trim()

  if (isPresetInitialization) {
    return {
      teamName: teamName.trim() || inferTeamName(projectPath),
      teamDescription: String(config?.description ?? '').trim() || null,
      leadMode: 'launch_new',
      presetId: String(config?.presetId ?? '').trim(),
      lead: {
        name: lead?.name ?? 'team-lead',
        cliTool: '',
        model: '',
        projectId: lead?.projectId || projectPath,
        description: null,
        roleId: null,
        roleName: null,
        focusArea: null,
        contextSummary: null,
        behaviorSummary: null,
        instructions: null,
        behavioralContract: null,
        capabilities: null,
      },
      agents: agents.map((agent, index) => ({
        name: agent?.name || `agent-${index + 1}`,
        cliTool: '',
        model: '',
        projectId: agent?.projectId || projectPath,
        description: null,
        roleId: null,
        roleName: null,
        focusArea: null,
        contextSummary: null,
        behaviorSummary: null,
        instructions: null,
        behavioralContract: null,
        capabilities: null,
      })),
    }
  }

  const leadModel = normalizeOptionalTool(lead?.tool)
    ? resolveMemberModel(lead, null, catalog)
    : { model: String(lead?.model ?? ''), reasoningEffort: optionalEffort(lead?.reasoningEffort) }

  return {
    teamName: teamName.trim() || inferTeamName(projectPath),
    teamDescription: String(config?.description ?? '').trim() || null,
    leadMode: 'launch_new',
    lead: {
      name: lead?.name ?? 'team-lead',
      cliTool: normalizeOptionalTool(lead?.tool),
      model: leadModel.model,
      reasoningEffort: leadModel.reasoningEffort,
      projectId: lead?.projectId || projectPath,
      description: lead?.description ?? 'Team lead',
      roleId: lead?.roleId ?? null,
      roleName: lead?.roleName ?? null,
      focusArea: lead?.focusArea ?? null,
      contextSummary: lead?.contextSummary ?? null,
      behaviorSummary: lead?.behaviorSummary ?? null,
      instructions: lead?.instructions ?? null,
      behavioralContract: lead?.behavioralContract ?? null,
      capabilities: Array.isArray(lead?.capabilities) ? lead.capabilities : null,
    },
    agents: agents.map((agent, index) => {
      const agentModel = resolveMemberModel(agent, null, catalog)
      return {
        name: agent?.name || `agent-${index + 1}`,
        cliTool: normalizeTool(agent?.tool),
        model: agentModel.model,
        reasoningEffort: agentModel.reasoningEffort,
        projectId: agent?.projectId || projectPath,
        description: agent?.description ?? null,
        roleId: agent?.roleId ?? null,
        roleName: agent?.roleName ?? null,
        focusArea: agent?.focusArea ?? null,
        contextSummary: agent?.contextSummary ?? null,
        behaviorSummary: agent?.behaviorSummary ?? null,
        instructions: agent?.instructions ?? null,
        behavioralContract: agent?.behavioralContract ?? null,
        capabilities: Array.isArray(agent?.capabilities) ? agent.capabilities : null,
      }
    }),
  }
}

export function slugifyRoleId(value) {
  const slug = String(value || '')
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9\s_-]+/g, '')
    .replace(/[\s_]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
  return slug || 'captured-role'
}

export function normalizeBehavioralContract(value) {
  const base = {
    communication: [],
    execution: [],
    escalation: [],
  }

  if (value && typeof value === 'object' && !Array.isArray(value)) {
    for (const key of Object.keys(base)) {
      if (Array.isArray(value[key])) {
        base[key] = value[key].map((entry) => String(entry || '').trim()).filter(Boolean)
      }
    }
    return base
  }

  if (Array.isArray(value)) {
    base.communication = value
      .map((entry) => {
        if (typeof entry === 'string') return entry.trim()
        if (!entry || typeof entry !== 'object') return ''
        const rule = String(entry.rule ?? entry.text ?? '').trim()
        const enabled = entry.enabled === undefined ? true : Boolean(entry.enabled)
        return enabled ? rule : ''
      })
      .filter(Boolean)
    return base
  }

  return base
}

export function contractHasRules(contract) {
  return (
    contract.communication.length > 0 ||
    contract.execution.length > 0 ||
    contract.escalation.length > 0
  )
}

function defaultBehavioralContract(roleName) {
  const safeName = String(roleName || 'agent').trim() || 'agent'
  return {
    communication: [`Report concise progress as ${safeName} and escalate blockers quickly.`],
    execution: ['Execute scoped tasks and verify outcomes before handoff.'],
    escalation: ['Escalate ambiguous requirements before taking risky actions.'],
  }
}

function normalizeCapabilities(capabilities, tool) {
  if (Array.isArray(capabilities)) {
    const normalized = capabilities
      .map((entry) => String(entry || '').trim().toLowerCase())
      .filter(Boolean)
    if (normalized.length > 0) return [...new Set(normalized)]
  }
  return [`${normalizeTool(tool)}-workflow`]
}

export function buildCapturedRoleTemplate(draft, catalog = EMPTY_MODEL_CATALOG) {
  const roleKind = draft.roleKind === 'lead' ? 'lead' : 'agent'
  const trimmedName = String(draft.name || '').trim()
  const normalizedRoleId = slugifyRoleId(draft.roleId)
  const includeInstructions = Boolean(draft.includeInstructions)
  const includeBehavioralContract = Boolean(draft.includeBehavioralContract)

  const instructionsFromNode = includeInstructions ? String(draft.description || '').trim() : ''
  const instructions = instructionsFromNode || `Captured runtime role for ${trimmedName}.`

  const capturedModel = resolveMemberModel(draft, null, catalog)
  const currentContract = normalizeBehavioralContract(draft.behavioralContract)
  const behavioralContract = includeBehavioralContract && contractHasRules(currentContract)
    ? currentContract
    : defaultBehavioralContract(trimmedName)

  return {
    schema: {
      kind: 'role_template',
      version: 1,
    },
    roleId: normalizedRoleId,
    name: trimmedName,
    version: '1.0.0',
    kind: roleKind,
    defaults: {
      cliTool: normalizeTool(draft.tool),
      model: capturedModel.model,
      reasoningEffort: capturedModel.reasoningEffort,
      defaultNamePattern: roleKind === 'lead' ? 'team-lead' : 'agent-{n}',
    },
    instructions,
    behavioralContract,
    capabilities: normalizeCapabilities(draft.capabilities, draft.tool),
    constraints: roleKind === 'lead'
      ? {
        minInstances: 1,
        maxInstances: 1,
        requiresLeadTool: null,
        allowedProjectBinding: 'lead_project',
      }
      : {
        minInstances: 0,
        maxInstances: 8,
        requiresLeadTool: null,
        allowedProjectBinding: 'any',
      },
  }
}
