import {
  INITIAL_RUNTIME_REFRESH_DELAY_MS,
  RUNTIME_STATUS_POLL_MS,
} from './meshTabGate.svelte.js'

function buildResumeTargetNames(config) {
  const members = [config?.lead, ...(config?.agents ?? [])].filter(Boolean)
  const offlineMembers = members
    .filter((member) => String(member?.status ?? '').trim().toLowerCase() === 'offline')
    .map((member) => String(member?.name ?? '').trim())
    .filter(Boolean)
  if (offlineMembers.length > 0) return offlineMembers
  return members.map((member) => String(member?.name ?? '').trim()).filter(Boolean)
}

function buildResumeProgressItems(targetNames, normalizeResumeTeamReport, report = null, fallbackError = '') {
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

function buildResumeTeamMessage(normalizeResumeTeamReport, report) {
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

export function createMeshTabRuntime({ state, refs, deps, gate }) {
  async function handleConfirmAction() {
    if (state.isResumingTeam) return
    if (!state.confirmContext) return
    const action = state.confirmContext
    state.confirmContext = null

    if (action.kind === 'disband') {
      try {
        const projectPath = deps.getProjectPath()
        const result = await deps.coordinationDisbandTeam(state.teamName)
        gate.invalidateDiscovery()
        deps.clearMeshCache(projectPath)
        deps.onDisband(result)
        state.runtimeMessage = result?.alreadyDisbanded
          ? 'Team was already disbanded.'
          : 'Team disbanded and active sessions were stopped.'
        state.mode = 'empty'
        state.selectedNodeId = null
        state.teamConfig = null
        state.teamRuntimeState = 'none'
      } catch (error) {
        state.errorMessage = error?.message || 'Failed to disband team.'
      }
      return
    }

    if (action.kind === 'remove' && action.memberName) {
      try {
        const report = await deps.coordinationRemoveMember(state.teamName, action.memberName)
        deps.onRemoveAgent(report)
        state.runtimeMessage = `Removed '${action.memberName}'.`
        state.selectedNodeId = null
        const sequence = ++refs.discoverySequence
        await gate.refreshProjectMeshSnapshot(sequence, { preserveNotices: true })
      } catch (error) {
        state.errorMessage = error?.message || `Failed to remove member '${action.memberName}'.`
      }
    }
  }

  async function resumeSelected() {
    if (state.isResumingTeam) return
    const currentNode = state.selectedNode
    if (!currentNode || currentNode.role !== 'agent') return
    try {
      const report = await deps.coordinationResumeMember(state.teamName, currentNode.name)
      if (!report?.resumed) {
        state.errorMessage = report?.message || `Failed to resume member '${currentNode.name}'.`
        return
      }
      state.runtimeMessage = `Resumed '${currentNode.name}'.`
      state.selectedNodeId = null
      const sequence = ++refs.discoverySequence
      await gate.refreshProjectMeshSnapshot(sequence, { preserveNotices: true })
    } catch (error) {
      state.errorMessage = error?.message || `Failed to resume member '${currentNode.name}'.`
    }
  }

  function stopSelected() {
    if (!state.selectedNode) return
    if (state.selectedNode.role === 'lead') {
      state.confirmContext = { kind: 'disband' }
      return
    }
    state.confirmContext = { kind: 'remove', memberName: state.selectedNode.name }
  }

  function focusSelectedPane() {
    if (state.selectedNode?.paneId) deps.onFocusPane(state.selectedNode.paneId)
  }

  function toggleNode(nodeId) {
    state.selectedNodeId = String(state.selectedNodeId) === String(nodeId) ? null : String(nodeId)
  }

  function clearSelectedNode() {
    state.selectedNodeId = null
  }

  function requestDisband() {
    if (state.isResumingTeam) return
    if (state.teamName) state.confirmContext = { kind: 'disband' }
  }

  async function resumeTeam() {
    if (!state.teamName || !state.canResumeTeam || state.isResumingTeam) return

    const targetNames = buildResumeTargetNames(state.teamConfig)
    state.teamResumeProgress = {
      inFlight: true,
      items: buildResumeProgressItems(targetNames, deps.normalizeResumeTeamReport),
    }
    state.errorMessage = ''
    state.runtimeMessage = ''

    try {
      const report = await deps.coordinationResumeTeam(state.teamName)
      state.teamResumeProgress = {
        inFlight: false,
        items: buildResumeProgressItems(targetNames, deps.normalizeResumeTeamReport, report),
      }

      state.runtimeMessage = buildResumeTeamMessage(deps.normalizeResumeTeamReport, report)

      const sequence = ++refs.discoverySequence
      await gate.refreshProjectMeshSnapshot(sequence, { preserveNotices: true })
    } catch (error) {
      const message = error?.message || 'Failed to resume team.'
      state.teamResumeProgress = {
        inFlight: false,
        items: buildResumeProgressItems(targetNames, deps.normalizeResumeTeamReport, null, message),
      }
      state.errorMessage = message
    }
  }

  function cancelConfirm() {
    state.confirmContext = null
  }

  function createRuntimePollingEffect() {
    if (state.mode !== 'runtime' || !state.teamName || !deps.getIsVisible() || !deps.getBackgroundWorkEnabled()) {
      return undefined
    }

    let disposed = false
    let isPolling = false

    const scheduleNextPoll = (delayMs = RUNTIME_STATUS_POLL_MS) => {
      if (disposed) return
      refs.runtimePollTimer = setTimeout(() => {
        void pollRuntimeStatus().finally(() => {
          scheduleNextPoll()
        })
      }, delayMs)
    }

    const pollRuntimeStatus = async () => {
      if (isPolling) return
      if (state.isResumingTeam) return
      if (refs.runtimeStatusRequest) return
      isPolling = true
      try {
        await gate.queueRuntimeTeamRefresh(state.teamName, refs.discoverySequence)
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
      gate.clearRuntimeTeamRefresh()
      gate.clearProjectSnapshotRefresh()
      if (refs.runtimePollTimer) {
        clearTimeout(refs.runtimePollTimer)
        refs.runtimePollTimer = null
      }
    }
  }

  return {
    cancelConfirm,
    clearSelectedNode,
    createRuntimePollingEffect,
    focusSelectedPane,
    handleConfirmAction,
    requestDisband,
    resumeSelected,
    resumeTeam,
    stopSelected,
    toggleNode,
  }
}
