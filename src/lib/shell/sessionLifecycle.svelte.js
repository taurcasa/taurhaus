import { setupSessionPollingLifecycle } from './events.svelte.js'
import {
  hasAttachedTmuxFocus,
  resolveProjectIdFromSession,
  resolveProjectIdFromTmuxFocusPayload,
} from './tmuxFocus.js'

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
  let tmuxFocusRefreshTimer = null

  function setForegroundProject(projectId) {
    state.foregroundProjectId = typeof projectId === 'string' && projectId.trim()
      ? projectId
      : null
  }

  function clearTmuxFocusRefreshTimer() {
    if (tmuxFocusRefreshTimer !== null) {
      clearTimeout(tmuxFocusRefreshTimer)
      tmuxFocusRefreshTimer = null
    }
  }

  function cleanup() {
    clearTmuxFocusRefreshTimer()
  }

  function logTmuxFocus(stage, details = {}) {
    logger.debug('[tmux-focus]', {
      stage,
      ...details,
    })
  }

  function scheduleForegroundProjectRefresh() {
    clearTmuxFocusRefreshTimer()
    tmuxFocusRefreshTimer = setTimeout(() => {
      tmuxFocusRefreshTimer = null
      void loadForegroundProject()
    }, 75)
  }

  async function loadForegroundProject() {
    try {
      const projectId = await ipc.getForegroundProject()
      logTmuxFocus('foreground-ipc-refresh', { projectId })
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

  function handleTmuxFocusChanged(payload) {
    const projectId = resolveProjectIdFromTmuxFocusPayload(payload, {
      projects: getProjects(),
      liveSessions: Array.from(sessionStore.getSessions().values()).flat(),
    })

    if (projectId) {
      logTmuxFocus('event-resolved-from-session-store', { payload, projectId })
      clearTmuxFocusRefreshTimer()
      setForegroundProject(projectId)
      return
    }

    if (hasAttachedTmuxFocus(payload)) {
      logTmuxFocus('event-scheduling-ipc-refresh', { payload })
      scheduleForegroundProjectRefresh()
      return
    }

    logTmuxFocus('event-cleared', { payload })
    clearTmuxFocusRefreshTimer()
    setForegroundProject(null)
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
    clearTmuxFocusRefreshTimer,
    cleanup,
    scheduleForegroundProjectRefresh,
    loadForegroundProject,
    setupPolling,
    handleDaemonDisconnected,
    markSessionBridgeLive,
    handleSessionsUpdated,
    handleTmuxFocusChanged,
    handleMeshFocusPane,
  }
}
