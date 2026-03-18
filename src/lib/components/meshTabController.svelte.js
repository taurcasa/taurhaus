import { untrack } from 'svelte'
import {
  composeTeam,
  coordinationAddAgent,
  coordinationDisbandTeam,
  coordinationGetProjectMeshSnapshot,
  coordinationGetLiveTeamStatus,
  coordinationRemoveMember,
  coordinationResumeTeam,
  coordinationResumeMember,
  getTeamPreset,
  listTeamPresets,
  listRoleTemplates,
  upsertTeamPreset,
  upsertRoleTemplate,
} from '../ipc.js'
import {
  normalizeProjectMeshSnapshot,
  normalizeResumeTeamReport,
} from '../ipc/coordinationResponses.js'
import { clearMeshCache, getMeshCache, getMeshCacheEntry, setMeshCache } from '../meshCache.svelte.js'
import {
  defaultModelForTool,
  normalizeTool,
} from '../meshDefaults.js'
import { normalizeProjectOption } from '../projectOptions.js'
import {
  buildCapturedRoleTemplate,
  buildInitializationRequest,
  buildTeamConfigFromPreset,
  buildTeamConfigFromRuntimeStatus,
  contractHasRules,
  createAgent,
  createLead,
  inferTeamName,
  normalizeBehavioralContract,
  slugifyRoleId,
} from './meshTabUtils.js'
import {
  buildRuntimeAgentName,
  createAgentFromRole,
  createLeadFromRole,
  emptyBuilderConfig,
  mergePresetCatalog,
  normalizeRoleKind,
} from './meshBuilderUtils.js'
import { refreshRuntimeTeamConfigWorkflow } from './meshTabGateWorkflow.js'
import { autoDismissNotice } from './meshTabNotifications.js'

const RUNTIME_STATUS_POLL_MS = 2000
const INITIAL_RUNTIME_REFRESH_DELAY_MS = 120
const PROJECT_SNAPSHOT_CACHE_MAX_AGE_MS = 5000

export function createMeshTabController({
  getProjectPath,
  getIsVisible = () => true,
  getBackgroundWorkEnabled = () => true,
  getAvailableProjects,
  onAddAgent,
  onDisband,
  onRemoveAgent,
  onFocusPane,
}) {
  const quickPresets = [
    {
      presetId: 'standard-team',
      name: 'Standard Dev Team',
      description: 'Orchestrator, architect, two full-stack developers, and a UI specialist',
      leadCount: 1,
      agentCount: 4,
      tools: ['claude', 'codex', 'gemini'],
      builtIn: true,
    },
    {
      presetId: 'fullstack-dev',
      name: 'Full Stack Dev Team',
      description: 'Lead with implementation and review agents',
      leadCount: 1,
      agentCount: 2,
      tools: ['claude', 'codex', 'gemini'],
      builtIn: true,
    },
    {
      presetId: 'research-dev',
      name: 'Research + Development Team',
      description: 'Lead with research and implementation collaboration',
      leadCount: 1,
      agentCount: 2,
      tools: ['claude', 'gemini', 'codex'],
      builtIn: true,
    },
    {
      presetId: 'review-team',
      name: 'Review Team',
      description: 'Lead with focused implementation and QA reviewers',
      leadCount: 1,
      agentCount: 2,
      tools: ['claude', 'codex'],
      builtIn: true,
    },
  ]

  let mode = $state('empty')
  let teamName = $state('')
  let teamConfig = $state(null)
  let slideOver = $state(null)
  let slideOverContext = $state(null)
  let selectedNodeId = $state(null)
  let initProgress = $state(null)
  let errorMessage = $state('')
  let runtimeMessage = $state('')
  let confirmContext = $state(null)
  let availabilityMessage = $state('')
  let roleTemplates = $state([])
  let loadingRoles = $state(false)
  let availablePresets = $state(quickPresets)
  let loadingPresets = $state(false)
  let presetsLoaded = $state(false)
  let captureRoleDialog = $state(null)
  let teamRuntimeState = $state('none')
  let teamResumeProgress = $state(null)

  let discoverySequence = 0
  let presetSelectionSequence = 0
  let runtimeMessageTimer = null
  let errorMessageTimer = null
  let runtimePollTimer = null
  let runtimeRefreshTimer = null
  let runtimeRefreshMeta = null
  let runtimeStatusRequest = null
  let runtimeStatusRequestMeta = null
  let queuedRuntimeStatusRequest = null
  let projectSnapshotRefreshTimer = null
  let teamResumeProgressTimer = null
  let hydrationPerf = null
  let pendingProjectSnapshot = null

  const selectedNode = $derived.by(() => {
    const config = teamConfig
    if (!config || !selectedNodeId) return null
    if (String(config.lead?.id ?? 'lead') === String(selectedNodeId)) {
      return { ...config.lead, id: String(config.lead?.id ?? 'lead'), role: 'lead' }
    }
    const agent = (config.agents ?? []).find((entry) => String(entry.id) === String(selectedNodeId))
    return agent ? { ...agent, role: 'agent' } : null
  })

  const canInitialize = $derived.by(() => {
    const config = teamConfig
    return Boolean(config?.lead) && Array.isArray(config?.agents)
  })

  const addAgentDraft = $derived(
    slideOver === 'addAgent' && slideOverContext && typeof slideOverContext === 'object'
      ? slideOverContext
      : null
  )

  const canSubmitAddAgent = $derived.by(() => {
    const draft = addAgentDraft
    if (!draft || draft.submitting) return false
    return (
      String(draft.roleId || '').trim().length > 0 &&
      String(draft.name || '').trim().length > 0 &&
      String(draft.tool || '').trim().length > 0 &&
      String(draft.model || '').trim().length > 0 &&
      String(draft.projectId || '').trim().length > 0
    )
  })

  const captureRoleDraft = $derived(
    captureRoleDialog && typeof captureRoleDialog === 'object' ? captureRoleDialog : null
  )

  const isResumingTeam = $derived.by(() => Boolean(teamResumeProgress?.inFlight))
  const canResumeTeam = $derived.by(
    () => teamRuntimeState === 'coldResume' || teamRuntimeState === 'degraded'
  )

  const canSaveCapturedRole = $derived.by(() => {
    const draft = captureRoleDraft
    if (!draft || draft.submitting) return false
    return String(draft.name || '').trim().length > 0 && String(draft.roleId || '').trim().length > 0
  })

  function nowMs() {
    if (typeof performance !== 'undefined' && typeof performance.now === 'function') {
      return performance.now()
    }
    return Date.now()
  }

  function logProjectSwitchPerf(stage, payload = {}) {
    if (!globalThis.__TAURHAUS_MESH_SWITCH_PERF__) return
    console.debug('[mesh.perf] project-switch', {
      stage,
      ...payload,
    })
  }

  function beginHydrationPerf(projectPath, sequence) {
    hydrationPerf = {
      projectPath,
      sequence,
      startedAt: nowMs(),
    }
    logProjectSwitchPerf('mesh-hydrate-start', { projectPath, sequence })
  }

  function finishHydrationPerf(stage, sequence, extra = {}) {
    if (!hydrationPerf) return
    if (hydrationPerf.sequence !== sequence) return
    logProjectSwitchPerf(stage, {
      projectPath: hydrationPerf.projectPath,
      sequence,
      elapsedMs: Number((nowMs() - hydrationPerf.startedAt).toFixed(1)),
      ...extra,
    })
    if (
      stage === 'mesh-hydrate-empty' ||
      stage === 'mesh-refresh-complete' ||
      stage === 'mesh-refresh-cancelled'
    ) {
      hydrationPerf = null
    }
  }

  function classifyTeamRuntimeStateFromMembers(teamName, members) {
    if (!teamName) return 'none'
    const roster = Array.isArray(members) ? members : []
    if (roster.length === 0) return 'active'
    const liveMembers = roster.filter((member) => {
      const status = String(member?.sessionStatus ?? member?.status ?? '').trim().toLowerCase()
      return status !== 'offline'
    }).length
    if (liveMembers === 0) return 'coldResume'
    if (liveMembers === roster.length) return 'active'
    return 'degraded'
  }

  function buildResumeTargetNames(config) {
    const members = [config?.lead, ...(config?.agents ?? [])].filter(Boolean)
    const offlineMembers = members
      .filter((member) => String(member?.status ?? '').trim().toLowerCase() === 'offline')
      .map((member) => String(member?.name ?? '').trim())
      .filter(Boolean)
    if (offlineMembers.length > 0) return offlineMembers
    return members.map((member) => String(member?.name ?? '').trim()).filter(Boolean)
  }

  function buildResumeProgressItems(targetNames, report = null, fallbackError = '') {
    const normalizedReport = normalizeResumeTeamReport(report)
    const names = Array.isArray(targetNames) ? [...targetNames] : []
    const resumedMembers = new Set(normalizedReport?.resumedMembers ?? [])
    const failedEntries = normalizedReport?.failedMembers ?? []
    const failedMap = new Map(
      failedEntries
        .map((entry) => ({
          memberName: entry?.memberName ?? '',
          message: entry?.message ?? 'Failed',
        }))
        .filter((entry) => entry.memberName)
        .map((entry) => [entry.memberName, entry])
    )

    for (const memberName of resumedMembers) {
      if (!names.includes(memberName)) names.push(memberName)
    }
    for (const memberName of failedMap.keys()) {
      if (!names.includes(memberName)) names.push(memberName)
    }

    return names.map((memberName) => {
      if (!normalizedReport && !fallbackError) {
        return { memberName, status: 'pending', message: 'Waiting to resume' }
      }
      if (resumedMembers.has(memberName)) {
        return { memberName, status: 'succeeded', message: 'Resumed' }
      }
      if (failedMap.has(memberName)) {
        return {
          memberName,
          status: 'failed',
          message: failedMap.get(memberName)?.message ?? 'Failed',
        }
      }
      if (fallbackError) {
        return { memberName, status: 'failed', message: fallbackError }
      }
      return { memberName, status: 'pending', message: 'Pending' }
    })
  }

  function buildResumeTeamMessage(report) {
    const normalizedReport = normalizeResumeTeamReport(report)
    if (!normalizedReport) return 'Team resume finished.'
    const resumedSummary = normalizedReport.resumedMembers.length
      ? `Resumed: ${normalizedReport.resumedMembers.join(', ')}.`
      : 'No members were resumed.'
    const failedSummary = normalizedReport.failedMembers.length
      ? `Failed: ${normalizedReport.failedMembers
          .map((entry) => `${entry?.memberName ?? 'unknown'}${entry?.message ? ` (${entry.message})` : ''}`)
          .join(', ')}.`
      : ''
    if (normalizedReport.failedMembers.length > 0) {
      return `Resume completed with failures. ${resumedSummary} ${failedSummary}`.trim()
    }
    return `Resume complete. ${resumedSummary}`.trim()
  }

  function buildAvailabilityMessage(snapshot) {
    const messages = []
    if (!snapshot.meshAvailable) messages.push('Mesh CLI is unavailable for this environment.')
    if (!snapshot.tmuxAvailable) messages.push('tmux is unavailable for this environment.')
    for (const warning of snapshot.warnings) {
      const message = String(warning || '').trim()
      if (message && !messages.includes(message)) messages.push(message)
    }
    return messages.join(' ')
  }

  function buildCachedSnapshotFromLiveStatus(snapshot, report) {
    const normalized = normalizeProjectMeshSnapshot(snapshot)
    const members = Array.isArray(report?.members)
      ? report.members.map((member) => ({
          name: member?.name ?? '',
          role: member?.role ?? 'member',
          cliTool: member?.cliTool ?? 'codex',
          model: member?.model ?? '',
          projectId: member?.projectId ?? '',
          description: member?.description ?? null,
          roleId: member?.roleId ?? null,
          roleName: member?.roleName ?? null,
          focusArea: member?.focusArea ?? null,
          contextSummary: member?.contextSummary ?? null,
          behaviorSummary: member?.behaviorSummary ?? null,
          sessionStatus: member?.sessionStatus ?? 'offline',
          paneId: member?.paneId ?? null,
        }))
      : []

    return {
      meshAvailable: normalized.meshAvailable,
      tmuxAvailable: normalized.tmuxAvailable,
      teamName: normalized.teamName,
      teamRuntimeState: classifyTeamRuntimeStateFromMembers(normalized.teamName, members),
      warnings: normalized.warnings,
      teamStatus: normalized.teamName
        ? {
            leadName: report?.leadName ?? 'team-lead',
            members,
          }
        : null,
    }
  }

  function applyProjectSnapshot(snapshot, projectPath, { preserveNotices = false } = {}) {
    const normalized = normalizeProjectMeshSnapshot(snapshot)
    availabilityMessage = buildAvailabilityMessage(normalized)
    teamRuntimeState = normalized.teamRuntimeState
    if (!preserveNotices) {
      errorMessage = ''
      runtimeMessage = ''
    }

    if (normalized.teamName && normalized.teamStatus) {
      teamName = normalized.teamName
      teamConfig = buildTeamConfigFromRuntimeStatus(
        {
          teamName: normalized.teamName,
          leadName: normalized.teamStatus?.leadName ?? 'team-lead',
          members: Array.isArray(normalized.teamStatus?.members) ? normalized.teamStatus.members : [],
        },
        projectPath
      )
      mode = 'runtime'
      return normalized
    }

    teamName = inferTeamName(projectPath)
    teamConfig = null
    teamRuntimeState = 'none'
    mode = 'empty'
    return normalized
  }

  function getProjectSnapshot(projectPath) {
    const normalizedProjectPath = String(projectPath ?? '').trim()
    if (!normalizedProjectPath) {
      return Promise.resolve(null)
    }
    if (pendingProjectSnapshot?.projectPath === normalizedProjectPath) {
      return pendingProjectSnapshot.promise
    }

    const promise = coordinationGetProjectMeshSnapshot(normalizedProjectPath).finally(() => {
      if (
        pendingProjectSnapshot?.projectPath === normalizedProjectPath &&
        pendingProjectSnapshot?.promise === promise
      ) {
        pendingProjectSnapshot = null
      }
    })

    pendingProjectSnapshot = {
      projectPath: normalizedProjectPath,
      promise,
    }
    return promise
  }

  async function refreshProjectMeshSnapshot(sequence, options = {}) {
    const projectPath = getProjectPath()
    const snapshot = await getProjectSnapshot(projectPath)
    if (!snapshot) return null
    if (sequence !== discoverySequence) return null
    setMeshCache(projectPath, snapshot)
    const normalized = applyProjectSnapshot(snapshot, projectPath, options)
    finishHydrationPerf(
      normalized.teamName && normalized.teamStatus ? 'mesh-hydrate-ready' : 'mesh-hydrate-empty',
      sequence,
      {
        mode,
        teamName: normalized.teamName,
      }
    )
    if (normalized.teamName && normalized.teamStatus) {
      scheduleRuntimeTeamRefresh(normalized.teamName, sequence, snapshot)
    }
    return normalized
  }

  function clearRuntimeTeamRefresh({ dropInFlight = false } = {}) {
    if (runtimeRefreshTimer) {
      clearTimeout(runtimeRefreshTimer)
      runtimeRefreshTimer = null
      if (runtimeRefreshMeta) {
        finishHydrationPerf('mesh-refresh-cancelled', runtimeRefreshMeta.sequence, {
          teamName: runtimeRefreshMeta.teamName,
        })
      }
      runtimeRefreshMeta = null
    }
    queuedRuntimeStatusRequest = null
    if (dropInFlight) {
      runtimeStatusRequest = null
      runtimeStatusRequestMeta = null
    }
  }

  function clearProjectSnapshotRefresh() {
    if (!projectSnapshotRefreshTimer) return
    clearTimeout(projectSnapshotRefreshTimer)
    projectSnapshotRefreshTimer = null
  }

  function scheduleProjectSnapshotRefresh(sequence) {
    clearProjectSnapshotRefresh()
    projectSnapshotRefreshTimer = setTimeout(() => {
      projectSnapshotRefreshTimer = null
      if (sequence !== discoverySequence) return
      if (!getIsVisible() || !getBackgroundWorkEnabled()) return
      void refreshProjectMeshSnapshot(sequence, { preserveNotices: true }).catch((error) => {
        if (sequence !== discoverySequence) return
        console.warn('[meshTab] background project snapshot refresh failed:', error)
      })
    }, INITIAL_RUNTIME_REFRESH_DELAY_MS)
  }

  function scheduleRuntimeTeamRefresh(nextTeamName, sequence, snapshot = null) {
    clearRuntimeTeamRefresh()
    runtimeRefreshMeta = {
      sequence,
      teamName: nextTeamName,
    }
    logProjectSwitchPerf('mesh-refresh-scheduled', {
      projectPath: getProjectPath(),
      sequence,
      teamName: nextTeamName,
      delayMs: INITIAL_RUNTIME_REFRESH_DELAY_MS,
    })
    runtimeRefreshTimer = setTimeout(() => {
      runtimeRefreshTimer = null
      const meta = runtimeRefreshMeta
      runtimeRefreshMeta = null
      if (sequence !== discoverySequence) return
      logProjectSwitchPerf('mesh-refresh-start', {
        projectPath: getProjectPath(),
        sequence,
        teamName: nextTeamName,
      })
      void queueRuntimeTeamRefresh(nextTeamName, sequence, snapshot)
        .catch((error) => {
          if (sequence !== discoverySequence) return
          console.warn('[meshTab] deferred runtime status refresh failed:', error)
        })
        .finally(() => {
          if (meta) {
            finishHydrationPerf('mesh-refresh-complete', meta.sequence, {
              teamName: meta.teamName,
            })
          }
        })
    }, INITIAL_RUNTIME_REFRESH_DELAY_MS)
  }

  async function refreshRuntimeTeamConfig(nextTeamName, sequence, snapshot = null) {
    let nextConfig = null
    await refreshRuntimeTeamConfigWorkflow({
      nextTeamName,
      sequence,
      getDiscoverySequence: () => discoverySequence,
      coordinationGetLiveTeamStatus,
      buildTeamConfigFromRuntimeStatus,
      getProjectPath,
      onTeamConfig: (value) => {
        nextConfig = value
        teamConfig = value
      },
    })
    if (nextConfig && snapshot) {
      setMeshCache(getProjectPath(), buildCachedSnapshotFromLiveStatus(snapshot, {
        teamName: nextTeamName,
        leadName: nextConfig.lead?.name ?? 'team-lead',
        members: [
          nextConfig.lead
            ? {
                name: nextConfig.lead.name,
                role: 'lead',
                cliTool: nextConfig.lead.tool,
                model: nextConfig.lead.model,
                projectId: nextConfig.lead.projectId,
                description: nextConfig.lead.description,
                roleId: nextConfig.lead.roleId,
                roleName: nextConfig.lead.roleName,
                focusArea: nextConfig.lead.focusArea,
                contextSummary: nextConfig.lead.contextSummary,
                behaviorSummary: nextConfig.lead.behaviorSummary,
                sessionStatus: nextConfig.lead.status,
                paneId: nextConfig.lead.paneId,
              }
            : null,
          ...(nextConfig.agents ?? []).map((member) => ({
            name: member.name,
            role: 'member',
            cliTool: member.tool,
            model: member.model,
            projectId: member.projectId,
            description: member.description,
            roleId: member.roleId,
            roleName: member.roleName,
            focusArea: member.focusArea,
            contextSummary: member.contextSummary,
            behaviorSummary: member.behaviorSummary,
            sessionStatus: member.status,
            paneId: member.paneId,
          })),
        ].filter(Boolean),
        }))
    }
  }

  function queueRuntimeTeamRefresh(nextTeamName, sequence, snapshot = null) {
    if (runtimeStatusRequest) {
      if (
        runtimeStatusRequestMeta?.sequence === sequence &&
        runtimeStatusRequestMeta?.teamName === nextTeamName
      ) {
        return runtimeStatusRequest
      }

      queuedRuntimeStatusRequest = { nextTeamName, sequence, snapshot }
      return runtimeStatusRequest
    }

    runtimeStatusRequestMeta = { teamName: nextTeamName, sequence }
    const request = refreshRuntimeTeamConfig(nextTeamName, sequence, snapshot)
    const wrappedRequest = request.finally(() => {
      if (runtimeStatusRequest === wrappedRequest) {
        runtimeStatusRequest = null
        runtimeStatusRequestMeta = null
      }

      const queued = queuedRuntimeStatusRequest
      queuedRuntimeStatusRequest = null
      if (
        queued &&
        queued.sequence === discoverySequence &&
        queued.nextTeamName &&
        getIsVisible() &&
        getBackgroundWorkEnabled()
      ) {
        void queueRuntimeTeamRefresh(queued.nextTeamName, queued.sequence, queued.snapshot)
      }
    })

    runtimeStatusRequest = wrappedRequest
    return wrappedRequest
  }

  async function hydrateProjectMesh(projectPath, sequence) {
    try {
      await refreshProjectMeshSnapshot(sequence)
    } catch (error) {
      if (sequence !== discoverySequence) return
      availabilityMessage = ''
      errorMessage = error?.message || 'Failed to load Mesh team state.'
      teamName = inferTeamName(projectPath)
      teamConfig = null
      mode = 'empty'
    }
  }

  function ensureHydrated(projectPath, { isVisible = true, backgroundWorkEnabled = true } = {}) {
    if (!projectPath || !isVisible || !backgroundWorkEnabled) return
    const sequence = ++discoverySequence
    beginHydrationPerf(projectPath, sequence)
    teamName = inferTeamName(projectPath)
    teamConfig = null
    selectedNodeId = null
    initProgress = null
    slideOver = null
    slideOverContext = null
    captureRoleDialog = null
    confirmContext = null
    availabilityMessage = ''
    teamRuntimeState = 'none'
    teamResumeProgress = null
    errorMessage = ''
    runtimeMessage = ''
    clearRuntimeTeamRefresh({ dropInFlight: true })
    clearProjectSnapshotRefresh()

    const cachedEntry = untrack(() => getMeshCacheEntry(projectPath))
    const cachedSnapshot = cachedEntry?.snapshot ?? null
    if (cachedSnapshot) {
      const normalized = applyProjectSnapshot(cachedSnapshot, projectPath)
      finishHydrationPerf(
        normalized.teamName && normalized.teamStatus ? 'mesh-hydrate-ready' : 'mesh-hydrate-empty',
        sequence,
        {
          mode,
          teamName: normalized.teamName,
          source: 'cache',
        }
      )
      const cachedAgeMs =
        typeof cachedEntry?.cachedAtMs === 'number' ? Math.max(0, Date.now() - cachedEntry.cachedAtMs) : Infinity
      if (cachedAgeMs >= PROJECT_SNAPSHOT_CACHE_MAX_AGE_MS) {
        scheduleProjectSnapshotRefresh(sequence)
      } else if (normalized.teamName && normalized.teamStatus) {
        scheduleRuntimeTeamRefresh(normalized.teamName, sequence, cachedSnapshot)
      }
      return
    }

    void hydrateProjectMesh(projectPath, sequence)
  }

  function ensureGateReady() {
    ensureHydrated(getProjectPath(), {
      isVisible: getIsVisible(),
      backgroundWorkEnabled: getBackgroundWorkEnabled(),
    })
  }

  function invalidateDiscovery() {
    discoverySequence += 1
    clearRuntimeTeamRefresh({ dropInFlight: true })
    clearProjectSnapshotRefresh()
    hydrationPerf = null
  }

  function closeSlideOver() {
    slideOver = null
    slideOverContext = null
  }

  function ensureBuilderConfig() {
    if (teamConfig) return teamConfig
    teamConfig = emptyBuilderConfig()
    return teamConfig
  }

  function resolveBuilderRole(roleId) {
    return roleTemplates.find((entry) => entry.roleId === roleId) ?? null
  }

  async function handlePresetSelect(preset) {
    invalidateDiscovery()
    const sequence = ++presetSelectionSequence
    const presetId = preset?.presetId ?? ''
    let resolvedPreset = preset
    let compositionResult = null

    try {
      const hydratedPreset = presetId ? await getTeamPreset(presetId) : null
      if (sequence !== presetSelectionSequence) return
      if (hydratedPreset && typeof hydratedPreset === 'object') {
        resolvedPreset = { ...preset, ...hydratedPreset }
      }

      const leadRoleId = resolvedPreset?.leadRoleId ?? ''
      const agentSlots = Array.isArray(resolvedPreset?.agentSlots) ? resolvedPreset.agentSlots : []

      if (leadRoleId) {
        compositionResult = await composeTeam({
          leadRoleId,
          agentSlots,
          overrides: {
            projectName: inferTeamName(getProjectPath()).replace(/-team$/, ''),
          },
        })
      }
    } catch (error) {
      console.error('Failed to hydrate quick preset details:', error)
    }

    if (sequence !== presetSelectionSequence) return
    teamConfig = buildTeamConfigFromPreset(resolvedPreset, compositionResult, getProjectPath())
    teamName = inferTeamName(getProjectPath())
    selectedNodeId = null
    mode = 'setup'
    closeSlideOver()
    runtimeMessage = ''
  }

  function handleStartCustom() {
    invalidateDiscovery()
    const projectPath = getProjectPath()
    teamConfig = emptyBuilderConfig()
    teamName = inferTeamName(projectPath)
    selectedNodeId = null
    mode = 'setup'
    closeSlideOver()
    runtimeMessage = ''
  }

  function handleTeamNameChange(value) {
    if (mode === 'empty') invalidateDiscovery()
    teamName = String(value ?? '')
    if (mode === 'empty') mode = 'setup'
  }

  function handleTeamDescriptionChange(value) {
    if (mode === 'empty') invalidateDiscovery()
    const next = ensureBuilderConfig()
    teamConfig = {
      ...next,
      description: String(value ?? ''),
    }
    if (mode === 'empty') mode = 'setup'
  }

  function handleAssignLeadRole(roleId) {
    if (mode === 'empty') invalidateDiscovery()
    const role = resolveBuilderRole(roleId)
    if (!role || normalizeRoleKind(role) !== 'lead') return
    const projectPath = getProjectPath()
    const next = ensureBuilderConfig()
      teamConfig = {
        ...next,
        lead: createLeadFromRole(role, projectPath),
      }
    if (!teamName.trim()) teamName = inferTeamName(projectPath)
    mode = 'setup'
    runtimeMessage = ''
  }

  function handleClearLead() {
    const next = ensureBuilderConfig()
    teamConfig = {
      ...next,
      lead: null,
    }
  }

  function handleAppendAgentRole(roleId) {
    if (mode === 'empty') invalidateDiscovery()
    const role = resolveBuilderRole(roleId)
    if (!role || normalizeRoleKind(role) === 'lead') return
    const projectPath = getProjectPath()
    const next = ensureBuilderConfig()
      teamConfig = {
        ...next,
        agents: [...(next.agents ?? []), createAgentFromRole(role, projectPath, next.agents ?? [])],
      }
    if (!teamName.trim()) teamName = inferTeamName(projectPath)
    mode = 'setup'
  }

  function handleUpdateLead(payload) {
    const next = ensureBuilderConfig()
    if (!next.lead) return
    teamConfig = {
      ...next,
      lead: {
        ...next.lead,
        ...payload,
      },
    }
  }

  function handleUpdateAgent(agentId, payload) {
    const next = ensureBuilderConfig()
    teamConfig = {
      ...next,
      agents: (next.agents ?? []).map((agent) => (
        agent.id === agentId
          ? { ...agent, ...payload }
          : agent
      )),
    }
  }

  function handleRemoveBuilderAgent(agentId) {
    const next = ensureBuilderConfig()
    teamConfig = {
      ...next,
      agents: (next.agents ?? []).filter((agent) => agent.id !== agentId),
    }
    if (selectedNodeId === agentId) selectedNodeId = null
  }

  function handleReorderBuilderAgent(sourceId, targetId) {
    if (!sourceId || !targetId || sourceId === targetId) return
    const next = ensureBuilderConfig()
    const currentAgents = [...(next.agents ?? [])]
    const sourceIndex = currentAgents.findIndex((agent) => agent.id === sourceId)
    const targetIndex = currentAgents.findIndex((agent) => agent.id === targetId)
    if (sourceIndex < 0 || targetIndex < 0) return
    const [moved] = currentAgents.splice(sourceIndex, 1)
    currentAgents.splice(targetIndex, 0, moved)
    teamConfig = {
      ...next,
      agents: currentAgents,
    }
  }

  function handleMoveBuilderAgentToEnd(sourceId) {
    if (!sourceId) return
    const next = ensureBuilderConfig()
    const currentAgents = [...(next.agents ?? [])]
    const sourceIndex = currentAgents.findIndex((agent) => agent.id === sourceId)
    if (sourceIndex < 0 || sourceIndex === currentAgents.length - 1) return
    const [moved] = currentAgents.splice(sourceIndex, 1)
    currentAgents.push(moved)
    teamConfig = {
      ...next,
      agents: currentAgents,
    }
  }

  async function handleSaveBuilderPreset() {
    const next = teamConfig
    const safeTeamName = String(teamName ?? '').trim()
    if (!next?.lead?.roleId || !safeTeamName) return

    try {
      await upsertTeamPreset({
        schema: {
          kind: 'team_preset',
          version: 1,
        },
        presetId: safeTeamName
          .toLowerCase()
          .replace(/[^a-z0-9\s_-]+/g, '')
          .replace(/[\s_]+/g, '-')
          .replace(/-+/g, '-')
          .replace(/^-|-$/g, '') || 'custom-preset',
        name: safeTeamName,
        description: String(next.description ?? '').trim() || 'Custom team preset',
        version: '1.0.0',
        leadRoleId: next.lead.roleId,
        agentSlots: (next.agents ?? []).map((agent) => ({
          roleId: agent.roleId || null,
          count: 1,
          projectBinding: 'lead_project',
          projectId: null,
          overrides: null,
        })),
        defaults: {
          teamNamePattern: '{project}-team',
          tmuxLayout: 'tiled',
        },
      })
      runtimeMessage = 'Preset saved to catalog.'
      errorMessage = ''
      presetsLoaded = false
      void loadTeamPresets()
    } catch (error) {
      errorMessage = error?.message || 'Failed to save preset.'
    }
  }

  function handleInitialize() {
    if (!canInitialize) return
    initProgress = buildInitializationRequest(teamConfig, teamName, getProjectPath())
    mode = 'initializing'
    selectedNodeId = null
    runtimeMessage = ''
  }

  async function handleInitializeSuccess(result) {
    const projectPath = getProjectPath()
    const completedRequest = initProgress
    const nextTeamName = result?.teamName || completedRequest?.teamName || teamName || inferTeamName(projectPath)
    teamName = nextTeamName
    initProgress = null
    runtimeMessage = result?.openedExisting ? 'Opened existing team.' : 'Team initialized successfully.'
    mode = 'runtime'
    selectedNodeId = null
    closeSlideOver()

    const sequence = ++discoverySequence
    try {
      await refreshProjectMeshSnapshot(sequence, { preserveNotices: true })
    } catch (error) {
      errorMessage = error?.message || 'Failed to load runtime team status.'
      teamConfig = {
        lead: createLead({ id: 'lead', name: 'team-lead', tool: 'claude', status: 'active' }, projectPath),
        agents: [],
        presetId: '',
        presetName: '',
        composition: null,
      }
    }
  }

  function openAddAgentPanel() {
    if (isResumingTeam) return
    const projectPath = getProjectPath()
    const projectOptions = (getAvailableProjects() ?? [])
      .map((project) => normalizeProjectOption(project, { stringLabel: 'raw', objectFallbackLabel: 'raw' }))
      .filter((project) => project.id)
    const defaultProject = projectOptions[0]?.id || projectPath || ''

    roleTemplates = []
    slideOver = 'addAgent'
    slideOverContext = {
      roleId: '',
      roleName: '',
      name: '',
      tool: 'codex',
      model: defaultModelForTool('codex'),
      projectId: defaultProject,
      description: '',
      instructions: '',
      focusArea: '',
      contextSummary: '',
      behaviorSummary: '',
      submitting: false,
      error: '',
      isLocked: false,
    }
    void loadRoleTemplates()
  }

  async function loadRoleTemplates() {
    loadingRoles = true
    try {
      const loaded = await listRoleTemplates()
      roleTemplates = Array.isArray(loaded) ? loaded.filter(Boolean) : []
    } catch (error) {
      console.error('Failed to load role templates:', error)
    } finally {
      loadingRoles = false
    }
  }

  async function loadTeamPresets() {
    loadingPresets = true
    try {
      const fetchedPresets = await listTeamPresets()
      availablePresets = mergePresetCatalog(quickPresets, Array.isArray(fetchedPresets) ? fetchedPresets : [])
    } catch (error) {
      console.error('Failed to load team presets:', error)
      availablePresets = quickPresets
    } finally {
      presetsLoaded = true
      loadingPresets = false
    }
  }

  function handleRoleChange(selectedRoleId) {
    const draft = addAgentDraft
    if (!draft) return
    if (!selectedRoleId) {
      slideOverContext = {
        ...draft,
        roleId: '',
        roleName: '',
        focusArea: '',
        contextSummary: '',
        behaviorSummary: '',
        instructions: '',
        isLocked: false,
      }
      return
    }

    const role = roleTemplates.find((entry) => entry.roleId === selectedRoleId)
    if (!role) return
    const tool = normalizeTool(role.cliTool || 'codex')
    const model = role.model || defaultModelForTool(tool)
    const instructions = role.instructions || ''
    slideOverContext = {
      ...draft,
      roleId: selectedRoleId,
      roleName: role.name || '',
      name: buildRuntimeAgentName(role, draft.projectId, teamConfig, getProjectPath()),
      tool,
      model,
      description: instructions,
      instructions,
      focusArea: role.focusArea || '',
      contextSummary: role.contextSummary || '',
      behaviorSummary: role.behaviorSummary || '',
      isLocked: true,
    }
  }

  function toggleAddAgentLock() {
    const draft = addAgentDraft
    if (!draft) return
    slideOverContext = { ...draft, isLocked: !draft.isLocked }
  }

  function updateAddAgentField(field, value) {
    const draft = addAgentDraft
    if (!draft) return
    const next = { ...draft, [field]: value }
    if (field === 'tool') next.model = defaultModelForTool(value)
    slideOverContext = next
  }

  async function submitAddAgent() {
    if (isResumingTeam) return
    const draft = addAgentDraft
    if (!draft || !canSubmitAddAgent) return
    slideOverContext = { ...draft, submitting: true, error: '' }

    try {
      const report = await coordinationAddAgent({
        teamName,
        agent: {
          name: String(draft.name || '').trim(),
          cliTool: normalizeTool(draft.tool),
          model: String(draft.model || '').trim(),
          projectId: String(draft.projectId || '').trim(),
          description: String(draft.description || '').trim() || null,
          roleId: String(draft.roleId || '').trim() || null,
          roleName: String(draft.roleName || '').trim() || null,
          focusArea: String(draft.focusArea || '').trim() || null,
          contextSummary: String(draft.contextSummary || '').trim() || null,
          behaviorSummary: String(draft.behaviorSummary || '').trim() || null,
          instructions: String(draft.instructions || '').trim() || null,
        },
      })

      onAddAgent(report)
      runtimeMessage = `Agent '${report?.memberName ?? String(draft.name || '').trim()}' added.`
      closeSlideOver()
      const sequence = ++discoverySequence
      await refreshProjectMeshSnapshot(sequence, { preserveNotices: true })
    } catch (error) {
      const latest = addAgentDraft
      if (!latest) return
      slideOverContext = { ...latest, submitting: false, error: error?.message || 'Failed to add agent.' }
    }
  }

  function openCaptureRoleDialog() {
    if (isResumingTeam) return
    if (!selectedNode || mode !== 'runtime') return
    const roleName = String(selectedNode.name || '').trim() || 'captured-role'
    const normalizedContract = normalizeBehavioralContract(selectedNode.behavioralContract)
    const description = String(selectedNode.description || '').trim()

    captureRoleDialog = {
      roleKind: selectedNode.role === 'lead' ? 'lead' : 'agent',
      name: roleName,
      roleId: slugifyRoleId(roleName),
      manualRoleId: false,
      tool: normalizeTool(selectedNode.tool),
      model: String(selectedNode.model || '').trim() || defaultModelForTool(selectedNode.tool),
      description,
      includeInstructions: description.length > 0,
      includeBehavioralContract: contractHasRules(normalizedContract),
      behavioralContract: normalizedContract,
      capabilities: Array.isArray(selectedNode.capabilities) ? selectedNode.capabilities : [],
      submitting: false,
      error: '',
    }
  }

  function closeCaptureRoleDialog() {
    captureRoleDialog = null
  }

  function updateCaptureRoleName(value) {
    const draft = captureRoleDraft
    if (!draft) return
    const name = String(value || '')
    captureRoleDialog = { ...draft, name, roleId: draft.manualRoleId ? draft.roleId : slugifyRoleId(name) }
  }

  function updateCaptureRoleId(value) {
    const draft = captureRoleDraft
    if (!draft) return
    captureRoleDialog = { ...draft, roleId: String(value || ''), manualRoleId: true }
  }

  function toggleCaptureRoleFlag(field) {
    const draft = captureRoleDraft
    if (!draft) return
    captureRoleDialog = { ...draft, [field]: !draft[field] }
  }

  async function submitCaptureRole() {
    const draft = captureRoleDraft
    if (!draft || !canSaveCapturedRole) return

    captureRoleDialog = { ...draft, submitting: true, error: '' }
    try {
      await upsertRoleTemplate(buildCapturedRoleTemplate(draft))
      runtimeMessage = 'Role saved to catalog'
      closeCaptureRoleDialog()
      void loadRoleTemplates()
    } catch (error) {
      const latest = captureRoleDraft
      if (!latest) return
      captureRoleDialog = { ...latest, submitting: false, error: error?.message || 'Failed to save role to catalog.' }
    }
  }

  async function handleConfirmAction() {
    if (isResumingTeam) return
    if (!confirmContext) return
    const action = confirmContext
    confirmContext = null

    if (action.kind === 'disband') {
      try {
        const projectPath = getProjectPath()
        const result = await coordinationDisbandTeam(teamName)
        invalidateDiscovery()
        clearMeshCache(projectPath)
        onDisband(result)
        runtimeMessage = result?.alreadyDisbanded
          ? 'Team was already disbanded.'
          : 'Team disbanded and active sessions were stopped.'
        mode = 'empty'
        selectedNodeId = null
        teamConfig = null
        teamRuntimeState = 'none'
      } catch (error) {
        errorMessage = error?.message || 'Failed to disband team.'
      }
      return
    }

    if (action.kind === 'remove' && action.memberName) {
      try {
        const report = await coordinationRemoveMember(teamName, action.memberName)
        onRemoveAgent(report)
        runtimeMessage = `Removed '${action.memberName}'.`
        selectedNodeId = null
        const sequence = ++discoverySequence
        await refreshProjectMeshSnapshot(sequence, { preserveNotices: true })
      } catch (error) {
        errorMessage = error?.message || `Failed to remove member '${action.memberName}'.`
      }
    }
  }

  async function resumeSelected() {
    if (isResumingTeam) return
    const currentNode = selectedNode
    if (!currentNode || currentNode.role !== 'agent') return
    try {
      const report = await coordinationResumeMember(
        teamName,
        currentNode.name,
      )
      if (!report?.resumed) {
        errorMessage = report?.message || `Failed to resume member '${currentNode.name}'.`
        return
      }
      runtimeMessage = `Resumed '${currentNode.name}'.`
      selectedNodeId = null
      const sequence = ++discoverySequence
      await refreshProjectMeshSnapshot(sequence, { preserveNotices: true })
    } catch (error) {
      errorMessage = error?.message || `Failed to resume member '${currentNode.name}'.`
    }
  }

  function stopSelected() {
    if (!selectedNode) return
    if (selectedNode.role === 'lead') {
      confirmContext = { kind: 'disband' }
      return
    }
    confirmContext = { kind: 'remove', memberName: selectedNode.name }
  }

  function focusSelectedPane() {
    if (selectedNode?.paneId) onFocusPane(selectedNode.paneId)
  }

  function handleReset() {
    teamConfig = null
    teamName = inferTeamName(getProjectPath())
    selectedNodeId = null
    initProgress = null
    mode = 'empty'
    runtimeMessage = ''
    errorMessage = ''
    closeSlideOver()
  }

  function toggleNode(nodeId) {
    selectedNodeId = String(selectedNodeId) === String(nodeId) ? null : String(nodeId)
  }

  function clearSelectedNode() {
    selectedNodeId = null
  }

  function openTemplates() {
    if (roleTemplates.length === 0 && !loadingRoles) {
      void loadRoleTemplates()
    }
    if (!presetsLoaded && !loadingPresets) {
      void loadTeamPresets()
    }
  }

  function requestDisband() {
    if (isResumingTeam) return
    if (teamName) confirmContext = { kind: 'disband' }
  }

  async function resumeTeam() {
    if (!teamName || !canResumeTeam || isResumingTeam) return

    const targetNames = buildResumeTargetNames(teamConfig)
    teamResumeProgress = {
      inFlight: true,
      items: buildResumeProgressItems(targetNames),
    }
    errorMessage = ''
    runtimeMessage = ''

    try {
      const report = await coordinationResumeTeam(teamName)
      teamResumeProgress = {
        inFlight: false,
        items: buildResumeProgressItems(targetNames, report),
      }

      const normalizedReport = normalizeResumeTeamReport(report)
      if (!normalizedReport?.resumed && normalizedReport?.failedMembers?.length) {
        runtimeMessage = buildResumeTeamMessage(normalizedReport)
      } else {
        runtimeMessage = buildResumeTeamMessage(normalizedReport)
      }

      const sequence = ++discoverySequence
      await refreshProjectMeshSnapshot(sequence, { preserveNotices: true })
    } catch (error) {
      const message = error?.message || 'Failed to resume team.'
      teamResumeProgress = {
        inFlight: false,
        items: buildResumeProgressItems(targetNames, null, message),
      }
      errorMessage = message
    }
  }

  function cancelConfirm() {
    confirmContext = null
  }

  $effect(() => {
    const projectPath = getProjectPath()
    const isVisible = getIsVisible()
    const backgroundWorkEnabled = getBackgroundWorkEnabled()
    if (!isVisible || !backgroundWorkEnabled) {
      invalidateDiscovery()
      return
    }
    untrack(() => {
      mode = 'empty'
      teamName = ''
      teamConfig = null
      slideOver = null
      slideOverContext = null
      captureRoleDialog = null
      selectedNodeId = null
      initProgress = null
      errorMessage = ''
      runtimeMessage = ''
      confirmContext = null
      availabilityMessage = ''
      clearRuntimeTeamRefresh({ dropInFlight: true })
      ensureHydrated(projectPath, { isVisible, backgroundWorkEnabled })
    })
  })

  $effect(() => {
    if (mode !== 'empty' && mode !== 'setup') return
    if (!teamName.trim()) {
      teamName = inferTeamName(getProjectPath())
    }
    if (loadingRoles || roleTemplates.length > 0) return
    void loadRoleTemplates()
  })

  $effect(() => {
    if (mode !== 'empty' && mode !== 'setup') return
    if (loadingPresets || presetsLoaded) return
    void loadTeamPresets()
  })

  $effect(() => {
    if (!selectedNodeId) {
      captureRoleDialog = null
      return
    }
    if (!selectedNode) {
      selectedNodeId = null
      captureRoleDialog = null
    }
  })

  $effect(() => {
    if (teamResumeProgress?.inFlight) {
      if (teamResumeProgressTimer) {
        clearTimeout(teamResumeProgressTimer)
        teamResumeProgressTimer = null
      }
      return
    }

    return autoDismissNotice({
      value: teamResumeProgress ? 'completed' : '',
      timeoutMs: 5000,
      getTimer: () => teamResumeProgressTimer,
      setTimer: (timer) => {
        teamResumeProgressTimer = timer
      },
      clearValue: () => {
        teamResumeProgress = null
      },
    })
  })

  $effect(() => {
    return autoDismissNotice({
      value: runtimeMessage,
      timeoutMs: 5000,
      getTimer: () => runtimeMessageTimer,
      setTimer: (timer) => {
        runtimeMessageTimer = timer
      },
      clearValue: () => {
        runtimeMessage = ''
      },
    })
  })

  $effect(() => {
    return autoDismissNotice({
      value: errorMessage,
      timeoutMs: 8000,
      getTimer: () => errorMessageTimer,
      setTimer: (timer) => {
        errorMessageTimer = timer
      },
      clearValue: () => {
        errorMessage = ''
      },
    })
  })

  $effect(() => {
    return () => {
      clearProjectSnapshotRefresh()
    }
  })

  $effect(() => {
    if (mode !== 'runtime' || !teamName || !getIsVisible() || !getBackgroundWorkEnabled()) return

    let disposed = false
    let isPolling = false

    const scheduleNextPoll = (delayMs = RUNTIME_STATUS_POLL_MS) => {
      if (disposed) return
      runtimePollTimer = setTimeout(() => {
        void pollRuntimeStatus().finally(() => {
          scheduleNextPoll()
        })
      }, delayMs)
    }

    const pollRuntimeStatus = async () => {
      if (isPolling) return
      if (isResumingTeam) return
      if (runtimeStatusRequest) return
      isPolling = true
      try {
        await queueRuntimeTeamRefresh(teamName, discoverySequence)
      } catch (error) {
        if (disposed) return
        console.warn('[meshTab] runtime status refresh failed:', error)
      } finally {
        isPolling = false
      }
    }

    scheduleNextPoll(RUNTIME_STATUS_POLL_MS + INITIAL_RUNTIME_REFRESH_DELAY_MS)

    return () => {
      disposed = true
      clearRuntimeTeamRefresh()
      clearProjectSnapshotRefresh()
      if (runtimePollTimer) {
        clearTimeout(runtimePollTimer)
        runtimePollTimer = null
      }
    }
  })

  return {
    get quickPresets() {
      return availablePresets
    },
    get mode() {
      return mode
    },
    get teamName() {
      return teamName
    },
    get teamConfig() {
      return teamConfig
    },
    get slideOver() {
      return slideOver
    },
    get slideOverContext() {
      return slideOverContext
    },
    get selectedNodeId() {
      return selectedNodeId
    },
    get initProgress() {
      return initProgress
    },
    get errorMessage() {
      return errorMessage
    },
    get runtimeMessage() {
      return runtimeMessage
    },
    get availabilityMessage() {
      return availabilityMessage
    },
    get confirmContext() {
      return confirmContext
    },
    get roleTemplates() {
      return roleTemplates
    },
    get teamRuntimeState() {
      return teamRuntimeState
    },
    get isResumingTeam() {
      return isResumingTeam
    },
    get teamResumeProgress() {
      return teamResumeProgress
    },
    get loadingRoles() {
      return loadingRoles
    },
    get selectedNode() {
      return selectedNode
    },
    get addAgentDraft() {
      return addAgentDraft
    },
    get canInitialize() {
      return canInitialize
    },
    get canSubmitAddAgent() {
      return canSubmitAddAgent
    },
    get captureRoleDraft() {
      return captureRoleDraft
    },
    get canSaveCapturedRole() {
      return canSaveCapturedRole
    },
    ensureGateReady,
    closeSlideOver,
    handlePresetSelect,
    handleStartCustom,
    handleTeamNameChange,
    handleTeamDescriptionChange,
    handleAssignLeadRole,
    handleClearLead,
    handleAppendAgentRole,
    handleUpdateLead,
    handleUpdateAgent,
    handleRemoveBuilderAgent,
    handleReorderBuilderAgent,
    handleMoveBuilderAgentToEnd,
    handleSaveBuilderPreset,
    handleInitialize,
    handleInitializeSuccess,
    openAddAgentPanel,
    handleRoleChange,
    toggleAddAgentLock,
    updateAddAgentField,
    submitAddAgent,
    openCaptureRoleDialog,
    closeCaptureRoleDialog,
    updateCaptureRoleName,
    updateCaptureRoleId,
    toggleCaptureRoleFlag,
    submitCaptureRole,
    handleConfirmAction,
    resumeTeam,
    resumeSelected,
    stopSelected,
    focusSelectedPane,
    handleReset,
    toggleNode,
    clearSelectedNode,
    openTemplates,
    requestDisband,
    cancelConfirm,
    dismissError: () => {
      errorMessage = ''
    },
    dismissRuntimeMessage: () => {
      runtimeMessage = ''
    },
    setInitializingBack: () => {
      initProgress = null
      mode = 'setup'
    },
  }
}
