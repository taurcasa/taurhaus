import { normalizeProjectPath } from '../pathUtils.js'

export function focusPayloadField(payload, snakeName, camelName) {
  const value = payload?.[snakeName] ?? payload?.[camelName] ?? null
  return typeof value === 'string' && value.trim() ? value.trim() : null
}

export function resolveProjectIdFromSession(session, projects = [], normalizePath = normalizeProjectPath) {
  const directProjectId = session?.project_id ?? session?.projectId ?? null
  if (typeof directProjectId === 'string' && directProjectId.trim()) {
    return directProjectId
  }

  const projectPath = session?.project_path ?? session?.projectPath ?? null
  if (typeof projectPath === 'string' && projectPath.trim()) {
    const normalizedSessionPath = normalizePath(projectPath)
    const matchingProject = projects.find((project) =>
      normalizePath(project?.path) === normalizedSessionPath
    )
    if (matchingProject?.id) {
      return matchingProject.id
    }
  }

  return null
}

export function hasAttachedTmuxFocus(payload) {
  return Boolean(
    focusPayloadField(payload, 'session', 'tmuxSession')
    && focusPayloadField(payload, 'window', 'tmuxWindow')
  )
}

export function resolveProjectIdFromTmuxFocusPayload(payload, {
  projects = [],
  liveSessions = [],
  normalizePath = normalizeProjectPath,
} = {}) {
  const directProjectId = payload?.project_id ?? payload?.projectId ?? null
  if (typeof directProjectId === 'string' && directProjectId.trim()) {
    return directProjectId
  }

  const focusSession = focusPayloadField(payload, 'session', 'tmuxSession')
  const focusWindow = focusPayloadField(payload, 'window', 'tmuxWindow')
  if (!focusSession || !focusWindow) {
    return null
  }

  const matchingSession = liveSessions.find((session) => {
    const sessionName = focusPayloadField(session, 'tmux_session', 'tmuxSession')
    if (sessionName !== focusSession) {
      return false
    }

    const windowIndex = focusPayloadField(session, 'tmux_window', 'tmuxWindow')
    const windowName = focusPayloadField(session, 'tmux_window_name', 'tmuxWindowName')
    return windowIndex === focusWindow || windowName === focusWindow
  })

  return resolveProjectIdFromSession(matchingSession, projects, normalizePath)
}
