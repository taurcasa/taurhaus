import { normalizeProjectPath } from '../pathUtils.js'
import { setupSessionPollingLifecycle } from './events.svelte.js'

// Resolve the project a live session belongs to, by id or by project path.
function resolveProjectIdFromSession(session, projects = []) {
  const directProjectId = session?.project_id ?? session?.projectId ?? null
  if (typeof directProjectId === 'string' && directProjectId.trim()) {
    return directProjectId
  }

  const projectPath = session?.project_path ?? session?.projectPath ?? null
  if (typeof projectPath !== 'string' || !projectPath.trim()) {
    return null
  }

  const normalizedSessionPath = normalizeProjectPath(projectPath)
  const matchingProject = projects.find(
    (project) => normalizeProjectPath(project?.path) === normalizedSessionPath
  )
  return matchingProject?.id ?? null
}

function errorMessage(error) {
  if (error && typeof error === 'object' && typeof error.message === 'string' && error.message.trim()) {
    return error.message
  }
  if (typeof error === 'string' && error.trim()) {
    return error
  }
  return String(error)
}

export function createShellSessionLifecycleController({
  state,
  getProjects,
  ipc,
  sessionStore,
  logger = console,
}) {
  function setForegroundProject(projectId) {
    state.foregroundProjectId = typeof projectId === 'string' && projectId.trim()
      ? projectId
      : null
  }

  async function loadForegroundProject() {
    try {
      const projectId = await ipc.getForegroundProject()
      logger.debug('[tmux-focus]', { stage: 'foreground-ipc-read', projectId })
      setForegroundProject(projectId)
    } catch (error) {
      logger.warn('[sessions] failed to load foreground project; clearing foreground marker', {
        error_message: errorMessage(error),
      })
      setForegroundProject(null)
    }
  }

  function setupPolling({ isTauri, startPolling, stopPolling, doc = document }) {
    return setupSessionPollingLifecycle({
      isTauri: isTauri(),
      sessionBridgeLive: state.sessionBridgeLive,
      startPolling,
      stopPolling,
      doc,
      logger,
    })
  }

  function handleDaemonDisconnected() {
    state.sessionBridgeLive = false
    sessionStore.markSessionPresenceStale()
  }

  function markSessionBridgeLive() {
    state.sessionBridgeLive = true
  }

  function handleSessionsUpdated(payload) {
    state.sessionBridgeLive = true
    if (payload !== undefined) {
      sessionStore.applyDaemonSessionUpdate(payload)
    }
  }

  // The daemon hub owns tmux focus and resolves it to a project, so the event
  // already carries the project id; a null payload means nothing is focused.
  function handleTmuxFocusChanged(payload) {
    const projectId = payload?.project_id ?? payload?.projectId ?? null
    logger.debug('[tmux-focus]', { stage: 'event', payload, projectId })
    setForegroundProject(projectId)
  }

  async function handleMeshFocusPane(paneId) {
    const normalizedPaneId = String(paneId || '').trim()
    if (!normalizedPaneId) return

    try {
      const sessions = await ipc.listClaudeSessions()
      const matchingSession = Array.isArray(sessions)
        ? sessions.find((session) => {
          const sessionPane = session?.tmux_pane ?? session?.tmuxPane ?? null
          return sessionPane === normalizedPaneId
        })
        : null

      const tmuxSession = matchingSession?.tmux_session ?? matchingSession?.tmuxSession ?? null
      const tmuxWindow = matchingSession?.tmux_window ?? matchingSession?.tmuxWindow ?? null
      const tmuxPane = matchingSession?.tmux_pane ?? matchingSession?.tmuxPane ?? null

      if (!tmuxSession || !tmuxWindow || !tmuxPane) {
        logger.warn('[mesh] focus pane skipped: missing tmux coordinates', {
          pane_id: normalizedPaneId,
        })
        return
      }

      setForegroundProject(resolveProjectIdFromSession(matchingSession, getProjects()))
      await ipc.navigateToSession(tmuxSession, tmuxWindow, tmuxPane, true)
    } catch (error) {
      logger.error('[mesh] focus pane failed:', {
        pane_id: normalizedPaneId,
        error_message: errorMessage(error),
      })
    }
  }

  return {
    get sessionBridgeLive() {
      return state.sessionBridgeLive
    },
    get foregroundProjectId() {
      return state.foregroundProjectId
    },
    setForegroundProject,
    loadForegroundProject,
    setupPolling,
    handleDaemonDisconnected,
    markSessionBridgeLive,
    handleSessionsUpdated,
    handleTmuxFocusChanged,
    handleMeshFocusPane,
  }
}
