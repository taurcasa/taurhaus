import {
  MOCK_CLAUDE_SESSIONS,
  MOCK_SESSION,
  MOCK_SESSIONS,
} from './mocks/index.js'
import { invokeOrMock } from './client.js'

export function getLatestSession(projectId) {
  return invokeOrMock('get_latest_session', { projectId }, () => {
    if (!projectId || projectId === 'missing-project') {
      return null
    }
    return MOCK_SESSION
  })
}

export function listSessions(projectId, limit = 20, offset = 0) {
  return invokeOrMock('list_sessions', { projectId, limit, offset }, () => MOCK_SESSIONS)
}

export function getSession(sessionId) {
  return invokeOrMock('get_session', { sessionId }, () => MOCK_SESSION)
}

export function listClaudeSessions() {
  return invokeOrMock('list_cli_sessions', undefined, () => MOCK_CLAUDE_SESSIONS)
}

export function launchClaudeSession(projectId, mode, cliTool) {
  return invokeOrMock('launch_cli_session', { projectId, mode, cliTool: cliTool ?? null }, () => ({
    tmux_session: 'taurhaus',
    tmux_window: 'project',
    tmux_pane: '%99',
  }))
}

export function stopClaudeSession(tmuxPane, cliTool) {
  return invokeOrMock('stop_cli_session', { tmuxPane, cliTool: cliTool ?? null }, () => undefined)
}

export function navigateToSession(tmuxSession, tmuxWindow, tmuxPane, openTerminal = false) {
  return invokeOrMock('navigate_to_session', { tmuxSession, tmuxWindow, tmuxPane, openTerminal }, () => undefined)
}

export function recordSessionActivity(projectId, cliTool, startedAt, endedAt, activeDurationMs, totalDurationMs) {
  return invokeOrMock('record_session_activity', { projectId, cliTool, startedAt, endedAt, activeDurationMs, totalDurationMs }, () => undefined)
}

export function getProjectActivity(projectId) {
  return invokeOrMock('get_project_activity', { projectId }, () => ({
    total_active_ms: 0,
    total_duration_ms: 0,
    session_count: 0,
    last_session_at: null,
  }))
}
