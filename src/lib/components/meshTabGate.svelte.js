const PROJECT_SNAPSHOT_CACHE_MAX_AGE_MS = 5000

export const INITIAL_RUNTIME_REFRESH_DELAY_MS = 120
export const RUNTIME_STATUS_POLL_MS = 2000

export function createMeshTabGate({ state, refs, deps }) {
  function isInternalDiscoveryWarning(message) {
    return String(message || '').trim().toLowerCase().startsWith('skipped team folder ')
  }

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
    refs.hydrationPerf = {
      projectPath,
      sequence,
      startedAt: nowMs(),
    }
    logProjectSwitchPerf('mesh-hydrate-start', { projectPath, sequence })
  }

  function finishHydrationPerf(stage, sequence, extra = {}) {
    const hydrationPerf = refs.hydrationPerf
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
      refs.hydrationPerf = null
    }
  }

  function classifyTeamRuntimeStateFromMembers(teamName, members) {
    if (!teamName) return 'none'
    const roster = Array.isArray(members) ? members : []
    if (roster.length === 0) return 'active'
    const liveMembers = roster.filter((member) => {
      const status = String(member?.sessionStatus ?? member?.status ?? '').trim().toLowerCase()
      return status === 'active' || status === 'idle'
    }).length
    if (liveMembers === 0) return 'coldResume'
    if (liveMembers === roster.length) return 'active'
    return 'degraded'
  }

  function buildAvailabilityMessage(snapshot) {
    const messages = []
    if (!snapshot.meshAvailable) messages.push('Mesh CLI is unavailable for this environment.')
    if (!snapshot.tmuxAvailable) messages.push('tmux is unavailable for this environment.')
    for (const warning of snapshot.warnings) {
      const message = String(warning || '').trim()
      if (isInternalDiscoveryWarning(message)) {
        console.warn('[mesh] suppressed internal discovery warning:', message)
        continue
      }
      if (message && !messages.includes(message)) messages.push(message)
    }
    return messages.join(' ')
  }

  function buildCachedSnapshotFromLiveStatus(snapshot, report) {
    const normalized = deps.normalizeProjectMeshSnapshot(snapshot)
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
    const normalized = deps.normalizeProjectMeshSnapshot(snapshot)
    state.availabilityMessage = buildAvailabilityMessage(normalized)
    state.teamRuntimeState = normalized.teamRuntimeState
    if (!preserveNotices) {
      state.errorMessage = ''
      state.runtimeMessage = ''
    }

    if (normalized.teamName && normalized.teamStatus) {
      state.teamName = normalized.teamName
      state.teamConfig = deps.buildTeamConfigFromRuntimeStatus(
        {
          teamName: normalized.teamName,
          leadName: normalized.teamStatus?.leadName ?? 'team-lead',
          members: Array.isArray(normalized.teamStatus?.members) ? normalized.teamStatus.members : [],
        },
        projectPath
      )
      state.mode = 'runtime'
      return normalized
    }

    state.teamName = deps.inferTeamName(projectPath)
    state.teamConfig = null
    state.teamRuntimeState = 'none'
    state.mode = 'empty'
    return normalized
  }

  function getProjectSnapshot(projectPath) {
    const normalizedProjectPath = String(projectPath ?? '').trim()
    if (!normalizedProjectPath) {
      return Promise.resolve(null)
    }
    if (refs.pendingProjectSnapshot?.projectPath === normalizedProjectPath) {
      return refs.pendingProjectSnapshot.promise
    }

    const promise = deps.coordinationGetProjectMeshSnapshot(normalizedProjectPath).finally(() => {
      if (
        refs.pendingProjectSnapshot?.projectPath === normalizedProjectPath &&
        refs.pendingProjectSnapshot?.promise === promise
      ) {
        refs.pendingProjectSnapshot = null
      }
    })

    refs.pendingProjectSnapshot = {
      projectPath: normalizedProjectPath,
      promise,
    }
    return promise
  }

  async function refreshProjectMeshSnapshot(sequence, options = {}) {
    const projectPath = deps.getProjectPath()
    const snapshot = await getProjectSnapshot(projectPath)
    if (!snapshot) return null
    if (sequence !== refs.discoverySequence) return null
    deps.setMeshCache(projectPath, snapshot)
    const normalized = applyProjectSnapshot(snapshot, projectPath, options)
    finishHydrationPerf(
      normalized.teamName && normalized.teamStatus ? 'mesh-hydrate-ready' : 'mesh-hydrate-empty',
      sequence,
      {
        mode: state.mode,
        teamName: normalized.teamName,
      }
    )
    if (normalized.teamName && normalized.teamStatus) {
      scheduleRuntimeTeamRefresh(normalized.teamName, sequence, snapshot)
    }
    return normalized
  }

  function clearRuntimeTeamRefresh({ dropInFlight = false } = {}) {
    if (refs.runtimeRefreshTimer) {
      clearTimeout(refs.runtimeRefreshTimer)
      refs.runtimeRefreshTimer = null
      if (refs.runtimeRefreshMeta) {
        finishHydrationPerf('mesh-refresh-cancelled', refs.runtimeRefreshMeta.sequence, {
          teamName: refs.runtimeRefreshMeta.teamName,
        })
      }
      refs.runtimeRefreshMeta = null
    }
    refs.queuedRuntimeStatusRequest = null
    if (dropInFlight) {
      refs.runtimeStatusRequest = null
      refs.runtimeStatusRequestMeta = null
    }
  }

  function clearProjectSnapshotRefresh() {
    if (!refs.projectSnapshotRefreshTimer) return
    clearTimeout(refs.projectSnapshotRefreshTimer)
    refs.projectSnapshotRefreshTimer = null
  }

  function scheduleProjectSnapshotRefresh(sequence) {
    clearProjectSnapshotRefresh()
    refs.projectSnapshotRefreshTimer = setTimeout(() => {
      refs.projectSnapshotRefreshTimer = null
      if (sequence !== refs.discoverySequence) return
      if (!deps.getIsVisible() || !deps.getBackgroundWorkEnabled()) return
      void refreshProjectMeshSnapshot(sequence, { preserveNotices: true }).catch((error) => {
        if (sequence !== refs.discoverySequence) return
        console.warn('[meshTab] background project snapshot refresh failed:', error)
      })
    }, INITIAL_RUNTIME_REFRESH_DELAY_MS)
  }

  function scheduleRuntimeTeamRefresh(nextTeamName, sequence, snapshot = null) {
    clearRuntimeTeamRefresh()
    refs.runtimeRefreshMeta = {
      sequence,
      teamName: nextTeamName,
    }
    logProjectSwitchPerf('mesh-refresh-scheduled', {
      projectPath: deps.getProjectPath(),
      sequence,
      teamName: nextTeamName,
      delayMs: INITIAL_RUNTIME_REFRESH_DELAY_MS,
    })
    refs.runtimeRefreshTimer = setTimeout(() => {
      refs.runtimeRefreshTimer = null
      const meta = refs.runtimeRefreshMeta
      refs.runtimeRefreshMeta = null
      if (sequence !== refs.discoverySequence) return
      logProjectSwitchPerf('mesh-refresh-start', {
        projectPath: deps.getProjectPath(),
        sequence,
        teamName: nextTeamName,
      })
      void queueRuntimeTeamRefresh(nextTeamName, sequence, snapshot)
        .catch((error) => {
          if (sequence !== refs.discoverySequence) return
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
    await deps.refreshRuntimeTeamConfigWorkflow({
      nextTeamName,
      sequence,
      getDiscoverySequence: () => refs.discoverySequence,
      coordinationGetLiveTeamStatus: deps.coordinationGetLiveTeamStatus,
      buildTeamConfigFromRuntimeStatus: deps.buildTeamConfigFromRuntimeStatus,
      getProjectPath: deps.getProjectPath,
      onTeamConfig: (value) => {
        nextConfig = value
        state.teamConfig = value
      },
    })
    if (!nextConfig) return

    const liveStatusMembers = [
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
    ].filter(Boolean)

    state.teamRuntimeState = classifyTeamRuntimeStateFromMembers(nextTeamName, liveStatusMembers)
    if (!snapshot) return

    deps.setMeshCache(
      deps.getProjectPath(),
      buildCachedSnapshotFromLiveStatus(snapshot, {
        teamName: nextTeamName,
        leadName: nextConfig.lead?.name ?? 'team-lead',
        members: liveStatusMembers,
      })
    )
  }

  function queueRuntimeTeamRefresh(nextTeamName, sequence, snapshot = null) {
    if (refs.runtimeStatusRequest) {
      if (
        refs.runtimeStatusRequestMeta?.sequence === sequence &&
        refs.runtimeStatusRequestMeta?.teamName === nextTeamName
      ) {
        return refs.runtimeStatusRequest
      }

      refs.queuedRuntimeStatusRequest = { nextTeamName, sequence, snapshot }
      return refs.runtimeStatusRequest
    }

    refs.runtimeStatusRequestMeta = { teamName: nextTeamName, sequence }
    const request = refreshRuntimeTeamConfig(nextTeamName, sequence, snapshot)
    const wrappedRequest = request.finally(() => {
      if (refs.runtimeStatusRequest === wrappedRequest) {
        refs.runtimeStatusRequest = null
        refs.runtimeStatusRequestMeta = null
      }

      const queued = refs.queuedRuntimeStatusRequest
      refs.queuedRuntimeStatusRequest = null
      if (
        queued &&
        queued.sequence === refs.discoverySequence &&
        queued.nextTeamName &&
        deps.getIsVisible() &&
        deps.getBackgroundWorkEnabled()
      ) {
        void queueRuntimeTeamRefresh(queued.nextTeamName, queued.sequence, queued.snapshot)
      }
    })

    refs.runtimeStatusRequest = wrappedRequest
    return wrappedRequest
  }

  async function hydrateProjectMesh(projectPath, sequence) {
    try {
      await refreshProjectMeshSnapshot(sequence)
    } catch (error) {
      if (sequence !== refs.discoverySequence) return
      state.availabilityMessage = ''
      state.errorMessage = error?.message || 'Failed to load Mesh team state.'
      state.teamName = deps.inferTeamName(projectPath)
      state.teamConfig = null
      state.mode = 'empty'
    }
  }

  function ensureHydrated(projectPath, { isVisible = true, backgroundWorkEnabled = true } = {}) {
    if (!projectPath || !isVisible || !backgroundWorkEnabled) return
    const sequence = ++refs.discoverySequence
    beginHydrationPerf(projectPath, sequence)
    state.teamName = deps.inferTeamName(projectPath)
    state.teamConfig = null
    state.selectedNodeId = null
    state.initProgress = null
    state.slideOver = null
    state.slideOverContext = null
    state.captureRoleDialog = null
    state.confirmContext = null
    state.availabilityMessage = ''
    state.teamRuntimeState = 'none'
    state.teamResumeProgress = null
    state.errorMessage = ''
    state.runtimeMessage = ''
    clearRuntimeTeamRefresh({ dropInFlight: true })
    clearProjectSnapshotRefresh()

    const cachedEntry = deps.untrack(() => deps.getMeshCacheEntry(projectPath))
    const cachedSnapshot = cachedEntry?.snapshot ?? null
    if (cachedSnapshot) {
      const normalized = applyProjectSnapshot(cachedSnapshot, projectPath)
      finishHydrationPerf(
        normalized.teamName && normalized.teamStatus ? 'mesh-hydrate-ready' : 'mesh-hydrate-empty',
        sequence,
        {
          mode: state.mode,
          teamName: normalized.teamName,
          source: 'cache',
        }
      )
      const cachedAgeMs =
        typeof cachedEntry?.cachedAtMs === 'number'
          ? Math.max(0, Date.now() - cachedEntry.cachedAtMs)
          : Infinity
      if (cachedAgeMs >= PROJECT_SNAPSHOT_CACHE_MAX_AGE_MS) {
        scheduleProjectSnapshotRefresh(sequence)
      } else if (normalized.teamName && normalized.teamStatus) {
        scheduleRuntimeTeamRefresh(normalized.teamName, sequence, cachedSnapshot)
      }
      return
    }

    state.mode = 'gate'
    void hydrateProjectMesh(projectPath, sequence)
  }

  function ensureGateReady() {
    const projectPath = deps.getProjectPath()
    if (!projectPath) return
    if (refs.pendingProjectSnapshot?.projectPath === projectPath) return
    ensureHydrated(projectPath, {
      isVisible: deps.getIsVisible(),
      backgroundWorkEnabled: deps.getBackgroundWorkEnabled(),
    })
  }

  function invalidateDiscovery() {
    refs.discoverySequence += 1
    clearRuntimeTeamRefresh({ dropInFlight: true })
    clearProjectSnapshotRefresh()
    refs.hydrationPerf = null
  }

  return {
    applyProjectSnapshot,
    buildCachedSnapshotFromLiveStatus,
    classifyTeamRuntimeStateFromMembers,
    clearProjectSnapshotRefresh,
    clearRuntimeTeamRefresh,
    ensureGateReady,
    ensureHydrated,
    finishHydrationPerf,
    invalidateDiscovery,
    queueRuntimeTeamRefresh,
    refreshProjectMeshSnapshot,
    scheduleProjectSnapshotRefresh,
    scheduleRuntimeTeamRefresh,
  }
}
