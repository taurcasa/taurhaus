import {
  applyNamePattern,
  resolveDefaultNamePattern,
  resolveRoleModel,
  resolveRoleReasoningEffort,
  resolveRoleTool,
} from '../meshDefaults.js'
import { createAgent, createLead, projectNameFromPath, slugifyRoleId } from './meshTabUtils.js'

export function emptyBuilderConfig() {
  return {
    description: '',
    lead: null,
    agents: [],
    presetId: '',
    presetName: '',
    initializationMode: 'custom',
    composition: null,
  }
}

export function normalizeRoleKind(role) {
  return String(role?.kind ?? '').trim().toLowerCase() === 'lead' ? 'lead' : 'agent'
}

function slugifyMemberName(value) {
  return String(value || '')
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '') || 'member'
}

function nextAgentNameForRole(role, projectPath, existingAgents = []) {
  const existingNames = new Set(
    (existingAgents ?? []).map((agent) => String(agent?.name ?? '').trim()).filter(Boolean)
  )
  const defaultPattern =
    resolveDefaultNamePattern(role) ?? `${role.roleId || slugifyMemberName(role?.name)}-{n}`
  const projectName = projectNameFromPath(projectPath)
  let index = 1

  while (index < 100) {
    const candidate = applyNamePattern(defaultPattern, index, projectName)
    const name = candidate || `${role.roleId || slugifyMemberName(role?.name)}-${index}`
    if (!existingNames.has(name)) return name
    index += 1
  }

  return `${role.roleId || slugifyMemberName(role?.name)}-${Date.now()}`
}

export function createLeadFromRole(role, projectPath) {
  const tool = resolveRoleTool(role, 'claude')
  const model = resolveRoleModel(role)
  return createLead(
    {
      id: 'lead',
      name: 'team-lead',
      tool,
      model,
      reasoningEffort: resolveRoleReasoningEffort(role),
      status: 'offline',
      projectId: projectPath,
      roleId: role?.roleId ?? null,
      roleName: role?.name ?? null,
      focusArea: role?.focusArea ?? null,
      contextSummary: role?.contextSummary ?? null,
      behaviorSummary: role?.behaviorSummary ?? null,
      instructions: role?.instructions ?? null,
      behavioralContract: role?.behavioralContract ?? null,
      capabilities: Array.isArray(role?.capabilities) ? role.capabilities : null,
      description: role?.instructions ?? role?.name ?? 'Team lead',
    },
    projectPath
  )
}

export function createAgentFromRole(role, projectPath, existingAgents = []) {
  const tool = resolveRoleTool(role, 'codex')
  const model = resolveRoleModel(role)
  const name = nextAgentNameForRole(role, projectPath, existingAgents)

  return createAgent(
    existingAgents.length,
    {
      id: name,
      name,
      tool,
      model,
      reasoningEffort: resolveRoleReasoningEffort(role),
      status: 'offline',
      projectId: projectPath,
      roleId: role?.roleId ?? null,
      roleName: role?.name ?? null,
      focusArea: role?.focusArea ?? null,
      contextSummary: role?.contextSummary ?? null,
      behaviorSummary: role?.behaviorSummary ?? null,
      instructions: role?.instructions ?? null,
      behavioralContract: role?.behavioralContract ?? null,
      capabilities: Array.isArray(role?.capabilities) ? role.capabilities : null,
      description: role?.instructions ?? role?.name ?? null,
    },
    projectPath
  )
}

export function buildRuntimeAgentName(role, projectId, teamConfig, projectPath) {
  const normalizedProjectId = String(projectId || '').trim() || projectPath
  const projectName = projectNameFromPath(normalizedProjectId)
  const pattern =
    resolveDefaultNamePattern(role) ||
    `${slugifyRoleId(String(role?.roleId || role?.name || 'agent'))}-{n}`
  const existingNames = new Set(
    [teamConfig?.lead?.name, ...(teamConfig?.agents ?? []).map((agent) => agent?.name)]
      .map((value) => String(value ?? '').trim())
      .filter(Boolean)
  )

  let index = 1
  while (index < 100) {
    const candidate = applyNamePattern(pattern, index, projectName)
    const fallback = `${slugifyRoleId(String(role?.roleId || role?.name || 'agent'))}-${index}`
    const nextName = String(candidate || fallback).trim()
    if (nextName && !existingNames.has(nextName)) {
      return nextName
    }
    index += 1
  }

  return `${slugifyRoleId(String(role?.roleId || role?.name || 'agent'))}-${Date.now()}`
}

export function mergePresetCatalog(quickPresets, fetchedPresets = []) {
  const merged = new Map()

  for (const preset of quickPresets) {
    const presetId = String(preset?.presetId ?? '').trim()
    if (!presetId) continue
    merged.set(presetId, preset)
  }

  for (const preset of fetchedPresets) {
    const presetId = String(preset?.presetId ?? '').trim()
    if (!presetId) continue
    const base = merged.get(presetId) ?? {}
    merged.set(presetId, {
      ...base,
      ...preset,
      presetId,
    })
  }

  return [...merged.values()]
}
