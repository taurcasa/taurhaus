import { getVisualHostSession, getVisualHostSessions } from '../mockState.js'

export function getSessionForProject(projectPath) {
  return getVisualHostSession(projectPath)
}

export function getSessionsForProject(projectPath) {
  return getVisualHostSessions(projectPath)
}

export function startPolling() {}

export function stopPolling() {}
