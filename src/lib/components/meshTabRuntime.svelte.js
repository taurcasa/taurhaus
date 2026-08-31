import { activityLevel, isLiveLevel } from '../activitySignal.js'
import {
  INITIAL_RUNTIME_REFRESH_DELAY_MS,
  RUNTIME_STATUS_POLL_MS,
} from './meshTabGate.svelte.js'

const RESUME_STAGE_LABELS = {
  prepare_member: 'Preparing member',
  acquire_pane: 'Acquiring pane',
  launch_session: 'Launching session',
  capture_session_identity: 'Capturing session identity',
  join_mesh: 'Joining mesh',
  start_member_daemon: 'Starting member daemon',
  commit_runtime: 'Saving runtime state',
  deliver_onboarding: 'Delivering onboarding',
}

function uniqueMemberNames(names) {
  return [...new Set((names ?? []).map((value) => String(value ?? '').trim()).filter(Boolean))]
}

function buildResumeTargetNames(config) {
  const members = [config?.lead, ...(config?.agents ?? [])].filter(Boolean)
  const offlineMembers = members
    .filter((member) => !isLiveLevel(activityLevel(member)))
    .map((member) => String(member?.name ?? '').trim())
    .filter(Boolean)
  if (offlineMembers.length > 0) return uniqueMemberNames(offlineMembers)
  return uniqueMemberNames(members.map((member) => String(member?.name ?? '').trim()))
}

function formatResumeStage(stage) {
  const normalized = String(stage ?? '').trim()
  if (!normalized) return 'Waiting'
  return (
    RESUME_STAGE_LABELS[normalized] ??
    normalized
      .split('_')
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(' ')
  )
}

function defaultResumeMessage(stage, status) {
  const label = formatResumeStage(stage)
  if (status === 'running') return `${label}...`
  if (status === 'succeeded') return label
  if (status === 'failed') return `${label} failed`
  return 'Waiting to resume'
}

function createResumeProgressItem(memberName, memberIndex = 0) {
  return {
    memberName,
    memberIndex,
    status: 'pending',
    stage: '',
    stageLabel: '',
    message: 'Waiting to resume',
    locked: false,
  }
}

function sortResumeItems(items) {
  return [...items].sort((left, right) => {
    const leftIndex = Number(left?.memberIndex ?? 0)
    const rightIndex = Number(right?.memberIndex ?? 0)
    if (leftIndex !== rightIndex) return leftIndex - rightIndex
    return String(left?.memberName ?? '').localeCompare(String(right?.memberName ?? ''))
  })
}

function createResumeProgressState(targetNames) {
  const names = uniqueMemberNames(targetNames)
  const items = names.map((memberName, index) => createResumeProgressItem(memberName, index + 1))
  return {
    inFlight: true,
    memberCount: items.length,
    completedCount: 0,
    currentIndex: items.length > 0 ? 1 : 0,
    activeMemberName: items[0]?.memberName ?? '',
    activeStage: '',
    activeStageLabel: '',
    items,
    summaryMessage: '',
    footerMessage: '',
  }
}

function updateResumeProgressMeta(progress) {
  const items = sortResumeItems(progress?.items ?? [])
  const memberCount = Math.max(
    Number(progress?.memberCount ?? 0),
    items.length
  )
  const completedCount = items.filter(
    (item) => item.status === 'succeeded' || item.status === 'failed'
  ).length
  const activeItem = items.find((item) => item.status === 'running') ?? null
  const currentIndex = activeItem
    ? Number(activeItem.memberIndex ?? 0)
    : progress?.inFlight
      ? Math.min(memberCount, completedCount + (memberCount > 0 ? 1 : 0))
      : completedCount

  return {
    ...progress,
    items,
    memberCount,
    completedCount,
    currentIndex,
    activeMemberName: activeItem?.memberName ?? '',
    activeStage: activeItem?.stage ?? '',
    activeStageLabel: activeItem?.stageLabel ?? '',
  }
}

function applyResumeProgressEvent(progress, normalizedEvent) {
  if (!normalizedEvent) return progress

  const current = progress
    ? {
        ...progress,
        items: [...(progress.items ?? [])],
      }
    : createResumeProgressState([normalizedEvent.memberName])

  const items = current.items.map((item) => {
    if (item.status !== 'running' || item.locked || item.memberName === normalizedEvent.memberName) {
      return item
    }
    return {
      ...item,
      status: 'pending',
      message: 'Waiting to resume',
    }
  })

  let index = items.findIndex((item) => item.memberName === normalizedEvent.memberName)
  if (index === -1) {
    items.push(
      createResumeProgressItem(
        normalizedEvent.memberName,
        normalizedEvent.memberIndex || items.length + 1
      )
    )
    index = items.length - 1
  }

  const existing = items[index]
  if (!existing.locked) {
    items[index] = {
      ...existing,
      memberIndex: normalizedEvent.memberIndex || existing.memberIndex,
      status: normalizedEvent.status,
      stage: normalizedEvent.stage,
      stageLabel: formatResumeStage(normalizedEvent.stage),
      message:
        normalizedEvent.message ||
        defaultResumeMessage(normalizedEvent.stage, normalizedEvent.status),
      locked:
        normalizedEvent.status === 'succeeded' || normalizedEvent.status === 'failed',
    }
  }

  return updateResumeProgressMeta({
    ...current,
    inFlight: true,
    memberCount: Math.max(
      Number(current.memberCount ?? 0),
      Number(normalizedEvent.memberCount ?? 0),
      items.length
    ),
    items,
    summaryMessage: '',
    footerMessage: '',
  })
}

function buildResumeTeamMessage(normalizeResumeTeamReport, report) {
  const normalizedReport = normalizeResumeTeamReport(report)
  if (!normalizedReport) return 'Team resume finished.'
  const resumedSummary = normalizedReport.resumedMembers.length
    ? `Resumed: ${normalizedReport.resumedMembers.join(', ')}.`
    : 'All members were already running.'
  const failedSummary = normalizedReport.failedMembers.length
    ? `Failed: ${normalizedReport.failedMembers
        .map((entry) => `${entry?.memberName ?? 'unknown'}${entry?.message ? ` (${entry.message})` : ''}`)
        .join(', ')}.`
    : ''
  if (normalizedReport.failedMembers.length > 0) {
    return `Resume completed with failures. ${resumedSummary} ${failedSummary}`.trim()
  }
  if (!normalizedReport.resumedMembers.length) {
    return 'All members were already running.'
  }
  if (normalizedReport.teamDaemonWarning) {
    return `Resume completed with a background service warning. ${resumedSummary}`.trim()
  }
  return `Resume complete. ${resumedSummary}`.trim()
}

function formatResumeWarning(warning) {
  if (!warning) return ''
  if (warning.includes(': created a replacement pane')) {
    const [memberName] = warning.split(':', 1)
    return `${memberName}: started a replacement terminal session.`
  }
  return warning
}

export function buildMemberActionMessage(message, warnings) {
  const formattedWarnings = (warnings ?? [])
    .map((warning) => formatResumeWarning(warning))
    .filter(Boolean)
  return formattedWarnings.length > 0
    ? `${message} Notes: ${formattedWarnings.join(' ')}`
    : message
}

function buildResumeFooterMessage(normalizeResumeTeamReport, report) {
  const normalizedReport = normalizeResumeTeamReport(report)
  if (!normalizedReport) return ''
  const formattedWarnings = normalizedReport.warnings
    .map((warning) => formatResumeWarning(warning))
    .filter(Boolean)
  if (
    normalizedReport.failedMembers.length === 0 &&
    formattedWarnings.length === 0 &&
    !normalizedReport.teamDaemonWarning
  ) {
    return ''
  }
  const footerParts = []
  if (normalizedReport.teamDaemonWarning) {
    footerParts.push(`Team background service warning: ${normalizedReport.teamDaemonWarning}`)
  } else if (normalizedReport.startedTeamDaemon) {
    footerParts.push('Team background service started successfully.')
  } else {
    footerParts.push('Team background service was already running.')
  }
  if (formattedWarnings.length > 0) {
    const label =
      normalizedReport.failedMembers.length > 0 || normalizedReport.teamDaemonWarning
        ? 'Additional notes'
        : 'Notes'
    footerParts.push(`${label}: ${formattedWarnings.join(' ')}`)
  }
  return footerParts.join(' ')
}

function finalizeResumeProgress(
  progress,
  targetNames,
  normalizeResumeTeamReport,
  report = null,
  fallbackError = ''
) {
  const normalizedReport = normalizeResumeTeamReport(report)
  const failedEntries = normalizedReport?.failedMembers ?? []
  const failedMap = new Map(
    failedEntries
      .map((entry) => ({
        memberName: entry?.memberName ?? '',
        message: entry?.message ?? 'Failed',
      }))
      .filter((entry) => entry.memberName)
      .map((entry) => [entry.memberName, entry.message])
  )
  const resumedMembers = new Set(normalizedReport?.resumedMembers ?? [])
  const names = uniqueMemberNames([
    ...targetNames,
    ...((progress?.items ?? []).map((item) => item.memberName)),
    ...(normalizedReport?.resumedMembers ?? []),
    ...failedMap.keys(),
  ])

  const existingByName = new Map((progress?.items ?? []).map((item) => [item.memberName, item]))
  const items = names.map((memberName, index) => {
    const existing = existingByName.get(memberName) ?? createResumeProgressItem(memberName, index + 1)
    const memberIndex = Number(existing.memberIndex ?? index + 1) || index + 1
    if (resumedMembers.has(memberName)) {
      return {
        ...existing,
        memberIndex,
        status: 'succeeded',
        message: existing.status === 'running' ? existing.message : 'Resumed',
        locked: true,
      }
    }
    if (failedMap.has(memberName)) {
      return {
        ...existing,
        memberIndex,
        status: 'failed',
        message: failedMap.get(memberName) ?? 'Failed',
        locked: true,
      }
    }
    if (fallbackError) {
      return {
        ...existing,
        memberIndex,
        status: 'failed',
        message: fallbackError,
        locked: true,
      }
    }
    return {
      ...existing,
      memberIndex,
      status: existing.status === 'running' ? 'pending' : existing.status,
      message:
        existing.status === 'succeeded' || existing.status === 'failed'
          ? existing.message
          : 'Pending',
      locked: existing.locked,
    }
  })

  return updateResumeProgressMeta({
    inFlight: false,
    memberCount: Math.max(Number(normalizedReport?.totalMembers ?? 0), items.length),
    items,
    summaryMessage: normalizedReport
      ? buildResumeTeamMessage(normalizeResumeTeamReport, normalizedReport)
      : '',
    footerMessage:
      fallbackError || !normalizedReport
        ? ''
        : buildResumeFooterMessage(normalizeResumeTeamReport, normalizedReport),
  })
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
      state.runtimeMessage = buildMemberActionMessage(
        `Resumed '${currentNode.name}'.`,
        report.warnings
      )
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
    state.teamResumeProgress = createResumeProgressState(targetNames)
    state.errorMessage = ''
    state.runtimeMessage = ''

    try {
      const report = await deps.coordinationResumeTeam(state.teamName)
      state.teamResumeProgress = finalizeResumeProgress(
        state.teamResumeProgress,
        targetNames,
        deps.normalizeResumeTeamReport,
        report
      )
      state.runtimeMessage = buildResumeTeamMessage(deps.normalizeResumeTeamReport, report)

      const sequence = ++refs.discoverySequence
      await gate.refreshProjectMeshSnapshot(sequence, { preserveNotices: true })
    } catch (error) {
      const message = error?.message || 'Failed to resume team.'
      state.teamResumeProgress = finalizeResumeProgress(
        state.teamResumeProgress,
        targetNames,
        deps.normalizeResumeTeamReport,
        null,
        message
      )
      state.errorMessage = message
    }
  }

  function cancelConfirm() {
    state.confirmContext = null
  }

  function createResumeTeamProgressEffect() {
    let cancelled = false
    let unlisten = null

    deps.onCoordinationResumeTeamProgress((event) => {
      const payload = deps.normalizeResumeTeamProgressEvent(event?.payload ?? event)
      if (!payload) return
      if (payload.operation !== 'resume_team') return
      if (!state.teamResumeProgress?.inFlight) return
      if (state.teamName && payload.teamName !== state.teamName) return
      state.teamResumeProgress = applyResumeProgressEvent(state.teamResumeProgress, payload)
    })
      .then((dispose) => {
        if (cancelled) {
          if (typeof dispose === 'function') dispose()
          return
        }
        unlisten = dispose
      })
      .catch((error) => {
        console.warn('[meshTab] failed to subscribe to resume progress:', error)
      })

    return () => {
      cancelled = true
      if (typeof unlisten === 'function') unlisten()
    }
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
    createResumeTeamProgressEffect,
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
